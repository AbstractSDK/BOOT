use std::{cell::RefCell, fmt::Debug, rc::Rc};

use cosmwasm_std::{
    testing::{MockApi, MockStorage},
    to_json_binary, Addr, Api, BankMsg, Binary, CosmosMsg, Empty, Event, WasmMsg,
};
use cw_multi_test::{
    ibc::IbcSimpleModule, App, AppResponse, BankKeeper, Contract, DistributionKeeper, Executor,
    FailingModule, GovFailingModule, MockApiBech32, StakeKeeper, TokenFactoryStargate, WasmKeeper,
};
use serde::Serialize;

use super::state::MockState;
use cw_orch_core::{
    contract::interface_traits::Uploadable,
    environment::{AccessConfig, ChainState, IndexResponse, StateInterface, TxHandler},
    CwEnvError,
};

pub type MockApp<A = MockApi> = App<
    BankKeeper,
    A,
    MockStorage,
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    StakeKeeper,
    DistributionKeeper,
    IbcSimpleModule,
    GovFailingModule,
    TokenFactoryStargate,
>;

/// Wrapper around a cw-multi-test [`App`](cw_multi_test::App) backend.
///
/// Stores a local state with a mapping of contract_id -> code_id/address
///
/// The state is customizable by implementing the [`StateInterface`] trait on a custom struct and providing it on the custom constructor.
///
/// The addresses used inside this environment are bech32 addresses. For instance, when creating a mock environment
/// let chain = Mock::new("sender");
/// the actual sender address can be generated using
/// let sender_addr = chain.addr_make("sender")
///
/// ## Example
/// ```
/// # use cosmwasm_std::{Addr, coin, Uint128};
/// use cw_orch_mock::Mock;
/// use cw_orch_core::environment::TxHandler;
///
/// let mock: Mock = Mock::new("sender");
///
/// // set a balance
/// mock.set_balance(&mock.sender_addr(), vec![coin(100u128, "token")]).unwrap();
///
/// // query the balance
/// let balance: Uint128 = mock.query_balance(&mock.sender_addr(), "token").unwrap();
/// assert_eq!(balance.u128(), 100u128);
/// ```
///
/// ## Example with custom state
/// ```
/// # use cosmwasm_std::{Addr, coin, Uint128};
/// use cw_orch_mock::Mock;
/// use cw_orch_core::environment::StateInterface;
/// // We just use the MockState as an example here, but you can implement your own state struct.
/// use cw_orch_mock::MockState as CustomState;
///
/// let mock: Mock = Mock::new_custom("sender", CustomState::new());
/// ```
pub struct MockBase<A: Api = MockApi, S: StateInterface = MockState> {
    /// Address used for the operations.
    pub sender: Addr,
    /// Inner mutable state storage for contract addresses and code-ids
    pub state: Rc<RefCell<S>>,
    /// Inner mutable cw-multi-test app backend
    pub app: Rc<RefCell<MockApp<A>>>,
}

pub type Mock<S = MockState> = MockBase<MockApi, S>;
pub type MockBech32<S = MockState> = MockBase<MockApiBech32, S>;

impl<A: Api, S: StateInterface> Clone for MockBase<A, S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            state: self.state.clone(),
            app: self.app.clone(),
        }
    }
}

impl<A: Api> MockBase<A, MockState> {
    pub fn with_chain_id(&mut self, chain_id: &str) {
        self.state.borrow_mut().set_chain_id(chain_id);
        self.app
            .borrow_mut()
            .update_block(|b| b.chain_id = chain_id.to_string());
    }
}

impl<A: Api, S: StateInterface> MockBase<A, S> {
    /// Upload a custom contract wrapper.
    /// Support for this is limited.
    pub fn upload_custom(
        &self,
        contract_id: &str,
        wrapper: Box<dyn Contract<Empty, Empty>>,
    ) -> Result<AppResponse, CwEnvError> {
        let code_id = self
            .app
            .borrow_mut()
            .store_code_with_creator(self.sender_addr(), wrapper);
        // add contract code_id to events manually
        let mut event = Event::new("store_code");
        event = event.add_attribute("code_id", code_id.to_string());
        let resp = AppResponse {
            events: vec![event],
            ..Default::default()
        };
        let code_id = IndexResponse::uploaded_code_id(&resp)?;
        self.state.borrow_mut().set_code_id(contract_id, code_id);
        Ok(resp)
    }
}
impl<A: Api, S: StateInterface> ChainState for MockBase<A, S> {
    type Out = Rc<RefCell<S>>;

