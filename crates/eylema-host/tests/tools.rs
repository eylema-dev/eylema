#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod support;
use eylema_contracts::{
    codec,
    platform::{ProjectId, SemanticProfileId},
    project_data::*,
    tool::*,
    *,
};
use eylema_host::{native::NativeHash, project_data::*, tool::*, *};
use eylema_journal::{Journal, Storage};
use eylema_runtime::{Grant, Limits, tool::ToolRegistry};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
#[derive(Clone)]
struct Memory(Rc<RefCell<Cursor<Vec<u8>>>>);
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.borrow_mut().read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for Memory {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.0.borrow_mut().seek(p)
    }
}
impl Storage for Memory {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.0.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Clone)]
struct Clock(Rc<Cell<u64>>);
impl DataClock for Clock {
    fn now(&mut self) -> DataResult<u64> {
        Ok(self.0.get())
    }
}
type Host = ToolHost<Memory, NativeHash, support::Services>;
type Data = ProjectStore<Memory, NativeHash, Clock>;
fn mem() -> Memory {
    Memory(Rc::new(RefCell::new(Cursor::new(vec![]))))
}
fn sid(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn registry() -> ToolRegistry {
    ToolRegistry::new(vec![
        ToolProfile::new(sid("tool:add"), support::rev(1), OwnedTool::AddTaskValue),
        ToolProfile::new(sid("tool:read"), support::rev(1), OwnedTool::ReadTaskValue),
    ])
    .unwrap()
}
fn reference() -> DataRef {
    DataRef {
        project: ProjectId::new("project:a").unwrap(),
        id: DataId::new("input").unwrap(),
    }
}
fn fixture_with_controller(
    enroll_controller: bool,
) -> (
    Host,
    Owner,
    DurableSession,
    Data,
    Owner,
    DataSession,
    Memory,
    Clock,
) {
    let storage = mem();
    let clock = Clock(Rc::new(Cell::new(1000)));
    let serv = support::Services {
        clock: clock.0.clone(),
        next: 0,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    };
    let (h, o) = DurableHost::create_new(
        Journal::create_new(storage.clone(), NativeHash, Default::default()).unwrap(),
        serv,
        Limits::default(),
    )
    .unwrap();
    let mut h = ToolHost::new(h, registry()).unwrap();
    let s = h.enroll(&o, support::grant()).unwrap();
    if enroll_controller {
        let mut g = support::grant();
        g.principal = PrincipalRef::new("principal:source-controller-fixture").unwrap();
        g.permissions.push(Permission::RequestCancellation);
        h.enroll_verified_human(&o, g).unwrap();
    }
    let (mut d, owner) = Data::create_new(
        Journal::create_new(mem(), NativeHash, project_journal_limits()).unwrap(),
        reference().project.clone(),
        clock.clone(),
    )
    .unwrap();
    let data_session = d
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: vec![reference().id],
                expires_at: 10000,
            },
        )
        .unwrap();
    (h, o, s, d, owner, data_session, storage, clock)
}
fn fixture() -> (
    Host,
    Owner,
    DurableSession,
    Data,
    Owner,
    DataSession,
    Memory,
    Clock,
) {
    fixture_with_controller(true)
}
fn intervention(plan: ToolPlan, required: bool) -> ToolPlan {
    let mut proposal = plan.proposal().fields().clone();
    proposal.actions = proposal
        .actions
        .into_iter()
        .map(|a| {
            let mut a = a.fields().clone();
            let r = &a.authority;
            a.authority = AuthorityRequirement::new(
                r.principal_ref().clone(),
                r.policy_revision(),
                r.resource_scope().clone(),
                r.permissions().to_vec(),
                OversightRequirements::new(
                    r.oversight().checkpoints().to_vec(),
                    r.oversight().requires_observation(),
                    required,
                    r.oversight().governance().cloned(),
                )
                .unwrap(),
            )
            .unwrap();
            ActionSpec::new(a).unwrap()
        })
        .collect();
    ToolPlan::new(
        plan.task().clone(),
        ActionProposal::new(proposal, plan.task()).unwrap(),
        plan.bindings().to_vec(),
    )
    .unwrap()
}
fn plan(t: &TaskSpec, values: &[u64], source: bool) -> ToolPlan {
    let p = support::proposal(t, values);
    let bindings = p
        .fields()
        .actions
        .iter()
        .map(|a| ToolBinding {
            action_id: a.fields().action_id.clone(),
            profile: sid("tool:add"),
            revision: support::rev(1),
            input_schema: sid("eyl:u64:v1"),
            output_schema: sid("eyl:task-value:v1"),
            source: source.then(reference),
        })
        .collect();
    let result = ToolPlan::new(t.clone(), p, bindings).unwrap();
    if source {
        intervention(result, true)
    } else {
        result
    }
}
fn input(d: &mut Data, o: &Owner, s: &DataSession, n: u64, expiry: u64) {
    d.collect(
        s,
        DataDraft {
            reference: reference(),
            kind: DataKind::Artifact,
            parents: vec![],
            payload: codec::encode(&codec::Value::Unsigned(n)).unwrap(),
            expires_at: expiry,
        },
    )
    .unwrap();
    d.set_policy(
        o,
        &reference(),
        support::rev(1),
        DataPermissions::new(vec![DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
}
#[test]
fn real_owned_tools_and_replays_report_confirmed_outputs_without_new_effects() {
    let (mut h, _, s, mut d, _, ds, _, _) = fixture();
    let t = support::task("task:tools", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let p = plan(&t, &[2, 3], false);
    let id = h.admit(&mut d, &ds, &s, p.clone(), &[]).unwrap();
    assert_eq!(
        h.report(&mut d, &s, &id).unwrap().status,
        ToolStatus::Pending
    );
    h.step(&mut d).unwrap();
    h.step(&mut d).unwrap();
    let result = h.report(&mut d, &s, &id).unwrap();
    assert_eq!(result.status, ToolStatus::Completed);
    assert_eq!(
        result
            .outputs
            .iter()
            .map(|o| codec::decode(&o.bytes).unwrap())
            .collect::<Vec<_>>(),
        vec![codec::Value::Unsigned(2), codec::Value::Unsigned(5)]
    );
    assert_eq!(h.admit(&mut d, &ds, &s, p.clone(), &[]).unwrap(), id);
    assert_eq!(h.step(&mut d).unwrap(), None);
    assert_eq!(h.report(&mut d, &s, &id).unwrap(), result);
    let mut a = p.proposal().fields().clone();
    let mut first = a.actions[0].fields().clone();
    first.payload = codec::encode(&codec::Value::Unsigned(4)).unwrap();
    a.actions[0] = ActionSpec::new(first).unwrap();
    let changed = ToolPlan::new(
        t,
        ActionProposal::new(a, p.task()).unwrap(),
        p.bindings().to_vec(),
    )
    .unwrap();
    assert!(h.admit(&mut d, &ds, &s, changed, &[]).is_err());
}
#[test]
fn source_permission_is_live_on_every_step_and_cumulative_output() {
    let (mut h, _, s, mut d, o, ds, _, _) = fixture();
    input(&mut d, &o, &ds, 2, 9000);
    let t = support::task("task:source", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(&mut d, &ds, &s, plan(&t, &[2, 2], true), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    d.withdraw(&o, &reference(), support::rev(2)).unwrap();
    assert!(matches!(
        h.step(&mut d),
        Err(ToolError::Data(DataError::Forbidden))
    ));
    assert!(matches!(
        h.report(&mut d, &s, &id),
        Err(ToolError::Data(DataError::Forbidden))
    ));
}
#[test]
fn denied_mismatched_expired_and_foreign_source_produce_no_effect() {
    let (mut h, _, s, mut d, o, ds, _, clock) = fixture();
    input(&mut d, &o, &ds, 2, 1100);
    let original = support::task("task:denied", 20, 50);
    let mut draft = original.fields().clone();
    draft.budget = Budget::new(20, 50, 1024, 1, Timestamp::new(1100).unwrap()).unwrap();
    let t = TaskSpec::new(draft).unwrap();
    h.create_task(&s, t.clone()).unwrap();
    assert!(matches!(
        h.admit(&mut d, &ds, &s, plan(&t, &[3], true), &[]),
        Err(ToolError::InputMismatch)
    ));
    let id = h.admit(&mut d, &ds, &s, plan(&t, &[2], true), &[]).unwrap();
    clock.0.set(1100);
    assert!(matches!(
        h.step(&mut d),
        Err(ToolError::Data(DataError::Expired))
    ));
    assert!(h.report(&mut d, &s, &id).is_err());
}
#[test]
fn verified_human_stop_finishes_even_when_source_has_been_withdrawn() {
    let (mut h, o, s, mut d, owner, ds, _, _) = fixture();
    let mut g: Grant = support::grant();
    g.principal = PrincipalRef::new("principal:verified-fixture").unwrap();
    g.permissions.push(Permission::RequestCancellation);
    let human = h.enroll_verified_human(&o, g).unwrap();
    input(&mut d, &owner, &ds, 2, 9000);
    let t = support::task("task:stop", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(&mut d, &ds, &s, plan(&t, &[2, 2], true), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    d.withdraw(&owner, &reference(), support::rev(2)).unwrap();
    h.request_cancellation(&human, &id).unwrap();
    assert_eq!(h.step(&mut d).unwrap(), Some(id.clone()));
    assert_eq!(h.step(&mut d).unwrap(), None);
    // No private output is exposed by stop receipt; the source gate still denies it.
    assert!(h.report(&mut d, &s, &id).is_err());
}
#[test]
fn partial_overflow_and_restart_unknown_remain_distinct() {
    let (mut h, _, s, mut d, _, ds, m, clock) = fixture();
    let t = support::task("task:overflow", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(&mut d, &ds, &s, plan(&t, &[u64::MAX, 1], false), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        h.report(&mut d, &s, &id).unwrap().status,
        ToolStatus::Partial
    );
    let t = support::task("task:unfinished", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(&mut d, &ds, &s, plan(&t, &[2, 3], false), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    h.close();
    let serv = support::Services {
        clock: clock.0,
        next: 10000,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    };
    let (mut recovered, o) = DurableHost::open(
        Journal::open(m, NativeHash, Default::default()).unwrap(),
        serv,
    )
    .unwrap();
    let fresh = recovered.enroll(&o, support::grant()).unwrap();
    let status = ToolHost::inspect_recovery(&recovered, &fresh, &id).unwrap();
    assert_eq!(status.status, ToolStatus::Unknown);
    assert_eq!(status.confirmed_actions, 1);
    assert_eq!(recovered.has_pending_work(), Ok(false));
    assert!(matches!(
        ToolHost::new(recovered, registry()),
        Err(ToolError::ProvenanceUnavailable)
    ));
}

#[test]
fn source_deadlines_must_bound_actual_runtime_action_deadlines() {
    let (mut h, _, s, mut d, o, ds, _, _) = fixture();
    input(&mut d, &o, &ds, 2, 1100);
    let t = support::task("task:deadline-binding", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    assert!(matches!(
        h.admit(&mut d, &ds, &s, plan(&t, &[2], true), &[]),
        Err(ToolError::DeadlineNotBound)
    ));
}
fn read_plan(t: &TaskSpec) -> ToolPlan {
    let p = support::proposal(t, &[0]);
    let mut a = p.fields().actions[0].fields().clone();
    a.operation_id = OperationId::new("readTaskValue").unwrap();
    a.side_effect_class = SideEffectClass::Read;
    a.payload = vec![];
    a.expected_revision = None;
    a.idempotency_key =
        IdempotencyKey::new(format!("{}:read", t.fields().task_id.as_str())).unwrap();
    let mut draft = p.fields().clone();
    draft.proposal_id = ProposalId::new(format!("{}:read", t.fields().task_id.as_str())).unwrap();
    draft.actions = vec![ActionSpec::new(a).unwrap()];
    let p = ActionProposal::new(draft, t).unwrap();
    let b = ToolBinding {
        action_id: p.fields().actions[0].fields().action_id.clone(),
        profile: sid("tool:read"),
        revision: support::rev(1),
        input_schema: sid("eyl:empty:v1"),
        output_schema: sid("eyl:task-value:v1"),
        source: None,
    };
    ToolPlan::new(t.clone(), p, vec![b]).unwrap()
}
#[test]
fn actual_read_handler_and_stale_oversized_or_unprivileged_mutations_are_distinct() {
    let (mut h, o, s, mut d, _, ds, _, _) = fixture();
    let t = support::task("task:authority", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let mut denied = support::grant();
    denied
        .permissions
        .retain(|p| *p != Permission::ExecuteAction);
    let denied = h.enroll(&o, denied).unwrap();
    assert!(
        h.admit(&mut d, &ds, &denied, plan(&t, &[9], false), &[])
            .is_err()
    );
    let valid = plan(&t, &[1], false);
    let mut pd = valid.proposal().fields().clone();
    let mut a = pd.actions[0].fields().clone();
    a.payload = vec![0; 17];
    pd.actions[0] = ActionSpec::new(a).unwrap();
    let oversized = ToolPlan::new(
        t.clone(),
        ActionProposal::new(pd, &t).unwrap(),
        valid.bindings().to_vec(),
    )
    .unwrap();
    assert!(h.admit(&mut d, &ds, &s, oversized, &[]).is_err());
    let id = h.admit(&mut d, &ds, &s, read_plan(&t), &[]).unwrap();
    h.step(&mut d).unwrap();
    let result = h.report(&mut d, &s, &id).unwrap();
    assert_eq!(
        codec::decode(&result.outputs[0].bytes).unwrap(),
        codec::Value::Unsigned(0)
    );
    assert_eq!(result.status, ToolStatus::Completed);
    let id = h.admit(&mut d, &ds, &s, valid.clone(), &[]).unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        h.report(&mut d, &s, &id).unwrap().status,
        ToolStatus::Completed
    );
    let mut pd = valid.proposal().fields().clone();
    pd.proposal_id = ProposalId::new("proposal:stale").unwrap();
    let mut a = pd.actions[0].fields().clone();
    a.idempotency_key = IdempotencyKey::new("key:stale").unwrap();
    pd.actions[0] = ActionSpec::new(a).unwrap();
    let stale = ToolPlan::new(
        t.clone(),
        ActionProposal::new(pd, &t).unwrap(),
        valid.bindings().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        h.admit(&mut d, &ds, &s, stale, &[]),
        Err(ToolError::Host(HostError::Runtime(
            ErrorCode::RevisionConflict
        )))
    ));
}
#[test]
fn later_source_free_read_cannot_disclose_a_withdrawn_derived_value() {
    let (mut h, _, s, mut d, o, ds, _, _) = fixture();
    input(&mut d, &o, &ds, 2, 9000);
    let t = support::task("task:derived", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let id = h.admit(&mut d, &ds, &s, plan(&t, &[2], true), &[]).unwrap();
    h.step(&mut d).unwrap();
    let current = h.task_spec(&mut d, &s, &t.fields().task_id).unwrap();
    let read = h
        .admit(
            &mut d,
            &ds,
            &s,
            intervention(read_plan(&current), true),
            &[],
        )
        .unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        codec::decode(&h.report(&mut d, &s, &read).unwrap().outputs[0].bytes).unwrap(),
        codec::Value::Unsigned(2)
    );
    d.withdraw(&o, &reference(), support::rev(2)).unwrap();
    assert!(h.report(&mut d, &s, &id).is_err());
    assert!(h.report(&mut d, &s, &read).is_err());
    assert!(h.task_spec(&mut d, &s, &t.fields().task_id).is_err());
    assert!(matches!(
        h.admit(
            &mut d,
            &ds,
            &s,
            intervention(read_plan(&current), true),
            &[]
        ),
        Err(ToolError::Data(DataError::Forbidden))
    ));
}
#[test]
fn equal_looking_foreign_store_foreign_session_and_missing_execute_right_fail() {
    let (mut h, _, s, mut d, o, ds, _, _) = fixture();
    input(&mut d, &o, &ds, 2, 9000);
    let (_, _, _, mut other, other_owner, foreign, _, _) = fixture();
    input(&mut other, &other_owner, &foreign, 2, 9000);
    let t = support::task("task:foreign-source", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    assert!(matches!(
        h.admit(&mut d, &foreign, &s, plan(&t, &[2], true), &[]),
        Err(ToolError::Data(DataError::Forbidden))
    ));
    let collect_only = d
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect]).unwrap(),
                scope: vec![reference().id],
                expires_at: 9000,
            },
        )
        .unwrap();
    assert!(matches!(
        h.admit(&mut d, &collect_only, &s, plan(&t, &[2], true), &[]),
        Err(ToolError::Data(DataError::Forbidden))
    ));
    let id = h.admit(&mut d, &ds, &s, plan(&t, &[2], true), &[]).unwrap();
    assert!(matches!(
        h.step(&mut other),
        Err(ToolError::WrongBackingStore)
    ));
    assert_eq!(h.report(&mut d, &s, &id).unwrap().outputs.len(), 0);
    h.step(&mut d).unwrap();
    assert_eq!(
        h.report(&mut d, &s, &id).unwrap().status,
        ToolStatus::Completed
    );
}
#[test]
fn sealed_data_session_ttl_also_bounds_action_deadline() {
    let (mut h, _, s, mut d, o, ds, _, _) = fixture();
    input(&mut d, &o, &ds, 2, 9000);
    let short = d
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Execute]).unwrap(),
                scope: vec![reference().id],
                expires_at: 1100,
            },
        )
        .unwrap();
    let t = support::task("task:session-deadline", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    assert!(matches!(
        h.admit(&mut d, &short, &s, plan(&t, &[2], true), &[]),
        Err(ToolError::DeadlineNotBound)
    ));
}
struct JumpServices {
    inner: support::Services,
    jump: Rc<Cell<Option<u64>>>,
}
impl eylema_runtime::HostServices for JumpServices {
    fn now(&mut self) -> std::result::Result<Timestamp, ErrorCode> {
        if let Some(now) = self.jump.take() {
            self.inner.clock.set(now);
        }
        self.inner.now()
    }
    fn execution_id(&mut self) -> std::result::Result<ExecutionId, ErrorCode> {
        self.inner.execution_id()
    }
    fn event_id(&mut self) -> std::result::Result<EventId, ErrorCode> {
        self.inner.event_id()
    }
    fn sha256(&mut self, b: &[u8]) -> std::result::Result<[u8; 32], ErrorCode> {
        self.inner.sha256(b)
    }
}
#[test]
fn expiry_between_source_staging_and_runtime_observation_has_no_effect() {
    let storage = mem();
    let clock = Clock(Rc::new(Cell::new(1000)));
    let jump = Rc::new(Cell::new(None));
    let serv = JumpServices {
        inner: support::Services {
            clock: clock.0.clone(),
            next: 0,
            duplicate_event: false,
            fail_clock: Rc::new(Cell::new(false)),
        },
        jump: jump.clone(),
    };
    let (raw, o) = DurableHost::create_new(
        Journal::create_new(storage.clone(), NativeHash, Default::default()).unwrap(),
        serv,
        Limits::default(),
    )
    .unwrap();
    let mut h = ToolHost::new(raw, registry()).unwrap();
    let s = h.enroll(&o, support::grant()).unwrap();
    let mut g = support::grant();
    g.principal = PrincipalRef::new("principal:handoff-controller-fixture").unwrap();
    g.permissions.push(Permission::RequestCancellation);
    h.enroll_verified_human(&o, g).unwrap();
    let (mut d, owner) = Data::create_new(
        Journal::create_new(mem(), NativeHash, project_journal_limits()).unwrap(),
        reference().project.clone(),
        clock.clone(),
    )
    .unwrap();
    let ds = d
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: vec![reference().id],
                expires_at: 9000,
            },
        )
        .unwrap();
    input(&mut d, &owner, &ds, 2, 1100);
    let orig = support::task("task:handoff-time", 20, 50);
    let mut td = orig.fields().clone();
    td.budget = Budget::new(20, 50, 1024, 1, Timestamp::new(1100).unwrap()).unwrap();
    let t = TaskSpec::new(td).unwrap();
    h.create_task(&s, t.clone()).unwrap();
    let id = h.admit(&mut d, &ds, &s, plan(&t, &[2], true), &[]).unwrap();
    jump.set(Some(1100));
    assert_eq!(h.step(&mut d).unwrap(), Some(id.clone()));
    assert!(h.report(&mut d, &s, &id).is_err());
    h.close();
    let serv = support::Services {
        clock: clock.0,
        next: 10000,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    };
    let (mut raw, o) = DurableHost::open(
        Journal::open(storage, NativeHash, Default::default()).unwrap(),
        serv,
    )
    .unwrap();
    let fresh = raw.enroll(&o, support::grant()).unwrap();
    assert_eq!(raw.task(&fresh, &t.fields().task_id).unwrap().value, 0);
    assert_eq!(
        raw.execution(&fresh, &id).unwrap().fields().state,
        ExecutionState::Failed
    );
}
#[test]
fn unauthorized_stop_cannot_set_bypass_and_verified_zero_effect_stop_is_stopped() {
    let (mut h, o, s, mut d, _, ds, _, _) = fixture();
    let incapable = h.enroll_verified_human(&o, support::grant()).unwrap();
    let mut g = support::grant();
    g.principal = PrincipalRef::new("principal:stop-fixture").unwrap();
    g.permissions.push(Permission::RequestCancellation);
    let human = h.enroll_verified_human(&o, g).unwrap();
    let t = support::task("task:zero-stop", 20, 50);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(&mut d, &ds, &s, plan(&t, &[2], false), &[])
        .unwrap();
    assert!(h.request_cancellation(&incapable, &id).is_err());
    h.request_cancellation(&human, &id).unwrap();
    h.step(&mut d).unwrap();
    let report = h.report(&mut d, &s, &id).unwrap();
    assert_eq!(report.status, ToolStatus::Stopped);
    assert!(report.outputs.is_empty());
    assert_eq!(h.step(&mut d).unwrap(), None);
}
#[test]
fn single_active_execution_is_not_replaced_by_a_second_task() {
    let (mut h, _, s, mut d, _, ds, _, _) = fixture();
    let a = support::task("task:first", 20, 50);
    let b = support::task("task:second", 20, 50);
    h.create_task(&s, a.clone()).unwrap();
    h.create_task(&s, b.clone()).unwrap();
    let first = h
        .admit(&mut d, &ds, &s, plan(&a, &[2], false), &[])
        .unwrap();
    assert!(matches!(
        h.admit(&mut d, &ds, &s, plan(&b, &[3], false), &[]),
        Err(ToolError::Busy)
    ));
    assert_eq!(h.step(&mut d).unwrap(), Some(first));
    let second = h
        .admit(&mut d, &ds, &s, plan(&b, &[3], false), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        codec::decode(&h.report(&mut d, &s, &second).unwrap().outputs[0].bytes).unwrap(),
        codec::Value::Unsigned(3)
    );
}
#[test]
fn retained_provenance_capacity_denies_without_eviction_or_an_extra_execution() {
    let (mut _unused, _, _, mut d, _, ds, _, clock) = fixture();
    let serv = support::Services {
        clock: clock.0,
        next: 0,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    };
    let limits = Limits {
        executions: 256,
        ..Limits::default()
    };
    let (raw, o) = DurableHost::create_new(
        Journal::create_new(mem(), NativeHash, Default::default()).unwrap(),
        serv,
        limits,
    )
    .unwrap();
    let mut h = ToolHost::new(raw, registry()).unwrap();
    let s = h.enroll(&o, support::grant()).unwrap();
    let task = support::task("task:provenance-capacity", 1000, 2000);
    h.create_task(&s, task.clone()).unwrap();
    let mut first = None;
    for n in 0..129 {
        let read = read_plan(&task);
        let mut proposal = read.proposal().fields().clone();
        proposal.proposal_id = ProposalId::new(format!("proposal:read:{n}")).unwrap();
        let mut action = proposal.actions[0].fields().clone();
        action.idempotency_key = IdempotencyKey::new(format!("key:read:{n}")).unwrap();
        proposal.actions[0] = ActionSpec::new(action).unwrap();
        let p = ToolPlan::new(
            task.clone(),
            ActionProposal::new(proposal, &task).unwrap(),
            read.bindings().to_vec(),
        )
        .unwrap();
        let result = h.admit(&mut d, &ds, &s, p, &[]);
        if n == 128 {
            assert!(matches!(
                result,
                Err(ToolError::Runtime(ErrorCode::BudgetExceeded))
            ));
            break;
        }
        let id = result.unwrap();
        first.get_or_insert(id.clone());
        h.step(&mut d).unwrap();
    }
    assert_eq!(h.step(&mut d).unwrap(), None);
    assert_eq!(
        h.report(&mut d, &s, &first.unwrap()).unwrap().status,
        ToolStatus::Completed
    );
}

#[test]
fn revoked_execution_actor_cannot_keep_a_terminal_job_latched_active() {
    let (mut h, o, s, mut d, _, ds, _, _) = fixture();
    let mut g = support::grant();
    g.principal = PrincipalRef::new("principal:governor-fixture").unwrap();
    g.permissions
        .extend([Permission::GovernPolicy, Permission::RequestCancellation]);
    let governor = h.enroll_verified_human(&o, g).unwrap();
    let t = support::task("task:revoked", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(&mut d, &ds, &s, plan(&t, &[2, 3], false), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    h.revoke_delegation(&governor, &t.fields().task_id, &s)
        .unwrap();
    assert_eq!(h.step(&mut d).unwrap(), Some(id.clone()));
    let fresh = h.enroll(&o, support::grant()).unwrap();
    assert_eq!(
        h.report(&mut d, &fresh, &id).unwrap().status,
        ToolStatus::Partial
    );
    let current = h.task_spec(&mut d, &fresh, &t.fields().task_id).unwrap();
    let read = h
        .admit(&mut d, &ds, &fresh, read_plan(&current), &[])
        .unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        codec::decode(&h.report(&mut d, &fresh, &read).unwrap().outputs[0].bytes).unwrap(),
        codec::Value::Unsigned(2)
    );
}
fn checkpoint_plan(t: &TaskSpec) -> ToolPlan {
    let p = plan(t, &[2], false);
    let mut draft = p.proposal().fields().clone();
    let mut action = draft.actions[0].fields().clone();
    action.authority = AuthorityRequirement::new(
        support::grant().principal,
        support::rev(3),
        support::scope(),
        vec![Permission::ExecuteAction],
        OversightRequirements::new(
            vec![CheckpointRef::new("checkpoint:human").unwrap()],
            true,
            true,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    draft.actions[0] = ActionSpec::new(action).unwrap();
    ToolPlan::new(
        t.clone(),
        ActionProposal::new(draft, t).unwrap(),
        p.bindings().to_vec(),
    )
    .unwrap()
}
#[test]
fn actual_scoped_human_checkpoint_is_required_and_cannot_be_replaced_by_labels() {
    let (mut h, o, s, mut d, _, ds, _, _) = fixture();
    let mut grant = support::grant();
    grant.principal = PrincipalRef::new("principal:approver-fixture").unwrap();
    grant
        .permissions
        .extend([Permission::ApproveAction, Permission::RequestCancellation]);
    let human = h.enroll_verified_human(&o, grant).unwrap();
    let t = support::task("task:approval", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let p = checkpoint_plan(&t);
    assert!(matches!(
        h.admit(&mut d, &ds, &s, p.clone(), &[]),
        Err(ToolError::Host(HostError::Runtime(
            ErrorCode::ApprovalRequired
        )))
    ));
    let binding = h
        .preview_approval(&s, &p, Timestamp::new(1500).unwrap())
        .unwrap();
    let proof = h.create_approval(&human, &p, binding).unwrap();
    let id = h.admit(&mut d, &ds, &s, p, &[proof]).unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        h.report(&mut d, &s, &id).unwrap().status,
        ToolStatus::Completed
    );
}
#[test]
fn expired_human_checkpoint_fails_before_owned_effect() {
    let (mut h, o, s, mut d, _, ds, _, clock) = fixture();
    let mut grant = support::grant();
    grant.principal = PrincipalRef::new("principal:expiring-approver").unwrap();
    grant
        .permissions
        .extend([Permission::ApproveAction, Permission::RequestCancellation]);
    let human = h.enroll_verified_human(&o, grant).unwrap();
    let t = support::task("task:expired-approval", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let p = checkpoint_plan(&t);
    let binding = h
        .preview_approval(&s, &p, Timestamp::new(1100).unwrap())
        .unwrap();
    let proof = h.create_approval(&human, &p, binding).unwrap();
    let id = h.admit(&mut d, &ds, &s, p, &[proof]).unwrap();
    clock.0.set(1100);
    h.step(&mut d).unwrap();
    let report = h.report(&mut d, &s, &id).unwrap();
    assert_eq!(report.status, ToolStatus::Failed);
    assert!(report.outputs.is_empty());
}

#[test]
fn source_plans_cannot_omit_intervention_or_admit_without_a_real_controller() {
    let (mut h, o, s, mut d, owner, ds, _, _) = fixture_with_controller(false);
    input(&mut d, &owner, &ds, 2, 9000);
    let t = support::task("task:controller-floor", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let omitted = intervention(plan(&t, &[2], true), false);
    assert!(matches!(
        h.admit(&mut d, &ds, &s, omitted.clone(), &[]),
        Err(ToolError::Runtime(ErrorCode::CapabilityUnavailable))
    ));
    let required = intervention(omitted.clone(), true);
    assert!(matches!(
        h.admit(&mut d, &ds, &s, required.clone(), &[]),
        Err(ToolError::Host(HostError::Runtime(
            ErrorCode::CapabilityUnavailable
        )))
    ));
    let mut g = support::grant();
    g.principal = PrincipalRef::new("principal:actual-scoped-controller").unwrap();
    g.permissions.push(Permission::RequestCancellation);
    let human = h.enroll_verified_human(&o, g).unwrap();
    assert!(matches!(
        h.admit(&mut d, &ds, &s, omitted, &[]),
        Err(ToolError::Runtime(ErrorCode::CapabilityUnavailable))
    ));
    let id = h.admit(&mut d, &ds, &s, required, &[]).unwrap();
    d.withdraw(&owner, &reference(), support::rev(2)).unwrap();
    h.request_cancellation(&human, &id).unwrap();
    assert_eq!(h.step(&mut d).unwrap(), Some(id));
    assert_eq!(h.step(&mut d).unwrap(), None);
}
#[test]
fn retained_sources_require_intervention_on_later_source_free_plans() {
    let (mut h, _, s, mut d, o, ds, _, _) = fixture_with_controller(true);
    input(&mut d, &o, &ds, 2, 9000);
    let t = support::task("task:retained-controller-floor", 30, 100);
    h.create_task(&s, t.clone()).unwrap();
    let id = h
        .admit(
            &mut d,
            &ds,
            &s,
            intervention(plan(&t, &[2], true), true),
            &[],
        )
        .unwrap();
    h.step(&mut d).unwrap();
    assert_eq!(
        h.report(&mut d, &s, &id).unwrap().status,
        ToolStatus::Completed
    );
    let current = h.task_spec(&mut d, &s, &t.fields().task_id).unwrap();
    assert!(matches!(
        h.admit(
            &mut d,
            &ds,
            &s,
            intervention(read_plan(&current), false),
            &[]
        ),
        Err(ToolError::Runtime(ErrorCode::CapabilityUnavailable))
    ));
}
