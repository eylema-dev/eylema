use super::{CpuControl, CpuError as E};
#[derive(Clone, Copy, Debug)]
pub struct CpuBudget {
    pub context: usize,
    pub max_new_tokens: usize,
    pub max_resident_bytes: u64,
    pub max_work: u64,
}
impl CpuBudget {
    pub(crate) fn validate(&self) -> Result<(), E> {
        if !(1..=64).contains(&self.context)
            || !(1..=8).contains(&self.max_new_tokens)
            || self.max_new_tokens > self.context
        {
            return Err(E::ContextLimit);
        }
        if self.max_resident_bytes == 0 || self.max_resident_bytes > 2_684_354_560 {
            return Err(E::ResidentLimit);
        }
        if self.max_work == 0 || self.max_work > 80_000_000_000 {
            return Err(E::WorkLimit);
        }
        Ok(())
    }
}
pub(crate) struct Meter {
    pub control: CpuControl,
    pub budget: CpuBudget,
    pub resident: u64,
    pub peak: u64,
}
impl Meter {
    pub(crate) fn embedding(
        context: usize,
        max_resident_bytes: u64,
        max_work: u64,
        control: CpuControl,
        resident: u64,
    ) -> Result<Self, E> {
        if !(1..=4096).contains(&context) {
            return Err(E::ContextLimit);
        }
        if max_resident_bytes == 0 || max_resident_bytes > 2_684_354_560 {
            return Err(E::ResidentLimit);
        }
        if max_work == 0 || max_work > 2_500_000_000_000 {
            return Err(E::WorkLimit);
        }
        control.check()?;
        if resident > max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        if control.observed_work() > max_work {
            return Err(E::WorkLimit);
        }
        control.restrict_work(max_work)?;
        Ok(Self {
            control,
            budget: CpuBudget {
                context,
                max_new_tokens: 0,
                max_resident_bytes,
                max_work,
            },
            resident,
            peak: resident,
        })
    }

    pub(crate) fn decision(
        b: crate::decision::DecisionBudget,
        control: CpuControl,
        resident: u64,
    ) -> Result<Self, E> {
        b.validate()?;
        control.check()?;
        if resident > b.max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        control.restrict_work(b.max_work)?;
        Ok(Self {
            control,
            budget: CpuBudget {
                context: b.context,
                max_new_tokens: 0,
                max_resident_bytes: b.max_resident_bytes,
                max_work: b.max_work,
            },
            resident,
            peak: resident,
        })
    }

    pub fn new(budget: CpuBudget, control: CpuControl, resident: u64) -> Result<Self, E> {
        budget.validate()?;
        control.check()?;
        if resident > budget.max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        if control.observed_work() > budget.max_work {
            return Err(E::WorkLimit);
        }
        control.restrict_work(budget.max_work)?;
        Ok(Self {
            control,
            budget,
            resident,
            peak: resident,
        })
    }
    pub fn preflight(&self, n: u64) -> Result<(), E> {
        self.control.check()?;
        if self
            .control
            .observed_work()
            .checked_add(n)
            .is_none_or(|v| v > self.budget.max_work)
        {
            Err(E::WorkLimit)
        } else {
            Ok(())
        }
    }
    pub fn charge(&mut self, n: u64) -> Result<(), E> {
        self.preflight(n)?;
        self.control.charge_limited(n, self.budget.max_work)
    }
    pub fn hold(&mut self, n: u64) -> Result<(), E> {
        self.control.check()?;
        let next = self.resident.checked_add(n).ok_or(E::ResidentLimit)?;
        if next > self.budget.max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        self.resident = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
    pub fn zeros(&mut self, n: usize) -> Result<Vec<f32>, E> {
        let bytes = n.checked_mul(4).ok_or(E::ResidentLimit)?;
        self.preflight(n as u64)?;
        self.hold(bytes as u64)?;
        let mut v = Vec::new();
        v.try_reserve_exact(n).map_err(|_| E::AllocationFailed)?;
        let actual = v.capacity().checked_mul(4).ok_or(E::ResidentLimit)?;
        if actual > bytes {
            self.hold((actual - bytes) as u64)?;
        }
        while v.len() < n {
            let count = (n - v.len()).min(1024);
            self.charge(count as u64)?;
            v.extend_from_slice(&[0.; 1024][..count]);
        }
        Ok(v)
    }
}
