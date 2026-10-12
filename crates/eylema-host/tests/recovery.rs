#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod support;
use eylema_contracts::{codec::Value, *};
use eylema_host::*;
use eylema_journal::{Journal, Limits as JournalLimits, Sha256, Storage};
use eylema_runtime::{CancellationOutcome, Grant, Limits};
use std::cell::{Cell, RefCell};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::rc::Rc;
#[derive(Clone)]
struct Memory {
    live: Rc<RefCell<Cursor<Vec<u8>>>>,
    stable: Rc<RefCell<Vec<u8>>>,
    syncs: Rc<Cell<usize>>,
    fail_at: Rc<Cell<Option<usize>>>,
}
impl Memory {
    fn new() -> Self {
        Self {
            live: Rc::new(RefCell::new(Cursor::new(vec![]))),
            stable: Rc::new(RefCell::new(vec![])),
            syncs: Rc::new(Cell::new(0)),
            fail_at: Rc::new(Cell::new(None)),
        }
    }
    fn crash(&self) {
        *self.live.borrow_mut() = Cursor::new(self.stable.borrow().clone());
        self.fail_at.set(None)
    }
    fn data(&self) -> Vec<u8> {
        self.live.borrow().get_ref().clone()
    }
}
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.live.borrow_mut().read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.live.borrow_mut().write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for Memory {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.live.borrow_mut().seek(p)
    }
}
impl Storage for Memory {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.live.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        self.syncs.set(self.syncs.get() + 1);
        if self.fail_at.get() == Some(self.syncs.get()) {
            return Err(io::ErrorKind::Other.into());
        }
        *self.stable.borrow_mut() = self.data();
        Ok(())
    }
}
struct Hash;
impl Sha256 for Hash {
    fn digest(&mut self, b: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        eylema_platform::sha256(b).map_err(|_| eylema_journal::JournalError::HashUnavailable)
    }
}
fn services(next: u64) -> support::Services {
    support::Services {
        clock: Rc::new(Cell::new(1000)),
        next,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    }
}
type Host = DurableHost<Memory, Hash, support::Services>;
fn fixture() -> (Host, Owner, DurableSession, Memory) {
    let memory = Memory::new();
    let journal = Journal::create_new(memory.clone(), Hash, JournalLimits::default()).unwrap();
    let (mut host, owner) = Host::create_new(journal, services(0), Limits::default()).unwrap();
    let session = host.enroll(&owner, support::grant()).unwrap();
    (host, owner, session, memory)
}
fn reopen(memory: &Memory) -> (Host, Owner, DurableSession) {
    let journal = Journal::open(memory.clone(), Hash, JournalLimits::default()).unwrap();
    let (mut host, owner) = Host::open(journal, services(10000)).unwrap();
    let session = host.enroll(&owner, support::grant()).unwrap();
    (host, owner, session)
}
fn human_grant() -> Grant {
    let mut g = support::grant();
    g.principal = PrincipalRef::new("principal:verified-fixture").unwrap();
    g.permissions.extend([
        Permission::RequestCancellation,
        Permission::ApproveAction,
        Permission::GovernPolicy,
    ]);
    g
}
fn task_and_run(
    host: &mut Host,
    s: &DurableSession,
    name: &str,
) -> (TaskSpec, ActionProposal, ExecutionId) {
    let t = support::task(name, 20, 50);
    host.create_task(s, t.clone()).unwrap();
    let p = support::proposal(&t, &[2, 3]);
    let id = host.admit(s, t.clone(), p.clone()).unwrap();
    (t, p, id)
}
#[test]
fn completed_identities_results_revisions_events_observations_and_replay_survive_multiple_restarts()
{
    let (mut h, _, s, m) = fixture();
    let (t, p, id) = task_and_run(&mut h, &s, "task:complete");
    h.step().unwrap();
    h.step().unwrap();
    let task = h.task(&s, &t.fields().task_id).unwrap();
    let result = h.execution(&s, &id).unwrap();
    let events = h.events(&s, &t.fields().task_id, None, 256).unwrap();
    let observations: Vec<_> = events
        .iter()
        .flat_map(|e| &e.fields().evidence)
        .map(|e| h.observation(&s, e).unwrap())
        .collect();
    h.close();
    for _ in 0..3 {
        let (mut h, _, fresh) = reopen(&m);
        assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap(), task);
        assert_eq!(h.execution(&fresh, &id).unwrap(), result);
        assert_eq!(
            h.events(&fresh, &t.fields().task_id, None, 256).unwrap(),
            events
        );
        for o in &observations {
            assert_eq!(h.observation(&fresh, &o.evidence).unwrap(), *o)
        }
        assert_eq!(h.admit(&fresh, t.clone(), p.clone()).unwrap(), id);
        assert_eq!(h.step().unwrap(), None);
        assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 5);
        assert!(h.recovery_report().unwrap().unfinished.is_empty());
        h.close();
    }
}
#[test]
fn partial_unfinished_execution_retains_confirmed_value_and_becomes_unknown_without_auto_step() {
    let (mut h, _, s, m) = fixture();
    let (t, p, id) = task_and_run(&mut h, &s, "task:partial");
    h.step().unwrap();
    let first = h.execution(&s, &id).unwrap().fields().effects[0].clone();
    let count = h.events(&s, &t.fields().task_id, None, 256).unwrap().len();
    h.close();
    let (mut h, _, fresh) = reopen(&m);
    let e = h.execution(&fresh, &id).unwrap();
    assert_eq!(e.fields().state, ExecutionState::OutcomeUnknown);
    assert_eq!(e.fields().effects, vec![first]);
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 2);
    assert_eq!(h.recovery_report().unwrap().unfinished, vec![id.clone()]);
    assert_eq!(
        h.events(&fresh, &t.fields().task_id, None, 256)
            .unwrap()
            .len(),
        count + 1
    );
    assert_eq!(h.admit(&fresh, t, p).unwrap(), id);
    assert_eq!(h.step().unwrap(), None);
    h.close();
    let (h, _, fresh) = reopen(&m);
    assert_eq!(h.execution(&fresh, &id).unwrap(), e);
}
#[test]
fn commit_sync_failure_returns_no_success_and_lost_bytes_leave_explicit_unknown_next_effect() {
    let (mut h, _, s, m) = fixture();
    let (t, p, id) = task_and_run(&mut h, &s, "task:uncertain");
    h.step().unwrap();
    m.fail_at.set(Some(m.syncs.get() + 2));
    assert!(matches!(
        h.step(),
        Err(HostError::Journal(
            eylema_journal::JournalError::WriteUncertain { .. }
        ))
    ));
    assert_eq!(h.task(&s, &t.fields().task_id), Err(HostError::Poisoned));
    assert_eq!(h.step(), Err(HostError::Poisoned));
    h.close();
    m.crash();
    let (mut h, _, fresh) = reopen(&m);
    let e = h.execution(&fresh, &id).unwrap();
    assert_eq!(e.fields().state, ExecutionState::OutcomeUnknown);
    assert_eq!(
        e.fields().effects[0].fields().outcome,
        EffectOutcome::Applied
    );
    assert_eq!(
        e.fields().effects[1].fields().outcome,
        EffectOutcome::Unknown
    );
    assert!(e.fields().effects[1].fields().evidence.is_empty());
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 2);
    assert!(h.recovery_report().unwrap().abandoned_attempt.is_some());
    assert_eq!(h.admit(&fresh, t, p).unwrap(), id);
    assert_eq!(h.step().unwrap(), None);
}
#[test]
fn complete_commit_bytes_after_failed_sync_restore_same_result_without_retroactive_receipt() {
    let (mut h, _, s, m) = fixture();
    let (t, p, id) = task_and_run(&mut h, &s, "task:complete-unacked");
    h.step().unwrap();
    m.fail_at.set(Some(m.syncs.get() + 2));
    assert!(h.step().is_err());
    h.close();
    m.fail_at.set(None);
    let (mut h, _, fresh) = reopen(&m);
    assert_eq!(
        h.execution(&fresh, &id).unwrap().fields().state,
        ExecutionState::Succeeded
    );
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 5);
    assert_eq!(h.admit(&fresh, t, p).unwrap(), id);
    assert_eq!(h.step().unwrap(), None);
    assert_eq!(h.recovery_report().unwrap().abandoned_attempt, None);
}
#[test]
fn intent_sync_failure_has_no_effect_and_existing_prior_state_is_preserved() {
    let (mut h, _, s, m) = fixture();
    let (t, _, id) = task_and_run(&mut h, &s, "task:intent-failed");
    h.step().unwrap();
    m.fail_at.set(Some(m.syncs.get() + 1));
    assert!(h.step().is_err());
    h.close();
    m.crash();
    let (h, _, fresh) = reopen(&m);
    let e = h.execution(&fresh, &id).unwrap();
    assert_eq!(e.fields().effects.len(), 1);
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 2);
    assert_eq!(h.recovery_report().unwrap().abandoned_attempt, None);
}
#[test]
fn foreign_owner_and_old_epoch_handles_never_restore_current_authority() {
    let (mut h, owner, s, m) = fixture();
    let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
    let (other, foreign, _, _) = fixture();
    let before = m.data();
    assert!(matches!(
        h.enroll(&foreign, support::grant()),
        Err(HostError::Runtime(ErrorCode::Forbidden))
    ));
    assert_eq!(m.data(), before);
    other.close();
    let t = support::task("task:authority", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    h.close();
    let (mut h, _, fresh) = reopen(&m);
    assert_eq!(
        h.task(&s, &t.fields().task_id),
        Err(HostError::Runtime(ErrorCode::Forbidden))
    );
    assert!(matches!(
        h.enroll(&owner, support::grant()),
        Err(HostError::Runtime(ErrorCode::Forbidden))
    ));
    let p = support::proposal(&t, &[2, 3]);
    let id = h.admit(&fresh, t, p).unwrap();
    assert!(matches!(
        h.request_cancellation(&human, &id),
        Err(HostError::Runtime(ErrorCode::Forbidden))
    ));
}
#[test]
fn approval_revocation_audit_oversight_policy_and_governance_remain_history_not_live_credentials() {
    let (mut h, owner, s, m) = fixture();
    let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
    let t = support::task("task:oversight", 20, 80);
    let o = OversightRequirements::new(
        vec![CheckpointRef::new("checkpoint:1").unwrap()],
        true,
        true,
        None,
    )
    .unwrap();
    h.create_governed_task(&owner, &s, t.clone(), o.clone())
        .unwrap();
    let p = support::proposal(&t, &[2, 3]);
    let b = h
        .preview_approval(human.session(), &t, &p, Timestamp::new(1800).unwrap())
        .unwrap();
    let aid = h.create_approval(&human, &t, &p, b).unwrap();
    let a = h.revoke_approval(&human, &aid).unwrap();
    let g = GovernanceRef::new("governance:1").unwrap();
    let gr = h
        .register_governance(&human, &t.fields().task_id, g.clone())
        .unwrap();
    let change = h
        .advance_policy(&human, &t.fields().task_id, Revision::new(4).unwrap())
        .unwrap();
    h.close();
    let (mut h, owner, fresh) = reopen(&m);
    let stored = h.approval(&fresh, &aid).unwrap();
    assert_eq!(stored.binding, a.binding);
    assert_eq!(stored.verified_principal, a.verified_principal);
    assert_eq!(stored.decided_at, a.decided_at);
    assert_eq!(stored.revocation, a.revocation);
    assert_eq!(
        h.accepted_oversight(&fresh, &t.fields().task_id).unwrap(),
        o
    );
    assert_eq!(h.governance_record(&fresh, &g).unwrap(), gr);
    assert_eq!(
        h.policy_changes(&fresh, &t.fields().task_id).unwrap(),
        vec![change]
    );
    assert_eq!(
        h.admit_with_approvals(&fresh, t.clone(), p.clone(), std::slice::from_ref(&aid)),
        Err(HostError::Runtime(ErrorCode::RevisionConflict))
    );
    let mut grant = human_grant();
    grant.policy_revision = Revision::new(4).unwrap();
    let newhuman = h.enroll_verified_human(&owner, grant).unwrap();
    assert_eq!(
        h.admit_with_approvals(&fresh, t, p, &[aid]),
        Err(HostError::Runtime(ErrorCode::RevisionConflict))
    );
    assert_eq!(
        h.revoke_approval(&newhuman, &a.approval_id)
            .unwrap()
            .revocation,
        a.revocation
    );
}
#[test]
fn effective_and_late_cancellation_results_survive_and_pending_receipt_does_not_claim_stop() {
    for finish in [false, true] {
        let (mut h, owner, s, m) = fixture();
        let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
        let (t, _, id) = task_and_run(
            &mut h,
            &s,
            if finish {
                "task:cancelled"
            } else {
                "task:pendingcancel"
            },
        );
        h.step().unwrap();
        let receipt = h.request_cancellation(&human, &id).unwrap();
        assert!(matches!(receipt, CancellationOutcome::Requested(_)));
        if finish {
            h.step().unwrap();
        }
        let before = h.execution(&s, &id).unwrap();
        h.close();
        let (mut h, owner, fresh) = reopen(&m);
        let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
        let after = h.execution(&fresh, &id).unwrap();
        if finish {
            assert_eq!(after, before)
        } else {
            assert_eq!(after.fields().state, ExecutionState::OutcomeUnknown);
            assert!(after.fields().effective_stop_evidence.is_none());
        }
        assert_eq!(
            h.request_cancellation(&human, &id).unwrap(),
            CancellationOutcome::AlreadyTerminal(Box::new(after))
        );
        assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 2);
    }
}
#[test]
fn changed_payload_conflicts_and_repeated_delivery_are_retained_exactly() {
    let (mut h, _, s, m) = fixture();
    let (t, p, id) = task_and_run(&mut h, &s, "task:conflict");
    h.step().unwrap();
    h.step().unwrap();
    let bad = support::proposal(&t, &[7, 3]);
    assert_eq!(
        h.admit(&s, t.clone(), bad.clone()),
        Err(HostError::Runtime(ErrorCode::IdempotencyConflict))
    );
    h.close();
    let (mut h, _, fresh) = reopen(&m);
    assert_eq!(
        h.admit(&fresh, t.clone(), bad),
        Err(HostError::Runtime(ErrorCode::IdempotencyConflict))
    );
    assert_eq!(h.admit(&fresh, t, p).unwrap(), id);
}
#[test]
fn semantic_bad_version_gap_transcript_result_and_digest_fail_closed_without_writes() {
    for kind in 0..5 {
        let (h, _, _, m) = fixture();
        h.close();
        let mut journal = Journal::open(m.clone(), Hash, JournalLimits::default()).unwrap();
        let records = journal.read_records().unwrap();
        let last_attempt = records
            .iter()
            .filter_map(|r| match &r.payload {
                Value::List(v) if matches!(v.first(),Some(Value::Text(t)) if t=="intent") => {
                    if let Value::Unsigned(n) = v[2] {
                        Some(n)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .max()
            .unwrap();
        let attempt = last_attempt + 1;
        let intent = Value::List(vec![
            Value::Text("intent".into()),
            Value::Unsigned(if kind == 0 { 9 } else { 1 }),
            Value::Unsigned(if kind == 1 { attempt + 1 } else { attempt }),
            Value::List(vec![Value::Unsigned(3)]),
        ]);
        journal.append(&intent).unwrap();
        if kind >= 2 {
            let (transcript, result, digest) = match kind {
                2 => (
                    Value::List(vec![Value::List(vec![
                        Value::Unsigned(0),
                        Value::List(vec![Value::Text("ok".into()), Value::Unsigned(1000)]),
                    ])]),
                    Value::List(vec![Value::Text("ok".into()), Value::Null]),
                    vec![0; 32],
                ),
                3 => (
                    Value::List(vec![]),
                    Value::List(vec![Value::Text("ok".into()), Value::Unsigned(99)]),
                    vec![0; 32],
                ),
                _ => (
                    Value::List(vec![]),
                    Value::List(vec![Value::Text("ok".into()), Value::Null]),
                    vec![0; 32],
                ),
            };
            journal
                .append(&Value::List(vec![
                    Value::Text("commit".into()),
                    Value::Unsigned(1),
                    Value::Unsigned(attempt),
                    result,
                    transcript,
                    Value::Bytes(digest),
                ]))
                .unwrap();
        }
        journal.close();
        let before = m.data();
        let journal = Journal::open(m.clone(), Hash, JournalLimits::default()).unwrap();
        assert!(matches!(
            Host::open(journal, services(10000)),
            Err(HostError::IncompatibleFormat
                | HostError::InvalidRecord
                | HostError::ReplayMismatch)
        ));
        assert_eq!(m.data(), before);
    }
}
#[test]
fn finite_transaction_capacity_rejects_before_intent_and_leaves_a_recovery_slot() {
    let m = Memory::new();
    let limits = JournalLimits {
        records: 4,
        ..JournalLimits::default()
    };
    let journal = Journal::create_new(m.clone(), Hash, limits).unwrap();
    let (mut h, owner) = Host::create_new(journal, services(0), Limits::default()).unwrap();
    let s = h.enroll(&owner, support::grant()).unwrap();
    let before = m.data();
    let t = support::task("task:budget", 20, 40);
    assert_eq!(
        h.create_task(&s, t.clone()),
        Err(HostError::Journal(
            eylema_journal::JournalError::BudgetExceeded
        ))
    );
    assert_eq!(m.data(), before);
    assert_eq!(
        h.task(&s, &t.fields().task_id),
        Err(HostError::Runtime(ErrorCode::InvalidRequest))
    );
    h.close();
    let journal = Journal::open(m.clone(), Hash, limits).unwrap();
    let (mut h, owner) = Host::open(journal, services(10000)).unwrap();
    assert!(h.recovery_report().unwrap().unfinished.is_empty());
    assert!(matches!(
        h.enroll(&owner, support::grant()),
        Err(HostError::Journal(
            eylema_journal::JournalError::BudgetExceeded
        ))
    ));
}
#[test]
fn historical_clock_failure_replays_and_fresh_recovery_clock_failure_writes_nothing() {
    let m = Memory::new();
    let service = services(0);
    let fail = service.fail_clock.clone();
    let journal = Journal::create_new(m.clone(), Hash, JournalLimits::default()).unwrap();
    let (mut h, owner) = Host::create_new(journal, service, Limits::default()).unwrap();
    let s = h.enroll(&owner, support::grant()).unwrap();
    let (t, _, id) = task_and_run(&mut h, &s, "task:clock");
    fail.set(true);
    assert_eq!(
        h.step(),
        Err(HostError::Runtime(ErrorCode::CapabilityUnavailable))
    );
    assert_eq!(
        h.execution(&s, &id).unwrap().fields().state,
        ExecutionState::Queued
    );
    h.close();
    let before = m.data();
    let service = services(10000);
    service.fail_clock.set(true);
    let journal = Journal::open(m.clone(), Hash, JournalLimits::default()).unwrap();
    assert!(matches!(
        Host::open(journal, service),
        Err(HostError::Runtime(ErrorCode::CapabilityUnavailable))
    ));
    assert_eq!(m.data(), before);
    let (h, _, fresh) = reopen(&m);
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 0);
    assert_eq!(
        h.execution(&fresh, &id).unwrap().fields().state,
        ExecutionState::OutcomeUnknown
    );
}
#[test]
fn revoked_delegation_cannot_act_before_restart_and_its_partial_failure_survives() {
    let (mut h, owner, s, m) = fixture();
    let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
    let (t, _, id) = task_and_run(&mut h, &s, "task:delegate");
    h.step().unwrap();
    let revoked = h
        .revoke_delegation(&human, &t.fields().task_id, &s)
        .unwrap();
    assert_eq!(revoked.verified_principal, human_grant().principal);
    h.step().unwrap();
    assert_eq!(
        h.task(&s, &t.fields().task_id),
        Err(HostError::Runtime(ErrorCode::Forbidden))
    );
    let before = h.execution(human.session(), &id).unwrap();
    assert_eq!(before.fields().state, ExecutionState::Failed);
    h.close();
    let (h, _, fresh) = reopen(&m);
    assert_eq!(h.execution(&fresh, &id).unwrap(), before);
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap().value, 2);
}
#[test]
fn authorized_decision_returns_are_committed_with_approve_govern_permissions_without_observe_grant()
{
    let (mut h, owner, s, m) = fixture();
    let mut g = human_grant();
    g.permissions = vec![Permission::ApproveAction, Permission::GovernPolicy];
    let human = h.enroll_verified_human(&owner, g).unwrap();
    let t = support::task("task:narrow-human", 20, 80);
    let o = OversightRequirements::new(
        vec![CheckpointRef::new("checkpoint:1").unwrap()],
        false,
        false,
        None,
    )
    .unwrap();
    h.create_governed_task(&owner, &s, t.clone(), o).unwrap();
    let p = support::proposal(&t, &[2, 3]);
    let b = h
        .preview_approval(&s, &t, &p, Timestamp::new(1800).unwrap())
        .unwrap();
    let aid = h.create_approval(&human, &t, &p, b).unwrap();
    assert_eq!(
        h.approval(human.session(), &aid),
        Err(HostError::Runtime(ErrorCode::Forbidden))
    );
    let revoked = h.revoke_approval(&human, &aid).unwrap();
    assert!(revoked.revocation.is_some());
    let reference = GovernanceRef::new("governance:narrow").unwrap();
    let governance = h
        .register_governance(&human, &t.fields().task_id, reference.clone())
        .unwrap();
    let policy = h
        .advance_policy(&human, &t.fields().task_id, Revision::new(4).unwrap())
        .unwrap();
    h.close();
    let (h, _, fresh) = reopen(&m);
    assert_eq!(
        h.approval(&fresh, &aid).unwrap().revocation,
        revoked.revocation
    );
    assert_eq!(h.governance_record(&fresh, &reference).unwrap(), governance);
    assert_eq!(
        h.policy_changes(&fresh, &t.fields().task_id).unwrap(),
        vec![policy]
    );
}

#[test]
fn incompatible_command_collections_reject_before_intent_and_preserve_confirmed_replay() {
    for kind in 0..4 {
        let (mut h, owner, s, m) = fixture();
        let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
        let (t, p, id) = task_and_run(&mut h, &s, "task:command-bounds");
        let mut binding = h
            .preview_approval(&s, &t, &p, Timestamp::new(1800).unwrap())
            .unwrap();
        h.step().unwrap();
        h.step().unwrap();
        let task = h.task(&s, &t.fields().task_id).unwrap();
        let result = h.execution(&s, &id).unwrap();
        let events = h.events(&s, &t.fields().task_id, None, 256).unwrap();
        let bytes = m.data();
        let syncs = m.syncs.get();
        let rejected = match kind {
            0 | 1 => {
                let mut grant = human_grant();
                grant.permissions = vec![Permission::Read; 9];
                if kind == 0 {
                    h.enroll(&owner, grant).map(|_| ())
                } else {
                    h.enroll_verified_human(&owner, grant).map(|_| ())
                }
            }
            2 => {
                let ids = vec![EventId::new("event:oversized").unwrap(); MAX_REFERENCES + 1];
                h.admit_with_approvals(&s, t.clone(), p.clone(), &ids)
                    .map(|_| ())
            }
            _ => {
                binding.checkpoints =
                    vec![CheckpointRef::new("checkpoint:oversized").unwrap(); MAX_REFERENCES + 1];
                h.create_approval(&human, &t, &p, binding).map(|_| ())
            }
        };
        assert_eq!(rejected, Err(HostError::Runtime(ErrorCode::InvalidRequest)));
        assert!(
            m.data() == bytes,
            "rejected command wrote an intent: {kind}"
        );
        assert_eq!(m.syncs.get(), syncs);
        assert_eq!(h.task(&s, &t.fields().task_id).unwrap(), task);
        assert_eq!(h.execution(&s, &id).unwrap(), result);
        assert_eq!(
            h.events(&s, &t.fields().task_id, None, 256).unwrap(),
            events
        );
        h.close();
        let (mut h, _, fresh) = reopen(&m);
        assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap(), task);
        assert_eq!(h.execution(&fresh, &id).unwrap(), result);
        assert_eq!(
            h.events(&fresh, &t.fields().task_id, None, 256).unwrap(),
            events
        );
        assert_eq!(h.admit(&fresh, t, p).unwrap(), id);
        assert_eq!(h.step().unwrap(), None);
    }
}

#[test]
fn maximum_permission_grant_is_usable_and_survives_replay() {
    let (mut h, owner, _, m) = fixture();
    let mut grant = human_grant();
    grant.permissions.push(Permission::ProposeAction);
    assert_eq!(grant.permissions.len(), 8);
    grant.principal = support::grant().principal;
    let human = h.enroll_verified_human(&owner, grant).unwrap();
    let (t, p, id) = task_and_run(&mut h, human.session(), "task:all-permissions");
    h.step().unwrap();
    h.step().unwrap();
    let result = h.execution(human.session(), &id).unwrap();
    h.close();
    let (mut h, _, fresh) = reopen(&m);
    assert_eq!(h.execution(&fresh, &id).unwrap(), result);
    assert_eq!(h.admit(&fresh, t, p).unwrap(), id);
    assert_eq!(h.step().unwrap(), None);
}

#[test]
fn maximum_approval_vector_known_rejection_is_replayable() {
    let (mut h, _, s, m) = fixture();
    let t = support::task("task:max-approval-ids", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let p = support::proposal(&t, &[2, 3]);
    let ids: Vec<_> = (0..MAX_REFERENCES)
        .map(|i| EventId::new(format!("event:missing-{i}")).unwrap())
        .collect();
    let before = m.data();
    assert_eq!(
        h.admit_with_approvals(&s, t.clone(), p, &ids),
        Err(HostError::Runtime(ErrorCode::ApprovalMismatch))
    );
    assert!(m.data().len() > before.len());
    let task = h.task(&s, &t.fields().task_id).unwrap();
    let events = h.events(&s, &t.fields().task_id, None, 256).unwrap();
    h.close();
    let (h, _, fresh) = reopen(&m);
    assert_eq!(h.task(&fresh, &t.fields().task_id).unwrap(), task);
    assert_eq!(
        h.events(&fresh, &t.fields().task_id, None, 256).unwrap(),
        events
    );
}

#[test]
fn maximum_checkpoint_binding_and_oversight_are_durable_without_relaxing_limits() {
    let (mut h, owner, s, m) = fixture();
    let human = h.enroll_verified_human(&owner, human_grant()).unwrap();
    let checkpoints = (0..MAX_REFERENCES)
        .map(|i| CheckpointRef::new(format!("checkpoint:{i}")).unwrap())
        .collect();
    let oversight = OversightRequirements::new(checkpoints, false, false, None).unwrap();
    let t = support::task("task:max-checkpoints", 20, 80);
    h.create_governed_task(&owner, &s, t.clone(), oversight.clone())
        .unwrap();
    let p = support::proposal(&t, &[2, 3]);
    let binding = h
        .preview_approval(&s, &t, &p, Timestamp::new(1800).unwrap())
        .unwrap();
    assert_eq!(binding.checkpoints.len(), MAX_REFERENCES);
    let id = h.create_approval(&human, &t, &p, binding).unwrap();
    let approval = h.approval(&s, &id).unwrap();
    h.close();
    let (h, _, fresh) = reopen(&m);
    let restored = h.approval(&fresh, &id).unwrap();
    assert_eq!(restored.approval_id, approval.approval_id);
    assert_eq!(restored.binding, approval.binding);
    assert_eq!(restored.verified_principal, approval.verified_principal);
    assert_eq!(restored.decided_at, approval.decided_at);
    assert_eq!(restored.revocation, approval.revocation);
    assert_eq!(
        h.accepted_oversight(&fresh, &t.fields().task_id).unwrap(),
        oversight
    );
}
