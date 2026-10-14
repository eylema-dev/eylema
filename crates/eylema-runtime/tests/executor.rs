mod support;
use eylema_contracts::*;
use eylema_runtime::*;
use support::*;

#[test]
fn actual_owned_state_and_event_observations_follow_success() {
    let (mut r, _, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 10, 20);
    let p = proposal(&t, &[2, 3]);
    r.create_task(&s, t.clone()).unwrap();
    let id = r.admit(&s, t.clone(), p).unwrap();
    assert_eq!(
        r.execution(&s, &id).unwrap().fields().state,
        ExecutionState::Queued
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &id).unwrap().fields().state,
        ExecutionState::Running
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
    r.step().unwrap();
    let done = r.execution(&s, &id).unwrap();
    assert_eq!(done.fields().state, ExecutionState::Succeeded);
    assert_eq!(done.fields().effects.len(), 2);
    let state = r.task(&s, &t.fields().task_id).unwrap();
    assert_eq!(
        (state.value, state.performed_steps, state.charged_steps),
        (5, 2, 2)
    );
    assert_eq!(state.task.fields().revision, rev(4));
    assert_eq!(done.fields().state_revision, rev(2));
    let events = r.events(&s, &t.fields().task_id, None, 256).unwrap();
    assert_eq!(events.len(), 6);
    for e in events {
        for evidence in &e.fields().evidence {
            let observed = r.observation(&s, evidence).unwrap();
            assert_eq!(observed.event_id, e.fields().event_id);
        }
    }
    assert!(r.step().unwrap().is_none());
}
#[test]
fn overflow_failure_preserves_known_partial_effects_and_live_revision() {
    let (mut r, _, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 10, 20);
    r.create_task(&s, t.clone()).unwrap();
    let id = r
        .admit(&s, t.clone(), proposal(&t, &[u64::MAX, 1]))
        .unwrap();
    r.step().unwrap();
    r.step().unwrap();
    let e = r.execution(&s, &id).unwrap();
    assert_eq!(e.fields().state, ExecutionState::Failed);
    assert_eq!(
        e.fields().error.as_ref().unwrap().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        e.fields().effects[0].fields().outcome,
        EffectOutcome::Applied
    );
    assert_eq!(
        e.fields().effects[1].fields().outcome,
        EffectOutcome::NotApplied
    );
    let state = r.task(&s, &t.fields().task_id).unwrap();
    assert_eq!(state.value, u64::MAX);
    assert_eq!(state.task.fields().revision, rev(3));
}
#[test]
fn cross_runtime_host_session_and_self_claimed_principal_do_not_authorize() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let (mut other, other_host, other_s, _, _) = fixture(Limits::default());
    assert!(matches!(
        r.enroll(&other_host, grant()),
        Err(ErrorCode::Forbidden)
    ));
    assert!(matches!(
        other.enroll(&host, grant()),
        Err(ErrorCode::Forbidden)
    ));
    let t = task("task:1", 10, 20);
    assert_eq!(
        r.create_task(&other_s, t.clone()),
        Err(ErrorCode::Forbidden)
    );
    r.create_task(&s, t.clone()).unwrap();
    let mut pd = proposal(&t, &[1]).fields().clone();
    let mut ad = pd.actions[0].fields().clone();
    ad.authority = AuthorityRequirement::new(
        PrincipalRef::new("principal:claimed").unwrap(),
        rev(3),
        scope(),
        vec![Permission::ExecuteAction],
        OversightRequirements::new(vec![], false, false, None).unwrap(),
    )
    .unwrap();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), ActionProposal::new(pd, &t).unwrap()),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
    let mut denied = grant();
    denied.permissions = vec![Permission::CreateTask, Permission::Read];
    let ds = r.enroll(&host, denied).unwrap();
    assert_eq!(
        r.admit(&ds, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::Forbidden)
    );
}
#[test]
fn checkpoint_intervention_governance_and_missing_observation_fail_closed() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 10, 20);
    r.create_task(&s, t.clone()).unwrap();
    for (checkpoints, intervention, governance, code) in [
        (
            vec![CheckpointRef::new("review:1").unwrap()],
            false,
            None,
            ErrorCode::ApprovalRequired,
        ),
        (vec![], true, None, ErrorCode::CapabilityUnavailable),
        (
            vec![],
            false,
            Some(GovernanceRef::new("policy:3").unwrap()),
            ErrorCode::CapabilityUnavailable,
        ),
    ] {
        let mut pd = proposal(&t, &[1]).fields().clone();
        let mut ad = pd.actions[0].fields().clone();
        ad.authority = AuthorityRequirement::new(
            grant().principal,
            rev(3),
            scope(),
            vec![Permission::ExecuteAction],
            OversightRequirements::new(checkpoints, true, intervention, governance).unwrap(),
        )
        .unwrap();
        pd.actions[0] = ActionSpec::new(ad).unwrap();
        assert_eq!(
            r.admit(&s, t.clone(), ActionProposal::new(pd, &t).unwrap()),
            Err(code)
        );
    }
    let mut g = grant();
    g.permissions.retain(|p| *p != Permission::ObserveEvents);
    let no_observation = r.enroll(&host, g).unwrap();
    assert_eq!(
        r.admit(&no_observation, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::Forbidden)
    );
}
#[test]
fn replay_returns_same_queued_and_completed_result_and_changed_content_conflicts() {
    let (mut r, _, s, clock, _) = fixture(Limits::default());
    let t = task("task:1", 10, 20);
    let p = proposal(&t, &[2]);
    r.create_task(&s, t.clone()).unwrap();
    let id = r.admit(&s, t.clone(), p.clone()).unwrap();
    assert_eq!(r.admit(&s, t.clone(), p.clone()).unwrap(), id);
    assert_eq!(
        r.admit(&s, t.clone(), proposal(&t, &[3])),
        Err(ErrorCode::IdempotencyConflict)
    );
    r.step().unwrap();
    clock.set(3000);
    assert_eq!(r.admit(&s, t.clone(), p).unwrap(), id);
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().performed_steps, 1);
}
#[test]
fn admission_and_running_deadlines_do_not_claim_effective_stop() {
    let (mut r, _, s, clock, _) = fixture(Limits::default());
    let t = task("task:1", 10, 20);
    r.create_task(&s, t.clone()).unwrap();
    clock.set(2000);
    assert_eq!(
        r.admit(&s, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::DeadlineExceeded)
    );
    clock.set(1000);
    let id = r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    r.step().unwrap();
    clock.set(2000);
    r.step().unwrap();
    let e = r.execution(&s, &id).unwrap();
    assert_eq!(e.fields().state, ExecutionState::Failed);
    assert_eq!(
        e.fields().error.as_ref().unwrap().code(),
        ErrorCode::DeadlineExceeded
    );
    assert!(e.fields().effective_stop_evidence.is_none());
    assert_eq!(e.fields().effects.len(), 1);
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
}
#[test]
fn finite_event_queue_resident_and_task_lease_reject_without_owned_writes() {
    let (mut r, _, s, _, _) = fixture(Limits {
        queued: 1,
        ..Limits::default()
    });
    let t = task("task:1", 10, 20);
    let t2 = task("task:2", 10, 20);
    r.create_task(&s, t.clone()).unwrap();
    r.create_task(&s, t2.clone()).unwrap();
    let id = r.admit(&s, t.clone(), proposal(&t, &[1])).unwrap();
    assert_eq!(
        r.admit(&s, t2.clone(), proposal(&t2, &[1])),
        Err(ErrorCode::BudgetExceeded)
    );
    assert_eq!(r.task(&s, &t2.fields().task_id).unwrap().charged_steps, 0);
    r.step().unwrap();
    let id2 = r.admit(&s, t2.clone(), proposal(&t2, &[1])).unwrap();
    assert_ne!(id, id2);
    r.step().unwrap();
    let (mut r, _, s, _, _) = fixture(Limits::default());
    let t = task("task:events", 10, 4);
    r.create_task(&s, t.clone()).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::BudgetExceeded)
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().charged_steps, 0);
    let (mut r, _, s, _, _) = fixture(Limits {
        resident_wire_bytes: 1,
        ..Limits::default()
    });
    assert_eq!(
        r.create_task(&s, task("task:resident", 10, 20)),
        Err(ErrorCode::BudgetExceeded)
    );
}
#[test]
fn failed_host_clock_leaves_queued_work_and_effects_unchanged() {
    let (mut r, _, s, _, fail) = fixture(Limits::default());
    let t = task("task:1", 10, 20);
    r.create_task(&s, t.clone()).unwrap();
    let id = r.admit(&s, t.clone(), proposal(&t, &[2])).unwrap();
    fail.set(true);
    assert_eq!(r.step(), Err(ErrorCode::CapabilityUnavailable));
    assert_eq!(
        r.execution(&s, &id).unwrap().fields().state,
        ExecutionState::Queued
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
    fail.set(false);
    r.step().unwrap();
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
}

#[test]
fn stale_state_policy_unsupported_target_operation_and_capability_fail_before_effect() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let mut g = grant();
    g.policy_revision = rev(4);
    let wrong_policy = r.enroll(&host, g).unwrap();
    assert_eq!(
        r.admit(&wrong_policy, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::RevisionConflict)
    );
    for (operation, capability) in [
        ("unknownOperation", "task:state"),
        ("addTaskValue", "unavailable:cap"),
    ] {
        let mut pd = proposal(&t, &[1]).fields().clone();
        let mut ad = pd.actions[0].fields().clone();
        ad.operation_id = OperationId::new(operation).unwrap();
        ad.required_capabilities = vec![CapabilityId::new(capability).unwrap()];
        pd.actions[0] = ActionSpec::new(ad).unwrap();
        assert_eq!(
            r.admit(&s, t.clone(), ActionProposal::new(pd, &t).unwrap()),
            Err(ErrorCode::CapabilityUnavailable)
        );
    }
    let id = r.admit(&s, t.clone(), proposal(&t, &[1])).unwrap();
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &id).unwrap().fields().state,
        ExecutionState::Succeeded
    );
    let mut pd = proposal(&t, &[1]).fields().clone();
    let mut ad = pd.actions[0].fields().clone();
    pd.proposal_id = ProposalId::new("proposal:stale").unwrap();
    ad.idempotency_key = IdempotencyKey::new("new:attempt").unwrap();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), ActionProposal::new(pd, &t).unwrap()),
        Err(ErrorCode::RevisionConflict)
    );
    let mut td = task("task:gpu", 10, 20).fields().clone();
    td.target = ExecutionTarget::new(
        ExecutionContext::NativeMacos,
        Backend::NativeGpu,
        GpuApi::Metal,
    )
    .unwrap();
    let gpu = TaskSpec::new(td).unwrap();
    r.create_task(&s, gpu.clone()).unwrap();
    assert_eq!(
        r.admit(&s, gpu.clone(), proposal(&gpu, &[1])),
        Err(ErrorCode::CapabilityUnavailable)
    );
    assert_eq!(r.task(&s, &gpu.fields().task_id).unwrap().value, 0);
}
#[test]
fn fifo_task_lease_and_partial_replay_conflicts_are_explicit() {
    let (mut r, _, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 10, 30);
    let t2 = task("task:2", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    r.create_task(&s, t2.clone()).unwrap();
    let id = r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    let id2 = r.admit(&s, t2.clone(), proposal(&t2, &[7])).unwrap();
    let mut pd = proposal(&t, &[1]).fields().clone();
    let mut ad = pd.actions[0].fields().clone();
    pd.proposal_id = ProposalId::new("proposal:lease").unwrap();
    ad.idempotency_key = IdempotencyKey::new("new:while-active").unwrap();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    let before = r.task(&s, &t.fields().task_id).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), ActionProposal::new(pd, &t).unwrap()),
        Err(ErrorCode::BudgetExceeded)
    );
    assert_eq!(before, r.task(&s, &t.fields().task_id).unwrap());
    assert_eq!(r.step().unwrap(), Some(id.clone()));
    assert_eq!(r.task(&s, &t2.fields().task_id).unwrap().value, 0);
    assert_eq!(r.step().unwrap(), Some(id));
    assert_eq!(r.step().unwrap(), Some(id2));
    assert_eq!(
        r.admit(&s, t.clone(), proposal(&t, &[2, 3, 4])),
        Err(ErrorCode::IdempotencyConflict)
    );
}
#[test]
fn invalid_limits_sessions_id_sources_and_cursor_bounds_fail() {
    let (_, _, _, clock, fail) = fixture(Limits::default());
    let services = Services {
        clock: clock.clone(),
        fail_clock: fail.clone(),
        next: 0,
        duplicate_event: true,
    };
    let (mut r, host) = Runtime::new(services, Limits::default()).unwrap();
    let s = r.enroll(&host, grant()).unwrap();
    let t = task("task:1", 10, 20);
    r.create_task(&s, t.clone()).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().charged_steps, 0);
    assert!(matches!(
        r.events(&s, &t.fields().task_id, None, 257),
        Err(ErrorCode::InvalidRequest)
    ));
    assert!(matches!(
        r.events(
            &s,
            &t.fields().task_id,
            Some(&EventId::new("unknown:cursor").unwrap()),
            1
        ),
        Err(ErrorCode::CursorExpired)
    ));
    let (mut r, host, _, _, _) = fixture(Limits {
        sessions: 1,
        ..Limits::default()
    });
    assert!(matches!(
        r.enroll(&host, grant()),
        Err(ErrorCode::BudgetExceeded)
    ));
    let services = Services {
        clock,
        fail_clock: fail,
        next: 0,
        duplicate_event: false,
    };
    assert!(matches!(
        Runtime::new(
            services,
            Limits {
                tasks: 0,
                ..Limits::default()
            }
        ),
        Err(ErrorCode::BudgetExceeded)
    ));
}
#[test]
fn step_budget_exhaustion_and_read_permission_are_independent_of_claimed_requirement() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 1, 20);
    r.create_task(&s, t.clone()).unwrap();
    let id = r.admit(&s, t.clone(), proposal(&t, &[1])).unwrap();
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &id).unwrap().fields().state,
        ExecutionState::Succeeded
    );
    let current = r.task(&s, &t.fields().task_id).unwrap().task;
    let mut pd = proposal(&current, &[1]).fields().clone();
    let mut ad = pd.actions[0].fields().clone();
    pd.proposal_id = ProposalId::new("proposal:fresh-budget").unwrap();
    ad.idempotency_key = IdempotencyKey::new("fresh:over-budget").unwrap();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    assert_eq!(
        r.admit(
            &s,
            current.clone(),
            ActionProposal::new(pd, &current).unwrap()
        ),
        Err(ErrorCode::BudgetExceeded)
    );
    let t2 = task("task:read", 10, 20);
    r.create_task(&s, t2.clone()).unwrap();
    let mut pd = proposal(&t2, &[1]).fields().clone();
    let mut ad = pd.actions[0].fields().clone();
    ad.operation_id = OperationId::new("readTaskValue").unwrap();
    ad.side_effect_class = SideEffectClass::Read;
    ad.payload.clear();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    let read = ActionProposal::new(pd, &t2).unwrap();
    let mut g = grant();
    g.permissions.retain(|p| *p != Permission::Read);
    let denied = r.enroll(&host, g).unwrap();
    assert_eq!(
        r.admit(&denied, t2.clone(), read.clone()),
        Err(ErrorCode::Forbidden)
    );
    let id = r.admit(&s, t2.clone(), read).unwrap();
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &id).unwrap().fields().effects[0]
            .fields()
            .outcome,
        EffectOutcome::NotApplied
    );
    assert_eq!(
        r.task(&s, &t2.fields().task_id)
            .unwrap()
            .task
            .fields()
            .revision,
        rev(2)
    );
}

