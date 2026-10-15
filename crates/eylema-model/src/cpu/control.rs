use super::CpuError as E;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
struct State {
    cancelled: AtomicBool,
    work: AtomicU64,
    limit: AtomicU64,
    deadline: Instant,
}
#[derive(Clone)]
pub struct CpuControl(Arc<State>);
impl CpuControl {
    pub fn new(deadline: Instant) -> Result<Self, E> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(E::Deadline)?;
        if remaining.is_zero() || remaining > Duration::from_secs(600) {
            return Err(E::Deadline);
        }
        Ok(Self(Arc::new(State {
            cancelled: AtomicBool::new(false),
            work: AtomicU64::new(0),
            limit: AtomicU64::new(u64::MAX),
            deadline,
        })))
    }
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn observed_work(&self) -> u64 {
        self.0.work.load(Ordering::Acquire)
    }
    pub(crate) fn check(&self) -> Result<(), E> {
        if self.0.cancelled.load(Ordering::Acquire) {
            Err(E::Cancelled)
        } else if Instant::now() >= self.0.deadline {
            Err(E::Deadline)
        } else {
            Ok(())
        }
    }
    pub(crate) fn restrict_work(&self, limit: u64) -> Result<(), E> {
        self.check()?;
        self.0.limit.fetch_min(limit, Ordering::AcqRel);
        if self.observed_work() > limit {
            Err(E::WorkLimit)
        } else {
            Ok(())
        }
    }
    pub(crate) fn charge(&self, n: u64) -> Result<(), E> {
        self.charge_limited(n, self.0.limit.load(Ordering::Acquire))
    }
    pub(crate) fn charge_limited(&self, mut n: u64, limit: u64) -> Result<(), E> {
        self.check()?;
        while n > 0 {
            self.check()?;
            let block = n.min(4096);
            self.0
                .work
                .try_update(Ordering::AcqRel, Ordering::Acquire, |x| {
                    x.checked_add(block)
                        .filter(|&v| v <= limit.min(self.0.limit.load(Ordering::Acquire)))
                })
                .map_err(|_| E::WorkLimit)?;
            n -= block;
        }
        self.check()
    }
}
impl std::fmt::Debug for CpuControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuControl")
            .field("work", &self.observed_work())
            .finish_non_exhaustive()
    }
}
