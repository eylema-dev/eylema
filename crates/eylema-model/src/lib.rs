//! Own bounded weight bytes and declared layout checks, not model execution.
#![forbid(unsafe_code)]
mod error;
mod header;
mod layout;
mod loader;
pub use error::LoadError;
pub use layout::{StoredDtype, TensorSpec, WeightLayout};
#[cfg(test)]
extern crate self as eylema_model;
#[cfg(test)]
mod header_tests;

pub use loader::{LoadBudget, LoadedWeights, load};
pub mod tokenizer;
