mod support;
use eylema_contracts::*;
use eylema_runtime::*;
use support::*;
#[test]
fn scheduler_flag_is_host_bound_read_only_and_tracks_actual_queue() {
    let (mut r, h, s, _, fail) = fixture(Limits::default());
    let (_, foreign, _, _, _) = fixture(Limits::default());
    assert_eq!(r.has_pending_work(&foreign), Err(ErrorCode::Forbidden));
    assert_eq!(r.has_pending_work(&h), Ok(false));
    let t = task("task:scheduler", 20, 40);
    r.create_task(&s, t.clone()).unwrap();
    r.admit(&s, t.clone(), proposal(&t, &[2, 3])).unwrap();
    let digest = r.recovery_digest(&h).unwrap();
    fail.set(true);
    assert_eq!(r.has_pending_work(&h), Ok(true));
    assert_eq!(r.recovery_digest(&h).unwrap(), digest);
    fail.set(false);
    r.step().unwrap();
    assert_eq!(r.has_pending_work(&h), Ok(true));
    r.step().unwrap();
    assert_eq!(r.has_pending_work(&h), Ok(false));
}
#[test]
fn recovery_retains_confirmed_effects_idempotency_and_terminal_results_with_new_enrollment() {
    let (mut r, h, s, _, _) = fixture(Limits::default());
    let t = task("task:recovery", 20, 40);
    r.create_task(&s, t.clone()).unwrap();
    let p = proposal(&t, &[2, 3]);
    let id = r.admit(&s, t.clone(), p.clone()).unwrap();
    r.step().unwrap();
    let before = r.task(&s, &t.fields().task_id).unwrap();
    let first = r.execution(&s, &id).unwrap().fields().effects[0].clone();
    let digest = r.recovery_digest(&h).unwrap();
    assert_eq!(r.recovery_digest(&h).unwrap(), digest);
    assert_eq!(r.recover_unfinished(&h, true).unwrap(), vec![id.clone()]);
    assert_eq!(r.task(&s, &t.fields().task_id), Err(ErrorCode::Forbidden));
    let fresh = r.enroll(&h, grant()).unwrap();
    let after = r.task(&fresh, &t.fields().task_id).unwrap();
    assert_eq!(after, before);
    let unknown = r.execution(&fresh, &id).unwrap();
    assert_eq!(unknown.fields().state, ExecutionState::OutcomeUnknown);
    assert_eq!(unknown.fields().effects[0], first);
    assert_eq!(
        unknown.fields().effects[1].fields().outcome,
        EffectOutcome::Unknown
    );
    assert!(unknown.fields().effects[1].fields().evidence.is_empty());
    assert!(unknown.fields().effective_stop_evidence.is_none());
    assert_eq!(r.admit(&fresh, t, p).unwrap(), id);
    assert_eq!(r.step().unwrap(), None);
    for e in &unknown.fields().result_evidence {
        assert_eq!(r.observation(&fresh, e).unwrap().after, 2);
    }
    r.recover_unfinished(&h, false).unwrap();
    let fresh = r.enroll(&h, grant()).unwrap();
    assert_eq!(r.execution(&fresh, &id).unwrap(), unknown);
}
#[test]
fn recovery_clock_or_foreign_host_failure_mutates_nothing_and_success_keeps_completed_records() {
    let (mut r, h, s, _, fail) = fixture(Limits::default());
    let t = task("task:complete", 20, 40);
    r.create_task(&s, t.clone()).unwrap();
    let p = proposal(&t, &[2, 3]);
    let id = r.admit(&s, t.clone(), p).unwrap();
    r.step().unwrap();
    r.step().unwrap();
    let terminal = r.execution(&s, &id).unwrap();
    let events = r.events(&s, &t.fields().task_id, None, 256).unwrap();
    let digest = r.recovery_digest(&h).unwrap();
    fail.set(true);
    assert_eq!(
        r.recover_unfinished(&h, false),
        Err(ErrorCode::CapabilityUnavailable)
    );
    assert_eq!(r.recovery_digest(&h).unwrap(), digest);
    assert_eq!(r.execution(&s, &id).unwrap(), terminal);
    fail.set(false);
    let (_, foreign, _, _, _) = fixture(Limits::default());
    assert_eq!(
        r.recover_unfinished(&foreign, true),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(r.recovery_digest(&foreign), Err(ErrorCode::Forbidden));
    assert!(r.recover_unfinished(&h, false).unwrap().is_empty());
    let fresh = r.enroll(&h, grant()).unwrap();
    assert_eq!(r.execution(&fresh, &id).unwrap(), terminal);
    assert_eq!(
        r.events(&fresh, &t.fields().task_id, None, 256).unwrap(),
        events
    );
}
