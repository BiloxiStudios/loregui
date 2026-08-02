//! Shared strict Option-A governance contract and evaluator.
//!
//! Operation-specific implementations live beside this module.  They must use
//! the schemas in [`contract`] and the injected evaluator in [`evaluator`]; no
//! operation is permitted to create a weaker private variant.

pub mod contract;
pub mod evaluator;
