//! Bounded host-side admission for transient Metal, view and model work.
//!
//! Byte reservations cover only caller-declared transient buffers. A lease's
//! cancellation flag is cooperative: workers must check it between bounded
//! work chunks. Dropping a lease releases its reservation; it cannot undo an
//! external effect or preempt a non-cooperative platform call.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum ResourceClass {
    Metal = 0,
    View = 1,
    Model = 2,
}
impl ResourceClass {
    const ALL: [Self; 3] = [Self::Metal, Self::View, Self::Model];
    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLimits {
    /// Maximum admitted requests waiting for a lease, across all classes.
    pub queued: usize,
    /// Maximum admitted requests waiting for a lease, in any one class.
    pub queued_per_class: usize,
    /// Maximum simultaneously active leases, across all classes.
    pub active: usize,
    /// Maximum simultaneously active leases in each class.
    pub active_per_class: [usize; 3],
    /// Maximum caller-declared transient bytes reserved, including queued work.
    pub reserved_bytes: usize,
    /// Maximum caller-declared transient bytes reserved by one class.
    pub reserved_bytes_per_class: [usize; 3],
    /// Maximum transient bytes one request may reserve.
    pub request_bytes: usize,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            queued: 32,
            queued_per_class: 16,
            active: 3,
            active_per_class: [1, 1, 1],
            reserved_bytes: 16 * 1024 * 1024,
            reserved_bytes_per_class: [8 * 1024 * 1024, 4 * 1024 * 1024, 8 * 1024 * 1024],
            request_bytes: 1024 * 1024,
        }
    }
}
impl ResourceLimits {
    fn validate(self) -> Result<(), ResourceError> {
        if self.queued == 0
            || self.queued_per_class == 0
            || self.queued_per_class > self.queued
            || self.active == 0
            || self.active_per_class.contains(&0)
            || self.reserved_bytes == 0
            || self.reserved_bytes_per_class.contains(&0)
            || self.request_bytes == 0
            || self.request_bytes > self.reserved_bytes
            || self
                .reserved_bytes_per_class
                .iter()
                .any(|n| *n > self.reserved_bytes)
            || self.request_bytes
                > self
                    .reserved_bytes_per_class
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0)
        {
            return Err(ResourceError::InvalidLimits);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceError {
    InvalidLimits,
    InvalidRequest,
    CapacityExceeded,
    UnknownRequest,
    LockPoisoned,
    Cancelled,
}
impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ResourceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cancellation {
    /// A queued request was removed and its reservation released immediately.
    Effective,
    /// Active work was signaled and must observe the lease token cooperatively.
    Requested,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClassSnapshot {
    pub queued: usize,
    pub active: usize,
    pub reserved_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub queued: usize,
    pub active: usize,
    pub reserved_bytes: usize,
    pub classes: [ClassSnapshot; 3],
}
impl ResourceSnapshot {
    pub fn class(&self, class: ResourceClass) -> ClassSnapshot {
        self.classes[class.index()]
    }
}

struct Request {
    id: u64,
    class: ResourceClass,
    bytes: usize,
    cancelled: Arc<AtomicBool>,
}
struct Active {
    id: u64,
    class: ResourceClass,
    cancelled: Arc<AtomicBool>,
}
struct State {
    pending: VecDeque<Request>,
    active: Vec<Active>,
    reserved_bytes: usize,
    reserved_by_class: [usize; 3],
    queued_by_class: [usize; 3],
    active_by_class: [usize; 3],
    next_id: u64,
    next_class: usize,
}
struct Inner {
    limits: ResourceLimits,
    state: Mutex<State>,
}

/// Cloneable, thread-safe bounded admission manager.
#[derive(Clone)]
pub struct ResourceManager {
    inner: Arc<Inner>,
}
impl ResourceManager {
    pub fn new(limits: ResourceLimits) -> Result<Self, ResourceError> {
        limits.validate()?;
        Ok(Self {
            inner: Arc::new(Inner {
                limits,
                state: Mutex::new(State {
                    pending: VecDeque::new(),
                    active: Vec::new(),
                    reserved_bytes: 0,
                    reserved_by_class: [0; 3],
                    queued_by_class: [0; 3],
                    active_by_class: [0; 3],
                    next_id: 1,
                    next_class: 0,
                }),
            }),
        })
    }

    /// Reserve declared transient bytes and enqueue atomically or leave state unchanged.
    pub fn admit(&self, class: ResourceClass, bytes: usize) -> Result<u64, ResourceError> {
        if bytes == 0 || bytes > self.inner.limits.request_bytes {
            return Err(ResourceError::InvalidRequest);
        }
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| ResourceError::LockPoisoned)?;
        let index = class.index();
        if state.pending.len() >= self.inner.limits.queued
            || state.queued_by_class[index] >= self.inner.limits.queued_per_class
            || state
                .reserved_bytes
                .checked_add(bytes)
                .is_none_or(|n| n > self.inner.limits.reserved_bytes)
            || state.reserved_by_class[index]
                .checked_add(bytes)
                .is_none_or(|n| n > self.inner.limits.reserved_bytes_per_class[index])
        {
            return Err(ResourceError::CapacityExceeded);
        }
        let id = state.next_id;
        state.next_id = state
            .next_id
            .checked_add(1)
            .ok_or(ResourceError::CapacityExceeded)?;
        state.reserved_bytes += bytes;
        state.reserved_by_class[index] += bytes;
        state.queued_by_class[index] += 1;
        state.pending.push_back(Request {
            id,
            class,
            bytes,
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        Ok(id)
    }

    /// Dispatch one request using FIFO within each class and round-robin class priority.
    pub fn dispatch_next(&self) -> Result<Option<ResourceLease>, ResourceError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| ResourceError::LockPoisoned)?;
        if state.active.len() >= self.inner.limits.active {
            return Ok(None);
        }
        let mut selected = None;
        for offset in 0..ResourceClass::ALL.len() {
            let index = (state.next_class + offset) % ResourceClass::ALL.len();
            let class = ResourceClass::ALL[index];
            if state.active_by_class[index] >= self.inner.limits.active_per_class[index] {
                continue;
            }
            if let Some(position) = state
                .pending
                .iter()
                .position(|request| request.class == class)
            {
                selected = Some((position, index));
                break;
            }
        }
        let Some((position, index)) = selected else {
            return Ok(None);
        };
        let request = state
            .pending
            .remove(position)
            .expect("selected pending request");
        state.queued_by_class[index] -= 1;
        state.active_by_class[index] += 1;
        state.next_class = (index + 1) % ResourceClass::ALL.len();
        state.active.push(Active {
            id: request.id,
            class: request.class,
            cancelled: request.cancelled.clone(),
        });
        Ok(Some(ResourceLease {
            manager: Arc::downgrade(&self.inner),
            id: request.id,
            class: request.class,
            bytes: request.bytes,
            cancelled: request.cancelled,
            released: false,
        }))
    }

    /// Remove pending work immediately; active work receives a cooperative stop signal.
    pub fn cancel(&self, id: u64) -> Result<Cancellation, ResourceError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| ResourceError::LockPoisoned)?;
        if let Some(position) = state.pending.iter().position(|request| request.id == id) {
            let request = state
                .pending
                .remove(position)
                .expect("selected pending request");
            let index = request.class.index();
            request.cancelled.store(true, Ordering::Release);
            state.queued_by_class[index] -= 1;
            state.reserved_by_class[index] -= request.bytes;
            state.reserved_bytes -= request.bytes;
            return Ok(Cancellation::Effective);
        }
        if let Some(active) = state.active.iter().find(|active| active.id == id) {
            active.cancelled.store(true, Ordering::Release);
            return Ok(Cancellation::Requested);
        }
        Err(ResourceError::UnknownRequest)
    }

