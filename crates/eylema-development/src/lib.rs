//! Owned development control state. This package performs no external effects.
//!
//! Authority, clock, check and review observations come from the trusted host,
//! never directly from provider/issue text. Process isolation follows in G39.
#![forbid(unsafe_code)]

mod need;
mod run;
pub use need::*;
pub use run::*;
