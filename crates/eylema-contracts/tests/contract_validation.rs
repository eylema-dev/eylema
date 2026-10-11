use eylema_contracts::*;

fn context() -> RequestContext {
    RequestContext {
        request_id: RequestId::new("req:1").unwrap(),
        correlation_id: CorrelationId::new("trace:1").unwrap(),
        contract_version: ContractVersion::CURRENT,
    }
}
fn revision(n: u64) -> Revision {
    Revision::new(n).unwrap()
}
fn budget() -> Budget {
    Budget::new(10, 20, 1024, 1, Timestamp::new(2000).unwrap()).unwrap()
}
fn scope(name: &str) -> ResourceScope {
    ResourceScope::new(vec![ResourceId::new(name).unwrap()]).unwrap()
}
fn target() -> ExecutionTarget {
    ExecutionTarget::new(
        ExecutionContext::NativeMacos,
        Backend::NativeCpu,
        GpuApi::None,
    )
    .unwrap()
}
fn task() -> TaskSpec {
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
fn authority(resource: &str) -> AuthorityRequirement {
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
fn action() -> ActionDraft {
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
fn proposal() -> ProposalDraft {
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
fn execution() -> ExecutionDraft {
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
fn evidence() -> EvidenceRef {
    EvidenceRef::new("observation:1").unwrap()
}
fn effect(outcome: EffectOutcome) -> EffectRecord {
    EffectRecord::new(EffectDraft {
        action_id: ActionId::new("action:1").unwrap(),
        outcome,
        evidence: vec![evidence()],
    })
    .unwrap()
}
fn error(code: ErrorCode) -> ProtocolError {
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

#[test]
fn exact_runtime_version_is_independent_and_future_versions_fail() {
    assert_eq!(
        ContractVersion::new(0, 1, 0).unwrap().components(),
        (0, 1, 0)
    );
    for tuple in [(0, 0, 0), (0, 1, 1), (0, 2, 0), (1, 0, 0), (u16::MAX, 1, 0)] {
        let failure = ContractVersion::new(tuple.0, tuple.1, tuple.2).unwrap_err();
        assert_eq!(failure.code(), ErrorCode::InvalidRequest);
        assert_eq!(failure.field, "contract_version");
    }
    assert_eq!(
        task().fields().context.contract_version,
        ContractVersion::CURRENT
    );
    assert_eq!(action().operation_version, ContractVersion::CURRENT);
}

#[test]
fn opaque_ids_reject_unbounded_unicode_control_and_path_inputs() {
    assert!(TaskId::new("t".repeat(MAX_ID_BYTES)).is_ok());
    for value in [
        "".into(),
        "t".repeat(MAX_ID_BYTES + 1),
        "task with space".into(),
        "task\0".into(),
        "task/child".into(),
        "task\\child".into(),
        "görev".into(),
        "*".into(),
    ] {
        assert!(TaskId::new(value).is_err());
    }
    assert!(Revision::new(0).is_err());
    assert!(Timestamp::new(0).is_err());
    assert_eq!(Revision::new(u64::MAX).unwrap().get(), u64::MAX);
}

#[test]
fn bilingual_text_uses_byte_bounds_and_rejects_empty_or_hidden_controls() {
    let mut draft = task().fields().clone();
    draft.objective = "Görevi incele; execution kimliği değişmez.\nReview the task.".into();
    assert!(TaskSpec::new(draft.clone()).is_ok());
    for value in [
        "".into(),
        " \t\n".into(),
        "x".repeat(MAX_TEXT_BYTES + 1),
        "a\0b".into(),
        "ö".repeat(MAX_TEXT_BYTES),
    ] {
        draft.objective = value;
        assert!(TaskSpec::new(draft.clone()).is_err());
    }
}

#[test]
fn finite_budgets_reject_zero_and_protocol_oversize() {
    let deadline = Timestamp::new(1).unwrap();
    for (steps, events, bytes, parallel) in [
        (0, 1, 1, 1),
        (1, 0, 1, 1),
        (1, 1, 0, 1),
        (1, 1, 1, 0),
        (1, 1, MAX_PAYLOAD_BYTES as u32 + 1, 1),
    ] {
        assert!(Budget::new(steps, events, bytes, parallel, deadline).is_err());
    }
    assert_eq!(
        Budget::new(1, 1, MAX_PAYLOAD_BYTES as u32, 1, deadline)
            .unwrap()
            .max_payload_bytes(),
        MAX_PAYLOAD_BYTES as u32
    );
}

#[test]
fn all_context_backend_gpu_combinations_reject_cross_surface_claims() {
    let mut accepted = 0;
    for context in [ExecutionContext::NativeMacos, ExecutionContext::Browser] {
        for backend in [
            Backend::NativeCpu,
            Backend::NativeGpu,
            Backend::WasmCpu,
            Backend::WebGpu,
        ] {
            for api in [GpuApi::None, GpuApi::Metal, GpuApi::WebGpu] {
                let expected = match context {
                    ExecutionContext::NativeMacos => [
                        (Backend::NativeCpu, GpuApi::None),
                        (Backend::NativeGpu, GpuApi::Metal),
                    ]
                    .contains(&(backend, api)),
                    ExecutionContext::Browser => [
                        (Backend::WasmCpu, GpuApi::None),
                        (Backend::WebGpu, GpuApi::WebGpu),
                    ]
                    .contains(&(backend, api)),
                };
                assert_eq!(
                    ExecutionTarget::new(context, backend, api).is_ok(),
                    expected
                );
                accepted += usize::from(expected);
            }
        }
    }
    assert_eq!(accepted, 4);
}

#[test]
fn scopes_and_composed_oversight_preserve_distinct_requirements() {
    assert!(ResourceScope::new(vec![]).is_err());
    let id = ResourceId::new("state:1").unwrap();
    assert!(ResourceScope::new(vec![id.clone(), id]).is_err());
    assert!(
        ResourceScope::new(
            (0..=MAX_REFERENCES)
                .map(|i| ResourceId::new(format!("r:{i}")).unwrap())
                .collect()
        )
        .is_err()
    );
    let a = authority("state:1");
    assert_eq!(a.principal_ref().as_str(), "principal:unverified");
    assert!(a.oversight().requires_observation());
    assert!(a.oversight().requires_intervention());
    assert_eq!(a.oversight().checkpoints().len(), 1);
    assert!(a.oversight().governance().is_some());
    assert!(
        AuthorityRequirement::new(
            a.principal_ref().clone(),
            revision(3),
            scope("state:1"),
            vec![],
            a.oversight().clone()
        )
        .is_err()
    );
    assert!(
        AuthorityRequirement::new(
            a.principal_ref().clone(),
            revision(3),
            scope("state:1"),
            vec![Permission::Read, Permission::Read],
            a.oversight().clone()
        )
        .is_err()
    );
    let checkpoint = CheckpointRef::new("checkpoint:1").unwrap();
    assert!(
        OversightRequirements::new(vec![checkpoint.clone(), checkpoint], false, false, None)
            .is_err()
    );
}

#[test]
fn actions_reject_mismatched_scope_missing_precondition_and_large_payloads() {
    for mode in 0..4 {
        let mut draft = action();
        match mode {
            0 => draft.authority = authority("other:state"),
            1 => draft.expected_revision = None,
            2 => draft.payload = vec![0; 1025],
            _ => draft
                .required_capabilities
                .push(draft.required_capabilities[0].clone()),
        }
        assert!(ActionSpec::new(draft).is_err());
    }
    let mut read = action();
    read.side_effect_class = SideEffectClass::Read;
    read.expected_revision = None;
    assert!(ActionSpec::new(read).is_ok());
}

#[test]
fn proposals_reject_identity_revision_policy_scope_target_budget_and_duplicate_drift() {
    for mode in 0..11 {
        let mut draft = proposal();
        let mut step = action();
        match mode {
            0 => draft.task_id = TaskId::new("other:task").unwrap(),
            1 => draft.state_revision = revision(1),
            2 => draft.policy_revision = revision(2),
            3 => draft.actions.clear(),
            4 => draft.actions.push(draft.actions[0].clone()),
            5 => step.task_id = TaskId::new("other:task").unwrap(),
            6 => {
                step.resource_scope = scope("other:resource");
                step.authority = authority("other:resource");
            }
            7 => step.expected_revision = Some(revision(1)),
            8 => {
                step.target =
                    ExecutionTarget::new(ExecutionContext::Browser, Backend::WasmCpu, GpuApi::None)
                        .unwrap()
            }
            9 => step.budget = Budget::new(11, 20, 1024, 1, Timestamp::new(2000).unwrap()).unwrap(),
            _ => step.budget = Budget::new(10, 20, 1024, 1, Timestamp::new(2001).unwrap()).unwrap(),
        }
        if mode >= 5 {
            draft.actions = vec![ActionSpec::new(step).unwrap()];
        }
        assert!(ActionProposal::new(draft, &task()).is_err(), "mode {mode}");
    }
    let mut draft = proposal();
    let mut second = action();
    second.action_id = ActionId::new("action:2").unwrap();
    second.payload = vec![1; 1024];
    draft.actions.push(ActionSpec::new(second).unwrap());
    assert!(ActionProposal::new(draft, &task()).is_err());
}

#[test]
fn cancellation_receipt_is_not_effective_stop_and_late_stop_does_not_rewrite_success() {
    let p = ActionProposal::new(proposal(), &task()).unwrap();
    let mut draft = execution();
    draft.state = ExecutionState::CancelRequested;
    assert!(
        !ExecutionRecord::new(draft.clone(), &p)
            .unwrap()
            .fields()
            .state
            .is_terminal()
    );
    draft.effective_stop_evidence = Some(evidence());
    assert!(ExecutionRecord::new(draft.clone(), &p).is_err());
    draft.state = ExecutionState::Cancelled;
    assert!(ExecutionRecord::new(draft.clone(), &p).is_err());
    draft.result_evidence.push(evidence());
    draft.effects.push(effect(EffectOutcome::Applied));
    assert!(ExecutionRecord::new(draft.clone(), &p).is_ok());
    draft.state = ExecutionState::Succeeded;
    draft.effective_stop_evidence = None;
    assert!(ExecutionRecord::new(draft.clone(), &p).is_ok());
    draft.effective_stop_evidence = Some(evidence());
    assert!(ExecutionRecord::new(draft, &p).is_err());
}

#[test]
fn outcome_unknown_cannot_be_promoted_to_success_failure_or_cancelled() {
    let p = ActionProposal::new(proposal(), &task()).unwrap();
    let mut draft = execution();
    draft.effects.push(effect(EffectOutcome::Unknown));
    draft.result_evidence.push(evidence());
    for state in [
        ExecutionState::Succeeded,
        ExecutionState::Failed,
        ExecutionState::Cancelled,
    ] {
        draft.state = state;
        draft.error = (state == ExecutionState::Failed).then(|| error(ErrorCode::Forbidden));
        draft.effective_stop_evidence = (state == ExecutionState::Cancelled).then(evidence);
        assert!(ExecutionRecord::new(draft.clone(), &p).is_err());
    }
    draft.state = ExecutionState::OutcomeUnknown;
    draft.error = Some(error(ErrorCode::OutcomeUnknown));
    draft.effective_stop_evidence = None;
    assert!(ExecutionRecord::new(draft.clone(), &p).is_ok());
    draft.error = Some(error(ErrorCode::Forbidden));
    assert!(ExecutionRecord::new(draft, &p).is_err());
}

#[test]
fn failure_snapshots_require_correlated_errors_and_preserve_partial_effects() {
    let p = ActionProposal::new(proposal(), &task()).unwrap();
    let mut draft = execution();
    draft.state = ExecutionState::Failed;
    draft.result_evidence.push(evidence());
    draft.effects.push(effect(EffectOutcome::Applied));
    assert!(ExecutionRecord::new(draft.clone(), &p).is_err());
    draft.error = Some(error(ErrorCode::DeadlineExceeded));
    let observed = ExecutionRecord::new(draft.clone(), &p).unwrap();
    assert_eq!(
        observed.fields().effects[0].fields().outcome,
        EffectOutcome::Applied
    );
    draft.context.correlation_id = CorrelationId::new("other:trace").unwrap();
    assert!(ExecutionRecord::new(draft, &p).is_err());
    // Editing a detached input draft cannot mutate a previously validated record.
    let mut detached = observed.fields().clone();
    detached.effects.clear();
    assert_eq!(observed.fields().effects.len(), 1);
}

#[test]
fn executions_reject_foreign_duplicate_and_unstarted_effects_and_stale_bindings() {
    let p = ActionProposal::new(proposal(), &task()).unwrap();
    for mode in 0..7 {
        let mut draft = execution();
        match mode {
            0 => draft.task_id = TaskId::new("other:task").unwrap(),
            1 => draft.proposal_id = ProposalId::new("other:proposal").unwrap(),
            2 => draft.plan_revision = revision(2),
            3 => draft.state_revision = revision(1),
            4 => draft.effects.push(effect(EffectOutcome::Applied)),
            5 => {
                draft.state = ExecutionState::Running;
                draft.effects.push(
                    EffectRecord::new(EffectDraft {
                        action_id: ActionId::new("foreign:action").unwrap(),
                        outcome: EffectOutcome::NotApplied,
                        evidence: vec![],
                    })
                    .unwrap(),
                );
            }
            _ => {
                draft.state = ExecutionState::Running;
                draft.effects = vec![effect(EffectOutcome::Applied); 2];
            }
        }
        assert!(ExecutionRecord::new(draft, &p).is_err());
    }
    assert!(
        EffectRecord::new(EffectDraft {
            action_id: ActionId::new("action:1").unwrap(),
            outcome: EffectOutcome::Applied,
            evidence: vec![]
        })
        .is_err()
    );
}

#[test]
fn errors_keep_stable_language_neutral_codes_and_honest_retry_metadata() {
    let names: std::collections::BTreeSet<_> =
        ErrorCode::ALL.map(ErrorCode::as_str).into_iter().collect();
    assert_eq!(names.len(), 13);
    assert!(names.contains("OUTCOME_UNKNOWN"));
    for code in ErrorCode::ALL {
        let en = ProtocolError::new(
            code,
            "Access denied".into(),
            Some("authority.resource_scope".into()),
            false,
            context().correlation_id,
            None,
        )
        .unwrap();
        let tr = ProtocolError::new(
            code,
            "Erişim reddedildi".into(),
            None,
            false,
            context().correlation_id,
            None,
        )
        .unwrap();
        assert_eq!(en.code(), tr.code());
        assert_ne!(en.message(), tr.message());
        assert_eq!(en.field_path(), Some("authority.resource_scope"));
    }
    assert!(
        ProtocolError::new(
            ErrorCode::Forbidden,
            "Denied".into(),
            None,
            false,
            context().correlation_id,
            Some(5)
        )
        .is_err()
    );
    assert!(
        ProtocolError::new(
            ErrorCode::Forbidden,
            "".into(),
            None,
            false,
            context().correlation_id,
            None
        )
        .is_err()
    );
    assert!(
        ProtocolError::new(
            ErrorCode::Forbidden,
            "Denied".into(),
            Some("bad\0path".into()),
            false,
            context().correlation_id,
            None
        )
        .is_err()
    );
    let retry = ProtocolError::new(
        ErrorCode::CapabilityUnavailable,
        "Temporarily unavailable".into(),
        None,
        true,
        context().correlation_id,
        Some(5),
    )
    .unwrap();
    assert!(retry.retryable());
    assert_eq!(retry.retry_after_ms(), Some(5));
}

#[test]
fn events_require_execution_identity_when_applicable_and_bounded_evidence() {
    let mut draft = EventDraft {
        contract_version: ContractVersion::CURRENT,
        event_id: EventId::new("event:1").unwrap(),
        task_id: task().fields().task_id.clone(),
        execution_id: None,
        state_revision: revision(2),
        timestamp: Timestamp::new(1).unwrap(),
        correlation_id: context().correlation_id,
        kind: EventKind::TaskCreated,
        evidence: vec![evidence()],
    };
    assert!(EventRecord::new(draft.clone()).is_ok());
    draft.kind = EventKind::CancellationRequested;
    assert!(EventRecord::new(draft.clone()).is_err());
    draft.execution_id = Some(execution().execution_id);
    assert!(EventRecord::new(draft.clone()).is_ok());
    draft.evidence.push(evidence());
    assert!(EventRecord::new(draft.clone()).is_err());
    draft.evidence.clear();
    assert!(EventRecord::new(draft).is_err());
}
