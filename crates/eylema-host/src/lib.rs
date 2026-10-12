//! Owned durable embedded host. Journal data is a privileged trusted-host replay
//! boundary, never public request authority or physical power-loss acceptance.
#![forbid(unsafe_code)]
use eylema_contracts::{
    codec::{self, Value},
    *,
};
use eylema_journal::{Journal, JournalError, Sha256, Storage};
use eylema_runtime::*;
use std::sync::Arc;
mod command;
mod encoding;
mod services;
use command::{Command, State};
use encoding::*;
const VERSION: u64 = 1;
const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_COMMIT_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostError {
    Runtime(ErrorCode),
    Journal(JournalError),
    InvalidRecord,
    IncompatibleFormat,
    ReplayMismatch,
    Poisoned,
}
impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for HostError {}
impl From<JournalError> for HostError {
    fn from(e: JournalError) -> Self {
        Self::Journal(e)
    }
}
pub type Result<T> = std::result::Result<T, HostError>;
pub struct Owner {
    identity: Arc<()>,
}
#[derive(Clone)]
pub struct DurableSession {
    identity: Arc<()>,
    slot: usize,
}
#[derive(Clone)]
pub struct DurableHuman(DurableSession);
impl DurableHuman {
    pub fn session(&self) -> &DurableSession {
        &self.0
    }
}
impl std::fmt::Debug for DurableSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DurableSession(<opaque>)")
    }
}
/// An abandoned intent is explicit uncertainty, never a successful old receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryReport {
    pub abandoned_attempt: Option<u64>,
    pub unfinished: Vec<ExecutionId>,
}
pub struct DurableHost<T: Storage, H: Sha256, S: HostServices> {
    identity: Arc<()>,
    journal: Journal<T, H>,
    state: State<S>,
    services: services::Services<S>,
    attempt: u64,
    active_from: usize,
    poisoned: bool,
    report: RecoveryReport,
}
fn encoded_length(v: &Value, limit: usize) -> Result<usize> {
    let n = codec::encode(v)
        .map_err(|_| HostError::InvalidRecord)?
        .len();
    if n > limit {
        return Err(HostError::Runtime(ErrorCode::BudgetExceeded));
    }
    Ok(n)
}
fn version(v: &Value) -> Result<()> {
    if unsigned(v)? != VERSION {
        Err(HostError::IncompatibleFormat)
    } else {
        Ok(())
    }
}
impl<T: Storage, H: Sha256, S: HostServices> DurableHost<T, H, S> {
    pub fn create_new(
        mut journal: Journal<T, H>,
        host_services: S,
        runtime_limits: eylema_runtime::Limits,
    ) -> Result<(Self, Owner)> {
        if !journal.read_records()?.is_empty() {
            return Err(HostError::InvalidRecord);
        }
        let services = services::Services::new(host_services);
        let state = State::new(services.clone(), runtime_limits)?;
        let config = list(vec![
            text("host"),
            Value::Unsigned(VERSION),
            limits(runtime_limits),
        ]);
        let n = encoded_length(&config, MAX_COMMAND_BYTES)?;
        journal.check_append_capacity(&[
            n,
            MAX_COMMIT_BYTES,
            MAX_COMMIT_BYTES,
            MAX_COMMIT_BYTES,
        ])?;
        journal.append(&config)?;
        let identity = Arc::new(());
        let owner = Owner {
            identity: identity.clone(),
        };
        Ok((
            Self {
                identity,
                journal,
                state,
                services,
                attempt: 0,
                active_from: 0,
                poisoned: false,
                report: RecoveryReport {
                    abandoned_attempt: None,
                    unfinished: vec![],
                },
            },
            owner,
        ))
    }
    pub fn open(mut journal: Journal<T, H>, host_services: S) -> Result<(Self, Owner)> {
        let records = journal.read_records()?;
        let config = &records.first().ok_or(HostError::InvalidRecord)?.payload;
        let [tag, v, l] = fields(config)?;
        if string(tag)? != "host" {
            return Err(HostError::InvalidRecord);
        }
        version(v)?;
        let services = services::Services::new(host_services);
        let mut state = State::new(services.clone(), decode_limits(l)?)?;
        let mut attempt = 0u64;
        let mut pending: Option<(u64, Command)> = None;
        for record in records.iter().skip(1) {
            let row = values(&record.payload, 8)?;
            let tag = string(row.first().ok_or(HostError::InvalidRecord)?)?;
            match tag {
                "intent" => {
                    let [_, v, n, c] = fields(&record.payload)?;
                    version(v)?;
                    if pending.is_some()
                        || unsigned(n)? != attempt.checked_add(1).ok_or(HostError::InvalidRecord)?
                    {
                        return Err(HostError::InvalidRecord);
                    }
                    encoded_length(&record.payload, MAX_COMMAND_BYTES)?;
                    attempt = unsigned(n)?;
                    pending = Some((attempt, Command::decode(c)?));
                }
                "commit" => {
                    let [_, v, n, result, transcript, digest] = fields(&record.payload)?;
                    version(v)?;
                    encoded_length(&record.payload, MAX_COMMIT_BYTES)?;
                    let (id, command) = pending.take().ok_or(HostError::InvalidRecord)?;
                    if unsigned(n)? != id {
                        return Err(HostError::InvalidRecord);
                    }
                    services.replay(decode_calls(transcript)?)?;
                    let actual = outcome(state.execute(&command));
                    services.finish()?;
                    if actual != *result || state.digest()? != hash(digest)? {
                        return Err(HostError::ReplayMismatch);
                    }
                }
                "recovery" => {
                    let [_, v, n, abandoned, result, transcript, digest, command] =
                        fields(&record.payload)?;
                    version(v)?;
                    encoded_length(&record.payload, MAX_COMMIT_BYTES)?;
                    if unsigned(n)? != attempt.checked_add(1).ok_or(HostError::InvalidRecord)? {
                        return Err(HostError::InvalidRecord);
                    }
                    let expected = pending
                        .as_ref()
                        .map(|p| p.0)
                        .map_or(Value::Null, Value::Unsigned);
                    if *abandoned != expected {
                        return Err(HostError::InvalidRecord);
                    }
                    let inflight = matches!(pending.as_ref().map(|p| &p.1), Some(Command::Step));
                    let command = Command::decode(command)?;
                    if !matches!(command,Command::Recover(p) if p==inflight) {
                        return Err(HostError::InvalidRecord);
                    }
                    services.replay(decode_calls(transcript)?)?;
                    let actual = outcome(state.execute(&command));
                    services.finish()?;
                    if actual != *result || state.digest()? != hash(digest)? {
                        return Err(HostError::ReplayMismatch);
                    }
                    decode_outcome(result)?;
                    pending = None;
                    attempt = unsigned(n)?;
                }
                _ => return Err(HostError::InvalidRecord),
            }
        }
        let abandoned = pending.as_ref().map(|p| p.0);
        let inflight = matches!(pending.as_ref().map(|p| &p.1), Some(Command::Step));
        let command = Command::Recover(inflight);
        journal.check_append_capacity(&[MAX_COMMIT_BYTES])?;
        services.record()?;
        let result = outcome(state.execute(&command));
        let transcript = services.finish()?;
        let ids = decode_outcome(&result)?;
        let unfinished = values(&ids, 65536)?
            .iter()
            .map(|v| typed(ExecutionId::new(string(v)?)))
            .collect::<Result<_>>()?;
        let digest = state.digest()?;
        attempt = attempt.checked_add(1).ok_or(HostError::InvalidRecord)?;
        let recovery = list(vec![
            text("recovery"),
            Value::Unsigned(VERSION),
            Value::Unsigned(attempt),
            abandoned.map_or(Value::Null, Value::Unsigned),
            result,
            calls(&transcript),
            Value::Bytes(digest.to_vec()),
            command.encode()?,
        ]);
        encoded_length(&recovery, MAX_COMMIT_BYTES)?;
        journal.append(&recovery)?;
        let active_from = state.slots.len();
        let identity = Arc::new(());
        let owner = Owner {
            identity: identity.clone(),
        };
        Ok((
            Self {
                identity,
                journal,
                state,
                services,
                attempt,
                active_from,
                poisoned: false,
                report: RecoveryReport {
                    abandoned_attempt: abandoned,
                    unfinished,
                },
            },
            owner,
        ))
    }
    fn healthy(&self) -> Result<()> {
        if self.poisoned {
            Err(HostError::Poisoned)
        } else {
            Ok(())
        }
    }
    fn owner(&self, owner: &Owner) -> Result<()> {
        self.healthy()?;
        if !Arc::ptr_eq(&self.identity, &owner.identity) {
            Err(HostError::Runtime(ErrorCode::Forbidden))
        } else {
            Ok(())
        }
    }
    fn session(&self, s: &DurableSession) -> Result<usize> {
        self.healthy()?;
        if !Arc::ptr_eq(&self.identity, &s.identity)
            || s.slot < self.active_from
            || s.slot >= self.state.slots.len()
        {
            Err(HostError::Runtime(ErrorCode::Forbidden))
        } else {
            Ok(s.slot)
        }
    }
    fn human(&self, h: &DurableHuman) -> Result<usize> {
        let slot = self.session(&h.0)?;
        if self.state.slots[slot].human.is_none() {
            Err(HostError::Runtime(ErrorCode::Forbidden))
        } else {
            Ok(slot)
        }
    }
    fn transaction(&mut self, command: Command) -> Result<Value> {
        self.healthy()?;
        let attempt = self
            .attempt
            .checked_add(1)
            .ok_or(HostError::Runtime(ErrorCode::BudgetExceeded))?;
        let encoded_command = command.encode()?;
        encoded_length(&encoded_command, MAX_COMMAND_BYTES)?;
        // The same-version reader must accept every intent we write. In
        // particular, public command collections can exceed decoder bounds
        // even when the runtime would return a known rejection.
        let replay_command = Command::decode(&encoded_command)
            .map_err(|_| HostError::Runtime(ErrorCode::InvalidRequest))?;
        if replay_command.encode()? != encoded_command {
            return Err(HostError::Runtime(ErrorCode::InvalidRequest));
        }
        let intent = list(vec![
            text("intent"),
            Value::Unsigned(VERSION),
            Value::Unsigned(attempt),
            encoded_command,
        ]);
        let n = encoded_length(&intent, MAX_COMMAND_BYTES)?;
        // One additional worst-case envelope stays available for recovery.
        self.journal
            .check_append_capacity(&[n, MAX_COMMIT_BYTES, MAX_COMMIT_BYTES])?;
        let result = (|| {
            self.journal.append(&intent)?;
            self.services.record()?;
            let result = outcome(self.state.execute(&command));
            let transcript = self.services.finish()?;
            let digest = self.state.digest()?;
            let commit = list(vec![
                text("commit"),
                Value::Unsigned(VERSION),
                Value::Unsigned(attempt),
                result.clone(),
                calls(&transcript),
                Value::Bytes(digest.to_vec()),
            ]);
            encoded_length(&commit, MAX_COMMIT_BYTES)?;
            self.journal.append(&commit)?;
            Ok(result)
        })();
        let result = match result {
            Ok(v) => v,
            Err(e) => {
                self.poisoned = true;
                return Err(e);
            }
        };
        self.attempt = attempt;
        decode_outcome(&result)
    }
    pub fn enroll(&mut self, owner: &Owner, grant: Grant) -> Result<DurableSession> {
        self.owner(owner)?;
        let slot = index(&self.transaction(Command::Enroll(grant, false))?)?;
        Ok(DurableSession {
            identity: self.identity.clone(),
            slot,
        })
    }
    /// Host attestation after independent verification, not a request flag.
    pub fn enroll_verified_human(&mut self, owner: &Owner, grant: Grant) -> Result<DurableHuman> {
        self.owner(owner)?;
        let slot = index(&self.transaction(Command::Enroll(grant, true))?)?;
        Ok(DurableHuman(DurableSession {
            identity: self.identity.clone(),
            slot,
        }))
    }
    pub fn create_task(&mut self, s: &DurableSession, t: TaskSpec) -> Result<()> {
        let slot = self.session(s)?;
        self.transaction(Command::Create(slot, t, None))?;
        Ok(())
    }
    pub fn create_governed_task(
        &mut self,
        owner: &Owner,
        s: &DurableSession,
        t: TaskSpec,
        o: OversightRequirements,
    ) -> Result<()> {
        self.owner(owner)?;
        let slot = self.session(s)?;
        self.transaction(Command::Create(slot, t, Some(o)))?;
        Ok(())
    }
    pub fn admit(
        &mut self,
        s: &DurableSession,
        t: TaskSpec,
        p: ActionProposal,
    ) -> Result<ExecutionId> {
        self.admit_with_approvals(s, t, p, &[])
    }
    pub fn admit_with_approvals(
        &mut self,
        s: &DurableSession,
        t: TaskSpec,
        p: ActionProposal,
        a: &[EventId],
    ) -> Result<ExecutionId> {
        let slot = self.session(s)?;
        typed(ExecutionId::new(string(
            &self.transaction(Command::Admit(slot, t, p, a.to_vec()))?,
        )?))
    }
    pub fn step(&mut self) -> Result<Option<ExecutionId>> {
        let v = self.transaction(Command::Step)?;
        if v == Value::Null {
            Ok(None)
        } else {
            Ok(Some(typed(ExecutionId::new(string(&v)?))?))
        }
    }
    pub fn preview_approval(
        &mut self,
        s: &DurableSession,
        t: &TaskSpec,
        p: &ActionProposal,
        e: Timestamp,
    ) -> Result<ApprovalBinding> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .preview_approval(&self.state.slots[slot].session, t, p, e)
            .map_err(HostError::Runtime)
    }
    pub fn create_approval(
        &mut self,
        h: &DurableHuman,
        t: &TaskSpec,
        p: &ActionProposal,
        b: ApprovalBinding,
    ) -> Result<EventId> {
        let slot = self.human(h)?;
        typed(EventId::new(string(&self.transaction(
            Command::Approve(slot, t.clone(), p.clone(), b),
        )?)?))
    }
    pub fn revoke_approval(&mut self, h: &DurableHuman, id: &EventId) -> Result<ApprovalRecord> {
        let slot = self.human(h)?;
        self.transaction(Command::RevokeApproval(slot, id.clone()))?;
        self.state
            .last_approval
            .take()
            .ok_or(HostError::ReplayMismatch)
    }
    pub fn register_governance(
        &mut self,
        h: &DurableHuman,
        t: &TaskId,
        g: GovernanceRef,
    ) -> Result<PolicyChange> {
        let slot = self.human(h)?;
        self.transaction(Command::Governance(slot, t.clone(), g.clone()))?;
        self.state
            .last_policy
            .take()
            .ok_or(HostError::ReplayMismatch)
    }
    pub fn advance_policy(
        &mut self,
        h: &DurableHuman,
        t: &TaskId,
        next: Revision,
    ) -> Result<PolicyChange> {
        let slot = self.human(h)?;
        self.transaction(Command::Policy(slot, t.clone(), next))?;
        self.state
            .last_policy
            .take()
            .ok_or(HostError::ReplayMismatch)
    }
    pub fn revoke_delegation(
        &mut self,
        h: &DurableHuman,
        t: &TaskId,
        d: &DurableSession,
    ) -> Result<Revocation> {
        let slot = self.human(h)?;
        let delegated = self.session(d)?;
        let value = self.transaction(Command::RevokeDelegation(slot, t.clone(), delegated))?;
        let [p, n] = fields(&value)?;
        Ok(Revocation {
            verified_principal: typed(PrincipalRef::new(string(p)?))?,
            timestamp: typed(Timestamp::new(unsigned(n)?))?,
        })
    }
    pub fn request_cancellation(
        &mut self,
        h: &DurableHuman,
        id: &ExecutionId,
    ) -> Result<CancellationOutcome> {
        let slot = self.human(h)?;
        let value = self.transaction(Command::Cancel(slot, id.clone()))?;
        let [kind, v] = fields(&value)?;
        match string(kind)? {
            "requested" => {
                let [e, r, p, t] = fields(v)?;
                Ok(CancellationOutcome::Requested(CancellationReceipt {
                    execution_id: typed(ExecutionId::new(string(e)?))?,
                    receipt_event: typed(EventId::new(string(r)?))?,
                    verified_principal: typed(PrincipalRef::new(string(p)?))?,
                    requested_at: typed(Timestamp::new(unsigned(t)?))?,
                }))
            }
            "terminal" => {
                let codec::WireRecord::Execution { execution, .. } = record(v)? else {
                    return Err(HostError::ReplayMismatch);
                };
                Ok(CancellationOutcome::AlreadyTerminal(execution))
            }
            _ => Err(HostError::ReplayMismatch),
        }
    }
    pub fn task(&self, s: &DurableSession, id: &TaskId) -> Result<TaskSnapshot> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .task(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn execution(&self, s: &DurableSession, id: &ExecutionId) -> Result<ExecutionRecord> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .execution(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn events(
        &self,
        s: &DurableSession,
        id: &TaskId,
        after: Option<&EventId>,
        limit: usize,
    ) -> Result<Vec<EventRecord>> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .events(&self.state.slots[slot].session, id, after, limit)
            .map_err(HostError::Runtime)
    }
    pub fn observation(&self, s: &DurableSession, id: &EvidenceRef) -> Result<Observation> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .observation(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn approval(&self, s: &DurableSession, id: &EventId) -> Result<ApprovalRecord> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .approval(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn accepted_oversight(
        &self,
        s: &DurableSession,
        id: &TaskId,
    ) -> Result<OversightRequirements> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .accepted_oversight(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn policy_changes(&self, s: &DurableSession, id: &TaskId) -> Result<Vec<PolicyChange>> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .policy_changes(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn governance_record(
        &self,
        s: &DurableSession,
        id: &GovernanceRef,
    ) -> Result<PolicyChange> {
        let slot = self.session(s)?;
        self.state
            .runtime
            .governance_record(&self.state.slots[slot].session, id)
            .map_err(HostError::Runtime)
    }
    pub fn recovery_report(&self) -> Result<&RecoveryReport> {
        self.healthy()?;
        Ok(&self.report)
    }
    pub fn close(self) {
        drop(self)
    }
}
