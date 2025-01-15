use std::cell::RefCell;
use std::rc::Rc;

use cosmwasm_std::testing::MockApi;
use cosmwasm_std::{Addr, Coin, Uint128};
use cw_multi_test::{AppBuilder, TokenFactoryStargate};
use cw_orch_core::environment::{BankQuerier, BankSetter, TxHandler};
use cw_orch_core::{
    environment::{DefaultQueriers, StateInterface},
    CwEnvError,
};
use cw_utils::NativeBalance;

use crate::queriers::bank::MockBankQuerier;
use crate::{Mock, MockState};

impl<S: StateInterface> Mock<S> {
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
        balances: &[(impl Into<String> + Clone, &[cosmwasm_std::Coin])],
    ) -> Result<(), CwEnvError> {
        self.app
            .borrow_mut()
            .init_modules(|router, _, storage| -> Result<(), CwEnvError> {
                for (addr, coins) in balances {
                    router.bank.init_balance(
                        storage,
                        &Addr::unchecked(addr.clone()),
                        coins.to_vec(),
                    )?;
                }
                Ok(())
            })
    }

    /// Query the (bank) balance of a native token for and address.
    /// Returns the amount of the native token.
    pub fn query_balance(&self, address: &Addr, denom: &str) -> Result<Uint128, CwEnvError> {
        Ok(self
            .bank_querier()
            .balance(address, Some(denom.to_string()))?
            .first()
            .map(|c| c.amount)
            .unwrap_or_default())
    }

    /// Fetch all the balances of an address.
    pub fn query_all_balances(
        &self,
        address: &Addr,
    ) -> Result<Vec<cosmwasm_std::Coin>, CwEnvError> {
        self.bank_querier().balance(address, None)
    }
}

impl Mock {
    /// Create a mock environment with the default mock state.
    pub fn new(sender: impl Into<String>) -> Self {
        Mock::new_custom(sender, MockState::new())
    }

    pub fn new_with_chain_id(sender: impl Into<String>, chain_id: &str) -> Self {
        let chain = Mock::new_custom(sender, MockState::new());
        chain
            .app
            .borrow_mut()
            .update_block(|b| b.chain_id = chain_id.to_string());

        chain
    }
}
impl<S: StateInterface> Mock<S> {
    /// Create a mock environment with a custom mock state.
    /// The state is customizable by implementing the `StateInterface` trait on a custom struct and providing it on the custom constructor.
    pub fn new_custom(sender: impl Into<String>, custom_state: S) -> Self {
        let state = Rc::new(RefCell::new(custom_state));
        let app = AppBuilder::new_custom()
            .with_stargate(TokenFactoryStargate)
            .build(|_, _, _| {});
        let sender: String = sender.into();
        let sender = app.api().addr_make(&sender);
        let app = Rc::new(RefCell::new(app));

        Self { sender, state, app }
    }
}

impl<S: StateInterface> BankSetter for Mock<S> {
    type T = MockBankQuerier<MockApi>;

    fn set_balance(
        &mut self,
        address: &Addr,
        amount: Vec<Coin>,
    ) -> Result<(), <Self as TxHandler>::Error> {
        (*self).set_balance(address, amount)
    }
}
