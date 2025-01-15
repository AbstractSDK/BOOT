use std::{cell::RefCell, fmt::Debug, io::Read, rc::Rc};

use clone_cw_multi_test::tokenfactory::TokenFactoryStargate;
use clone_cw_multi_test::wasm_emulation::query::ContainsRemote;
use clone_cw_multi_test::{
    wasm_emulation::{channel::RemoteChannel, storage::analyzer::StorageAnalyzer},
    App, AppBuilder, BankKeeper, Contract, Executor, WasmKeeper,
};
use clone_cw_multi_test::{
    DistributionKeeper, FailingModule, GovFailingModule, IbcFailingModule, MockApiBech32,
    StakeKeeper,
};
use cosmwasm_std::testing::MockStorage;
use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Empty, Event, StdError, StdResult,
    Uint128, WasmMsg,
};
use cw_orch_core::{
    contract::interface_traits::{ContractInstance, Uploadable},
    environment::{
        AccessConfig, BankQuerier, BankSetter, ChainInfoOwned, ChainState, DefaultQueriers,
        IndexResponse, StateInterface, TxHandler,
    },
    CwEnvError,
};
use cw_orch_daemon::{queriers::Node, read_network_config, DEFAULT_DEPLOYMENT, RUNTIME};
use cw_utils::NativeBalance;
use serde::Serialize;
use tokio::runtime::Runtime;

use crate::{contract::CloneTestingContract, queriers::bank::CloneBankQuerier};

use super::state::MockState;

pub type CloneTestingApp = App<
    BankKeeper,
    MockApiBech32,
    MockStorage,
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    StakeKeeper,
    DistributionKeeper,
    IbcFailingModule,
    GovFailingModule,
    TokenFactoryStargate,
>;

/// Wrapper around a cw-multi-test [`App`](cw_multi_test::App) backend.
///
/// Stores a local state with a mapping of contract_id -> code_id/address
///
/// The state is customizable by implementing the [`StateInterface`] trait on a custom struct and providing it on the custom constructor.
///
/// ## Example
/// ```
/// # use cosmwasm_std::{Addr, coin, Uint128};
/// use cw_orch_clone_testing::CloneTesting;
/// use cw_orch_core::environment::TxHandler;
///
/// let chain = cw_orch_daemon::networks::JUNO_1;
/// let mock: CloneTesting = CloneTesting::new(chain.clone()).unwrap();
/// let sender = mock.sender();
///
/// // set a balance
/// mock.set_balance(&sender, vec![coin(100u128, "token")]).unwrap();
///
/// // query the balance
/// let balance: Uint128 = mock.query_balance(&sender, "token").unwrap();
/// assert_eq!(balance.u128(), 100u128);
/// ```
///
/// ## Example with custom state
/// ```
/// # use cosmwasm_std::{Addr, coin, Uint128};
/// use cw_orch_clone_testing::CloneTesting;
/// use cw_orch_core::environment::StateInterface;
/// // We just use the MockState as an example here, but you can implement your own state struct.
/// use cw_orch_clone_testing::MockState as CustomState;
///
/// let rt = tokio::runtime::Runtime::new().unwrap();
/// let chain = cw_orch_daemon::networks::JUNO_1;
/// let mock: CloneTesting = CloneTesting::new_custom(&rt, chain.clone(), CustomState::new(chain.clone().into(), "mock")).unwrap();
/// ```
#[derive(Clone)]
pub struct CloneTesting<S: StateInterface = MockState> {
    /// Chain data of the chain you want to fork
    pub chain: ChainInfoOwned,
    /// Address used for the operations.
    pub sender: Addr,
    /// Inner mutable state storage for contract addresses and code-ids
    pub state: Rc<RefCell<S>>,
    /// Inner mutable cw-multi-test app backend
    pub app: Rc<RefCell<CloneTestingApp>>,
}

