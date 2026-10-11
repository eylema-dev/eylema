use crate::primitives::{require, text, unique};
use crate::*;
use std::collections::BTreeSet;

/// Drafts are editable inputs. Only successful constructors produce validated records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDraft {
    pub context: RequestContext,
    pub task_id: TaskId,
    pub revision: Revision,
    pub objective: String,
    pub resource_scope: ResourceScope,
    pub budget: Budget,
    pub target: ExecutionTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSpec(TaskDraft);
impl TaskSpec {
    pub fn new(draft: TaskDraft) -> Result<Self, ValidationError> {
        text(&draft.objective, "task.objective")?;
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &TaskDraft {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SideEffectClass {
    Read,
    RecordMutation,
    Compute,
    ExternalEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionDraft {
    pub context: RequestContext,
    pub action_id: ActionId,
    pub task_id: TaskId,
    pub operation_id: OperationId,
    pub operation_version: ContractVersion,
    pub side_effect_class: SideEffectClass,
    pub resource_scope: ResourceScope,
    pub expected_revision: Option<Revision>,
    pub authority: AuthorityRequirement,
    pub required_capabilities: Vec<CapabilityId>,
    pub budget: Budget,
    pub target: ExecutionTarget,
    pub idempotency_key: IdempotencyKey,
    /// Opaque bounded application bytes; canonical G09 enclosing wire does not
    /// interpret or trust this content.
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSpec(ActionDraft);
impl ActionSpec {
    pub fn new(draft: ActionDraft) -> Result<Self, ValidationError> {
        require(
            draft.resource_scope == *draft.authority.resource_scope(),
            "action.authority.resource_scope",
            "required authority must bind the declared effect scope",
        )?;
        require(
            draft.side_effect_class == SideEffectClass::Read || draft.expected_revision.is_some(),
            "action.expected_revision",
            "non-read task actions require a state precondition",
        )?;
        require(
            draft.payload.len() <= draft.budget.max_payload_bytes() as usize,
            "action.payload",
            "payload exceeds declared budget",
        )?;
        unique(&draft.required_capabilities, "action.required_capabilities")?;
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &ActionDraft {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalDraft {
    pub context: RequestContext,
    pub proposal_id: ProposalId,
    pub task_id: TaskId,
    pub plan_revision: Revision,
    pub state_revision: Revision,
    pub policy_revision: Revision,
    pub actions: Vec<ActionSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionProposal(ProposalDraft);
impl ActionProposal {
    pub fn new(draft: ProposalDraft, task: &TaskSpec) -> Result<Self, ValidationError> {
        let task = task.fields();
        require(
            draft.task_id == task.task_id,
            "proposal.task_id",
            "proposal must bind its task",
        )?;
        require(
            draft.state_revision == task.revision,
            "proposal.state_revision",
            "proposal state revision differs from task snapshot",
        )?;
        require(
            !draft.actions.is_empty()
                && draft.actions.len() <= MAX_ACTIONS
                && draft.actions.len() as u64 <= task.budget.max_steps(),
            "proposal.actions",
            "plan must be nonempty and within action and step bounds",
        )?;
        let mut ids = BTreeSet::new();
        let mut bytes = 0usize;
        for action in &draft.actions {
            let action = action.fields();
            require(
                ids.insert(&action.action_id),
                "proposal.actions",
                "duplicate action identifier",
            )?;
            require(
                action.task_id == draft.task_id,
                "proposal.actions.task_id",
                "action belongs to another task",
            )?;
            require(
                action.resource_scope.is_subset_of(&task.resource_scope),
                "proposal.actions.resource_scope",
                "action exceeds task resource declaration",
            )?;
            require(
                action.authority.policy_revision() == draft.policy_revision,
                "proposal.actions.policy_revision",
                "action and proposal policy revisions differ",
            )?;
            require(
                action
                    .expected_revision
                    .is_none_or(|revision| revision == draft.state_revision),
                "proposal.actions.expected_revision",
                "action precondition differs from proposal snapshot",
            )?;
            require(
                action.target == task.target,
                "proposal.actions.target",
                "action differs from task execution target",
            )?;
            require(
                action.budget.max_steps() <= task.budget.max_steps()
                    && action.budget.max_events() <= task.budget.max_events()
                    && action.budget.max_payload_bytes() <= task.budget.max_payload_bytes()
                    && action.budget.max_concurrency() <= task.budget.max_concurrency()
                    && action.budget.deadline().unix_ms() <= task.budget.deadline().unix_ms(),
                "proposal.actions.budget",
                "action resource declaration exceeds parent task",
            )?;
            bytes = bytes
                .checked_add(action.payload.len())
                .ok_or(ValidationError {
                    field: "proposal.payload",
                    message: "payload size overflow",
                })?;
        }
        require(
            bytes <= task.budget.max_payload_bytes() as usize,
            "proposal.payload",
            "aggregate payload exceeds task budget",
        )?;
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &ProposalDraft {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionState {
    Accepted,
    Queued,
    Running,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
    OutcomeUnknown,
}
impl ExecutionState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::OutcomeUnknown
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectOutcome {
    Applied,
    NotApplied,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDraft {
    pub action_id: ActionId,
    pub outcome: EffectOutcome,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRecord(EffectDraft);
impl EffectRecord {
    pub fn new(draft: EffectDraft) -> Result<Self, ValidationError> {
        unique(&draft.evidence, "effect.evidence")?;
        require(
            draft.outcome != EffectOutcome::Applied || !draft.evidence.is_empty(),
            "effect.evidence",
            "applied effect requires an evidence reference",
        )?;
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &EffectDraft {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionDraft {
    pub context: RequestContext,
    pub execution_id: ExecutionId,
    pub task_id: TaskId,
    pub proposal_id: ProposalId,
    pub plan_revision: Revision,
    pub state_revision: Revision,
    pub state: ExecutionState,
    pub effects: Vec<EffectRecord>,
    pub result_evidence: Vec<EvidenceRef>,
    pub effective_stop_evidence: Option<EvidenceRef>,
    pub error: Option<ProtocolError>,
}

/// Observed snapshot declaration, never proof of execution or a state-machine transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRecord(ExecutionDraft);
impl ExecutionRecord {
    pub fn new(draft: ExecutionDraft, proposal: &ActionProposal) -> Result<Self, ValidationError> {
        let proposal = proposal.fields();
        require(
            draft.state_revision.get() >= proposal.state_revision.get(),
            "execution.state_revision",
            "execution snapshot cannot precede proposal state revision",
        )?;
        require(
            draft.task_id == proposal.task_id
                && draft.proposal_id == proposal.proposal_id
                && draft.plan_revision == proposal.plan_revision,
            "execution.proposal",
            "execution must bind exact proposal/task/plan identities",
        )?;
        require(
            draft.effects.len() <= proposal.actions.len(),
            "execution.effects",
            "too many effects",
        )?;
        let mut ids = BTreeSet::new();
        for effect in &draft.effects {
            require(
                ids.insert(&effect.fields().action_id),
                "execution.effects",
                "duplicate action effect",
            )?;
            require(
                proposal
                    .actions
                    .iter()
                    .any(|a| a.fields().action_id == effect.fields().action_id),
                "execution.effects",
                "effect refers to an action outside this proposal",
            )?;
        }
        unique(&draft.result_evidence, "execution.result_evidence")?;
        if let Some(error) = &draft.error {
            require(
                error.correlation_id() == &draft.context.correlation_id,
                "execution.error.correlation_id",
                "error correlation differs from execution",
            )?;
        }
        match draft.state {
            ExecutionState::Accepted
            | ExecutionState::Queued
            | ExecutionState::Running
            | ExecutionState::CancelRequested => {
                require(
                    draft.error.is_none()
                        && draft.result_evidence.is_empty()
                        && draft.effective_stop_evidence.is_none(),
                    "execution.state",
                    "active or cancel-request receipt cannot claim terminal result/stop",
                )?;
                if matches!(
                    draft.state,
                    ExecutionState::Accepted | ExecutionState::Queued
                ) {
                    require(
                        draft.effects.is_empty(),
                        "execution.effects",
                        "unstarted execution cannot claim effects",
                    )?;
                }
            }
            ExecutionState::Succeeded => {
                require(
                    draft.error.is_none()
                        && !draft.result_evidence.is_empty()
                        && draft.effective_stop_evidence.is_none()
                        && draft.effects.len() == proposal.actions.len()
                        && draft
                            .effects
                            .iter()
                            .all(|e| e.fields().outcome != EffectOutcome::Unknown),
                    "execution.state",
                    "success requires complete known effect records and result evidence",
                )?;
            }
            ExecutionState::Failed => {
                require(
                    draft.error.is_some()
                        && !draft.result_evidence.is_empty()
                        && draft.effective_stop_evidence.is_none(),
                    "execution.state",
                    "failure requires error and evidence",
                )?;
            }
            ExecutionState::Cancelled => {
                require(
                    draft.error.is_none()
                        && draft.effective_stop_evidence.is_some()
                        && !draft.result_evidence.is_empty(),
                    "execution.state",
                    "cancelled requires effective-stop and partial-result evidence",
                )?;
            }
            ExecutionState::OutcomeUnknown => {
                require(
                    draft
                        .error
                        .as_ref()
                        .is_some_and(|e| e.code() == ErrorCode::OutcomeUnknown)
                        && !draft.result_evidence.is_empty()
                        && draft.effective_stop_evidence.is_none(),
                    "execution.state",
                    "unknown outcome requires explicit OUTCOME_UNKNOWN and observation evidence",
                )?;
            }
        }
        if draft.state.is_terminal() && draft.state != ExecutionState::OutcomeUnknown {
            require(
                !draft
                    .effects
                    .iter()
                    .any(|e| e.fields().outcome == EffectOutcome::Unknown)
                    && !draft
                        .error
                        .as_ref()
                        .is_some_and(|e| e.code() == ErrorCode::OutcomeUnknown),
                "execution.state",
                "unreconciled outcomes must remain outcome_unknown",
            )?;
        }
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &ExecutionDraft {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    TaskCreated,
    ActionProposed,
    ExecutionAccepted,
    ExecutionProgress,
    CancellationRequested,
    CancellationEffective,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionOutcomeUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventDraft {
    pub contract_version: ContractVersion,
    pub event_id: EventId,
    pub task_id: TaskId,
    pub execution_id: Option<ExecutionId>,
    pub state_revision: Revision,
    pub timestamp: Timestamp,
    pub correlation_id: CorrelationId,
    pub kind: EventKind,
    pub evidence: Vec<EvidenceRef>,
}

/// Stream ordering, uniqueness across deliveries and evidence resolution are runtime concerns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRecord(EventDraft);
impl EventRecord {
    pub fn new(draft: EventDraft) -> Result<Self, ValidationError> {
        let execution_event = !matches!(
            draft.kind,
            EventKind::TaskCreated | EventKind::ActionProposed
        );
        require(
            draft.execution_id.is_some() == execution_event,
            "event.execution_id",
            "event kind requires matching execution identity presence",
        )?;
        unique(&draft.evidence, "event.evidence")?;
        require(
            !draft.evidence.is_empty(),
            "event.evidence",
            "event requires an evidence reference",
        )?;
        Ok(Self(draft))
    }
    pub fn fields(&self) -> &EventDraft {
        &self.0
    }
}
