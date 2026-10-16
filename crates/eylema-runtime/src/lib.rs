//! Safe bounded embedded executor. Host enrollment is a trusted process boundary;
//! opaque handles are not network authentication or human approval credentials.
#![forbid(unsafe_code)]
use eylema_contracts::{
    codec::{self, Value},
    *,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

mod oversight;
mod recovery;
pub mod resource;
pub mod resource_estimate;
pub use oversight::*;

pub type Result<T> = std::result::Result<T, ErrorCode>;
/// The embedding host supplies observed time, standard crypto and fresh IDs.
/// This boundary is trusted and fallible; request data cannot install a provider.
pub trait HostServices {
    fn now(&mut self) -> Result<Timestamp>;
    fn execution_id(&mut self) -> Result<ExecutionId>;
    fn event_id(&mut self) -> Result<EventId>;
    fn sha256(&mut self, bytes: &[u8]) -> Result<[u8; 32]>;
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub tasks: usize,
    pub executions: usize,
    pub sessions: usize,
    pub queued: usize,
    pub events: usize,
    pub idempotency: usize,
    pub resident_wire_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            tasks: 64,
            executions: 128,
            sessions: 16,
            queued: 32,
            events: 4096,
            idempotency: 4096,
            resident_wire_bytes: 16 * 1024 * 1024,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<()> {
        if [
            self.tasks,
            self.executions,
            self.sessions,
            self.queued,
            self.events,
            self.idempotency,
        ]
        .into_iter()
        .any(|v| v == 0 || v > 65536)
            || self.resident_wire_bytes == 0
            || self.resident_wire_bytes > 1024 * 1024 * 1024
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(())
    }
}
/// Only Runtime::new returns this enrollment capability. No serialization.
pub struct HostKey {
    identity: Arc<()>,
}
#[derive(Clone)]
pub struct Session {
    identity: Arc<()>,
    slot: usize,
}
#[derive(Clone, Debug)]
pub struct Grant {
    pub principal: PrincipalRef,
    pub resource_scope: ResourceScope,
    pub permissions: Vec<Permission>,
    pub policy_revision: Revision,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSnapshot {
    pub task: TaskSpec,
    pub value: u64,
    pub performed_steps: u64,
    pub charged_steps: u64,
    pub charged_events: u64,
}
/// Actual observations of owned memory, keyed by the refs carried in events.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub evidence: EvidenceRef,
    pub event_id: EventId,
    pub task_id: TaskId,
    pub execution_id: Option<ExecutionId>,
    pub timestamp: Timestamp,
    pub before: u64,
    pub after: u64,
}
struct TaskState {
    snapshot: TaskSnapshot,
    owner: PrincipalRef,
    lease: Option<ExecutionId>,
    policy_revision: Revision,
    oversight: OversightRequirements,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Key(PrincipalRef, OperationId, Vec<ResourceId>, IdempotencyKey);
struct Replay {
    hash: [u8; 32],
    execution: ExecutionId,
}
struct Entry {
    task: TaskSpec,
    proposal: ActionProposal,
    draft: ExecutionDraft,
    session: Session,
    next: usize,
    expected_live: Revision,
    event_ids: VecDeque<EventId>,
    cancel_ids: VecDeque<EventId>,
    cancellation: Option<CancellationReceipt>,
    approval_ids: Vec<EventId>,
    payload_hash: [u8; 32],
}
struct EventMetadata<'a> {
    task: &'a TaskId,
    execution: Option<&'a ExecutionId>,
    revision: Revision,
    context: &'a RequestContext,
    kind: EventKind,
}
pub struct Runtime<S: HostServices> {
    identity: Arc<()>,
    services: S,
    limits: Limits,
    grants: Vec<Grant>,
    human_slots: BTreeSet<usize>,
    retired_slots: BTreeSet<usize>,
    revoked_slots: BTreeMap<usize, Revocation>,
    approvals: BTreeMap<EventId, ApprovalRecord>,
    governance: BTreeMap<GovernanceRef, GovernanceRecord>,
    policy_changes: Vec<PolicyChange>,
    tasks: BTreeMap<TaskId, TaskState>,
    executions: BTreeMap<ExecutionId, Entry>,
    queue: VecDeque<ExecutionId>,
    replays: BTreeMap<Key, Replay>,
    plans: BTreeMap<(ProposalId, u64), [u8; 32]>,
    events: Vec<EventRecord>,
    observations: BTreeMap<EvidenceRef, Observation>,
    event_ids: BTreeSet<EventId>,
    charged_event_slots: usize,
    resident_wire_bytes: usize,
}
impl<S: HostServices> Runtime<S> {
    pub fn new(services: S, limits: Limits) -> Result<(Self, HostKey)> {
        limits.validate()?;
        let identity = Arc::new(());
        let host = HostKey {
            identity: identity.clone(),
        };
        Ok((
            Self {
                identity,
                services,
                limits,
                grants: vec![],
                human_slots: BTreeSet::new(),
                retired_slots: BTreeSet::new(),
                revoked_slots: BTreeMap::new(),
                approvals: BTreeMap::new(),
                governance: BTreeMap::new(),
                policy_changes: vec![],
                tasks: BTreeMap::new(),
                executions: BTreeMap::new(),
                queue: VecDeque::new(),
                replays: BTreeMap::new(),
                plans: BTreeMap::new(),
                events: vec![],
                observations: BTreeMap::new(),
                event_ids: BTreeSet::new(),
                charged_event_slots: 0,
                resident_wire_bytes: 0,
            },
            host,
        ))
    }
    pub fn enroll(&mut self, host: &HostKey, grant: Grant) -> Result<Session> {
        if !Arc::ptr_eq(&self.identity, &host.identity) {
            return Err(ErrorCode::Forbidden);
        }
        if self.grants.len() >= self.limits.sessions {
            return Err(ErrorCode::BudgetExceeded);
        }
        if grant.permissions.is_empty()
            || grant.permissions.len() > 8
            || grant
                .permissions
                .iter()
                .enumerate()
                .any(|(i, v)| grant.permissions[..i].contains(v))
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let slot = self.grants.len();
        self.grants.push(grant);
        Ok(Session {
            identity: self.identity.clone(),
            slot,
        })
    }

    /// Trusted scheduler inspection; conveys no task data or new authority.
    pub fn has_pending_work(&self, host: &HostKey) -> Result<bool> {
        if !Arc::ptr_eq(&self.identity, &host.identity) {
            return Err(ErrorCode::Forbidden);
        }
        Ok(!self.queue.is_empty())
    }
    /// Read-only FIFO inspection for the trusted scheduler owner. This is exactly
    /// the head used by step(), not a mutable queue or session authority.
    pub fn pending_execution(&self, host: &HostKey) -> Result<Option<ExecutionId>> {
        if !Arc::ptr_eq(&self.identity, &host.identity) {
            return Err(ErrorCode::Forbidden);
        }
        Ok(self.queue.front().cloned())
    }
    fn grant(&self, session: &Session) -> Result<&Grant> {
        if !Arc::ptr_eq(&self.identity, &session.identity)
            || self.revoked_slots.contains_key(&session.slot)
            || self.retired_slots.contains(&session.slot)
        {
            return Err(ErrorCode::Forbidden);
        }
        self.grants.get(session.slot).ok_or(ErrorCode::Forbidden)
    }
    fn permission(
        &self,
        session: &Session,
        scope: &ResourceScope,
        permission: Permission,
    ) -> Result<&Grant> {
        let g = self.grant(session)?;
        if !g.permissions.contains(&permission) || !scope.is_subset_of(&g.resource_scope) {
            return Err(ErrorCode::Forbidden);
        }
        Ok(g)
    }
    fn authorized(&self, session: &Session, proposal: &ActionProposal) -> Result<()> {
        let grant = self.grant(session)?;
        if grant.policy_revision != proposal.fields().policy_revision {
            return Err(ErrorCode::RevisionConflict);
        }
        for action in &proposal.fields().actions {
            let a = action.fields();
            let required = &a.authority;
            self.permission(session, &a.resource_scope, Permission::ExecuteAction)?;
            if required.principal_ref() != &grant.principal
                || required.policy_revision() != grant.policy_revision
                || required
                    .permissions()
                    .iter()
                    .any(|p| !grant.permissions.contains(p))
            {
                return Err(ErrorCode::Forbidden);
            }
            self.check_oversight(session, &a.task_id, &a.resource_scope, required.oversight())?;
            if let Some(state) = self.tasks.get(&a.task_id) {
                if state.policy_revision != grant.policy_revision {
                    return Err(ErrorCode::RevisionConflict);
                }
                self.check_oversight(
                    session,
                    &a.task_id,
                    &state.snapshot.task.fields().resource_scope,
                    &state.oversight,
                )?;
            }
            if a.target.context() != ExecutionContext::NativeMacos
                || a.target.backend() != Backend::NativeCpu
                || a.target.gpu_api() != GpuApi::None
                || a.required_capabilities
                    .iter()
                    .any(|c| c.as_str() != "task:state")
            {
                return Err(ErrorCode::CapabilityUnavailable);
            }
            match a.operation_id.as_str() {
                "readTaskValue"
                    if a.side_effect_class == SideEffectClass::Read && a.payload.is_empty() =>
                {
                    self.permission(session, &a.resource_scope, Permission::Read)?;
                }
                "addTaskValue" if a.side_effect_class == SideEffectClass::RecordMutation => {
                    if !matches!(codec::decode(&a.payload), Ok(Value::Unsigned(_))) {
                        return Err(ErrorCode::InvalidRequest);
                    }
                }
                _ => return Err(ErrorCode::CapabilityUnavailable),
            }
        }
        Ok(())
    }
    fn reserve_ids(&mut self, count: usize) -> Result<VecDeque<EventId>> {
        let mut ids = VecDeque::with_capacity(count);
        let mut seen = BTreeSet::new();
        for _ in 0..count {
            let id = self.services.event_id()?;
            if self.event_ids.contains(&id) || !seen.insert(id.clone()) {
                return Err(ErrorCode::InvalidRequest);
            }
            ids.push_back(id);
        }
        Ok(ids)
    }
    fn capacity(&self, additional_wire: usize, event_slots: usize) -> Result<()> {
        if self
            .resident_wire_bytes
            .checked_add(additional_wire)
            .is_none_or(|n| n > self.limits.resident_wire_bytes)
            || self
                .charged_event_slots
                .checked_add(event_slots)
                .is_none_or(|n| n > self.limits.events)
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(())
    }
    pub fn create_task(&mut self, session: &Session, task: TaskSpec) -> Result<()> {
        self.create_task_policy(
            session,
            task,
            OversightRequirements::new(vec![], false, false, None).expect("empty oversight"),
        )
    }
    /// Initial accepted policy is supplied by the trusted host, never a task request.
    /// It remains immutable; policy revision advance cannot remove obligations.
    pub fn create_governed_task(
        &mut self,
        host: &HostKey,
        session: &Session,
        task: TaskSpec,
        oversight: OversightRequirements,
    ) -> Result<()> {
        if !Arc::ptr_eq(&self.identity, &host.identity) {
            return Err(ErrorCode::Forbidden);
        }
        self.create_task_policy(session, task, oversight)
    }
    fn create_task_policy(
        &mut self,
        session: &Session,
        task: TaskSpec,
        oversight: OversightRequirements,
    ) -> Result<()> {
        let policy_revision = self.grant(session)?.policy_revision;
        let owner = self
            .permission(
                session,
                &task.fields().resource_scope,
                Permission::CreateTask,
            )?
            .principal
            .clone();
        if self.tasks.contains_key(&task.fields().task_id) {
            return Err(ErrorCode::IdempotencyConflict);
        }
        if self.tasks.len() >= self.limits.tasks {
            return Err(ErrorCode::BudgetExceeded);
        }
        let now = self.services.now()?;
        if now.unix_ms() >= task.fields().budget.deadline().unix_ms() {
            return Err(ErrorCode::DeadlineExceeded);
        }
        let bytes = codec::encode_record(&codec::WireRecord::Task(task.clone()))
            .map_err(|_| ErrorCode::InvalidRequest)?
            .len()
            + 64
            + oversight
                .checkpoints()
                .iter()
                .map(|r| r.as_str().len())
                .sum::<usize>()
            + oversight.governance().map_or(0, |r| r.as_str().len());
        self.capacity(bytes, 1)?;
        let event = self
            .reserve_ids(1)?
            .pop_front()
            .ok_or(ErrorCode::InvalidRequest)?;
        let now = self.services.now()?;
        if now.unix_ms() >= task.fields().budget.deadline().unix_ms() {
            return Err(ErrorCode::DeadlineExceeded);
        }
        let task_id = task.fields().task_id.clone();
        let revision = task.fields().revision;
        let context = task.fields().context.clone();
        self.emit(
            event.clone(),
            EventMetadata {
                task: &task_id,
                execution: None,
                revision,
                context: &context,
                kind: EventKind::TaskCreated,
            },
            now,
            (0, 0),
        );
        self.event_ids.insert(event);
        self.resident_wire_bytes += bytes;
        self.charged_event_slots += 1;
        self.tasks.insert(
            task_id,
            TaskState {
                snapshot: TaskSnapshot {
                    task,
                    value: 0,
                    performed_steps: 0,
                    charged_steps: 0,
                    charged_events: 1,
                },
                owner,
                lease: None,
                policy_revision,
                oversight,
            },
        );
        Ok(())
    }
    pub fn admit(
        &mut self,
        session: &Session,
        task: TaskSpec,
        proposal: ActionProposal,
    ) -> Result<ExecutionId> {
        self.admit_with_approvals(session, task, proposal, &[])
    }
    pub fn admit_with_approvals(
        &mut self,
        session: &Session,
        task: TaskSpec,
        proposal: ActionProposal,
        approval_ids: &[EventId],
    ) -> Result<ExecutionId> {
        ActionProposal::new(proposal.fields().clone(), &task)
            .map_err(|_| ErrorCode::InvalidRequest)?;
        self.authorized(session, &proposal)?;
        let principal = self.grant(session)?.principal.clone();
        let keys: Vec<_> = proposal
            .fields()
            .actions
            .iter()
            .map(|a| {
                let a = a.fields();
                Key(
                    principal.clone(),
                    a.operation_id.clone(),
                    a.resource_scope.resources().cloned().collect(),
                    a.idempotency_key.clone(),
                )
            })
            .collect();
        if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
            return Err(ErrorCode::IdempotencyConflict);
        }
        let bytes = codec::canonical_approval_payload(&task, &proposal)
            .map_err(|_| ErrorCode::InvalidRequest)?;
        let hash = self.services.sha256(&bytes)?;
        let plan_key = (
            proposal.fields().proposal_id.clone(),
            proposal.fields().plan_revision.get(),
        );
        if self.plans.get(&plan_key).is_some_and(|old| old != &hash) {
            return Err(ErrorCode::IdempotencyConflict);
        }
        let prior: Vec<_> = keys.iter().filter_map(|k| self.replays.get(k)).collect();
        if !prior.is_empty() {
            if prior.len() != keys.len()
                || prior
                    .iter()
                    .any(|r| r.hash != hash || r.execution != prior[0].execution)
            {
                return Err(ErrorCode::IdempotencyConflict);
            }
            return Ok(prior[0].execution.clone());
        }
        let approval_now = self.services.now()?;
        self.check_approvals(&task, &proposal, approval_ids, hash, approval_now)?;
        let task_id = &task.fields().task_id;
        let stored = self.tasks.get(task_id).ok_or(ErrorCode::InvalidRequest)?;
        if stored.owner != principal {
            return Err(ErrorCode::Forbidden);
        }
        let mut requested = task.fields().clone();
        requested.context = stored.snapshot.task.fields().context.clone();
        if &requested != stored.snapshot.task.fields() {
            return Err(ErrorCode::RevisionConflict);
        }
        if stored.lease.is_some()
            || self.queue.len() >= self.limits.queued
            || self.executions.len() >= self.limits.executions
            || self
                .replays
                .len()
                .checked_add(keys.len())
                .is_none_or(|n| n > self.limits.idempotency)
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let value = stored.snapshot.value;
        let n = proposal.fields().actions.len();
        let cancellable = self.intervention_available(
            &task.fields().resource_scope,
            proposal.fields().policy_revision,
        );
        let event_slots = n + 3 + if cancellable { 2 } else { 0 };
        if stored
            .snapshot
            .charged_steps
            .checked_add(n as u64)
            .is_none_or(|s| s > task.fields().budget.max_steps())
            || stored
                .snapshot
                .charged_events
                .checked_add(event_slots as u64)
                .is_none_or(|e| e > task.fields().budget.max_events())
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let wire_bytes = codec::encode_record(&codec::WireRecord::Proposal {
            task: task.clone(),
            proposal: proposal.clone(),
        })
        .map_err(|_| ErrorCode::InvalidRequest)?
        .len();
        self.capacity(wire_bytes, event_slots)?;
        let now = self.services.now()?;
        if now.unix_ms() >= task.fields().budget.deadline().unix_ms()
            || proposal
                .fields()
                .actions
                .iter()
                .any(|a| now.unix_ms() >= a.fields().budget.deadline().unix_ms())
        {
            return Err(ErrorCode::DeadlineExceeded);
        }
        let execution = self.services.execution_id()?;
        if self.executions.contains_key(&execution) {
            return Err(ErrorCode::IdempotencyConflict);
        }
        let mut event_ids = self.reserve_ids(event_slots)?;
        let mut cancel_ids = VecDeque::new();
        if cancellable {
            cancel_ids.push_front(event_ids.pop_back().expect("reserved cancellation ID"));
            cancel_ids.push_front(event_ids.pop_back().expect("reserved cancellation ID"));
        }
        let draft = ExecutionDraft {
            context: proposal.fields().context.clone(),
            execution_id: execution.clone(),
            task_id: task_id.clone(),
            proposal_id: proposal.fields().proposal_id.clone(),
            plan_revision: proposal.fields().plan_revision,
            state_revision: proposal.fields().state_revision,
            state: ExecutionState::Queued,
            effects: vec![],
            result_evidence: vec![],
            effective_stop_evidence: None,
            error: None,
        };
        ExecutionRecord::new(draft.clone(), &proposal).map_err(|_| ErrorCode::InvalidRequest)?;
        let entry = Entry {
            task: task.clone(),
            proposal,
            draft,
            session: session.clone(),
            next: 0,
            expected_live: task.fields().revision,
            event_ids: VecDeque::new(),
            cancel_ids,
            cancellation: None,
            approval_ids: approval_ids.to_vec(),
            payload_hash: hash,
        };
        let now = self.services.now()?;
        if now.unix_ms() >= task.fields().budget.deadline().unix_ms()
            || entry
                .proposal
                .fields()
                .actions
                .iter()
                .any(|a| now.unix_ms() >= a.fields().budget.deadline().unix_ms())
        {
            return Err(ErrorCode::DeadlineExceeded);
        }
        self.check_approvals(&task, &entry.proposal, approval_ids, hash, now)?;
        // All input checks/metadata reservations precede any owned-state commit.
        self.event_ids.extend(event_ids.iter().cloned());
        self.event_ids.extend(entry.cancel_ids.iter().cloned());
        self.charged_event_slots += event_slots;
        self.resident_wire_bytes += wire_bytes;
        let accepted = event_ids.pop_front().expect("reserved acceptance event");
        self.emit(
            accepted,
            EventMetadata {
                task: task_id,
                execution: Some(&execution),
                revision: task.fields().revision,
                context: &entry.draft.context,
                kind: EventKind::ExecutionAccepted,
            },
            now,
            (value, value),
        );
        let stored = self.tasks.get_mut(task_id).expect("validated task");
        stored.lease = Some(execution.clone());
        stored.snapshot.charged_steps += n as u64;
        stored.snapshot.charged_events += event_slots as u64;
        self.plans.insert(plan_key, hash);
        for key in keys {
            self.replays.insert(
                key,
                Replay {
                    hash,
                    execution: execution.clone(),
                },
            );
        }
        self.executions
            .insert(execution.clone(), Entry { event_ids, ..entry });
        self.queue.push_back(execution.clone());
        Ok(execution)
    }
    fn emit(
        &mut self,
        id: EventId,
        metadata: EventMetadata<'_>,
        now: Timestamp,
        values: (u64, u64),
    ) -> EvidenceRef {
        let EventMetadata {
            task,
            execution,
            revision,
            context,
            kind,
        } = metadata;
        let (before, after) = values;
        let evidence =
            EvidenceRef::new(id.as_str()).expect("event IDs obey shared reference bounds");
        let event = EventRecord::new(EventDraft {
            contract_version: ContractVersion::CURRENT,
            event_id: id.clone(),
            task_id: task.clone(),
            execution_id: execution.cloned(),
            state_revision: revision,
            timestamp: now,
            correlation_id: context.correlation_id.clone(),
            kind,
            evidence: vec![evidence.clone()],
        })
        .expect("owned event invariants");
        self.observations.insert(
            evidence.clone(),
            Observation {
                evidence: evidence.clone(),
                event_id: id,
                task_id: task.clone(),
                execution_id: execution.cloned(),
                timestamp: now,
                before,
                after,
            },
        );
        self.events.push(event);
        evidence
    }
    /// Executes at most one bounded built-in action; one active task lease.
    pub fn step(&mut self) -> Result<Option<ExecutionId>> {
        let Some(id) = self.queue.front().cloned() else {
            return Ok(None);
        };
        let now = self.services.now()?; // No effect if the host clock fails.
        let mut entry = self
            .executions
            .remove(&id)
            .expect("queued execution exists");
        let outcome = self.step_entry(&id, &mut entry, now);
        if entry.draft.state.is_terminal() {
            self.queue.pop_front();
            self.tasks
                .get_mut(&entry.draft.task_id)
                .expect("leased task")
                .lease = None;
        }
        self.executions.insert(id.clone(), entry);
        outcome?;
        Ok(Some(id))
    }
    fn step_entry(&mut self, id: &ExecutionId, entry: &mut Entry, now: Timestamp) -> Result<()> {
        let task_id = entry.draft.task_id.clone();
        let state = self.tasks.get(&task_id).expect("leased task");
        let before = state.snapshot.value;
        if entry.draft.state == ExecutionState::CancelRequested {
            return self.stop_remaining(id, entry, now);
        }
        if entry.draft.state == ExecutionState::Queued {
            entry.draft.state = ExecutionState::Running;
            let event = entry.event_ids.pop_front().expect("reserved running event");
            self.emit(
                event,
                EventMetadata {
                    task: &task_id,
                    execution: Some(id),
                    revision: entry.expected_live,
                    context: &entry.draft.context,
                    kind: EventKind::ExecutionProgress,
                },
                now,
                (before, before),
            );
        }
        let action = entry.proposal.fields().actions[entry.next].clone();
        let checks = self
            .authorized(&entry.session, &entry.proposal)
            .and_then(|_| {
                self.check_approvals(
                    &entry.task,
                    &entry.proposal,
                    &entry.approval_ids,
                    entry.payload_hash,
                    now,
                )?;
                let state = self.tasks.get(&task_id).expect("leased task");
                if state.lease.as_ref() != Some(id)
                    || state.snapshot.task.fields().revision != entry.expected_live
                {
                    return Err(ErrorCode::RevisionConflict);
                }
                if now.unix_ms() >= entry.task.fields().budget.deadline().unix_ms()
                    || now.unix_ms() >= action.fields().budget.deadline().unix_ms()
                {
                    return Err(ErrorCode::DeadlineExceeded);
                }
                Ok(())
            });
        if let Err(code) = checks {
            return self.finish(id, entry, now, Some(code));
        }
        let mut after = before;
        let mut next_revision = entry.expected_live;
        let mut failure = None;
        if action.fields().operation_id.as_str() == "addTaskValue" {
            let delta = match codec::decode(&action.fields().payload) {
                Ok(Value::Unsigned(n)) => n,
                _ => return self.finish(id, entry, now, Some(ErrorCode::InvalidRequest)),
            };
            match (
                before.checked_add(delta),
                entry.expected_live.get().checked_add(1),
            ) {
                (Some(value), Some(revision)) => {
                    after = value;
                    next_revision = Revision::new(revision).expect("positive checked revision")
                }
                _ => failure = Some(ErrorCode::InvalidRequest),
            }
        }
        let event = entry
            .event_ids
            .pop_front()
            .expect("reserved action observation");
        // No external adapter: this is a synchronous checked mutation of owned
        // memory only. IDs/capacity/argument checks were reserved before effects.
        if failure.is_none() {
            let state = self.tasks.get_mut(&task_id).expect("leased task");
            let mut task = state.snapshot.task.fields().clone();
            task.revision = next_revision;
            state.snapshot.task = TaskSpec::new(task).expect("only valid revision changed");
            state.snapshot.value = after;
            entry.expected_live = next_revision;
        }
        self.tasks
            .get_mut(&task_id)
            .expect("leased task")
            .snapshot
            .performed_steps += 1;
        let evidence = self.emit(
            event,
            EventMetadata {
                task: &task_id,
                execution: Some(id),
                revision: entry.expected_live,
                context: &entry.draft.context,
                kind: EventKind::ExecutionProgress,
            },
            now,
            (before, after),
        );
        let outcome =
            if failure.is_some() || action.fields().side_effect_class == SideEffectClass::Read {
                EffectOutcome::NotApplied
            } else {
                EffectOutcome::Applied
            };
        entry.draft.effects.push(
            EffectRecord::new(EffectDraft {
                action_id: action.fields().action_id.clone(),
                outcome,
                evidence: vec![evidence],
            })
            .expect("bounded own effect"),
        );
        entry.next += 1;
        if failure.is_some() || entry.next == entry.proposal.fields().actions.len() {
            self.finish(id, entry, now, failure)?;
        }
        Ok(())
    }
    fn finish(
        &mut self,
        id: &ExecutionId,
        entry: &mut Entry,
        now: Timestamp,
        failure: Option<ErrorCode>,
    ) -> Result<()> {
        let state = self.tasks.get(&entry.draft.task_id).expect("leased task");
        let value = state.snapshot.value;
        entry.draft.state = if failure.is_some() {
            ExecutionState::Failed
        } else {
            ExecutionState::Succeeded
        };
        entry.draft.error = failure.map(|code| {
            ProtocolError::new(
                code,
                "Owned task step could not complete within its declared conditions".into(),
                None,
                false,
                entry.draft.context.correlation_id.clone(),
                None,
            )
            .expect("fixed correlated error")
        });
        let event = entry
            .event_ids
            .pop_back()
            .expect("reserved terminal observation");
        let evidence = self.emit(
            event,
            EventMetadata {
                task: &entry.draft.task_id,
                execution: Some(id),
                revision: entry.expected_live,
                context: &entry.draft.context,
                kind: if failure.is_some() {
                    EventKind::ExecutionFailed
                } else {
                    EventKind::ExecutionCompleted
                },
            },
            now,
            (value, value),
        );
        entry.draft.result_evidence = vec![evidence];
        ExecutionRecord::new(entry.draft.clone(), &entry.proposal)
            .map_err(|_| ErrorCode::OutcomeUnknown)?;
        Ok(())
    }
    pub fn task(&self, session: &Session, id: &TaskId) -> Result<TaskSnapshot> {
        let state = self.tasks.get(id).ok_or(ErrorCode::InvalidRequest)?;
        self.permission(
            session,
            &state.snapshot.task.fields().resource_scope,
            Permission::Read,
        )?;
        Ok(state.snapshot.clone())
    }
    pub fn execution(&self, session: &Session, id: &ExecutionId) -> Result<ExecutionRecord> {
        let e = self.executions.get(id).ok_or(ErrorCode::InvalidRequest)?;
        self.permission(session, &e.task.fields().resource_scope, Permission::Read)?;
        ExecutionRecord::new(e.draft.clone(), &e.proposal).map_err(|_| ErrorCode::OutcomeUnknown)
    }
    pub fn events(
        &self,
        session: &Session,
        task: &TaskId,
        after: Option<&EventId>,
        limit: usize,
    ) -> Result<Vec<EventRecord>> {
        let state = self.tasks.get(task).ok_or(ErrorCode::InvalidRequest)?;
        self.permission(
            session,
            &state.snapshot.task.fields().resource_scope,
            Permission::ObserveEvents,
        )?;
        if limit == 0 || limit > 256 {
            return Err(ErrorCode::InvalidRequest);
        }
        let stream: Vec<_> = self
            .events
            .iter()
            .filter(|e| &e.fields().task_id == task)
            .collect();
        let start = match after {
            Some(id) => {
                stream
                    .iter()
                    .position(|e| &e.fields().event_id == id)
                    .ok_or(ErrorCode::CursorExpired)?
                    + 1
            }
            None => 0,
        };
        Ok(stream
            .into_iter()
            .skip(start)
            .take(limit)
            .cloned()
            .collect())
    }
    pub fn observation(&self, session: &Session, reference: &EvidenceRef) -> Result<Observation> {
        let value = self
            .observations
            .get(reference)
            .ok_or(ErrorCode::InvalidRequest)?;
        let state = self
            .tasks
            .get(&value.task_id)
            .ok_or(ErrorCode::InvalidRequest)?;
        self.permission(
            session,
            &state.snapshot.task.fields().resource_scope,
            Permission::ObserveEvents,
        )?;
        Ok(value.clone())
    }
}

pub mod tool;

pub mod routing;