impl CloneTesting {
    /// Creates a new valid account
    pub fn addr_make(&self, account_name: impl Into<String>) -> Addr {
        self.app.borrow().api().addr_make(&account_name.into())
    }

    pub fn addr_make_with_balance(
        &self,
        account_name: impl Into<String>,
        balance: Vec<Coin>,
    ) -> Result<Addr, CwEnvError> {
        let addr = self.app.borrow().api().addr_make(&account_name.into());
        self.set_balance(&addr, balance)?;

        Ok(addr)
    }

    /// Set the bank balance of an address.
    pub fn set_balance(
        &self,
        address: &Addr,
        amount: Vec<cosmwasm_std::Coin>,
    ) -> Result<(), CwEnvError> {
        self.app
            .borrow_mut()
            .init_modules(|router, _, storage| router.bank.init_balance(storage, address, amount))
            .map_err(Into::into)
    }

    /// Adds the bank balance of an address.
    pub fn add_balance(
        &self,
        address: &Addr,
        amount: Vec<cosmwasm_std::Coin>,
    ) -> Result<(), CwEnvError> {
        let b = self.query_all_balances(address)?;
        let new_amount = NativeBalance(b) + NativeBalance(amount);
        self.app
            .borrow_mut()
            .init_modules(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, address, new_amount.into_vec())
            })
            .map_err(Into::into)
    }

    /// Set the balance for multiple coins at once.
    pub fn set_balances(
        &self,
        balances: &[(&Addr, &[cosmwasm_std::Coin])],
    ) -> Result<(), CwEnvError> {
        self.app
            .borrow_mut()
            .init_modules(|router, _, storage| -> Result<(), CwEnvError> {
                for (addr, coins) in balances {
                    router.bank.init_balance(storage, addr, coins.to_vec())?;
                }
                Ok(())
            })
    }

    /// Query the (bank) balance of a native token for and address.
    /// Returns the amount of the native token.
    pub fn query_balance(&self, address: &Addr, denom: &str) -> Result<Uint128, CwEnvError> {
        Ok(self
            .bank_querier()
            .balance(address, Some(denom.to_string()))?[0]
            .amount)
    }

    /// Fetch all the balances of an address.
    pub fn query_all_balances(
        &self,
        address: &Addr,
    ) -> Result<Vec<cosmwasm_std::Coin>, CwEnvError> {
        self.bank_querier().balance(address, None)
    }

    pub fn upload_wasm<T: Uploadable + ContractInstance<CloneTesting>>(
        &self,
        contract: &T,
    ) -> Result<<Self as TxHandler>::Response, CwEnvError> {
        let mut file = std::fs::File::open(T::wasm(&self.chain).path())?;
        let mut wasm = Vec::<u8>::new();
        file.read_to_end(&mut wasm)?;
        let code_id = self.app.borrow_mut().store_wasm_code(wasm);

        contract.set_code_id(code_id);
        println!("{code_id}");

        // add contract code_id to events manually
        let mut event = Event::new("store_code");
        event = event.add_attribute("code_id", code_id.to_string());
        let resp = AppResponse {
            events: vec![event],
            ..Default::default()
        };
        Ok(resp)
    }
}

impl CloneTesting<MockState> {
    /// Create a mock environment with the default mock state.
    pub fn new(chain: impl Into<ChainInfoOwned>) -> Result<Self, CwEnvError> {
        Self::new_with_runtime(&RUNTIME, chain)
    }

    /// Create a mock environment with the default mock state.
    /// It uses a custom runtime object to control async requests
    pub fn new_with_runtime(
        rt: &Runtime,
        chain: impl Into<ChainInfoOwned>,
    ) -> Result<Self, CwEnvError> {
        let chain_data = chain.into();
        CloneTesting::new_custom(
            rt,
            chain_data.clone(),
            MockState::new(chain_data, DEFAULT_DEPLOYMENT),
        )
    }

