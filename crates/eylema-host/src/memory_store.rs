//! Owned durable attributed memory. Persistence is not permission or source truth.
use crate::{
    Owner,
    project_data::{DataClock, DataError, DataSession, ProjectStore},
};
use eylema_contracts::{
    Revision,
    memory::durable::*,
    project_data::{DataRef, DataUse},
};
use eylema_journal::{Journal, JournalError, Limits, Sha256, Storage};
use eylema_retrieval::{CancelToken, Control, RetrievalBudget, RetrievalError};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
mod guard;
mod query;
mod wire;
mod write;
use guard::{Gate, GuardedStorage, Mode};
pub use query::{MemoryCachedQuery, MemoryCitation, MemoryContext, MemoryStatus};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryStoreError {
    InvalidRequest,
    Forbidden,
    Conflict,
    IncompleteConflict,
    IncompatibleProfile,
    NeedsReopen,
    WriteOutcomeUnknown,
    Limit,
    Cancelled,
    Deadline,
    TokenCounterUnavailable,
    Backend,
}
pub type MemoryResult<T> = std::result::Result<T, MemoryStoreError>;
impl From<RecordError> for MemoryStoreError {
    fn from(e: RecordError) -> Self {
        match e {
            RecordError::Limit => Self::Limit,
            RecordError::Incompatible => Self::IncompatibleProfile,
            _ => Self::InvalidRequest,
        }
    }
}
impl From<DataError> for MemoryStoreError {
    fn from(e: DataError) -> Self {
        match e {
            DataError::Forbidden
            | DataError::Expired
            | DataError::WrongProject
            | DataError::NotFound => Self::Forbidden,
            DataError::BudgetExceeded => Self::Limit,
            _ => Self::Backend,
        }
    }
}
impl From<RetrievalError> for MemoryStoreError {
    fn from(e: RetrievalError) -> Self {
        match e {
            RetrievalError::Cancelled => Self::Cancelled,
            RetrievalError::Deadline => Self::Deadline,
            RetrievalError::LimitExceeded => Self::Limit,
            RetrievalError::TokenCounterUnavailable => Self::TokenCounterUnavailable,
            _ => Self::IncompatibleProfile,
        }
    }
}
impl From<JournalError> for MemoryStoreError {
    fn from(e: JournalError) -> Self {
        match e {
            JournalError::WriteUncertain { .. } => Self::WriteOutcomeUnknown,
            JournalError::NeedsRecovery | JournalError::ExternalChange => Self::NeedsReopen,
            JournalError::BudgetExceeded => Self::Limit,
            _ => Self::Backend,
        }
    }
}
#[derive(Clone, Default)]
pub struct MemoryCancel {
    flag: Arc<AtomicBool>,
    child: CancelToken,
}
impl MemoryCancel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.child.cancel()
    }
}
pub struct MemoryBudget {
    pub max_work: u64,
    pub max_live_bytes: usize,
    pub deadline: Instant,
}
pub struct MemoryControl {
    inner: Control,
    cancel: MemoryCancel,
    deadline: Instant,
    credits: usize,
}
impl MemoryControl {
    pub fn new(b: MemoryBudget, cancel: MemoryCancel) -> MemoryResult<Self> {
        if b.deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(5) {
            return Err(MemoryStoreError::Limit);
        }
        let mut inner = Control::new(
            RetrievalBudget {
                max_work: b.max_work,
                max_live_bytes: b.max_live_bytes,
                deadline: b.deadline,
            },
            cancel.child.clone(),
        )?;
        inner.reserve(8 * 1024 * 1024)?;
        Ok(Self {
            inner,
            cancel,
            deadline: b.deadline,
            credits: 0,
        })
    }
    pub fn work_used(&self) -> u64 {
        self.inner.work_used()
    }
    pub fn peak_live_bytes(&self) -> usize {
        self.inner.peak_live_bytes()
    }
    fn prepay_calls(&mut self, n: usize) -> MemoryResult<()> {
        self.inner
            .charge(n.checked_mul(4194304).ok_or(MemoryStoreError::Limit)?)?;
        self.credits = self.credits.checked_add(n).ok_or(MemoryStoreError::Limit)?;
        Ok(())
    }
    fn call<U, E: Into<MemoryStoreError>>(
        &mut self,
        f: impl FnOnce() -> std::result::Result<U, E>,
    ) -> MemoryResult<U> {
        if self.credits > 0 {
            self.credits -= 1
        } else {
            self.inner.charge(4194304)?
        }
        self.inner.reserve(524288)?;
        let result = f().map_err(Into::into);
        self.inner.release(524288);
        self.inner.check()?;
        result
    }
    pub(super) fn inner_mut(&mut self) -> &mut Control {
        &mut self.inner
    }
}
pub struct MemoryOwner {
    identity: Arc<()>,
}
pub struct MemorySession {
    identity: Arc<()>,
    data_identity: Arc<()>,
    source: DataSession,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryReceipt {
    pub generation: u64,
    pub revision: Revision,
    pub journal_digest: [u8; 32],
}
#[derive(Clone)]
struct Stored {
    record: Record,
    vector: Option<Vec<f32>>,
    deleted: bool,
}
struct SharedHash<H: Sha256>(Rc<RefCell<H>>);
impl<H: Sha256> Clone for SharedHash<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Sha256> Sha256 for SharedHash<H> {
    fn digest(&mut self, b: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        self.0.borrow_mut().digest(b)
    }
}
pub struct MemoryStore<T: Storage, H: Sha256> {
    identity: Arc<()>,
    data_identity: Arc<()>,
    profile: StoreProfile,
    journal: Journal<GuardedStorage<T>, SharedHash<H>>,
    gate: Gate,
    hash: SharedHash<H>,
    records: BTreeMap<RecordId, Stored>,
    generation: u64,
    poisoned: bool,
}
fn limits() -> Limits {
    Limits {
        records: 512,
        payload_bytes: 8192,
        file_bytes: 2097152,
    }
}
impl<T: Storage, H: Sha256> MemoryStore<T, H> {
    pub fn create_new<D: Storage, F: Sha256, C: DataClock>(
        storage: T,
        hash: H,
        data: &mut ProjectStore<D, F, C>,
        owner: &Owner,
        profile: StoreProfile,
        c: &mut MemoryControl,
    ) -> MemoryResult<(Self, MemoryOwner)> {
        Self::load(storage, hash, data, owner, profile, c, false)
    }
    pub fn open<D: Storage, F: Sha256, C: DataClock>(
        storage: T,
        hash: H,
        data: &mut ProjectStore<D, F, C>,
        owner: &Owner,
        profile: StoreProfile,
        c: &mut MemoryControl,
    ) -> MemoryResult<(Self, MemoryOwner)> {
        Self::load(storage, hash, data, owner, profile, c, true)
    }
    fn load<D: Storage, F: Sha256, C: DataClock>(
        storage: T,
        hash: H,
        data: &mut ProjectStore<D, F, C>,
        owner: &Owner,
        profile: StoreProfile,
        c: &mut MemoryControl,
        reopen: bool,
    ) -> MemoryResult<(Self, MemoryOwner)> {
        profile.validate()?;
        let expected = wire::header(&profile)?;
        let (project, data_identity) = c.call(|| {
            data.retrieval_binding(owner)
                .map(|(p, i)| (p.clone(), i.clone()))
        })?;
        if project != profile.project {
            return Err(MemoryStoreError::Forbidden);
        }
        let gate = Gate::default();
        let n = gate.begin(if reopen { Mode::Replay } else { Mode::Create }, c)?;
        let hash = SharedHash(Rc::new(RefCell::new(hash)));
        let guarded = GuardedStorage::new(storage, gate.clone());
        let opened = (|| -> MemoryResult<_> {
            let mut journal = if reopen {
                Journal::open(guarded, hash.clone(), limits())?
            } else {
                Journal::create_new(guarded, hash.clone(), limits())?
            };
            let records = if reopen {
                journal.read_records()?
            } else {
                let encoded = eylema_contracts::codec::encode(&expected)
                    .map_err(|_| MemoryStoreError::InvalidRequest)?;
                guard::PayloadGuard::validate(&encoded)?;
                journal.append(&expected)?;
                vec![]
            };
            Ok((journal, records))
        })();
        let (journal, events) = match opened {
            Ok(v) => v,
            Err(e) => {
                gate.end(n, c);
                return Err(e);
            }
        };
        let identity = Arc::new(());
        let owner = MemoryOwner {
            identity: identity.clone(),
        };
        let mut s = Self {
            identity,
            data_identity,
            profile,
            journal,
            gate,
            hash,
            records: BTreeMap::new(),
            generation: 0,
            poisoned: false,
        };
        let result = (|| -> MemoryResult<()> {
            if reopen {
                if events.first().is_none_or(|r| r.payload != expected) {
                    return Err(MemoryStoreError::IncompatibleProfile);
                }
                for event in events.into_iter().skip(1) {
                    s.apply(&event.payload, c)?;
                }
            }
            c.inner.check()?;
            Ok(())
        })();
        s.gate.end(n, c);
        result?;
        Ok((s, owner))
    }
    pub fn enroll<D: Storage, F: Sha256, C: DataClock>(
        &self,
        owner: &MemoryOwner,
        data: &mut ProjectStore<D, F, C>,
        session: &DataSession,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemorySession> {
        self.owner(owner)?;
        self.backing(data)?;
        c.inner.charge(128)?;
        Ok(MemorySession {
            identity: self.identity.clone(),
            data_identity: self.data_identity.clone(),
            source: session.clone(),
        })
    }
    fn owner(&self, o: &MemoryOwner) -> MemoryResult<()> {
        if !Arc::ptr_eq(&self.identity, &o.identity) {
            Err(MemoryStoreError::Forbidden)
        } else {
            Ok(())
        }
    }
    fn backing<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
    ) -> MemoryResult<()> {
        let (p, i) = data.memory_binding();
        if p != &self.profile.project || !Arc::ptr_eq(&self.data_identity, i) {
            Err(MemoryStoreError::Forbidden)
        } else {
            Ok(())
        }
    }
    fn session<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
        s: &MemorySession,
    ) -> MemoryResult<()> {
        if self.poisoned {
            return Err(MemoryStoreError::NeedsReopen);
        }
        self.backing(data)?;
        if !Arc::ptr_eq(&self.identity, &s.identity)
            || !Arc::ptr_eq(&self.data_identity, &s.data_identity)
        {
            Err(MemoryStoreError::Forbidden)
        } else {
            Ok(())
        }
    }
    fn pin<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
        session: &DataSession,
        p: &SourcePin,
        purpose: DataUse,
        c: &mut MemoryControl,
    ) -> MemoryResult<()> {
        let m =
            c.call(|| data.authorized_metadata_at_observed_time(session, &p.reference, purpose))?;
        if m.digest != p.put_digest {
            return Err(MemoryStoreError::Forbidden);
        }
        Ok(())
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn close(self) {
        self.journal.close()
    }
}
