use super::*;
use eylema_retrieval::{CancelToken, Control, RetrievalBudget};
use std::time::{Duration, Instant};
pub struct RouteControl {
    pub(super) inner: Control,
}
impl RouteControl {
    pub fn new(b: RetrievalBudget, t: CancelToken) -> Result<Self, RouteError> {
        if b.max_work > 10_000_000
            || b.max_live_bytes > 64 * 1024 * 1024
            || b.deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(5)
        {
            return Err(RouteError::Limit);
        }
        Ok(Self {
            inner: Control::new(b, t)?,
        })
    }
    pub fn work_used(&self) -> u64 {
        self.inner.work_used()
    }
    pub fn peak_live_bytes(&self) -> usize {
        self.inner.peak_live_bytes()
    }
    pub(super) fn check(&self) -> Result<(), RouteError> {
        Ok(self.inner.check()?)
    }
    pub(super) fn charge(&mut self, n: usize) -> Result<(), RouteError> {
        Ok(self.inner.charge(n)?)
    }
    pub(super) fn reserve(&mut self, n: usize) -> Result<(), RouteError> {
        Ok(self.inner.reserve(n)?)
    }
    pub(super) fn release(&mut self, n: usize) {
        self.inner.release(n)
    }
}