    pub fn new_with_deployment_id(
        rt: &Runtime,
        chain: impl Into<ChainInfoOwned>,
        deployment_id: &str,
    ) -> Result<Self, CwEnvError> {
        let chain_data = chain.into();
        CloneTesting::new_custom(
            rt,
            chain_data.clone(),
            MockState::new(chain_data, deployment_id),
        )
    }
}

impl<S: StateInterface> CloneTesting<S> {
    /// Create a mock environment with a custom mock state.
    /// The state is customizable by implementing the `StateInterface` trait on a custom struct and providing it on the custom constructor.
    pub fn new_custom(
        rt: &Runtime,
        chain: impl Into<ChainInfoOwned>,
        custom_state: S,
    ) -> Result<Self, CwEnvError> {
        let chain: ChainInfoOwned = chain.into();
        let chain = if let Some(chain_info) = read_network_config(&chain.chain_id) {
            chain.overwrite_with(chain_info)
        } else {
            chain
        };
        let state = Rc::new(RefCell::new(custom_state));

        let pub_address_prefix = chain.network_info.pub_address_prefix.clone();
        let remote_channel = RemoteChannel::new(
            rt,
            &chain
                .grpc_urls
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            &chain.chain_id,
            &chain.network_info.pub_address_prefix,
        )
        .unwrap();

        let wasm = WasmKeeper::<Empty, Empty>::new().with_remote(remote_channel.clone());

        let bank = BankKeeper::new().with_remote(remote_channel.clone());

        // We update the block_height
        let block_info = remote_channel
            .rt
            .block_on(Node::new_async(remote_channel.channel.clone())._block_info())
            .unwrap();

        // Finally we instantiate a new app
        let app = AppBuilder::default()
            .with_wasm(wasm)
            .with_bank(bank)
            .with_api(MockApiBech32::new(&pub_address_prefix))
            .with_block(block_info)
            .with_remote(remote_channel.clone())
            .with_stargate(TokenFactoryStargate);

        let app = Rc::new(RefCell::new(app.build(|_, _, _| {})));
        let sender = app.borrow().api().addr_make("sender");

        Ok(Self {
            chain,
            sender: sender.clone(),
            state,
            app,
        })
    }

    pub fn storage_analysis(&self) -> StorageAnalyzer {
        StorageAnalyzer::new(&self.app.borrow()).unwrap()
    }
}

impl<S: StateInterface> ChainState for CloneTesting<S> {
    type Out = Rc<RefCell<S>>;

    fn state(&self) -> Self::Out {
        self.state.clone()
    }

    fn can_load_state_from_state_file(&self) -> bool {
        true
    }
}

// Execute on the test chain, returns test response type
impl<S: StateInterface> TxHandler for CloneTesting<S> {
    type Response = AppResponse;
    type Error = CwEnvError;
    type ContractSource = Box<dyn Contract<Empty, Empty>>;
    type Sender = Addr;

    fn sender(&self) -> &cosmwasm_std::Addr {
        &self.sender
    }

    fn sender_addr(&self) -> Addr {
        self.sender.clone()
    }

    fn set_sender(&mut self, sender: Self::Sender) {
        self.sender = sender;
    }

    fn upload<T: Uploadable>(&self, _contract: &T) -> Result<Self::Response, CwEnvError> {
        let wrapper_contract = CloneTestingContract::new(T::wrapper());
        let code_id = self
            .app
            .borrow_mut()
            .store_code_with_creator(self.sender_addr(), Box::new(wrapper_contract));
        // add contract code_id to events manually
        let mut event = Event::new("store_code");
        event = event.add_attribute("code_id", code_id.to_string());
        let resp = AppResponse {
            events: vec![event],
            ..Default::default()
        };
        Ok(resp)
    }

    fn upload_with_access_config<T: Uploadable>(
        &self,
        contract_source: &T,
        _access_config: Option<AccessConfig>,
    ) -> Result<Self::Response, Self::Error> {
        log::debug!("Uploading with access is not enforced when using Clone Testing");
        self.upload(contract_source)
    }

