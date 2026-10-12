#![forbid(unsafe_code)]
//! Native trusted host. Public inputs are not identity or publication authority.
//! Task code executes only in a default-deny macOS single-process sandbox.

mod files;
mod process;
mod workspace;

pub use files::{Artifact, ArtifactLimits};
pub use process::{CancelHandle, ProcessLimits, StopReason, WorkerResult, WorkerSpec};
pub use workspace::Workspace;

#[derive(Debug)]
pub enum Error {
    InvalidInput,
    InvalidState,
    Scope,
    AlreadyExists,
    Unsupported,
    Git,
    Coordinator(eylema_development::Error),
    Io(std::io::ErrorKind),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.kind())
    }
}
impl From<eylema_development::Error> for Error {
    fn from(e: eylema_development::Error) -> Self {
        Self::Coordinator(e)
    }
}
