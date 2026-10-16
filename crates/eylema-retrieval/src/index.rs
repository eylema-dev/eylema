use crate::{guard::SnapshotGuard, wire, *};
use RetrievalError as E;
use eylema_contracts::codec::Value;
use eylema_contracts::platform::ProjectId;
use eylema_journal::{Journal, JournalError, Limits, Sha256, Storage, SynchronizedReceipt};
use std::{
    io,
    sync::{Arc, Mutex},
};
pub struct IndexStore<T: Storage, H: Sha256> {
    journal: Journal<SnapshotGuard<T>, H>,
    current: Snapshot,
    content_hash: H,
    poisoned: bool,
    error: Arc<Mutex<Option<E>>>,
}
fn limits() -> Limits {
    Limits {
        records: 4,
        payload_bytes: wire::MAX_SNAPSHOT,
        file_bytes: 5 * 1024 * 1024,
    }
}
fn admit(n: usize, c: &mut Control) -> RetrievalResult<()> {
    if n > 5 * 1024 * 1024 {
        return Err(E::LimitExceeded);
    }
    let nodes = 4usize * 32768;
    let envelope = nodes
        .checked_mul(std::mem::size_of::<Value>() * 2)
        .and_then(|x| {
            x.checked_add(n * 6 + wire::MAX_SNAPSHOT + 8 * 128 * std::mem::size_of::<Value>())
        })
        .ok_or(E::LimitExceeded)?;
    c.reserve(envelope)?;
    c.charge(n * 12 + nodes * 2)?;
    Ok(())
}
fn error(e: JournalError, guard: &Arc<Mutex<Option<E>>>) -> E {
    guard
        .lock()
        .expect("guard error lock")
        .unwrap_or_else(|| match e {
            JournalError::Io {
                kind: io::ErrorKind::InvalidData,
                ..
            } => E::CorruptSnapshot,
            _ => E::Journal(e),
        })
}
impl<T: Storage, H: Sha256> IndexStore<T, H> {
    pub fn create(
        storage: T,
        journal_hash: H,
        content_hash: H,
        initial: Snapshot,
        c: &mut Control,
    ) -> RetrievalResult<Self> {
        if initial.generation() != 1 {
            return Err(E::StaleGeneration);
        }
        c.reserve(initial.retained_bytes())?;
        let v = initial.encode(c)?;
        let n = wire::extent(&v, c)?;
        c.charge(n * 8)?;
        c.reserve(n * 6)?;
        let (deadline, token) = c.read_guard();
        let guard = SnapshotGuard::new(storage, deadline, token);
        let err = guard.error();
        c.check()?;
        let mut journal =
            Journal::create_new(guard, journal_hash, limits()).map_err(|e| error(e, &err))?;
        c.check()?;
        journal.append(&v).map_err(|e| error(e, &err))?;
        c.check()?;
        Ok(Self {
            journal,
            current: initial,
            content_hash,
            poisoned: false,
            error: err,
        })
    }
    pub fn open(
        mut storage: T,
        journal_hash: H,
        mut content_hash: H,
        project: &ProjectId,
        binding: &RepresentationBinding,
        c: &mut Control,
    ) -> RetrievalResult<Self> {
        binding.validate()?;
        let n = storage.byte_len().map_err(|e| {
            E::Journal(JournalError::Io {
                stage: eylema_journal::IoStage::Metadata,
                kind: e.kind(),
            })
        })?;
        let n = usize::try_from(n).map_err(|_| E::LimitExceeded)?;
        admit(n, c)?;
        let (deadline, token) = c.read_guard();
        let guard = SnapshotGuard::new(storage, deadline, token);
        let err = guard.error();
        let mut journal =
            Journal::open(guard, journal_hash, limits()).map_err(|e| error(e, &err))?;
        let records = journal.read_records().map_err(|e| error(e, &err))?;
        if records.is_empty() {
            return Err(E::CorruptSnapshot);
        }
        let mut current = None;
        for (i, r) in records.iter().enumerate() {
            let s = Snapshot::decode(&r.payload, &mut content_hash, c)?;
            if s.project() != project {
                return Err(E::WrongProject);
            }
            if s.binding() != binding {
                return Err(E::IncompatibleBinding);
            }
            if s.generation() != i as u64 + 1 {
                return Err(E::CorruptSnapshot);
            }
            current = Some(s);
        }
        c.check()?;
        Ok(Self {
            journal,
            current: current.ok_or(E::CorruptSnapshot)?,
            content_hash,
            poisoned: false,
            error: err,
        })
    }
    pub fn current(&self) -> &Snapshot {
        &self.current
    }
    pub fn require_reopen(&mut self) {
        self.poisoned = true;
    }
    pub fn append(
        &mut self,
        next: Snapshot,
        expected_generation: u64,
        c: &mut Control,
    ) -> RetrievalResult<SynchronizedReceipt> {
        if self.poisoned {
            return Err(E::NeedsReopen);
        }
        if expected_generation != self.current.generation()
            || expected_generation.checked_add(1) != Some(next.generation())
        {
            return Err(E::StaleGeneration);
        }
        if next.project() != self.current.project() {
            return Err(E::WrongProject);
        }
        if next.binding() != self.current.binding() {
            return Err(E::IncompatibleBinding);
        }
        c.reserve(self.current.retained_bytes() + next.retained_bytes())?;
        let v = next.encode(c)?;
        let n = wire::extent(&v, c)?;
        self.journal.check_append_capacity(&[n])?;
        c.reserve(n * 6)?;
        c.charge(n * 8)?;
        c.check()?;
        let receipt = match self.journal.append(&v) {
            Ok(r) => r,
            Err(e) => {
                self.poisoned = true;
                return Err(error(e, &self.error));
            }
        };
        if let Err(e) = c.check() {
            self.poisoned = true;
            return Err(e);
        }
        self.current = next;
        Ok(receipt)
    }
    /// Validate using the store's dedicated content hasher, never its journal hasher.
    pub fn build_next(
        &mut self,
        project: ProjectId,
        binding: RepresentationBinding,
        generation: u64,
        documents: Vec<DocumentInput>,
        c: &mut Control,
    ) -> RetrievalResult<Snapshot> {
        Snapshot::build(
            project,
            binding,
            generation,
            documents,
            &mut self.content_hash,
            c,
        )
    }
    pub fn is_reopen_required(&self) -> bool {
        self.poisoned
    }
}
