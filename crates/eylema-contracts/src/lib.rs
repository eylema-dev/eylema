//! Portable workspace build identity, consumed by the native bootstrap host.
//!
//! Task/authority/wire contracts arrive in subsequent scoped Goals. This crate
//! establishes compilation and dependency boundaries, not task execution.
#![forbid(unsafe_code)]

/// Package build identity; independent of protocol or schema versions.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
