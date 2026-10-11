//! Owned shared development fixtures; not compiled into production libraries.
#![allow(dead_code)]
use eylema_contracts::*;

pub fn context() -> RequestContext {
    RequestContext {
        request_id: RequestId::new("req:1").unwrap(),
        correlation_id: CorrelationId::new("trace:1").unwrap(),
        contract_version: ContractVersion::CURRENT,
    }
}
pub fn revision(n: u64) -> Revision {
    Revision::new(n).unwrap()
}
pub fn budget() -> Budget {
    Budget::new(10, 20, 1024, 1, Timestamp::new(2000).unwrap()).unwrap()
}
pub fn scope(name: &str) -> ResourceScope {
    ResourceScope::new(vec![ResourceId::new(name).unwrap()]).unwrap()
}
pub fn target() -> ExecutionTarget {
    ExecutionTarget::new(
        ExecutionContext::NativeMacos,
        Backend::NativeCpu,
        GpuApi::None,
    )
    .unwrap()
}
pub fn task() -> TaskSpec {
    TaskSpec::new(TaskDraft {
        context: context(),
        task_id: TaskId::new("task:1").unwrap(),
        revision: revision(2),
        objective: "Inspect the bounded task state".into(),
        resource_scope: scope("state:1"),
        budget: budget(),
        target: target(),
    })
    .unwrap()
}
pub fn authority(resource: &str) -> AuthorityRequirement {
    AuthorityRequirement::new(
        PrincipalRef::new("principal:unverified").unwrap(),
        revision(3),
        scope(resource),
        vec![Permission::ExecuteAction],
        OversightRequirements::new(
            vec![CheckpointRef::new("review:1").unwrap()],
            true,
            true,
            Some(GovernanceRef::new("policy:3").unwrap()),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn action() -> ActionDraft {
    ActionDraft {
        context: context(),
        action_id: ActionId::new("action:1").unwrap(),
        task_id: TaskId::new("task:1").unwrap(),
        operation_id: OperationId::new("updateTaskState").unwrap(),
        operation_version: ContractVersion::CURRENT,
        side_effect_class: SideEffectClass::RecordMutation,
        resource_scope: scope("state:1"),
        expected_revision: Some(revision(2)),
        authority: authority("state:1"),
        required_capabilities: vec![CapabilityId::new("task:state").unwrap()],
        budget: budget(),
        target: target(),
        idempotency_key: IdempotencyKey::new("idem:1").unwrap(),
        payload: b"opaque input".to_vec(),
    }
}
pub fn proposal() -> ProposalDraft {
    ProposalDraft {
        context: context(),
        proposal_id: ProposalId::new("proposal:1").unwrap(),
        task_id: TaskId::new("task:1").unwrap(),
        plan_revision: revision(1),
        state_revision: revision(2),
        policy_revision: revision(3),
        actions: vec![ActionSpec::new(action()).unwrap()],
    }
}
pub fn execution() -> ExecutionDraft {
    ExecutionDraft {
        context: context(),
        execution_id: ExecutionId::new("execution:1").unwrap(),
        task_id: TaskId::new("task:1").unwrap(),
        proposal_id: ProposalId::new("proposal:1").unwrap(),
        plan_revision: revision(1),
        state_revision: revision(2),
        state: ExecutionState::Queued,
        effects: vec![],
        result_evidence: vec![],
        effective_stop_evidence: None,
        error: None,
    }
}
pub fn evidence() -> EvidenceRef {
    EvidenceRef::new("observation:1").unwrap()
}
pub fn effect(outcome: EffectOutcome) -> EffectRecord {
    EffectRecord::new(EffectDraft {
        action_id: ActionId::new("action:1").unwrap(),
        outcome,
        evidence: vec![evidence()],
    })
    .unwrap()
}
pub fn error(code: ErrorCode) -> ProtocolError {
    ProtocolError::new(
        code,
        "Observed failure".into(),
        None,
        false,
        context().correlation_id,
        None,
    )
    .unwrap()
}