    fn execute<E: Serialize + Debug>(
        &self,
        exec_msg: &E,
        coins: &[cosmwasm_std::Coin],
        contract_address: &Addr,
    ) -> Result<Self::Response, CwEnvError> {
        self.app
            .borrow_mut()
            .execute_contract(
                self.sender.clone(),
                contract_address.to_owned(),
                exec_msg,
                coins,
            )
            .map_err(From::from)
            .map(Into::into)
    }

    fn instantiate<I: Serialize + Debug>(
        &self,
        code_id: u64,
        init_msg: &I,
        label: Option<&str>,
        admin: Option<&Addr>,
        coins: &[cosmwasm_std::Coin],
    ) -> Result<Self::Response, CwEnvError> {
        let addr = self.app.borrow_mut().instantiate_contract(
            code_id,
            self.sender.clone(),
            init_msg,
            coins,
            label.unwrap_or("contract_init"),
            admin.map(|a| a.to_string()),
        )?;
        // add contract address to events manually
        let mut event = Event::new("instantiate");
        event = event.add_attribute("_contract_address", addr);
        let resp = AppResponse {
            events: vec![event],
            ..Default::default()
        };
        Ok(resp)
    }

    fn migrate<M: Serialize + Debug>(
        &self,
        migrate_msg: &M,
        new_code_id: u64,
        contract_address: &Addr,
    ) -> Result<Self::Response, CwEnvError> {
        self.app
            .borrow_mut()
            .migrate_contract(
                self.sender.clone(),
                contract_address.clone(),
                migrate_msg,
                new_code_id,
            )
            .map_err(From::from)
            .map(Into::into)
    }

    fn instantiate2<I: Serialize + Debug>(
        &self,
        code_id: u64,
        init_msg: &I,
        label: Option<&str>,
        admin: Option<&Addr>,
        coins: &[cosmwasm_std::Coin],
        salt: Binary,
    ) -> Result<Self::Response, Self::Error> {
        let resp = self.app.borrow_mut().execute(
            self.sender.clone(),
            CosmosMsg::Wasm(WasmMsg::Instantiate2 {
                admin: admin.map(|a| a.to_string()),
                code_id,
                label: label.unwrap_or("contract_init").to_string(),
                msg: to_json_binary(init_msg)?,
                funds: coins.to_vec(),
                salt,
            }),
        )?;

        let app_resp = AppResponse {
            events: resp.events,
            data: resp.data,
        };

        Ok(app_resp)
    }

    fn bank_send(
        &self,
        receiver: &Addr,
        amount: &[cosmwasm_std::Coin],
    ) -> Result<Self::Response, Self::Error> {
        self.app
            .borrow_mut()
            .execute(
                self.sender.clone(),
                BankMsg::Send {
                    to_address: receiver.to_string(),
                    amount: amount.to_vec(),
                }
                .into(),
            )
            .map_err(From::from)
            .map(Into::into)
    }
}

/// Custom AppResponse type for working with the IndexResponse trait
#[derive(Default, Clone, Debug)]
pub struct AppResponse {
    pub events: Vec<Event>,
    pub data: Option<Binary>,
}

impl From<clone_cw_multi_test::AppResponse> for AppResponse {
    fn from(value: clone_cw_multi_test::AppResponse) -> Self {
        AppResponse {
            events: value.events,
            data: value.data,
        }
    }
}

impl From<AppResponse> for clone_cw_multi_test::AppResponse {
    fn from(value: AppResponse) -> Self {
        clone_cw_multi_test::AppResponse {
            events: value.events,
            data: value.data,
        }
    }
}

impl IndexResponse for AppResponse {
    fn events(&self) -> Vec<Event> {
        self.events.clone()
    }

    fn data(&self) -> Option<Binary> {
        self.data.clone()
    }