    pub fn snapshot(&self) -> Result<ResourceSnapshot, ResourceError> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| ResourceError::LockPoisoned)?;
        let mut classes = [ClassSnapshot::default(); 3];
        for (i, class) in classes.iter_mut().enumerate() {
            *class = ClassSnapshot {
                queued: state.queued_by_class[i],
                active: state.active_by_class[i],
                reserved_bytes: state.reserved_by_class[i],
            };
        }
        Ok(ResourceSnapshot {
            queued: state.pending.len(),
            active: state.active.len(),
            reserved_bytes: state.reserved_bytes,
            classes,
        })
    }
}

/// Active resource reservation. A cancelled lease remains reserved until its
/// worker observes `check_cancelled` and drops/finishes the lease.
pub struct ResourceLease {
    manager: Weak<Inner>,
    id: u64,
    class: ResourceClass,
    bytes: usize,
    cancelled: Arc<AtomicBool>,
    released: bool,
}
impl ResourceLease {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn class(&self) -> ResourceClass {
        self.class
    }
    pub fn reserved_bytes(&self) -> usize {
        self.bytes
    }
    pub fn cancellation_requested(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn check_cancelled(&self) -> Result<(), ResourceError> {
        if self.cancellation_requested() {
            Err(ResourceError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn finish(mut self) {
        self.release();
    }
    fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let Some(manager) = self.manager.upgrade() else {
            return;
        };
        let mut state = match manager.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(position) = state.active.iter().position(|active| active.id == self.id) {
            let active = state.active.remove(position);
            let index = active.class.index();
            state.active_by_class[index] -= 1;
            state.reserved_by_class[index] -= self.bytes;
            state.reserved_bytes -= self.bytes;
        }
    }
}
impl Drop for ResourceLease {
    fn drop(&mut self) {
        self.release();
    }
}