#[test]
fn proposal_revision_identity_cannot_be_rebound_using_fresh_idempotency_keys() {
    let (mut r, _, s, _, _) = fixture(Limits::default());
    let t = task("task:1", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    r.admit(&s, t.clone(), proposal(&t, &[2])).unwrap();
    let mut pd = proposal(&t, &[3]).fields().clone();
    let mut ad = pd.actions[0].fields().clone();
    ad.idempotency_key = IdempotencyKey::new("fresh:conflicting-plan").unwrap();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), ActionProposal::new(pd, &t).unwrap()),
        Err(ErrorCode::IdempotencyConflict)
    );
}
#[test]
fn deadline_is_rechecked_after_fallible_host_metadata_reservations() {
    struct Delayed {
        clock: u64,
        event: u64,
        delay: bool,
    }
    impl HostServices for Delayed {
        fn now(&mut self) -> Result<Timestamp> {
            Ok(Timestamp::new(self.clock).unwrap())
        }
        fn execution_id(&mut self) -> Result<ExecutionId> {
            Ok(ExecutionId::new("execution:delayed").unwrap())
        }
        fn event_id(&mut self) -> Result<EventId> {
            self.event += 1;
            if self.delay || self.event > 1 {
                self.clock = 2000
            }
            Ok(EventId::new(format!("event:{}", self.event)).unwrap())
        }
        fn sha256(&mut self, b: &[u8]) -> Result<[u8; 32]> {
            eylema_platform::sha256(b).map_err(|_| ErrorCode::CapabilityUnavailable)
        }
    }
    let (mut r, host) = Runtime::new(
        Delayed {
            clock: 1000,
            event: 0,
            delay: true,
        },
        Limits::default(),
    )
    .unwrap();
    let s = r.enroll(&host, grant()).unwrap();
    let t = task("task:1", 10, 20);
    assert_eq!(
        r.create_task(&s, t.clone()),
        Err(ErrorCode::DeadlineExceeded)
    );
    assert!(matches!(
        r.task(&s, &t.fields().task_id),
        Err(ErrorCode::InvalidRequest)
    ));
    let (mut r, host) = Runtime::new(
        Delayed {
            clock: 1000,
            event: 0,
            delay: false,
        },
        Limits::default(),
    )
    .unwrap();
    let s = r.enroll(&host, grant()).unwrap();
    r.create_task(&s, t.clone()).unwrap();
    let before = r.task(&s, &t.fields().task_id).unwrap();
    assert_eq!(
        r.admit(&s, t.clone(), proposal(&t, &[1])),
        Err(ErrorCode::DeadlineExceeded)
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap(), before);
    assert_eq!(
        r.events(&s, &t.fields().task_id, None, 256).unwrap().len(),
        1
    );
}

