use super::CpuError as E;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
struct State {
    #[cfg(test)]
    stop_after: AtomicU64,
    cancelled: AtomicBool,
    work: AtomicU64,
    limit: AtomicU64,
    group: CpuWorkGroup,
    deadline: Instant,
}
struct GroupState {
    work: AtomicU64,
    limit: u64,
}
/// Shared aggregate work accounting for a sequence of independently bounded jobs.
#[derive(Clone)]
pub struct CpuWorkGroup(Arc<GroupState>);
impl CpuWorkGroup {
    pub fn new(max_work: u64) -> Result<Self, E> {
        if max_work == 0 {
            return Err(E::WorkLimit);
        }
        Ok(Self(Arc::new(GroupState {
            work: AtomicU64::new(0),
            limit: max_work,
        })))
    }
    pub fn observed_work(&self) -> u64 {
        self.0.work.load(Ordering::Acquire)
    }
    fn reserve(&self, n: u64) -> Result<(), E> {
        self.0
            .work
            .try_update(Ordering::AcqRel, Ordering::Acquire, |x| {
                x.checked_add(n).filter(|&v| v <= self.0.limit)
            })
            .map(|_| ())
            .map_err(|_| E::WorkLimit)
    }
    fn release(&self, n: u64) {
        let prior = self.0.work.fetch_sub(n, Ordering::AcqRel);
        debug_assert!(prior >= n);
    }
}
#[derive(Clone)]
pub struct CpuControl(Arc<State>);
impl CpuControl {
    pub fn new(deadline: Instant) -> Result<Self, E> {
        Self::new_with_work_group(deadline, u64::MAX, CpuWorkGroup::new(u64::MAX)?)
    }
    /// Creates an independent per-job counter inside a shared aggregate work cap.
    pub fn new_job(deadline: Instant, job_limit: u64, group: CpuWorkGroup) -> Result<Self, E> {
        Self::new_with_work_group(deadline, job_limit, group)
    }
    fn new_with_work_group(
        deadline: Instant,
        job_limit: u64,
        group: CpuWorkGroup,
    ) -> Result<Self, E> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(E::Deadline)?;
        if job_limit == 0 {
            return Err(E::WorkLimit);
        }
        if remaining.is_zero() || remaining > Duration::from_secs(600) {
            return Err(E::Deadline);
        }
        Ok(Self(Arc::new(State {
            cancelled: AtomicBool::new(false),
            #[cfg(test)]
            stop_after: AtomicU64::new(u64::MAX),
            work: AtomicU64::new(0),
            limit: AtomicU64::new(job_limit),
            group,
            deadline,
        })))
    }
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn observed_work(&self) -> u64 {
        self.0.work.load(Ordering::Acquire)
    }
    pub fn aggregate_work(&self) -> u64 {
        self.0.group.observed_work()
    }
    pub(crate) fn same_execution(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.group.0, &other.0.group.0)
    }
    #[cfg(test)]
    pub(crate) fn stop_after(&self, n: u64) {
        self.0.stop_after.store(n, Ordering::Release);
    }
    pub(crate) fn check(&self) -> Result<(), E> {
        #[cfg(test)]
        if self.observed_work() >= self.0.stop_after.load(Ordering::Acquire) {
            self.cancel();
        }

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
            self.0.group.reserve(block)?;
            let charged = self
                .0
                .work
                .try_update(Ordering::AcqRel, Ordering::Acquire, |x| {
                    x.checked_add(block)
                        .filter(|&v| v <= limit.min(self.0.limit.load(Ordering::Acquire)))
                })
                .is_ok();
            if !charged {
                self.0.group.release(block);
                return Err(E::WorkLimit);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_identity_is_shared_only_by_clones() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let original = CpuControl::new(deadline).unwrap();
        let clone = original.clone();
        let separate = CpuControl::new(deadline).unwrap();
        assert!(original.same_execution(&clone));
        assert!(!original.same_execution(&separate));
    }

    #[test]
    fn bounded_jobs_enforce_local_and_shared_work_caps_before_overrun() {
        let group = CpuWorkGroup::new(1000).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let a = CpuControl::new_job(deadline, 700, group.clone()).unwrap();
        let b = CpuControl::new_job(deadline, 700, group.clone()).unwrap();
        assert!(a.same_execution(&b));
        a.charge(600).unwrap();
        assert_eq!(a.observed_work(), 600);
        b.charge(400).unwrap();
        assert_eq!(group.observed_work(), 1000);
        assert_eq!(b.charge(1), Err(E::WorkLimit));
        assert_eq!(b.observed_work(), 400);
        assert_eq!(group.observed_work(), 1000);
        let c = CpuControl::new_job(deadline, 700, group.clone()).unwrap();
        assert_eq!(c.charge(4), Err(E::WorkLimit));
        assert_eq!(group.observed_work(), 1000);

        let local_group = CpuWorkGroup::new(2000).unwrap();
        let local = CpuControl::new_job(deadline, 700, local_group.clone()).unwrap();
        local.charge(600).unwrap();
        assert_eq!(local.charge(101), Err(E::WorkLimit));
        assert_eq!(local.observed_work(), 600);
        assert_eq!(local_group.observed_work(), 600);
    }
}
