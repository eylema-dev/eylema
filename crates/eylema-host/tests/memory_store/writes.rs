use super::records::*;
use eylema_contracts::{memory::durable::*, project_data::*};
use eylema_host::{memory_store::*, native::NativeHash};
#[test]
fn canonical_generation_and_source_projection() {
    let (mut d, o, ds, _) = data();
    let m = Mem::default();
    let (mut s, owner) =
        MemoryStore::create_new(m.clone(), NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    let ms = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    let source = pin(&mut d, &ds, "source");
    let receipt = s
        .propose(
            &mut d,
            &ms,
            0,
            None,
            entity(source.clone()),
            None,
            r("entity:1"),
            &mut c(),
        )
        .unwrap();
    assert_eq!(receipt.generation, 1);
    let meta = d.inspect(&o, &r("entity:1")).unwrap();
    assert_eq!(meta.parents, vec![r("source")]);
    assert_eq!(meta.permissions, DataPermissions::NONE);
    let writes = m.writes.get();
    assert_eq!(
        s.propose(
            &mut d,
            &ms,
            0,
            None,
            claim(source),
            None,
            r("bad:1"),
            &mut c()
        )
        .err(),
        Some(MemoryStoreError::Conflict)
    );
    assert_eq!(m.writes.get(), writes);
    s.close();
    let (s, _) = MemoryStore::open(m, NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    assert_eq!(s.generation(), 1);
}
#[test]
fn sync_failure_poison_and_orphan() {
    for failure in [1, 2, 3] {
        let (mut d, o, ds, _) = data();
        let m = Mem::default();
        let (mut s, owner) =
            MemoryStore::create_new(m.clone(), NativeHash, &mut d, &o, profile(), &mut c())
                .unwrap();
        let ms = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
        let source = pin(&mut d, &ds, "source");
        m.fail.set(failure);
        assert_eq!(
            s.propose(
                &mut d,
                &ms,
                0,
                None,
                entity(source.clone()),
                None,
                r("entity:1"),
                &mut c()
            )
            .err(),
            Some(MemoryStoreError::WriteOutcomeUnknown)
        );
        assert!(d.inspect(&o, &r("entity:1")).is_ok());
        assert_eq!(
            s.propose(
                &mut d,
                &ms,
                0,
                None,
                entity(source),
                None,
                r("entity:2"),
                &mut c()
            )
            .err(),
            Some(MemoryStoreError::NeedsReopen)
        );
    }
}
#[test]
fn reviewed_source_inherits_complete_lineage() {
    let (mut d, o, ds, _) = data();
    let (mut s, owner) =
        MemoryStore::create_new(Mem::default(), NativeHash, &mut d, &o, profile(), &mut c())
            .unwrap();
    let ms = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    let source = pin(&mut d, &ds, "source");
    s.propose(
        &mut d,
        &ms,
        0,
        None,
        entity(source),
        None,
        r("entity:1"),
        &mut c(),
    )
    .unwrap();
    let review = pin(&mut d, &ds, "review");
    let receipt = s
        .review(
            &owner,
            &mut d,
            &o,
            &ms,
            1,
            &id("pump-a"),
            rev(1),
            review,
            r("entity:2"),
            &mut c(),
        )
        .unwrap();
    assert_eq!(receipt.generation, 2);
    let meta = d.inspect(&o, &r("entity:2")).unwrap();
    assert_eq!(meta.parents, vec![r("review"), r("source")]);
    assert_eq!(meta.permissions, DataPermissions::NONE);
}
#[test]
fn commit_then_revoke_hides_receipt() {
    let (mut d, o, ds, clock) = data();
    let m = Mem::default();
    let (mut s, owner) =
        MemoryStore::create_new(m.clone(), NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    let ms = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    let source = pin(&mut d, &ds, "source");
    *m.sync_clock.borrow_mut() = Some(clock.0.clone());
    assert_eq!(
        s.propose(
            &mut d,
            &ms,
            0,
            None,
            entity(source),
            None,
            r("entity:1"),
            &mut c()
        )
        .err(),
        Some(MemoryStoreError::WriteOutcomeUnknown)
    );
    assert_eq!(s.generation(), 1);
    s.close();
    m.sync_clock.borrow_mut().take();
    let (s, _) = MemoryStore::open(m, NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    assert_eq!(s.generation(), 1);
}
#[test]
fn only_live_tool_witness_promotes_episode() {
    use super::support;
    use eylema_contracts::{ActionProposal, tool::*};
    use eylema_host::{DurableHost, tool::*};
    use eylema_journal::Journal;
    use eylema_runtime::{Limits, tool::ToolRegistry};
    use std::{cell::Cell, rc::Rc};
    let (mut d, o, ds, clock) = data();
    let services = support::Services {
        clock: clock.0.clone(),
        next: 0,
        duplicate_event: false,
        fail_clock: Rc::new(Cell::new(false)),
    };
    let (host, host_owner) = DurableHost::create_new(
        Journal::create_new(Mem::default(), NativeHash, Default::default()).unwrap(),
        services,
        Limits::default(),
    )
    .unwrap();
    let sid = |s: &str| eylema_contracts::platform::SemanticProfileId::new(s).unwrap();
    let registry = ToolRegistry::new(vec![ToolProfile::new(
        sid("tool:add"),
        rev(1),
        OwnedTool::AddTaskValue,
    )])
    .unwrap();
    let mut tools = ToolHost::new(host, registry).unwrap();
    let actor = tools.enroll(&host_owner, support::grant()).unwrap();
    let task = support::task("task:memory", 20, 50);
    tools.create_task(&actor, task.clone()).unwrap();
    let proposal: ActionProposal = support::proposal(&task, &[2]);
    let binding = ToolBinding {
        action_id: proposal.fields().actions[0].fields().action_id.clone(),
        profile: sid("tool:add"),
        revision: rev(1),
        input_schema: sid("eyl:u64:v1"),
        output_schema: sid("eyl:task-value:v1"),
        source: None,
    };
    let plan = ToolPlan::new(task, proposal, vec![binding]).unwrap();
    let execution = tools.admit(&mut d, &ds, &actor, plan, &[]).unwrap();
    tools.step(&mut d).unwrap();
    assert_eq!(
        tools.report(&mut d, &actor, &execution).unwrap().status,
        ToolStatus::Completed
    );
    let (mut memory, owner) =
        MemoryStore::create_new(Mem::default(), NativeHash, &mut d, &o, profile(), &mut c())
            .unwrap();
    let ms = memory.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    let mut draft = entity(pin(&mut d, &ds, "source"));
    draft.id = id("episode");
    draft.kind = RecordKind::Episode;
    draft.subject = None;
    draft.episode_status = Some(EpisodeStatus::Failed);
    draft.episode_text = Some("caller cannot dictate status".into());
    memory
        .observe(
            &tools,
            &mut d,
            &actor,
            &ms,
            0,
            draft,
            &execution,
            r("claim:1"),
            &mut c(),
        )
        .unwrap();
    let metadata = d.inspect(&o, &r("claim:1")).unwrap();
    assert_eq!(metadata.parents, vec![r("source")]);
    let raw = d.read(&ds, &r("claim:1"), DataUse::Collect);
    assert!(raw.is_err());
    d.set_policy(
        &o,
        &r("claim:1"),
        rev(1),
        DataPermissions::new(vec![DataUse::Collect]).unwrap(),
        false,
    )
    .unwrap();
    let body = String::from_utf8(d.read(&ds, &r("claim:1"), DataUse::Collect).unwrap()).unwrap();
    assert!(body.contains("completed:"));
    assert!(body.contains("value=2"));
    assert!(!body.contains("caller cannot dictate"));
}
