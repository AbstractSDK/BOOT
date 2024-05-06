//! State interfaces for execution environments.

use crate::error::CwEnvError;
use cosmwasm_std::Addr;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// State accessor trait.
/// Indicates that the type has access to an underlying state.
pub trait ChainState {
    /// The type of the underlying state.
    type Out: StateInterface;
    /// Get the underlying state.
    fn state(&self) -> Self::Out;
}

/// This Interface allows for managing the local state of a deployment on any CosmWasm-supported environment.
pub trait StateInterface {
    /// Get the address of a contract using the specified contract id.
    fn get_address(&self, contract_id: &str) -> Result<Addr, CwEnvError>;

    /// Set the address of a contract using the specified contract id.
    fn set_address(&mut self, contract_id: &str, address: &Addr);

    /// Get the code id for a contract with the specified contract id.
    fn get_code_id(&self, contract_id: &str) -> Result<u64, CwEnvError>;

    /// Set the code id for a contract with the specified contract id.
    fn set_code_id(&mut self, contract_id: &str, code_id: u64);

    /// Get all addresses related to this deployment.
    fn get_all_addresses(&self) -> Result<HashMap<String, Addr>, CwEnvError>;

    /// Get all codes related to this deployment.
    fn get_all_code_ids(&self) -> Result<HashMap<String, u64>, CwEnvError>;
}

impl<S: StateInterface> StateInterface for Rc<RefCell<S>> {
    fn get_address(&self, contract_id: &str) -> Result<Addr, CwEnvError> {
        (**self).borrow().get_address(contract_id)
    }

    fn set_address(&mut self, contract_id: &str, address: &Addr) {
        (**self).borrow_mut().set_address(contract_id, address)
    }

    fn get_code_id(&self, contract_id: &str) -> Result<u64, CwEnvError> {
        (**self).borrow().get_code_id(contract_id)
    }

    fn set_code_id(&mut self, contract_id: &str, code_id: u64) {
        (**self).borrow_mut().set_code_id(contract_id, code_id)
    }

    fn get_all_addresses(&self) -> Result<HashMap<String, Addr>, CwEnvError> {
        (**self).borrow().get_all_addresses()
    }

    fn get_all_code_ids(&self) -> Result<HashMap<String, u64>, CwEnvError> {
        (**self).borrow().get_all_code_ids()
    }
}

// impl<S: StateInterface> StateInterface for Rc<S> {
//     fn get_address(&self, contract_id: &str) -> Result<Addr, CwEnvError> {
//         (**self).get_address(contract_id)
//     }

//     fn set_address(&mut self, contract_id: &str, address: &Addr) {
//         (*Rc::make_mut(self)).set_address(contract_id, address)
//     }

//     fn get_code_id(&self, contract_id: &str) -> Result<u64, CwEnvError> {
//         (**self).get_code_id(contract_id)
//     }

//     fn set_code_id(&mut self, contract_id: &str, code_id: u64) {
//         (*Rc::make_mut(self)).set_code_id(contract_id, code_id)
//     }

//     fn get_all_addresses(&self) -> Result<HashMap<String, Addr>, CwEnvError> {
//         (**self).get_all_addresses()
//     }

//     fn get_all_code_ids(&self) -> Result<HashMap<String, u64>, CwEnvError> {
//         (**self).get_all_code_ids()
//     }
// }

// TODO: error handling
impl<S: StateInterface> StateInterface for Arc<Mutex<S>> {
    fn get_address(&self, contract_id: &str) -> Result<Addr, CwEnvError> {
        let locked_state = self.lock().unwrap();
        locked_state.get_address(contract_id)
    }

    fn set_address(&mut self, contract_id: &str, address: &Addr) {
        let mut locked_state = self.lock().unwrap();
        locked_state.set_address(contract_id, address)
    }

    fn get_code_id(&self, contract_id: &str) -> Result<u64, CwEnvError> {
        let locked_state = self.lock().unwrap();
        locked_state.get_code_id(contract_id)
    }

    fn set_code_id(&mut self, contract_id: &str, code_id: u64) {
        let mut locked_state = self.lock().unwrap();
        locked_state.set_code_id(contract_id, code_id)
    }

    fn get_all_addresses(&self) -> Result<HashMap<String, Addr>, CwEnvError> {
        let locked_state = self.lock().unwrap();
        locked_state.get_all_addresses()
    }

    fn get_all_code_ids(&self) -> Result<HashMap<String, u64>, CwEnvError> {
        let locked_state = self.lock().unwrap();
        locked_state.get_all_code_ids()
    }
}
