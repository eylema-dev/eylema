//! Project-private report composition; model proposals never grant authority.
mod source;
#[cfg(test)]
mod tests;
mod types;

mod control;
mod models;
mod wire;
mod worker;

pub(crate) fn worker_entry() -> types::Result<()> {
    worker::serve()
}
