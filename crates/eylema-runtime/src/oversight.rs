//! Embedded human-verification boundary and retained oversight decisions.
//! The host must verify the person before enrollment; these handles do not
//! implement login, prove a real human test or confer authority from strings.
use super::*;

#[derive(Clone)]
pub struct HumanSession(Session);
impl HumanSession {
    pub fn session(&self) -> &Session {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalBinding {
    pub task_id: TaskId,
    pub proposal_id: ProposalId,
    pub plan_revision: Revision,
    pub payload_sha256: [u8; 32],
    pub state_revision: Revision,
    pub policy_revision: Revision,
    pub resource_scope: ResourceScope,
    pub checkpoints: Vec<CheckpointRef>,
    pub expires_at: Timestamp,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Revocation {
    pub verified_principal: PrincipalRef,
    pub timestamp: Timestamp,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRecord {
    pub approval_id: EventId,
    pub binding: ApprovalBinding,
    pub verified_principal: PrincipalRef,
    pub decided_at: Timestamp,
    pub revocation: Option<Revocation>,
    pub(super) issuer: Session,
}
// Session equality is instance identity plus immutable enrollment slot.
impl PartialEq for Session {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity) && self.slot == other.slot
    }
}
impl Eq for Session {}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session(<opaque>)")
    }
}
#[derive(Clone)]
pub(super) struct GovernanceRecord {
    pub(super) task_id: TaskId,
    pub(super) resource_scope: ResourceScope,
    pub(super) policy_revision: Revision,
    pub(super) issuer: Session,
    pub(super) decision: PolicyChange,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyChange {
    pub decision_id: EventId,
    pub task_id: TaskId,
    pub verified_principal: PrincipalRef,
    pub previous_revision: Revision,
    pub accepted_revision: Revision,
    pub timestamp: Timestamp,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancellationReceipt {
    pub execution_id: ExecutionId,
    pub receipt_event: EventId,
    pub verified_principal: PrincipalRef,
    pub requested_at: Timestamp,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancellationOutcome {
    Requested(CancellationReceipt),
    AlreadyTerminal(Box<ExecutionRecord>),
}
impl<S: HostServices> Runtime<S> {
    /// Trusted host attestation: call only after independent person verification.
    /// Supplying HostKey is the trust boundary; no request flag can call this.
    pub fn enroll_verified_human(&mut self, host: &HostKey, grant: Grant) -> Result<HumanSession> {
        let session = self.enroll(host, grant)?;
        self.human_slots.insert(session.slot);
        Ok(HumanSession(session))
    }
    pub(super) fn human(&self, session: &Session) -> Result<&Grant> {
        let grant = self.grant(session)?;
        if !self.human_slots.contains(&session.slot) {
            return Err(ErrorCode::Forbidden);
        }
        Ok(grant)
    }
    fn human_permission(
        &self,
        human: &HumanSession,
        task: &TaskId,
        permission: Permission,
    ) -> Result<&Grant> {
        self.human(&human.0)?;
        let state = self.tasks.get(task).ok_or(ErrorCode::InvalidRequest)?;
        let grant = self.permission(
            &human.0,
            &state.snapshot.task.fields().resource_scope,
            permission,
        )?;
        if grant.policy_revision != state.policy_revision {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(grant)
    }
    pub(super) fn intervention_available(&self, scope: &ResourceScope, policy: Revision) -> bool {
        self.human_slots.iter().any(|slot| {
            !self.revoked_slots.contains_key(slot)
                && self.grants.get(*slot).is_some_and(|grant| {
                    grant.policy_revision == policy
                        && grant.permissions.contains(&Permission::RequestCancellation)
                        && scope.is_subset_of(&grant.resource_scope)
                })
        })
    }
    pub(super) fn check_oversight(
        &self,
        session: &Session,
        task: &TaskId,
        scope: &ResourceScope,
        oversight: &OversightRequirements,
    ) -> Result<()> {
        let grant = self.grant(session)?;
        // Streams and cancellation act on the whole task/execution. A narrow
        // action controller cannot satisfy these full-task capabilities.
        let task_scope = &self
            .tasks
            .get(task)
            .ok_or(ErrorCode::InvalidRequest)?
            .snapshot
            .task
            .fields()
            .resource_scope;
        if oversight.requires_intervention()
            && !self.intervention_available(task_scope, grant.policy_revision)
        {
            return Err(ErrorCode::CapabilityUnavailable);
        }
        if oversight.requires_observation() {
            self.permission(session, task_scope, Permission::ObserveEvents)?;
        }
        if let Some(reference) = oversight.governance() {
            let record = self
                .governance
                .get(reference)
                .ok_or(ErrorCode::CapabilityUnavailable)?;
            if &record.task_id != task
                || record.policy_revision != grant.policy_revision
                || !scope.is_subset_of(&record.resource_scope)
            {
                return Err(ErrorCode::RevisionConflict);
            }
            self.human(&record.issuer)?;
        }
        Ok(())
    }
    pub fn accepted_oversight(
        &self,
        session: &Session,
        task: &TaskId,
    ) -> Result<OversightRequirements> {
        let state = self.tasks.get(task).ok_or(ErrorCode::InvalidRequest)?;
        self.permission(
            session,
            &state.snapshot.task.fields().resource_scope,
            Permission::Read,
        )?;
        Ok(state.oversight.clone())
    }
    fn current_proposal(&self, task: &TaskSpec, proposal: &ActionProposal) -> Result<()> {
        ActionProposal::new(proposal.fields().clone(), task)
            .map_err(|_| ErrorCode::InvalidRequest)?;
        let state = self
            .tasks
            .get(&task.fields().task_id)
            .ok_or(ErrorCode::InvalidRequest)?;
        let mut expected = task.fields().clone();
        expected.context = state.snapshot.task.fields().context.clone();
        if &expected != state.snapshot.task.fields()
            || state.policy_revision != proposal.fields().policy_revision
        {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(())
    }
    fn binding(
        &self,
        task: &TaskSpec,
        proposal: &ActionProposal,
        hash: [u8; 32],
        expires_at: Timestamp,
    ) -> Result<ApprovalBinding> {
        let accepted = self
            .tasks
            .get(&task.fields().task_id)
            .ok_or(ErrorCode::InvalidRequest)?;
        let checkpoints: Vec<_> = proposal
            .fields()
            .actions
            .iter()
            .flat_map(|a| {
                a.fields()
                    .authority
                    .oversight()
                    .checkpoints()
                    .iter()
                    .cloned()
            })
            .chain(accepted.oversight.checkpoints().iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if checkpoints.len() > MAX_REFERENCES {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(ApprovalBinding {
            task_id: task.fields().task_id.clone(),
            proposal_id: proposal.fields().proposal_id.clone(),
            plan_revision: proposal.fields().plan_revision,
            payload_sha256: hash,
            state_revision: proposal.fields().state_revision,
            policy_revision: proposal.fields().policy_revision,
            resource_scope: task.fields().resource_scope.clone(),
            checkpoints,
            expires_at,
        })
    }
    pub fn preview_approval(
        &mut self,
        session: &Session,
        task: &TaskSpec,
        proposal: &ActionProposal,
        expires_at: Timestamp,
    ) -> Result<ApprovalBinding> {
        self.permission(session, &task.fields().resource_scope, Permission::Read)?;
        self.current_proposal(task, proposal)?;
        let bytes = codec::canonical_approval_payload(task, proposal)
            .map_err(|_| ErrorCode::InvalidRequest)?;
        let hash = self.services.sha256(&bytes)?;
        self.binding(task, proposal, hash, expires_at)
    }
    // Audit IDs/capacity are reserved before a decision; records use their own
    // lookup surface, not fabricated execution events or effect observations.
    fn decision_id(&mut self, task: &TaskId) -> Result<(EventId, Timestamp)> {
        let state = self.tasks.get(task).ok_or(ErrorCode::InvalidRequest)?;
        if state.snapshot.charged_events >= state.snapshot.task.fields().budget.max_events() {
            return Err(ErrorCode::BudgetExceeded);
        }
        self.capacity(16384, 1)?;
        let id = self.reserve_ids(1)?.pop_front().expect("reserved audit ID");
        let now = self.services.now()?;
        Ok((id, now))
    }
    fn commit_decision(&mut self, task: &TaskId, id: EventId) {
        self.event_ids.insert(id);
        self.charged_event_slots += 1;
        self.resident_wire_bytes += 16384;
        self.tasks
            .get_mut(task)
            .expect("validated audit task")
            .snapshot
            .charged_events += 1;
    }
    pub fn create_approval(
        &mut self,
        human: &HumanSession,
        task: &TaskSpec,
        proposal: &ActionProposal,
        binding: ApprovalBinding,
    ) -> Result<EventId> {
        let principal = self
            .human_permission(human, &task.fields().task_id, Permission::ApproveAction)?
            .principal
            .clone();
        self.current_proposal(task, proposal)?;
        let bytes = codec::canonical_approval_payload(task, proposal)
            .map_err(|_| ErrorCode::InvalidRequest)?;
        let hash = self.services.sha256(&bytes)?;
        if binding != self.binding(task, proposal, hash, binding.expires_at)?
            || binding.checkpoints.is_empty()
        {
            return Err(ErrorCode::ApprovalMismatch);
        }
        if self.approvals.len() >= self.limits.executions {
            return Err(ErrorCode::BudgetExceeded);
        }
        let (id, now) = self.decision_id(&binding.task_id)?;
        if now.unix_ms() >= binding.expires_at.unix_ms() {
            return Err(ErrorCode::ApprovalExpired);
        }
        self.commit_decision(&binding.task_id, id.clone());
        self.approvals.insert(
            id.clone(),
            ApprovalRecord {
                approval_id: id.clone(),
                binding,
                verified_principal: principal,
                decided_at: now,
                revocation: None,
                issuer: human.0.clone(),
            },
        );
        Ok(id)
    }
    pub fn approval(&self, session: &Session, id: &EventId) -> Result<ApprovalRecord> {
        let record = self.approvals.get(id).ok_or(ErrorCode::ApprovalMismatch)?;
        let state = self
            .tasks
            .get(&record.binding.task_id)
            .ok_or(ErrorCode::InvalidRequest)?;
        self.permission(
            session,
            &state.snapshot.task.fields().resource_scope,
            Permission::ObserveEvents,
        )?;
        Ok(record.clone())
    }
    pub(super) fn check_approvals(
        &self,
        task: &TaskSpec,
        proposal: &ActionProposal,
        ids: &[EventId],
        hash: [u8; 32],
        now: Timestamp,
    ) -> Result<()> {
        if ids.len() > MAX_REFERENCES || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(ErrorCode::InvalidRequest);
        }
        let expected = self.binding(task, proposal, hash, now)?;
        if expected.checkpoints.is_empty() && ids.is_empty() {
            return Ok(());
        }
        if ids.is_empty() {
            return Err(ErrorCode::ApprovalRequired);
        }
        let mut covered = BTreeSet::new();
        for id in ids {
            let record = self.approvals.get(id).ok_or(ErrorCode::ApprovalMismatch)?;
            self.human(&record.issuer)?;
            if record.revocation.is_some() {
                return Err(ErrorCode::Forbidden);
            }
            if now.unix_ms() >= record.binding.expires_at.unix_ms() {
                return Err(ErrorCode::ApprovalExpired);
            }
            let mut binding = record.binding.clone();
            binding.expires_at = now;
            if binding != expected {
                return Err(ErrorCode::ApprovalMismatch);
            }
            covered.extend(record.binding.checkpoints.iter().cloned());
        }
        if expected.checkpoints.iter().any(|c| !covered.contains(c)) {
            return Err(ErrorCode::ApprovalRequired);
        }
        Ok(())
    }
    pub fn revoke_approval(
        &mut self,
        human: &HumanSession,
        id: &EventId,
    ) -> Result<ApprovalRecord> {
        let task = self
            .approvals
            .get(id)
            .ok_or(ErrorCode::ApprovalMismatch)?
            .binding
            .task_id
            .clone();
        let principal = self
            .human_permission(human, &task, Permission::ApproveAction)?
            .principal
            .clone();
        if self
            .approvals
            .get(id)
            .expect("validated approval")
            .revocation
            .is_none()
        {
            let timestamp = self.services.now()?;
            self.approvals
                .get_mut(id)
                .expect("validated approval")
                .revocation = Some(Revocation {
                verified_principal: principal,
                timestamp,
            });
        }
        Ok(self.approvals.get(id).expect("validated approval").clone())
    }
    pub fn register_governance(
        &mut self,
        human: &HumanSession,
        task: &TaskId,
        reference: GovernanceRef,
    ) -> Result<PolicyChange> {
        let grant = self
            .human_permission(human, task, Permission::GovernPolicy)?
            .clone();
        if self.governance.contains_key(&reference) {
            return Err(ErrorCode::IdempotencyConflict);
        }
        if self.governance.len() >= self.limits.executions {
            return Err(ErrorCode::BudgetExceeded);
        }
        let (id, timestamp) = self.decision_id(task)?;
        let decision = PolicyChange {
            decision_id: id.clone(),
            task_id: task.clone(),
            verified_principal: grant.principal,
            previous_revision: grant.policy_revision,
            accepted_revision: grant.policy_revision,
            timestamp,
        };
        let scope = self
            .tasks
            .get(task)
            .expect("validated task")
            .snapshot
            .task
            .fields()
            .resource_scope
            .clone();
        self.commit_decision(task, id);
        self.governance.insert(
            reference,
            GovernanceRecord {
                task_id: task.clone(),
                resource_scope: scope,
                policy_revision: grant.policy_revision,
                issuer: human.0.clone(),
                decision: decision.clone(),
            },
        );
        Ok(decision)
    }
    pub fn governance_record(
        &self,
        session: &Session,
        reference: &GovernanceRef,
    ) -> Result<PolicyChange> {
        let record = self
            .governance
            .get(reference)
            .ok_or(ErrorCode::InvalidRequest)?;
        self.permission(session, &record.resource_scope, Permission::ObserveEvents)?;
        Ok(record.decision.clone())
    }
    pub fn advance_policy(
        &mut self,
        human: &HumanSession,
        task: &TaskId,
        next: Revision,
    ) -> Result<PolicyChange> {
        let grant = self
            .human_permission(human, task, Permission::GovernPolicy)?
            .clone();
        if next.get() <= grant.policy_revision.get() {
            return Err(ErrorCode::RevisionConflict);
        }
        if self.policy_changes.len() >= self.limits.executions {
            return Err(ErrorCode::BudgetExceeded);
        }
        let (id, timestamp) = self.decision_id(task)?;
        let record = PolicyChange {
            decision_id: id.clone(),
            task_id: task.clone(),
            verified_principal: grant.principal,
            previous_revision: grant.policy_revision,
            accepted_revision: next,
            timestamp,
        };
        self.commit_decision(task, id);
        self.tasks
            .get_mut(task)
            .expect("validated task")
            .policy_revision = next;
        self.policy_changes.push(record.clone());
        Ok(record)
    }
    pub fn policy_changes(&self, session: &Session, task: &TaskId) -> Result<Vec<PolicyChange>> {
        let state = self.tasks.get(task).ok_or(ErrorCode::InvalidRequest)?;
        self.permission(
            session,
            &state.snapshot.task.fields().resource_scope,
            Permission::ObserveEvents,
        )?;
        Ok(self
            .policy_changes
            .iter()
            .filter(|r| &r.task_id == task)
            .cloned()
            .collect())
    }
    pub fn revoke_delegation(
        &mut self,
        human: &HumanSession,
        task: &TaskId,
        delegated: &Session,
    ) -> Result<Revocation> {
        let principal = self
            .human_permission(human, task, Permission::GovernPolicy)?
            .principal
            .clone();
        if !Arc::ptr_eq(&self.identity, &delegated.identity) {
            return Err(ErrorCode::Forbidden);
        }
        let delegated_grant = self
            .grants
            .get(delegated.slot)
            .ok_or(ErrorCode::Forbidden)?;
        let scope = &self
            .tasks
            .get(task)
            .expect("validated task")
            .snapshot
            .task
            .fields()
            .resource_scope;
        if !delegated_grant.resource_scope.is_subset_of(scope) {
            return Err(ErrorCode::Forbidden);
        }
        if let Some(old) = self.revoked_slots.get(&delegated.slot) {
            return Ok(old.clone());
        }
        let record = Revocation {
            verified_principal: principal,
            timestamp: self.services.now()?,
        };
        self.revoked_slots.insert(delegated.slot, record.clone());
        Ok(record)
    }
    pub fn request_cancellation(
        &mut self,
        human: &HumanSession,
        id: &ExecutionId,
    ) -> Result<CancellationOutcome> {
        let entry = self.executions.get(id).ok_or(ErrorCode::InvalidRequest)?;
        let principal = self
            .human_permission(human, &entry.draft.task_id, Permission::RequestCancellation)?
            .principal
            .clone();
        if entry.draft.state.is_terminal() {
            return Ok(CancellationOutcome::AlreadyTerminal(Box::new(
                ExecutionRecord::new(entry.draft.clone(), &entry.proposal)
                    .map_err(|_| ErrorCode::OutcomeUnknown)?,
            )));
        }
        if let Some(receipt) = &entry.cancellation {
            return Ok(CancellationOutcome::Requested(receipt.clone()));
        }
        if entry.cancel_ids.len() != 2 {
            return Err(ErrorCode::CapabilityUnavailable);
        }
        let now = self.services.now()?;
        let mut entry = self.executions.remove(id).expect("validated execution");
        let event = entry
            .cancel_ids
            .pop_front()
            .expect("reserved receipt event");
        let receipt = CancellationReceipt {
            execution_id: id.clone(),
            receipt_event: event.clone(),
            verified_principal: principal,
            requested_at: now,
        };
        let value = self
            .tasks
            .get(&entry.draft.task_id)
            .expect("execution task")
            .snapshot
            .value;
        self.emit(
            event,
            EventMetadata {
                task: &entry.draft.task_id,
                execution: Some(id),
                revision: entry.expected_live,
                context: &entry.draft.context,
                kind: EventKind::CancellationRequested,
            },
            now,
            (value, value),
        );
        entry.draft.state = ExecutionState::CancelRequested;
        entry.cancellation = Some(receipt.clone());
        self.executions.insert(id.clone(), entry);
        Ok(CancellationOutcome::Requested(receipt))
    }
    pub(super) fn stop_remaining(
        &mut self,
        id: &ExecutionId,
        entry: &mut Entry,
        now: Timestamp,
    ) -> Result<()> {
        let value = self
            .tasks
            .get(&entry.draft.task_id)
            .expect("execution task")
            .snapshot
            .value;
        let event = entry
            .cancel_ids
            .pop_front()
            .expect("reserved effective stop event");
        let evidence = self.emit(
            event,
            EventMetadata {
                task: &entry.draft.task_id,
                execution: Some(id),
                revision: entry.expected_live,
                context: &entry.draft.context,
                kind: EventKind::CancellationEffective,
            },
            now,
            (value, value),
        );
        entry.draft.state = ExecutionState::Cancelled;
        entry.draft.effective_stop_evidence = Some(evidence.clone());
        entry.draft.result_evidence = vec![evidence];
        ExecutionRecord::new(entry.draft.clone(), &entry.proposal)
            .map_err(|_| ErrorCode::OutcomeUnknown)?;
        Ok(())
    }
}