    fn state(&self) -> Self::Out {
        self.state.clone()
    }
}

// Execute on the test chain, returns test response type
impl<A: Api, S: StateInterface> TxHandler for MockBase<A, S> {
    type Response = AppResponse;
    type Error = CwEnvError;
    type ContractSource = Box<dyn Contract<Empty, Empty>>;
    type Sender = Addr;

    fn sender(&self) -> &Self::Sender {
        &self.sender
    }

    fn sender_addr(&self) -> Addr {
        self.sender.clone()
    }

    fn set_sender(&mut self, sender: Self::Sender) {
        self.sender = sender;
    }

    fn upload<T: Uploadable>(&self, _contract: &T) -> Result<Self::Response, CwEnvError> {
        let code_id = self
            .app
            .borrow_mut()
            .store_code_with_creator(self.sender_addr(), T::wrapper());
        // add contract code_id to events manually
        let mut event = Event::new("store_code");
        event = event.add_attribute("code_id", code_id.to_string());
        let resp = AppResponse {
            events: vec![event],
            ..Default::default()
        };
        Ok(resp)
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
    }

    fn instantiate<I: Serialize + Debug>(
        &self,
        code_id: u64,
        init_msg: &I,
        label: Option<&str>,
        admin: Option<&Addr>,
        coins: &[cosmwasm_std::Coin],
    ) -> Result<Self::Response, CwEnvError> {
        let msg = WasmMsg::Instantiate {
            admin: admin.map(|a| a.to_string()),
            code_id,
            label: label.unwrap_or("contract_init").to_string(),
            msg: to_json_binary(init_msg)?,
            funds: coins.to_vec(),
        };
        let app = self
            .app
            .borrow_mut()
            .execute(self.sender.clone(), CosmosMsg::Wasm(msg))?;

        let resp = AppResponse {
            events: app.events,
            data: app.data,
        };
        Ok(resp)
    }

    fn instantiate2<I: Serialize + Debug>(
        &self,
        code_id: u64,
        init_msg: &I,
        label: Option<&str>,
        admin: Option<&Addr>,
        coins: &[cosmwasm_std::Coin],
        salt: Binary,
    ) -> Result<Self::Response, CwEnvError> {
        let msg = WasmMsg::Instantiate2 {
            admin: admin.map(|a| a.to_string()),
            code_id,
            label: label.unwrap_or("contract_init").to_string(),
            msg: to_json_binary(init_msg)?,
            funds: coins.to_vec(),
            salt,
        };

        let app = self
            .app
            .borrow_mut()
            .execute(self.sender.clone(), CosmosMsg::Wasm(msg))?;

        let resp = AppResponse {
            events: app.events,
            data: app.data,
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
    }

    fn upload_with_access_config<T: Uploadable>(
        &self,
        contract_source: &T,
        _access_config: Option<AccessConfig>,
    ) -> Result<Self::Response, Self::Error> {
        log::debug!("Uploading with access is not enforced when using Mock testing");
        self.upload(contract_source)
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
    }
}

#[cfg(test)]
mod test {

    use cosmwasm_std::{
        coins, to_json_binary, Addr, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,
        StdResult, Uint128,
    };
    use cw_multi_test::ContractWrapper;
    use cw_orch_core::environment::{BankQuerier, DefaultQueriers, QueryHandler};
    use speculoos::prelude::*;

    use crate::core::*;