    fn event_attr_value(&self, event_type: &str, attr_key: &str) -> StdResult<String> {
        for event in &self.events {
            if event.ty == event_type {
                for attr in &event.attributes {
                    if attr.key == attr_key {
                        return Ok(attr.value.clone());
                    }
                }
            }
        }
        Err(StdError::generic_err(format!(
            "missing combination (event: {}, attribute: {})",
            event_type, attr_key
        )))
    }

    fn event_attr_values(&self, event_type: &str, attr_key: &str) -> Vec<String> {
        let mut all_results = vec![];

        for event in &self.events {
            if event.ty == event_type {
                for attr in &event.attributes {
                    if attr.key == attr_key {
                        all_results.push(attr.value.clone());
                    }
                }
            }
        }
        all_results
    }
}

impl BankSetter for CloneTesting {
    type T = CloneBankQuerier;
    fn set_balance(
        &mut self,
        address: &Addr,
        amount: Vec<Coin>,
    ) -> Result<(), <Self as TxHandler>::Error> {
        (*self).set_balance(&Addr::unchecked(address), amount)
    }
}

#[cfg(test)]
mod test {
    use crate::core::*;
    use clone_cw_multi_test::LOCAL_RUST_CODE_OFFSET;
    use cosmwasm_std::{
        to_json_binary, Addr, Coin, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
    };
    use cw20::{BalanceResponse, MinterResponse};
    use cw_orch_core::contract::WasmPath;
    use cw_orch_core::environment::QueryHandler;
    use cw_orch_daemon::networks::JUNO_1;
    use cw_orch_mock::cw_multi_test::{Contract as MockContract, ContractWrapper};
    use speculoos::prelude::*;

    pub struct MockCw20;

    fn execute(
        _deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        msg: cw20::Cw20ExecuteMsg,
    ) -> Result<Response, cw20_base::ContractError> {
        match msg {
            cw20::Cw20ExecuteMsg::Mint { recipient, amount } => Ok(Response::default()
                .add_attribute("action", "mint")
                .add_attribute("recipient", recipient)
                .add_attribute("amount", amount)),
            _ => unimplemented!(),
        }
    }

    fn query(_deps: Deps, _env: Env, msg: cw20_base::msg::QueryMsg) -> StdResult<Binary> {
        match msg {
            cw20_base::msg::QueryMsg::Balance { address: _ } => {
                Ok(to_json_binary::<BalanceResponse>(&BalanceResponse {
                    balance: Uint128::from(100u128),
                })
                .unwrap())
            }
            _ => unimplemented!(),
        }
    }
    impl Uploadable for MockCw20 {
        fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
            unimplemented!()
        }

