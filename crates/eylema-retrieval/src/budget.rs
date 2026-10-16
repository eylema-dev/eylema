use crate::{RetrievalError as E, RetrievalResult as R};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst)
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
pub struct RetrievalBudget {
    pub max_work: u64,
    pub max_live_bytes: usize,
    pub deadline: Instant,
}
pub struct Control {
    budget: RetrievalBudget,
    token: CancelToken,
    work: u64,
    live: usize,
    peak: usize,
}
impl Control {
    pub fn new(budget: RetrievalBudget, token: CancelToken) -> R<Self> {
        if budget.max_work == 0
            || budget.max_work > 100_000_000
            || budget.max_live_bytes == 0
            || budget.max_live_bytes > 64 * 1024 * 1024
            || budget.deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(30)
        {
            return Err(E::LimitExceeded);
        }
        let c = Self {
            budget,
            token,
            work: 0,
            live: 0,
            peak: 0,
        };
        c.check()?;
        Ok(c)
    }
    pub fn work_used(&self) -> u64 {
        self.work
    }
    pub fn peak_live_bytes(&self) -> usize {
        self.peak
    }
    /// Adapter accounting only; these counters grant no authority and cannot widen limits.
    pub fn charge(&mut self, n: usize) -> R<()> {
        self.check()?;
        let next = self
            .work
            .checked_add(u64::try_from(n).map_err(|_| E::LimitExceeded)?)
            .ok_or(E::LimitExceeded)?;
        if next > self.budget.max_work {
            return Err(E::LimitExceeded);
        }
        self.work = next;
        Ok(())
    }
    pub fn reserve(&mut self, n: usize) -> R<()> {
        self.check()?;
        let next = self.live.checked_add(n).ok_or(E::LimitExceeded)?;
        if next > self.budget.max_live_bytes {
            return Err(E::LimitExceeded);
        }
        self.live = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
    pub fn release(&mut self, n: usize) {
        self.live = self.live.saturating_sub(n)
    }
    pub(crate) fn read_guard(&self) -> (Instant, CancelToken) {
        (self.budget.deadline, self.token.clone())
    }
    pub fn check(&self) -> R<()> {
        if self.token.cancelled() {
            Err(E::Cancelled)
        } else if Instant::now() >= self.budget.deadline {
            Err(E::Deadline)
        } else {
            Ok(())
        }
    }
}
