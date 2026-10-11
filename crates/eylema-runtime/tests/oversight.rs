mod support;
use eylema_contracts::*;
use eylema_runtime::*;
use support::*;

fn human(r: &mut Runtime<Services>, host: &HostKey) -> HumanSession {
    let mut g = grant();
    g.principal = PrincipalRef::new("principal:verified-person").unwrap();
    g.permissions = vec![
        Permission::Read,
        Permission::CreateTask,
        Permission::ProposeAction,
        Permission::ExecuteAction,
        Permission::ObserveEvents,
        Permission::RequestCancellation,
        Permission::ApproveAction,
        Permission::GovernPolicy,
    ];
    r.enroll_verified_human(host, g).unwrap()
}
fn guarded(
    t: &TaskSpec,
    deltas: &[u64],
    intervention: bool,
    governance: Option<GovernanceRef>,
) -> ActionProposal {
    let mut pd = proposal(t, deltas).fields().clone();
    for a in &mut pd.actions {
        let mut ad = a.fields().clone();
        ad.authority = AuthorityRequirement::new(
            grant().principal,
            rev(3),
            scope(),
            vec![Permission::ExecuteAction],
            OversightRequirements::new(
                vec![CheckpointRef::new("checkpoint:human").unwrap()],
                true,
                intervention,
                governance.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        *a = ActionSpec::new(ad).unwrap();
    }
    ActionProposal::new(pd, t).unwrap()
}
fn approve(
    r: &mut Runtime<Services>,
    h: &HumanSession,
    t: &TaskSpec,
    p: &ActionProposal,
) -> EventId {
    let b = r
        .preview_approval(h.session(), t, p, Timestamp::new(1500).unwrap())
        .unwrap();
    r.create_approval(h, t, p, b).unwrap()
}
#[test]
fn valid_checkpoint_observation_intervention_and_governance_resolve_real_records() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:human", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let reference = GovernanceRef::new("governance:accepted").unwrap();
    let decision = r
        .register_governance(&h, &t.fields().task_id, reference.clone())
        .unwrap();
    assert_eq!(r.governance_record(&s, &reference).unwrap(), decision);
    let p = guarded(&t, &[2, 3], true, Some(reference));
    assert_eq!(
        r.admit(&s, t.clone(), p.clone()),
        Err(ErrorCode::ApprovalRequired)
    );
    let id = approve(&mut r, &h, &t, &p);
    let e = r
        .admit_with_approvals(&s, t.clone(), p.clone(), std::slice::from_ref(&id))
        .unwrap();
    r.step().unwrap();
    r.step().unwrap();
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 5);
    assert_eq!(
        r.execution(&s, &e).unwrap().fields().state,
        ExecutionState::Succeeded
    );
    let record = r.approval(&s, &id).unwrap();
    assert_eq!(
        record.verified_principal,
        PrincipalRef::new("principal:verified-person").unwrap()
    );
    assert!(record.revocation.is_none());
}
#[test]
fn every_approval_binding_field_is_checked_before_committing_a_decision() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:bindings", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[1], false, None);
    let good = r
        .preview_approval(&s, &t, &p, Timestamp::new(1500).unwrap())
        .unwrap();
    let mut bad = vec![];
    let mut b = good.clone();
    b.payload_sha256[0] ^= 1;
    bad.push(b);
    let mut b = good.clone();
    b.task_id = TaskId::new("task:wrong").unwrap();
    bad.push(b);
    let mut b = good.clone();
    b.proposal_id = ProposalId::new("proposal:wrong").unwrap();
    bad.push(b);
    let mut b = good.clone();
    b.plan_revision = rev(2);
    bad.push(b);
    let mut b = good.clone();
    b.state_revision = rev(3);
    bad.push(b);
    let mut b = good.clone();
    b.policy_revision = rev(4);
    bad.push(b);
    let mut b = good.clone();
    b.resource_scope = ResourceScope::new(vec![ResourceId::new("state:other").unwrap()]).unwrap();
    bad.push(b);
    let mut b = good.clone();
    b.checkpoints = vec![CheckpointRef::new("checkpoint:other").unwrap()];
    bad.push(b);
    for b in bad {
        assert_eq!(
            r.create_approval(&h, &t, &p, b),
            Err(ErrorCode::ApprovalMismatch)
        );
    }
    let mut expired = good;
    expired.expires_at = Timestamp::new(1000).unwrap();
    assert_eq!(
        r.create_approval(&h, &t, &p, expired),
        Err(ErrorCode::ApprovalExpired)
    );
    let snapshot = r.task(&s, &t.fields().task_id).unwrap();
    assert_eq!(snapshot.value, 0);
    assert_eq!(snapshot.charged_events, 1);
}
#[test]
fn claimed_principal_cross_instance_human_and_missing_permissions_cannot_approve() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:authority", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[1], false, None);
    let (_, other_host, _, _, _) = fixture(Limits::default());
    assert!(matches!(
        r.enroll_verified_human(&other_host, grant()),
        Err(ErrorCode::Forbidden)
    ));
    let (mut other, oh, _, _, _) = fixture(Limits::default());
    let foreign = human(&mut other, &oh);
    let b = r
        .preview_approval(&s, &t, &p, Timestamp::new(1500).unwrap())
        .unwrap();
    assert_eq!(
        r.create_approval(&foreign, &t, &p, b.clone()),
        Err(ErrorCode::Forbidden)
    );
    let ordinary = r.enroll(&host, grant()).unwrap();
    assert_eq!(
        r.admit(&ordinary, t.clone(), p.clone()),
        Err(ErrorCode::ApprovalRequired)
    );
    let limited = r.enroll_verified_human(&host, grant()).unwrap();
    assert_eq!(
        r.create_approval(&limited, &t, &p, b.clone()),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(
        r.admit_with_approvals(
            &s,
            t.clone(),
            p.clone(),
            &[EventId::new("event:claimed-human").unwrap()]
        ),
        Err(ErrorCode::ApprovalMismatch)
    );
    r.create_approval(&h, &t, &p, b).unwrap();
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
}
#[test]
fn expired_approval_at_admission_and_between_steps_preserves_partial_effects() {
    let (mut r, host, s, clock, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:expiry", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[2, 3], false, None);
    let id = approve(&mut r, &h, &t, &p);
    clock.set(1500);
    assert_eq!(
        r.admit_with_approvals(&s, t.clone(), p.clone(), std::slice::from_ref(&id)),
        Err(ErrorCode::ApprovalExpired)
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
    clock.set(1000);
    let e = r
        .admit_with_approvals(&s, t.clone(), p.clone(), std::slice::from_ref(&id))
        .unwrap();
    r.step().unwrap();
    clock.set(1500);
    r.step().unwrap();
    let result = r.execution(&s, &e).unwrap();
    assert_eq!(result.fields().state, ExecutionState::Failed);
    assert_eq!(
        result.fields().error.as_ref().unwrap().code(),
        ErrorCode::ApprovalExpired
    );
    assert_eq!(result.fields().effects.len(), 1);
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
    assert_eq!(
        r.admit_with_approvals(&s, t, p, std::slice::from_ref(&id))
            .unwrap(),
        e
    ); // replay does no new approval use/effect
}
#[test]
fn revocation_history_is_retained_and_future_use_stops_without_erasing_effects() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:revoke", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[2, 3], false, None);
    let id = approve(&mut r, &h, &t, &p);
    let before = r.approval(&s, &id).unwrap();
    let e = r
        .admit_with_approvals(&s, t.clone(), p, std::slice::from_ref(&id))
        .unwrap();
    r.step().unwrap();
    let revoked = r.revoke_approval(&h, &id).unwrap();
    assert_eq!(revoked.binding, before.binding);
    assert_eq!(revoked.decided_at, before.decided_at);
    assert!(revoked.revocation.is_some());
    assert_eq!(r.revoke_approval(&h, &id).unwrap(), revoked);
    r.step().unwrap();
    let result = r.execution(&s, &e).unwrap();
    assert_eq!(result.fields().state, ExecutionState::Failed);
    assert_eq!(
        result.fields().error.as_ref().unwrap().code(),
        ErrorCode::Forbidden
    );
    assert!(result.fields().effective_stop_evidence.is_none());
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
}
#[test]
fn changed_payload_fresh_plan_or_stale_state_cannot_reuse_approval() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:reuse", 10, 40);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[2], false, None);
    let id = approve(&mut r, &h, &t, &p);
    let changed = guarded(&t, &[3], false, None);
    assert_eq!(
        r.admit_with_approvals(&s, t.clone(), changed, std::slice::from_ref(&id)),
        Err(ErrorCode::ApprovalMismatch)
    );
    let mut pd = p.fields().clone();
    pd.plan_revision = rev(2);
    let fresh = ActionProposal::new(pd, &t).unwrap();
    assert_eq!(
        r.admit_with_approvals(&s, t.clone(), fresh, std::slice::from_ref(&id)),
        Err(ErrorCode::ApprovalMismatch)
    );
    let e = r
        .admit_with_approvals(&s, t.clone(), p, std::slice::from_ref(&id))
        .unwrap();
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &e).unwrap().fields().state,
        ExecutionState::Succeeded
    );
    let current = r.task(&s, &t.fields().task_id).unwrap().task;
    let stale = guarded(&t, &[1], false, None);
    assert_eq!(
        r.preview_approval(&s, &t, &stale, Timestamp::new(1500).unwrap()),
        Err(ErrorCode::RevisionConflict)
    );
    let mut pd = guarded(&current, &[1], false, None).fields().clone();
    pd.proposal_id = ProposalId::new("proposal:newstate").unwrap();
    pd.actions[0] = {
        let mut a = pd.actions[0].fields().clone();
        a.idempotency_key = IdempotencyKey::new("newstate:key").unwrap();
        ActionSpec::new(a).unwrap()
    };
    let new = ActionProposal::new(pd, &current).unwrap();
    assert_eq!(
        r.admit_with_approvals(&s, current, new, std::slice::from_ref(&id)),
        Err(ErrorCode::ApprovalMismatch)
    );
}
#[test]
fn policy_advance_and_delegation_revocation_deny_next_step_with_audit() {
    for policy in [false, true] {
        let (mut r, host, s, _, _) = fixture(Limits::default());
        let h = human(&mut r, &host);
        let t = task(
            if policy {
                "task:policy"
            } else {
                "task:delegation"
            },
            10,
            40,
        );
        r.create_task(&s, t.clone()).unwrap();
        let p = proposal(&t, &[2, 3]);
        let e = r.admit(&s, t.clone(), p).unwrap();
        r.step().unwrap();
        if policy {
            let record = r.advance_policy(&h, &t.fields().task_id, rev(4)).unwrap();
            assert_eq!(
                r.policy_changes(&s, &t.fields().task_id).unwrap(),
                vec![record]
            );
        } else {
            let record = r.revoke_delegation(&h, &t.fields().task_id, &s).unwrap();
            assert_eq!(
                r.revoke_delegation(&h, &t.fields().task_id, &s).unwrap(),
                record
            );
            assert_eq!(r.task(&s, &t.fields().task_id), Err(ErrorCode::Forbidden));
        }
        r.step().unwrap();
        let result = r.execution(h.session(), &e).unwrap();
        assert_eq!(result.fields().state, ExecutionState::Failed);
        assert_eq!(
            result.fields().error.as_ref().unwrap().code(),
            if policy {
                ErrorCode::RevisionConflict
            } else {
                ErrorCode::Forbidden
            }
        );
        assert_eq!(r.task(h.session(), &t.fields().task_id).unwrap().value, 2);
    }
}
#[test]
fn queued_cancel_receipt_is_distinct_from_effective_stop_and_duplicate_is_stable() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:queued-stop", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let e = r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    let receipt = r.request_cancellation(&h, &e).unwrap();
    assert_eq!(r.request_cancellation(&h, &e).unwrap(), receipt);
    let active = r.execution(&s, &e).unwrap();
    assert_eq!(active.fields().state, ExecutionState::CancelRequested);
    assert!(active.fields().effective_stop_evidence.is_none());
    assert!(active.fields().result_evidence.is_empty());
    r.step().unwrap();
    let result = r.execution(&s, &e).unwrap();
    assert_eq!(result.fields().state, ExecutionState::Cancelled);
    let evidence = result.fields().effective_stop_evidence.as_ref().unwrap();
    assert_eq!(r.observation(&s, evidence).unwrap().after, 0);
    assert!(result.fields().effects.is_empty());
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().performed_steps, 0);
    assert_eq!(r.step().unwrap(), None);
    let kinds: Vec<_> = r
        .events(&s, &t.fields().task_id, None, 256)
        .unwrap()
        .iter()
        .map(|e| e.fields().kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            EventKind::TaskCreated,
            EventKind::ExecutionAccepted,
            EventKind::CancellationRequested,
            EventKind::CancellationEffective
        ]
    );
}
#[test]
fn running_stop_retains_applied_effect_and_works_at_full_reserved_capacity() {
    let (mut r, host, s, _, _) = fixture(Limits {
        events: 9,
        ..Limits::default()
    });
    let h = human(&mut r, &host);
    let t = task("task:capacity-stop", 10, 9);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[2, 3], true, None);
    let id = approve(&mut r, &h, &t, &p);
    let e = r
        .admit_with_approvals(&s, t.clone(), p, std::slice::from_ref(&id))
        .unwrap();
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().charged_events, 9);
    r.step().unwrap();
    r.request_cancellation(&h, &e).unwrap();
    r.step().unwrap();
    let result = r.execution(&s, &e).unwrap();
    assert_eq!(result.fields().state, ExecutionState::Cancelled);
    assert_eq!(result.fields().effects.len(), 1);
    assert_eq!(
        result.fields().effects[0].fields().outcome,
        EffectOutcome::Applied
    );
    assert!(result.fields().effective_stop_evidence.is_some());
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
}
#[test]
fn late_stop_preserves_exact_success_and_partial_failed_terminal_results() {
    for fail in [false, true] {
        let (mut r, host, s, _, _) = fixture(Limits::default());
        let h = human(&mut r, &host);
        let t = task(
            if fail {
                "task:late-failed"
            } else {
                "task:late-success"
            },
            10,
            30,
        );
        r.create_task(&s, t.clone()).unwrap();
        let e = r
            .admit(
                &s,
                t.clone(),
                proposal(&t, if fail { &[u64::MAX, 1] } else { &[2, 3] }),
            )
            .unwrap();
        r.step().unwrap();
        r.step().unwrap();
        let before = r.execution(&s, &e).unwrap();
        let events = r.events(&s, &t.fields().task_id, None, 256).unwrap();
        assert_eq!(
            r.request_cancellation(&h, &e).unwrap(),
            CancellationOutcome::AlreadyTerminal(Box::new(before.clone()))
        );
        assert_eq!(r.execution(&s, &e).unwrap(), before);
        assert_eq!(
            r.events(&s, &t.fields().task_id, None, 256).unwrap(),
            events
        );
    }
}
#[test]
fn cancellation_clock_failure_and_denied_or_foreign_human_do_not_change_state() {
    let (mut r, host, s, _, fail) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let limited = r.enroll_verified_human(&host, grant()).unwrap();
    let t = task("task:denied-stop", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let e = r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    let before = r.execution(&s, &e).unwrap();
    assert_eq!(
        r.request_cancellation(&limited, &e),
        Err(ErrorCode::Forbidden)
    );
    let (mut foreign, fh, _, _, _) = fixture(Limits::default());
    let outsider = human(&mut foreign, &fh);
    assert_eq!(
        r.request_cancellation(&outsider, &e),
        Err(ErrorCode::Forbidden)
    );
    fail.set(true);
    assert_eq!(
        r.request_cancellation(&h, &e),
        Err(ErrorCode::CapabilityUnavailable)
    );
    fail.set(false);
    assert_eq!(r.execution(&s, &e).unwrap(), before);
    r.request_cancellation(&h, &e).unwrap();
    fail.set(true);
    assert_eq!(r.step(), Err(ErrorCode::CapabilityUnavailable));
    fail.set(false);
    assert_eq!(
        r.execution(&s, &e).unwrap().fields().state,
        ExecutionState::CancelRequested
    );
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &e).unwrap().fields().state,
        ExecutionState::Cancelled
    );
}
#[test]
fn cancellation_stays_effective_after_revocation_or_policy_change() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:stop-revoked", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let e = r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    r.step().unwrap();
    r.request_cancellation(&h, &e).unwrap();
    r.revoke_delegation(&h, &t.fields().task_id, &s).unwrap();
    r.advance_policy(&h, &t.fields().task_id, rev(4)).unwrap();
    r.step().unwrap();
    assert_eq!(
        r.execution(h.session(), &e).unwrap().fields().state,
        ExecutionState::Cancelled
    );
    assert_eq!(r.task(h.session(), &t.fields().task_id).unwrap().value, 2);
}
#[test]
fn accepted_task_checkpoints_cannot_be_omitted_by_a_request_or_policy_revision() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let h = human(&mut r, &host);
    let t = task("task:protected", 10, 40);
    let required = OversightRequirements::new(
        vec![CheckpointRef::new("checkpoint:accepted-policy").unwrap()],
        true,
        true,
        None,
    )
    .unwrap();
    r.create_governed_task(&host, &s, t.clone(), required.clone())
        .unwrap();
    let p = proposal(&t, &[2, 3]);
    assert_eq!(
        r.admit(&s, t.clone(), p.clone()),
        Err(ErrorCode::ApprovalRequired)
    );
    assert_eq!(
        r.accepted_oversight(&s, &t.fields().task_id).unwrap(),
        required
    );
    let id = approve(&mut r, &h, &t, &p);
    let e = r
        .admit_with_approvals(&s, t.clone(), p, std::slice::from_ref(&id))
        .unwrap();
    r.step().unwrap();
    r.advance_policy(&h, &t.fields().task_id, rev(4)).unwrap();
    assert_eq!(
        r.accepted_oversight(&s, &t.fields().task_id).unwrap(),
        required
    );
    r.step().unwrap();
    assert_eq!(
        r.execution(&s, &e).unwrap().fields().state,
        ExecutionState::Failed
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 2);
}
#[test]
fn decision_capacity_clock_failure_and_unknown_governance_commit_no_effects() {
    let (mut r, host, s, _, fail) = fixture(Limits {
        executions: 1,
        ..Limits::default()
    });
    let h = human(&mut r, &host);
    let t = task("task:audit-bounds", 10, 40);
    r.create_task(&s, t.clone()).unwrap();
    let p = guarded(&t, &[1], false, None);
    let b = r
        .preview_approval(&s, &t, &p, Timestamp::new(1500).unwrap())
        .unwrap();
    fail.set(true);
    assert_eq!(
        r.create_approval(&h, &t, &p, b.clone()),
        Err(ErrorCode::CapabilityUnavailable)
    );
    fail.set(false);
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().charged_events, 1);
    let id = r.create_approval(&h, &t, &p, b.clone()).unwrap();
    assert_eq!(
        r.create_approval(&h, &t, &p, b),
        Err(ErrorCode::BudgetExceeded)
    );
    fail.set(true);
    assert_eq!(
        r.revoke_approval(&h, &id),
        Err(ErrorCode::CapabilityUnavailable)
    );
    fail.set(false);
    assert!(r.approval(&s, &id).unwrap().revocation.is_none());
    let unknown = guarded(
        &t,
        &[1],
        false,
        Some(GovernanceRef::new("governance:claimed").unwrap()),
    );
    assert_eq!(
        r.admit(&s, t.clone(), unknown),
        Err(ErrorCode::CapabilityUnavailable)
    );
    assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
}
#[test]
fn no_cancellation_capability_is_claimed_for_admission_without_reserved_controls() {
    let (mut r, host, s, _, _) = fixture(Limits::default());
    let t = task("task:no-controller", 10, 30);
    r.create_task(&s, t.clone()).unwrap();
    let e = r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    let h = human(&mut r, &host);
    assert_eq!(
        r.request_cancellation(&h, &e),
        Err(ErrorCode::CapabilityUnavailable)
    );
    assert_eq!(
        r.execution(&s, &e).unwrap().fields().state,
        ExecutionState::Queued
    );
}
#[test]
fn required_intervention_on_multi_resource_task_needs_a_full_task_controller() {
    for accepted in [false, true] {
        let (mut r, host, _, _, _) = fixture(Limits::default());
        let broad = ResourceScope::new(vec![
            ResourceId::new("state:1").unwrap(),
            ResourceId::new("state:2").unwrap(),
        ])
        .unwrap();
        let mut eg = grant();
        eg.resource_scope = broad.clone();
        let s = r.enroll(&host, eg).unwrap();
        let mut hg = grant();
        hg.principal = PrincipalRef::new("principal:narrow-controller").unwrap();
        hg.permissions.push(Permission::RequestCancellation);
        let narrow = r.enroll_verified_human(&host, hg.clone()).unwrap();
        let mut td = task(
            if accepted {
                "task:accepted-wide"
            } else {
                "task:declared-wide"
            },
            10,
            40,
        )
        .fields()
        .clone();
        td.resource_scope = broad.clone();
        let t = TaskSpec::new(td).unwrap();
        r.create_governed_task(
            &host,
            &s,
            t.clone(),
            OversightRequirements::new(vec![], true, accepted, None).unwrap(),
        )
        .unwrap();
        let mut pd = proposal(&t, &[2, 3]).fields().clone();
        if !accepted {
            for action in &mut pd.actions {
                let mut ad = action.fields().clone();
                ad.authority = AuthorityRequirement::new(
                    grant().principal,
                    rev(3),
                    scope(),
                    vec![Permission::ExecuteAction],
                    OversightRequirements::new(vec![], true, true, None).unwrap(),
                )
                .unwrap();
                *action = ActionSpec::new(ad).unwrap();
            }
        }
        let p = ActionProposal::new(pd, &t).unwrap();
        assert_eq!(
            r.admit(&s, t.clone(), p.clone()),
            Err(ErrorCode::CapabilityUnavailable)
        );
        assert_eq!(r.task(&s, &t.fields().task_id).unwrap().charged_events, 1);
        assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
        assert_eq!(r.step().unwrap(), None);
        hg.resource_scope = broad;
        let controller = r.enroll_verified_human(&host, hg).unwrap();
        let e = r.admit(&s, t.clone(), p).unwrap();
        assert_eq!(
            r.request_cancellation(&narrow, &e),
            Err(ErrorCode::Forbidden)
        );
        r.request_cancellation(&controller, &e).unwrap();
        r.step().unwrap();
        assert_eq!(
            r.execution(&s, &e).unwrap().fields().state,
            ExecutionState::Cancelled
        );
        assert_eq!(r.task(&s, &t.fields().task_id).unwrap().value, 0);
    }
}
#[test]
fn required_observation_cannot_be_satisfied_by_action_subset_stream_permission() {
    for accepted in [false, true] {
        let (mut r, host, _, _, _) = fixture(Limits::default());
        let broad = ResourceScope::new(vec![
            ResourceId::new("state:1").unwrap(),
            ResourceId::new("state:2").unwrap(),
        ])
        .unwrap();
        let mut eg = grant();
        eg.resource_scope = broad.clone();
        let owner = r.enroll(&host, eg).unwrap();
        let narrow = r.enroll(&host, grant()).unwrap();
        let mut td = task(
            if accepted {
                "task:accepted-observe"
            } else {
                "task:declared-observe"
            },
            10,
            30,
        )
        .fields()
        .clone();
        td.resource_scope = broad;
        let t = TaskSpec::new(td).unwrap();
        r.create_governed_task(
            &host,
            &owner,
            t.clone(),
            OversightRequirements::new(vec![], accepted, false, None).unwrap(),
        )
        .unwrap();
        let mut pd = proposal(&t, &[2]).fields().clone();
        if accepted {
            let mut ad = pd.actions[0].fields().clone();
            ad.authority = AuthorityRequirement::new(
                grant().principal,
                rev(3),
                scope(),
                vec![Permission::ExecuteAction],
                OversightRequirements::new(vec![], false, false, None).unwrap(),
            )
            .unwrap();
            pd.actions[0] = ActionSpec::new(ad).unwrap();
        }
        let p = ActionProposal::new(pd, &t).unwrap();
        assert_eq!(r.admit(&narrow, t.clone(), p), Err(ErrorCode::Forbidden));
        assert_eq!(r.task(&owner, &t.fields().task_id).unwrap().value, 0);
        assert_eq!(
            r.task(&owner, &t.fields().task_id).unwrap().charged_events,
            1
        );
    }
}