#[test]
fn trusted_fifo_head_is_read_only_instance_scoped_and_matches_worker_order() {
    let (mut r, key, s, _, _) = fixture(Limits::default());
    let (_, foreign, _, _, _) = fixture(Limits::default());
    assert_eq!(r.pending_execution(&foreign), Err(ErrorCode::Forbidden));
    assert_eq!(r.pending_execution(&key), Ok(None));
    let a = task("task:fifo-a", 10, 30);
    let b = task("task:fifo-b", 10, 30);
    r.create_task(&s, a.clone()).unwrap();
    r.create_task(&s, b.clone()).unwrap();
    let ae = r.admit(&s, a.clone(), proposal(&a, &[1, 2])).unwrap();
    let be = r.admit(&s, b.clone(), proposal(&b, &[7])).unwrap();
    assert_eq!(r.pending_execution(&key), Ok(Some(ae.clone())));
    assert_eq!(r.task(&s, &a.fields().task_id).unwrap().value, 0);
    r.step().unwrap();
    assert_eq!(r.pending_execution(&key), Ok(Some(ae)));
    r.step().unwrap();
    assert_eq!(r.pending_execution(&key), Ok(Some(be)));
    r.step().unwrap();
    assert_eq!(r.pending_execution(&key), Ok(None));
}
