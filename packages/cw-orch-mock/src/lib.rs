//! Integration testing execution environment backed by a [cw-multi-test](cw_multi_test) App.
//! It has an associated state that stores deployment information for easy retrieval and contract interactions.

// Export our fork
pub extern crate cw_multi_test;

mod core;
pub mod queriers;
mod state;

pub(crate) use self::core::MockBase;
pub use self::core::{Mock, MockBech32};
pub use state::MockState;
