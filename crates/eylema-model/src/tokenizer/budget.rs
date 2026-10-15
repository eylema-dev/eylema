use super::TokenizeError as E;
use std::cmp::Ordering;
#[derive(Clone, Copy, Debug)]
pub struct AdmissionBudget {
    pub max_resident_bytes: usize,
    pub max_read_calls: u32,
    pub max_interruptions: u32,
    pub max_work_per_artifact: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct TextBudget {
    pub max_resident_bytes: usize,
    pub max_tokens: usize,
    pub max_work: u64,
}
pub(super) struct Counters {
    pub control: Option<crate::cpu::CpuControl>,
    pub resident: usize,
    pub peak: usize,
    pub max_resident: usize,
    pub work: u64,
    pub max_work: u64,
    pub reads: u32,
    pub max_reads: u32,
    pub interruptions: u32,
    pub max_interruptions: u32,
    pub nodes: u32,
    pub unicode_checked: bool,
}
impl Counters {
    pub fn admission(b: AdmissionBudget) -> Result<Self, E> {
        if b.max_resident_bytes == 0
            || b.max_resident_bytes > 134217728
            || b.max_read_calls == 0
            || b.max_read_calls > 1024
            || b.max_interruptions > 16
            || b.max_work_per_artifact == 0
            || b.max_work_per_artifact > 134217728
        {
            return Err(E::InvalidInput);
        }
        Ok(Self {
            control: None,
            resident: 0,
            peak: 0,
            max_resident: b.max_resident_bytes,
            work: 0,
            max_work: b.max_work_per_artifact,
            reads: 0,
            max_reads: b.max_read_calls,
            interruptions: 0,
            max_interruptions: b.max_interruptions,
            nodes: 0,
            unicode_checked: false,
        })
    }
    pub fn text(b: TextBudget, retained: usize) -> Result<Self, E> {
        if b.max_tokens == 0
            || b.max_tokens > 4096
            || b.max_work == 0
            || b.max_work > 2000000
            || b.max_resident_bytes == 0
            || b.max_resident_bytes > 134217728
        {
            return Err(E::InvalidInput);
        }
        let mut c = Self::admission(AdmissionBudget {
            max_resident_bytes: b.max_resident_bytes,
            max_read_calls: 1024,
            max_interruptions: 16,
            max_work_per_artifact: b.max_work,
        })?;
        c.hold(retained)?;
        Ok(c)
    }
    pub fn charge(&mut self, n: usize) -> Result<(), E> {
        let next = self.work.checked_add(n as u64).ok_or(E::WorkLimit)?;
        if next > self.max_work {
            return Err(E::WorkLimit);
        }
        if let Some(control) = &self.control {
            control.charge(n as u64).map_err(control_error)?;
        }
        self.work = next;
        Ok(())
    }
    pub fn hold(&mut self, n: usize) -> Result<(), E> {
        if let Some(control) = &self.control {
            control.check().map_err(control_error)?;
        }
        let next = self.resident.checked_add(n).ok_or(E::ResidentLimit)?;
        if next > self.max_resident {
            return Err(E::ResidentLimit);
        }
        self.resident = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
    pub fn release(&mut self, n: usize) {
        self.resident = self.resident.saturating_sub(n)
    }
    pub fn vec<T>(&mut self, v: &mut Vec<T>, needed: usize) -> Result<(), E> {
        if needed > v.capacity() {
            let wanted = needed.max(v.capacity().saturating_mul(2)).max(8);
            let bytes = wanted
                .checked_sub(v.capacity())
                .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
                .ok_or(E::ResidentLimit)?;
            self.hold(bytes)?;
            v.try_reserve_exact(wanted - v.len())
                .map_err(|_| E::AllocationFailed)?;
        }
        Ok(())
    }
    pub fn push<T>(&mut self, v: &mut Vec<T>, item: T) -> Result<(), E> {
        self.vec(v, v.len().checked_add(1).ok_or(E::SizeLimit)?)?;
        v.push(item);
        Ok(())
    }
    pub fn bytes(&mut self, v: &mut Vec<u8>, s: &[u8]) -> Result<(), E> {
        let n = v.len().checked_add(s.len()).ok_or(E::SizeLimit)?;
        self.vec(v, n)?;
        v.extend_from_slice(s);
        Ok(())
    }
    pub fn drop_vec<T>(&mut self, v: Vec<T>) {
        let n = v.capacity() * std::mem::size_of::<T>();
        drop(v);
        self.release(n)
    }
}
pub(super) fn compare(c: &mut Counters, a: &str, b: &str) -> Result<Ordering, E> {
    for (x, y) in a.bytes().zip(b.bytes()) {
        c.charge(1)?;
        if x != y {
            return Ok(x.cmp(&y));
        }
    }
    c.charge(1)?;
    Ok(a.len().cmp(&b.len()))
}
// Own in-place heap sort: fallible comparisons, no hidden allocation or rank renumbering.
pub(super) fn sort<T, F>(v: &mut [T], c: &mut Counters, mut cmp: F) -> Result<(), E>
where
    F: FnMut(&T, &T, &mut Counters) -> Result<Ordering, E>,
{
    fn sift<T, F>(
        v: &mut [T],
        mut root: usize,
        end: usize,
        c: &mut Counters,
        cmp: &mut F,
    ) -> Result<(), E>
    where
        F: FnMut(&T, &T, &mut Counters) -> Result<Ordering, E>,
    {
        loop {
            let child = root * 2 + 1;
            if child >= end {
                break;
            }
            let mut best = child;
            if child + 1 < end && cmp(&v[child], &v[child + 1], c)? == Ordering::Less {
                best = child + 1
            }
            if cmp(&v[root], &v[best], c)? != Ordering::Less {
                break;
            }
            c.charge(1)?;
            v.swap(root, best);
            root = best;
        }
        Ok(())
    }
    for root in (0..v.len() / 2).rev() {
        sift(v, root, v.len(), c, &mut cmp)?
    }
    for end in (1..v.len()).rev() {
        v.swap(0, end);
        sift(v, 0, end, c, &mut cmp)?
    }
    Ok(())
}

pub(super) fn control_error(e: crate::cpu::CpuError) -> E {
    match e {
        crate::cpu::CpuError::Cancelled => E::Cancelled,
        crate::cpu::CpuError::Deadline => E::Deadline,
        _ => E::WorkLimit,
    }
}
