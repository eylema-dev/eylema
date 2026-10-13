//! Separate owned project-store journal. Trusted host bootstrap is not human identity.
//! Purpose gates protect this API, not hostile same-user files or already copied bytes.
use crate::Owner;
use eylema_contracts::{Revision, codec::Value, platform::ProjectId, project_data::*};
use eylema_journal::{Journal, JournalError, Sha256, Storage};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const VERSION: u64 = 1;
pub const fn project_journal_limits() -> eylema_journal::Limits {
    eylema_journal::Limits {
        records: 4096,
        payload_bytes: 70 * 1024,
        file_bytes: 64 * 1024 * 1024,
    }
}
fn journal_profile<T: Storage, H: Sha256>(journal: &Journal<T, H>) -> DataResult<()> {
    let l = journal.limits();
    let max = project_journal_limits();
    if l.records > max.records
        || l.payload_bytes > max.payload_bytes
        || l.file_bytes > max.file_bytes
    {
        Err(DataError::InvalidRecord)
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataError {
    InvalidRecord,
    WrongProject,
    Forbidden,
    Expired,
    NotFound,
    RevisionConflict,
    BudgetExceeded,
    ClockUnavailable,
    ClockRollback,
    Poisoned,
    Journal(JournalError),
}
pub type DataResult<T> = std::result::Result<T, DataError>;
impl From<JournalError> for DataError {
    fn from(value: JournalError) -> Self {
        Self::Journal(value)
    }
}
impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DataError {}
/// Trusted host clock in Unix milliseconds. Request/model data cannot replace it.
pub trait DataClock {
    fn now(&mut self) -> DataResult<u64>;
}
pub struct NativeDataClock;
impl DataClock for NativeDataClock {
    fn now(&mut self) -> DataResult<u64> {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| DataError::ClockUnavailable)?
            .as_millis();
        u64::try_from(n).map_err(|_| DataError::ClockUnavailable)
    }
}
#[derive(Clone)]
pub struct DataSession {
    identity: Arc<()>,
    slot: usize,
}
impl std::fmt::Debug for DataSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSession(<opaque>)")
    }
}
struct Item {
    metadata: DataMetadata,
    payload: Vec<u8>,
    depth: usize,
}
struct Session {
    grant: DataGrant,
    revoked: bool,
}
pub struct ProjectStore<T: Storage, H: Sha256, C: DataClock> {
    journal: Journal<T, H>,
    clock: C,
    project: ProjectId,
    identity: Arc<()>,
    items: BTreeMap<DataId, Item>,
    sessions: Vec<Session>,
    payload_bytes: usize,
    last_now: u64,
}
fn list(v: Vec<Value>) -> Value {
    Value::List(v)
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn fields<const N: usize>(v: &Value) -> DataResult<&[Value; N]> {
    match v {
        Value::List(v) => v
            .as_slice()
            .try_into()
            .map_err(|_| DataError::InvalidRecord),
        _ => Err(DataError::InvalidRecord),
    }
}
fn number(v: &Value) -> DataResult<u64> {
    if let Value::Unsigned(n) = v {
        Ok(*n)
    } else {
        Err(DataError::InvalidRecord)
    }
}
fn string(v: &Value) -> DataResult<&str> {
    if let Value::Text(s) = v {
        Ok(s)
    } else {
        Err(DataError::InvalidRecord)
    }
}
fn boolean(v: &Value) -> DataResult<bool> {
    if let Value::Bool(b) = v {
        Ok(*b)
    } else {
        Err(DataError::InvalidRecord)
    }
}
fn id(v: &Value) -> DataResult<DataId> {
    DataId::new(string(v)?).map_err(|_| DataError::InvalidRecord)
}
fn revision(n: u64) -> DataResult<Revision> {
    Revision::new(n).map_err(|_| DataError::InvalidRecord)
}
fn expiry(now: u64, expires: u64) -> DataResult<()> {
    if expires <= now || expires - now > MAX_DATA_TTL_MS {
        Err(DataError::InvalidRecord)
    } else {
        Ok(())
    }
}
impl<T: Storage, H: Sha256, C: DataClock> ProjectStore<T, H, C> {
    pub fn create_new(
        mut journal: Journal<T, H>,
        project: ProjectId,
        mut clock: C,
    ) -> DataResult<(Self, Owner)> {
        journal_profile(&journal)?;
        if !journal.read_records()?.is_empty() {
            return Err(DataError::InvalidRecord);
        }
        let now = clock.now()?;
        if now == 0 {
            return Err(DataError::ClockUnavailable);
        }
        journal.append(&list(vec![
            text("project-data"),
            Value::Unsigned(VERSION),
            text(project.as_str()),
            Value::Unsigned(now),
        ]))?;
        Ok(Self::fresh(journal, project, clock, now))
    }
    fn fresh(journal: Journal<T, H>, project: ProjectId, clock: C, now: u64) -> (Self, Owner) {
        let identity = Arc::new(());
        let owner = Owner {
            identity: identity.clone(),
        };
        (
            Self {
                journal,
                clock,
                project,
                identity,
                items: BTreeMap::new(),
                sessions: vec![],
                payload_bytes: 0,
                last_now: now,
            },
            owner,
        )
    }
    pub fn open(
        mut journal: Journal<T, H>,
        expected: ProjectId,
        clock: C,
    ) -> DataResult<(Self, Owner)> {
        journal_profile(&journal)?;
        let records = journal.read_records()?;
        let [tag, version, project, now] =
            fields(&records.first().ok_or(DataError::InvalidRecord)?.payload)?;
        if string(tag)? != "project-data" || number(version)? != VERSION {
            return Err(DataError::InvalidRecord);
        }
        let project = ProjectId::new(string(project)?).map_err(|_| DataError::InvalidRecord)?;
        if project != expected {
            return Err(DataError::WrongProject);
        }
        let now = number(now)?;
        if now == 0 {
            return Err(DataError::InvalidRecord);
        }
        let (mut s, o) = Self::fresh(journal, project, clock, now);
        for r in records.iter().skip(1) {
            s.replay(&r.payload, r.digest)?;
        }
        s.observe_time()?;
        Ok((s, o))
    }
    fn healthy(&self) -> DataResult<()> {
        if self.journal.is_poisoned() {
            Err(DataError::Poisoned)
        } else {
            Ok(())
        }
    }
    fn owner(&self, o: &Owner) -> DataResult<()> {
        self.healthy()?;
        if Arc::ptr_eq(&self.identity, &o.identity) {
            Ok(())
        } else {
            Err(DataError::Forbidden)
        }
    }
    fn session(&self, s: &DataSession, id: &DataId, purpose: DataUse) -> DataResult<()> {
        self.healthy()?;
        if !Arc::ptr_eq(&self.identity, &s.identity) {
            return Err(DataError::Forbidden);
        }
        let slot = self.sessions.get(s.slot).ok_or(DataError::Forbidden)?;
        if slot.revoked || !slot.grant.permissions.allows(purpose) || !slot.grant.scope.contains(id)
        {
            return Err(DataError::Forbidden);
        }
        Ok(())
    }
    fn session_expiry(&self, s: &DataSession, now: u64) -> DataResult<()> {
        if self.sessions[s.slot].grant.expires_at <= now {
            Err(DataError::Expired)
        } else {
            Ok(())
        }
    }
    fn scoped(&self, r: &DataRef) -> DataResult<()> {
        if r.project == self.project {
            Ok(())
        } else {
            Err(DataError::WrongProject)
        }
    }
    fn observe_time(&mut self) -> DataResult<u64> {
        self.healthy()?;
        let now = self.clock.now()?;
        if now < self.last_now {
            return Err(DataError::ClockRollback);
        }
        if now > self.last_now {
            self.journal
                .append(&list(vec![text("clock"), Value::Unsigned(now)]))?;
            self.last_now = now;
        }
        Ok(now)
    }
    pub fn enroll(&mut self, o: &Owner, grant: DataGrant) -> DataResult<DataSession> {
        self.owner(o)?;
        if self.sessions.len() >= MAX_DATA_SESSIONS || grant.scope.len() > MAX_DATA_SCOPE_IDS {
            return Err(DataError::BudgetExceeded);
        }
        if grant.scope.iter().collect::<BTreeSet<_>>().len() != grant.scope.len() {
            return Err(DataError::InvalidRecord);
        }
        let now = self.observe_time()?;
        expiry(now, grant.expires_at)?;
        let slot = self.sessions.len();
        self.sessions.push(Session {
            grant,
            revoked: false,
        });
        Ok(DataSession {
            identity: self.identity.clone(),
            slot,
        })
    }
    pub fn revoke(&mut self, o: &Owner, s: &DataSession) -> DataResult<()> {
        self.owner(o)?;
        if !Arc::ptr_eq(&self.identity, &s.identity) {
            return Err(DataError::Forbidden);
        }
        self.sessions
            .get_mut(s.slot)
            .ok_or(DataError::Forbidden)?
            .revoked = true;
        Ok(())
    }
    pub fn inspect(&self, o: &Owner, r: &DataRef) -> DataResult<DataMetadata> {
        self.owner(o)?;
        self.scoped(r)?;
        Ok(self
            .items
            .get(&r.id)
            .ok_or(DataError::NotFound)?
            .metadata
            .clone())
    }
    fn permitted(&self, id: &DataId, purpose: DataUse, now: u64) -> DataResult<()> {
        let mut pending = vec![id];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let item = self.items.get(id).ok_or(DataError::NotFound)?;
            let m = &item.metadata;
            if m.withdrawn
                || !m.permissions.allows(purpose)
                || (purpose.is_egress() && !m.reviewed_egress)
            {
                return Err(DataError::Forbidden);
            }
            if m.expires_at <= now {
                return Err(DataError::Expired);
            }
            for p in &m.parents {
                pending.push(&p.id);
            }
        }
        Ok(())
    }
    fn validate_draft(&self, d: &DataDraft, now: u64) -> DataResult<usize> {
        self.scoped(&d.reference)?;
        for p in &d.parents {
            self.scoped(p)?;
        }
        d.validate().map_err(|_| DataError::InvalidRecord)?;
        expiry(now, d.expires_at)?;
        if self.items.contains_key(&d.reference.id) {
            return Err(DataError::RevisionConflict);
        }
        if self.items.len() >= MAX_DATA_ITEMS
            || self
                .payload_bytes
                .checked_add(d.payload.len())
                .is_none_or(|n| n > MAX_DATA_STORE_BYTES)
        {
            return Err(DataError::BudgetExceeded);
        }
        let mut depth = 1;
        for p in &d.parents {
            let source = self.items.get(&p.id).ok_or(DataError::NotFound)?;
            if source.metadata.withdrawn {
                return Err(DataError::Forbidden);
            }
            if source.metadata.expires_at <= now {
                return Err(DataError::Expired);
            }
            if d.expires_at > source.metadata.expires_at {
                return Err(DataError::InvalidRecord);
            }
            depth = depth.max(source.depth + 1);
        }
        if depth > MAX_DATA_DEPTH {
            return Err(DataError::BudgetExceeded);
        }
        Ok(depth)
    }
    pub fn collect(&mut self, s: &DataSession, d: DataDraft) -> DataResult<DataMetadata> {
        self.scoped(&d.reference)?;
        for p in &d.parents {
            self.scoped(p)?;
        }
        self.session(s, &d.reference.id, DataUse::Collect)?;
        let now = self.observe_time()?;
        self.session_expiry(s, now)?;
        let depth = self.validate_draft(&d, now)?;
        for p in &d.parents {
            self.session(s, &p.id, DataUse::Collect)?;
            self.permitted(&p.id, DataUse::Collect, now)?;
        }
        let event = list(vec![
            text("put"),
            text(d.reference.id.as_str()),
            text(d.kind.as_str()),
            list(d.parents.iter().map(|p| text(p.id.as_str())).collect()),
            Value::Bytes(d.payload.clone()),
            Value::Unsigned(d.expires_at),
        ]);
        let receipt = self.journal.append(&event)?;
        let metadata = DataMetadata {
            reference: d.reference.clone(),
            kind: d.kind,
            parents: d.parents,
            digest: receipt.digest,
            revision: revision(1)?,
            permissions: DataPermissions::NONE,
            reviewed_egress: false,
            expires_at: d.expires_at,
            withdrawn: false,
        };
        self.payload_bytes += d.payload.len();
        self.items.insert(
            d.reference.id,
            Item {
                metadata: metadata.clone(),
                payload: d.payload,
                depth,
            },
        );
        Ok(metadata)
    }
    pub fn set_policy(
        &mut self,
        o: &Owner,
        r: &DataRef,
        expected: Revision,
        permissions: DataPermissions,
        reviewed_egress: bool,
    ) -> DataResult<DataMetadata> {
        self.owner(o)?;
        self.scoped(r)?;
        let now = self.observe_time()?;
        let item = self.items.get(&r.id).ok_or(DataError::NotFound)?;
        if item.metadata.withdrawn {
            return Err(DataError::Forbidden);
        }
        if item.metadata.expires_at <= now {
            return Err(DataError::Expired);
        }
        if item.metadata.revision != expected {
            return Err(DataError::RevisionConflict);
        }
        let next = expected
            .get()
            .checked_add(1)
            .ok_or(DataError::BudgetExceeded)?;
        let event = list(vec![
            text("policy"),
            text(r.id.as_str()),
            Value::Unsigned(next),
            Value::Unsigned(permissions.bits() as u64),
            Value::Bool(reviewed_egress),
        ]);
        self.journal.append(&event)?;
        let m = &mut self.items.get_mut(&r.id).expect("validated item").metadata;
        m.revision = revision(next)?;
        m.permissions = permissions;
        m.reviewed_egress = reviewed_egress;
        Ok(m.clone())
    }
    pub fn withdraw(
        &mut self,
        o: &Owner,
        r: &DataRef,
        expected: Revision,
    ) -> DataResult<DataMetadata> {
        self.owner(o)?;
        self.scoped(r)?;
        self.observe_time()?;
        let item = self.items.get(&r.id).ok_or(DataError::NotFound)?;
        if item.metadata.withdrawn {
            return Err(DataError::Forbidden);
        }
        if item.metadata.revision != expected {
            return Err(DataError::RevisionConflict);
        }
        let next = expected
            .get()
            .checked_add(1)
            .ok_or(DataError::BudgetExceeded)?;
        self.journal.append(&list(vec![
            text("withdraw"),
            text(r.id.as_str()),
            Value::Unsigned(next),
        ]))?;
        let m = &mut self.items.get_mut(&r.id).expect("validated item").metadata;
        m.revision = revision(next)?;
        m.withdrawn = true;
        m.permissions = DataPermissions::NONE;
        m.reviewed_egress = false;
        Ok(m.clone())
    }
    fn authorize(&mut self, s: &DataSession, r: &DataRef, purpose: DataUse) -> DataResult<()> {
        self.scoped(r)?;
        self.session(s, &r.id, purpose)?;
        let now = self.observe_time()?;
        self.session_expiry(s, now)?;
        self.permitted(&r.id, purpose, now)
    }
    pub fn read(&mut self, s: &DataSession, r: &DataRef, purpose: DataUse) -> DataResult<Vec<u8>> {
        if purpose.is_egress() {
            return Err(DataError::Forbidden);
        }
        self.authorize(s, r, purpose)?;
        Ok(self
            .items
            .get(&r.id)
            .expect("authorized item")
            .payload
            .clone())
    }
    pub fn export_manifest(
        &mut self,
        s: &DataSession,
        r: &DataRef,
        purpose: DataUse,
    ) -> DataResult<DataManifest> {
        if !purpose.is_egress() {
            return Err(DataError::Forbidden);
        }
        self.authorize(s, r, purpose)?;
        let m = &self.items.get(&r.id).expect("authorized item").metadata;
        Ok(DataManifest {
            reference: m.reference.clone(),
            kind: m.kind,
            parents: m.parents.clone(),
            digest: m.digest,
            revision: m.revision,
            expires_at: m.expires_at,
        })
    }
    fn replay(&mut self, event: &Value, record_digest: [u8; 32]) -> DataResult<()> {
        let Value::List(v) = event else {
            return Err(DataError::InvalidRecord);
        };
        let tag = string(v.first().ok_or(DataError::InvalidRecord)?)?;
        match tag {
            "clock" => {
                let [_, n] = fields(event)?;
                let n = number(n)?;
                if n <= self.last_now {
                    return Err(DataError::InvalidRecord);
                }
                self.last_now = n;
            }
            "put" => {
                let [_, i, kind, parents, payload, expires] = fields(event)?;
                let Value::List(parents) = parents else {
                    return Err(DataError::InvalidRecord);
                };
                if parents.len() > MAX_DATA_PARENTS {
                    return Err(DataError::BudgetExceeded);
                }
                let Value::Bytes(payload) = payload else {
                    return Err(DataError::InvalidRecord);
                };
                let d = DataDraft {
                    reference: DataRef {
                        project: self.project.clone(),
                        id: id(i)?,
                    },
                    kind: DataKind::parse(string(kind)?).map_err(|_| DataError::InvalidRecord)?,
                    parents: parents
                        .iter()
                        .map(|p| {
                            Ok(DataRef {
                                project: self.project.clone(),
                                id: id(p)?,
                            })
                        })
                        .collect::<DataResult<_>>()?,
                    payload: payload.clone(),
                    expires_at: number(expires)?,
                };
                let depth = self.validate_draft(&d, self.last_now)?;
                for p in &d.parents {
                    self.permitted(&p.id, DataUse::Collect, self.last_now)?;
                }
                self.payload_bytes += d.payload.len();
                let metadata = DataMetadata {
                    reference: d.reference.clone(),
                    kind: d.kind,
                    parents: d.parents,
                    digest: record_digest,
                    revision: revision(1)?,
                    permissions: DataPermissions::NONE,
                    reviewed_egress: false,
                    expires_at: d.expires_at,
                    withdrawn: false,
                };
                self.items.insert(
                    d.reference.id,
                    Item {
                        metadata,
                        payload: d.payload,
                        depth,
                    },
                );
            }
            "policy" | "withdraw" => {
                let i = id(v.get(1).ok_or(DataError::InvalidRecord)?)?;
                let item = self.items.get_mut(&i).ok_or(DataError::InvalidRecord)?;
                if item.metadata.withdrawn {
                    return Err(DataError::InvalidRecord);
                }
                let next = number(v.get(2).ok_or(DataError::InvalidRecord)?)?;
                if item.metadata.revision.get().checked_add(1) != Some(next) {
                    return Err(DataError::InvalidRecord);
                }
                if tag == "policy" {
                    let [_, _, _, p, reviewed] = fields(event)?;
                    if item.metadata.expires_at <= self.last_now {
                        return Err(DataError::InvalidRecord);
                    }
                    item.metadata.permissions = DataPermissions::from_bits(number(p)?)
                        .map_err(|_| DataError::InvalidRecord)?;
                    item.metadata.reviewed_egress = boolean(reviewed)?;
                } else {
                    let [_, _, _] = fields(event)?;
                    item.metadata.withdrawn = true;
                    item.metadata.permissions = DataPermissions::NONE;
                    item.metadata.reviewed_egress = false;
                }
                item.metadata.revision = revision(next)?;
            }
            _ => return Err(DataError::InvalidRecord),
        }
        Ok(())
    }
    pub fn close(self) {
        drop(self)
    }
}
