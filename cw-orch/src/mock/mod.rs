//! Integration testing execution environment backed by a [cw-multi-test](cw_multi_test) App.
//! It has an associated state that stores deployment information for easy retrieval and contract interactions.

mod core;
pub(crate) mod state;

pub use self::core::*;
pub use state::*;
