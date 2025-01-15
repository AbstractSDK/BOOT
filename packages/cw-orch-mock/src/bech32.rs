use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{testing::MockApi, Addr, Coin, Uint128};
use cw_multi_test::{AppBuilder, MockApiBech32, TokenFactoryStargate};
use cw_orch_core::{
    environment::{BankQuerier, BankSetter, DefaultQueriers, StateInterface, TxHandler},
    CwEnvError,
};
use cw_utils::NativeBalance;

use crate::{queriers::bank::MockBankQuerier, MockBase, MockBech32, MockState};

impl MockBase<MockApiBech32, MockState> {
    /// Create a mock environment with the default mock state.
    pub fn new(prefix: &'static str) -> Self {
        MockBech32::new_custom(prefix, MockState::new())
    }

    pub fn new_with_chain_id(prefix: &'static str, chain_id: &str) -> Self {
        let chain = MockBech32::new_custom(prefix, MockState::new());
        chain
            .app
            .borrow_mut()
            .update_block(|b| b.chain_id = chain_id.to_string());

        chain
    }
}

impl<S: StateInterface> MockBase<MockApiBech32, S> {
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
}

impl<S: StateInterface> MockBase<MockApi, S> {
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
}

impl Default for MockBase<MockApiBech32, MockState> {
    fn default() -> Self {
        MockBase::<MockApiBech32, MockState>::new_custom("mock", MockState::new())
    }
}

impl<S: StateInterface> MockBase<MockApiBech32, S> {
    /// Create a mock environment with a custom mock state.
    /// The state is customizable by implementing the `StateInterface` trait on a custom struct and providing it on the custom constructor.
    pub fn new_custom(prefix: &'static str, custom_state: S) -> Self {
        let state = Rc::new(RefCell::new(custom_state));
        let app = Rc::new(RefCell::new(
            AppBuilder::new_custom()
                .with_api(MockApiBech32::new(prefix))
                .with_stargate(TokenFactoryStargate)
                .build(|_, _, _| {}),
        ));

        // We create an address internally
        let sender = app.borrow().api().addr_make("sender");

        Self { sender, state, app }
    }
}

impl<S: StateInterface> MockBech32<S> {
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
        let addr = &address;
        let b = self.query_all_balances(addr)?;
        let new_amount = NativeBalance(b) + NativeBalance(amount);
        self.app
            .borrow_mut()
            .init_modules(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, addr, new_amount.into_vec())
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

impl<S: StateInterface> BankSetter for MockBech32<S> {
    type T = MockBankQuerier<MockApiBech32>;

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
    use cosmwasm_std::coins;

    use crate::MockBech32;
    use cw_orch_core::environment::{BankQuerier, DefaultQueriers};
    #[test]
    fn addr_make_with_balance() -> anyhow::Result<()> {
        let mock = MockBech32::new("mock");

        let address = mock.addr_make_with_balance("sender", coins(42765, "ujuno"))?;

        let balance = mock.bank_querier().balance(&address, None)?;

        assert_eq!(balance, coins(42765, "ujuno"));

        Ok(())
    }
}