        fn wrapper() -> Box<dyn MockContract<Empty, Empty>> {
            Box::new(
                ContractWrapper::new(execute, cw20_base::contract::instantiate, query)
                    .with_migrate(cw20_base::contract::migrate),
            )
        }
    }

    #[test]
    fn mock() -> anyhow::Result<()> {
        let amount = 1000000u128;
        let denom = "uosmo";
        let chain_info = JUNO_1;

        let chain = CloneTesting::new(chain_info)?;

        let sender = chain.sender_addr();
        let recipient = &chain.addr_make("recipient");

        chain
            .set_balance(recipient, vec![Coin::new(amount, denom)])
            .unwrap();
        let balance = chain.query_balance(recipient, denom).unwrap();

        asserting("address balance amount is correct")
            .that(&amount)
            .is_equal_to(balance.u128());

        asserting("sender is correct")
            .that(&sender)
            .is_equal_to(chain.sender_addr());

        let init_res = chain.upload(&MockCw20).unwrap();
        let code_id = (1 + LOCAL_RUST_CODE_OFFSET) as u64;
        asserting("contract initialized properly")
            .that(&init_res.events[0].attributes[0].value)
            .is_equal_to(code_id.to_string());

        let init_msg = cw20_base::msg::InstantiateMsg {
            name: String::from("Token"),
            symbol: String::from("TOK"),
            decimals: 6u8,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: sender.to_string(),
                cap: None,
            }),
            marketing: None,
        };
        let init_res = chain
            .instantiate(code_id, &init_msg, None, Some(&sender), &[])
            .unwrap();

        let contract_address = Addr::unchecked(&init_res.events[0].attributes[0].value);

        let exec_res = chain
            .execute(
                &cw20_base::msg::ExecuteMsg::Mint {
                    recipient: recipient.to_string(),
                    amount: Uint128::from(100u128),
                },
                &[],
                &contract_address,
            )
            .unwrap();

        asserting("that exect passed on correctly")
            .that(&exec_res.events[1].attributes[1].value)
            .is_equal_to(String::from("mint"));

        let query_res = chain
            .query::<cw20_base::msg::QueryMsg, BalanceResponse>(
                &cw20_base::msg::QueryMsg::Balance {
                    address: recipient.to_string(),
                },
                &contract_address,
            )
            .unwrap();

        asserting("that query passed on correctly")
            .that(&query_res.balance)
            .is_equal_to(Uint128::from(100u128));

        let migration_res = chain.migrate(&Empty {}, code_id, &contract_address);
        asserting("that migration passed on correctly")
            .that(&migration_res)
            .is_ok();

        Ok(())
    }

    #[test]
    fn custom_mock_env() -> anyhow::Result<()> {
        let amount = 1000000u128;
        let denom = "uosmo";
        let chain = JUNO_1;

        let rt = Runtime::new().unwrap();
        let mock_state = MockState::new(JUNO_1.into(), "default_id");

        let chain: CloneTesting = CloneTesting::<_>::new_custom(&rt, chain, mock_state)?;
        let recipient = chain.addr_make("recipient");

        chain
            .set_balances(&[(&recipient, &[Coin::new(amount, denom)])])
            .unwrap();

        let balances = chain.query_all_balances(&recipient).unwrap();
        asserting("recipient balances length is 1")
            .that(&balances.len())
            .is_equal_to(1);

        Ok(())
    }

    #[test]
    fn state_interface() {
        let contract_id = "my_contract";
        let code_id = 1u64;
        let address = &Addr::unchecked("TEST_ADDR");
        let mut mock_state = Rc::new(RefCell::new(MockState::new(JUNO_1.into(), "default_id")));

        mock_state.set_address(contract_id, address);
        asserting!("that address has been set")
            .that(&address)
            .is_equal_to(&mock_state.get_address(contract_id).unwrap());

        mock_state.set_code_id(contract_id, code_id);
        asserting!("that code_id has been set")
            .that(&code_id)
            .is_equal_to(mock_state.get_code_id(contract_id).unwrap());

        asserting!("that total code_ids is 1")
            .that(&mock_state.get_all_code_ids().unwrap().len())
            .is_greater_than_or_equal_to(1);

        asserting!("that total addresses is 1")
            .that(&mock_state.get_all_addresses().unwrap().len())
            .is_greater_than_or_equal_to(1);
    }

    #[test]
    fn add_balance() -> anyhow::Result<()> {
        let amount = 1000000u128;
        let denom_1 = "uosmo";
        let denom_2 = "osmou";
        let chain_info = JUNO_1;

        let chain = CloneTesting::new(chain_info)?;
        let recipient = &chain.addr_make("recipient");

        chain
            .add_balance(recipient, vec![Coin::new(amount, denom_1)])
            .unwrap();
        chain
            .add_balance(recipient, vec![Coin::new(amount, denom_2)])
            .unwrap();

        let balances = chain.query_all_balances(recipient).unwrap();
        asserting("recipient balances added")
            .that(&balances)
            .contains_all_of(&[&Coin::new(amount, denom_1), &Coin::new(amount, denom_2)]);
        Ok(())
    }
}
