//! Portable owned task contracts shared by native and future wire clients.
//!
//! Constructors validate structure and declared bindings only. Authentication,
//! authority, availability, effect evidence and canonical encoding are separate
//! controller/adapter responsibilities. No execution or permission is implied.
#![forbid(unsafe_code)]

pub mod codec;
pub mod decision;
pub mod memory;
pub mod platform;
mod primitives;
pub mod project_data;
mod records;
pub mod skill;
pub mod transport;

pub use primitives::*;
pub use records::*;

/// Package build identity; independent of protocol or schema versions.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod tool;