    const SENDER: &str = "cosmos123";
    const BALANCE_ADDR: &str = "cosmos456";

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
            cw20_base::msg::QueryMsg::Balance { address } => Ok(to_json_binary::<Response>(
                &Response::default()
                    .add_attribute("address", address)
                    .add_attribute("balance", String::from("0")),
            )
            .unwrap()),
            _ => unimplemented!(),
        }
    }

    #[test]
    fn mock() {
        let chain = MockBech32::new(SENDER);
        let sender = chain.sender_addr();
        let recipient = chain.addr_make(BALANCE_ADDR);
        let amount = 1000000u128;
        let denom = "uosmo";

        chain
            .set_balance(&recipient, vec![Coin::new(amount, denom)])
            .unwrap();
        let balance = chain.query_balance(&recipient, denom).unwrap();

        asserting("address balance amount is correct")
            .that(&amount)
            .is_equal_to(balance.u128());

        asserting("sender is correct")
            .that(&sender.to_string())
            .is_equal_to(chain.sender_addr().to_string());

        let contract_source = Box::new(
            ContractWrapper::new(execute, cw20_base::contract::instantiate, query)
                .with_migrate(cw20_base::contract::migrate),
        );

        let init_res = chain.upload_custom("cw20", contract_source).unwrap();
        asserting("contract initialized properly")
            .that(&init_res.events[0].attributes[0].value)
            .is_equal_to(String::from("1"));

        let init_msg = cw20_base::msg::InstantiateMsg {
            name: String::from("Token"),
            symbol: String::from("TOK"),
            decimals: 6u8,
            initial_balances: vec![],
            mint: None,
            marketing: None,
        };
        let init_res = chain
            .instantiate(1, &init_msg, None, Some(&Addr::unchecked(sender)), &[])
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

        asserting("that exec passed on correctly")
            .that(&exec_res.events[1].attributes[1].value)
            .is_equal_to(String::from("mint"));

        let query_res = chain
            .query::<cw20_base::msg::QueryMsg, Response>(
                &cw20_base::msg::QueryMsg::Balance {
                    address: recipient.to_string(),
                },
                &contract_address,
            )
            .unwrap();

        asserting("that query passed on correctly")
            .that(&query_res.attributes[1].value)
            .is_equal_to(String::from("0"));

        let migrate_msg = Empty {}; // cw20_base::msg::MigrateMsg{} Doesn't implement fmt::Debug
        let migration_res = chain.migrate(&migrate_msg, 1, &contract_address);
        asserting("that migration passed on correctly")
            .that(&migration_res)
            .is_ok();
    }

    #[test]
    fn custom_mock_env() {
        let mock_state = MockState::new();
        let chain = Mock::new_custom(SENDER, mock_state);

        let recipient = chain.addr_make(BALANCE_ADDR);
        let amount = 1000000u128;
        let denom = "uosmo";

        chain
            .set_balances(&[(recipient.clone(), &[Coin::new(amount, denom)])])
            .unwrap();

        let balances = chain.query_all_balances(&recipient).unwrap();
        asserting("recipient balances length is 1")
            .that(&balances.len())
            .is_equal_to(1);
    }

    #[test]
    fn state_interface() {
        let contract_id = "my_contract";
        let code_id = 1u64;
        let address = &Addr::unchecked(BALANCE_ADDR);
        let mut mock_state = Rc::new(RefCell::new(MockState::new()));

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
            .is_equal_to(1);

        asserting!("that total addresses is 1")
            .that(&mock_state.get_all_addresses().unwrap().len())
            .is_equal_to(1);
    }

    #[test]
    fn add_balance() {
        let chain = Mock::new(SENDER);
        let recipient = chain.addr_make(BALANCE_ADDR);
        let amount = 1000000u128;
        let denom_1 = "uosmo";
        let denom_2 = "osmou";

        chain
            .add_balance(&recipient, vec![Coin::new(amount, denom_1)])
            .unwrap();
        chain
            .add_balance(&recipient, vec![Coin::new(amount, denom_2)])
            .unwrap();

        let balances = chain.query_all_balances(&recipient).unwrap();
        asserting("recipient balances added")
            .that(&balances)
            .contains_all_of(&[&Coin::new(amount, denom_1), &Coin::new(amount, denom_2)])
    }

    #[test]
    fn bank_querier_works() -> Result<(), CwEnvError> {
        let denom = "urandom";
        let init_coins = coins(45, denom);
        let app = Mock::new(SENDER);
        let sender = &app.sender;
        app.set_balance(sender, init_coins.clone())?;
        assert_eq!(
            app.bank_querier()
                .balance(sender, Some(denom.to_string()))?,
            init_coins
        );
        assert_eq!(
            app.bank_querier().supply_of(denom.to_string())?,
            init_coins[0]
        );

        Ok(())
    }
}
