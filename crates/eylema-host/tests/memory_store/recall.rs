use super::records::*;
use eylema_contracts::{memory::durable::*, platform::RoleKind, project_data::*};
use eylema_host::project_data::DataSession;
use eylema_host::{memory_store::*, native::NativeHash};
pub fn q(generation: u64) -> QueryDraft {
    let p = profile();
    QueryDraft {
        project: p.project,
        profile: p.id,
        revision: rev(1),
        generation,
        role: RoleKind::Reasoning,
        purpose: DataUse::Execute,
        kinds: vec![RecordKind::Fact],
        origins: vec![Origin::DocumentClaim, Origin::Observation],
        knowledge: KnowledgeMode::Reusable,
        effective_at: 20,
        entities: vec![id("pump-a")],
        edge_predicates: vec![],
        result_predicates: vec![],
        seeds: vec![],
        hops: 0,
        rank: RankMode::Graph,
        representation: None,
        max_hits: 32,
        max_sources: 32,
        max_bytes: 65536,
        max_tokens: None,
    }
}
pub fn seed() -> (
    Data,
    eylema_host::Owner,
    DataSession,
    Clock,
    MemoryStore<Mem, NativeHash>,
    MemoryOwner,
    MemorySession,
    Mem,
) {
    let (mut d, o, ds, clock) = data();
    let mem = Mem::default();
    let (mut s, owner) =
        MemoryStore::create_new(mem.clone(), NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    let ms = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    let source = pin(&mut d, &ds, "source");
    let review = pin(&mut d, &ds, "review");
    let mut vector = vec![0.; 1024];
    vector[0] = 1.;
    for (draft, a, b) in [
        (entity(source.clone()), "entity:1", "entity:2"),
        (claim(source), "claim:1", "claim:2"),
    ] {
        let key = draft.id.clone();
        let generation = s.generation();
        s.propose(
            &mut d,
            &ms,
            generation,
            None,
            draft,
            Some(&vector),
            r(a),
            &mut c(),
        )
        .unwrap();
        let generation = s.generation();
        s.review(
            &owner,
            &mut d,
            &o,
            &ms,
            generation,
            &key,
            rev(1),
            review.clone(),
            r(b),
            &mut c(),
        )
        .unwrap();
        d.set_policy(
            &o,
            &r(b),
            rev(1),
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
    }
    (d, o, ds, clock, s, owner, ms, mem)
}
#[test]
fn current_claim_and_actual_g70_share_control() {
    let (mut d, _, _, _, s, _, session, _) = seed();
    for rank in [
        RankMode::Graph,
        RankMode::Lexical("owner".into()),
        RankMode::Vector({
            let mut v = vec![0.; 1024];
            v[1] = 1.;
            v
        }),
        RankMode::Hybrid {
            text: "owner".into(),
            vector: {
                let mut v = vec![0.; 1024];
                v[1] = 1.;
                v
            },
            vector_weight: 500000,
        },
    ] {
        let mut query = q(s.generation());
        if !matches!(rank, RankMode::Graph) {
            query.representation = Some(profile().representation)
        }
        query.rank = rank;
        let mut control = c();
        let result = s
            .query(&mut d, &session, &Query::new(query).unwrap(), &mut control)
            .unwrap();
        assert_eq!(result.status(), MemoryStatus::Evidence);
        assert_eq!(result.record_count(), 1);
        assert!(result.bytes().windows(4).any(|s| s == b"Mira"));
        assert!(control.work_used() > 4_194_304);
    }
}
#[test]
fn cached_reopened_and_historical_reauthorize() {
    let (mut d, o, ds, _, s, _, session, mem) = seed();
    let query = Query::new(q(s.generation())).unwrap();
    let cached = s.cache(&query).unwrap();
    assert!(s.query_cached(&mut d, &session, &cached, &mut c()).is_ok());
    let revision = d.inspect(&o, &r("source")).unwrap().revision;
    d.withdraw(&o, &r("source"), revision).unwrap();
    assert!(s.query_cached(&mut d, &session, &cached, &mut c()).is_err());
    s.close();
    let (s, owner) = MemoryStore::open(mem, NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    let ms = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    assert!(s.query_cached(&mut d, &ms, &cached, &mut c()).is_err());
    assert!(s.query(&mut d, &ms, &query, &mut c()).is_err());
}
#[test]
fn unknown_tokens_and_backend_failures_are_not_empty() {
    let (mut d, _, _, clock, s, _, session, _) = seed();
    let mut draft = q(s.generation());
    draft.max_tokens = Some(10);
    assert_eq!(
        s.query(&mut d, &session, &Query::new(draft).unwrap(), &mut c())
            .err(),
        Some(MemoryStoreError::TokenCounterUnavailable)
    );
    clock.0.set(0);
    assert_eq!(
        s.query(
            &mut d,
            &session,
            &Query::new(q(s.generation())).unwrap(),
            &mut c()
        )
        .err(),
        Some(MemoryStoreError::Backend)
    );
}
#[test]
fn final_sweep_returns_no_partial_context() {
    let (mut d, _, _, _, s, _, session, _) = seed();
    let token = MemoryCancel::new();
    let mut control = MemoryControl::new(
        MemoryBudget {
            max_work: 100_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        },
        token.clone(),
    )
    .unwrap();
    token.cancel();
    assert_eq!(
        s.query(
            &mut d,
            &session,
            &Query::new(q(s.generation())).unwrap(),
            &mut control
        )
        .err(),
        Some(MemoryStoreError::Cancelled)
    );
}
#[test]
fn final_handoff_cancellation_and_deadline_happen_after_positive_work() {
    for cancelled in [true, false] {
        let (mut data, _, _, clock, memory, _, session, _) = seed();
        let token = MemoryCancel::new();
        let before = clock.1.borrow().calls;
        if cancelled {
            clock.1.borrow_mut().cancel = Some((before + 2, token.clone()))
        } else {
            clock.1.borrow_mut().delay = Some((before + 2, std::time::Duration::from_millis(30)))
        }
        let mut control = MemoryControl::new(
            MemoryBudget {
                max_work: 100000000,
                max_live_bytes: 64 * 1024 * 1024,
                deadline: std::time::Instant::now()
                    + std::time::Duration::from_millis(if cancelled { 1000 } else { 20 }),
            },
            token,
        )
        .unwrap();
        let result = memory.query(
            &mut data,
            &session,
            &Query::new(q(memory.generation())).unwrap(),
            &mut control,
        );
        assert_eq!(
            result.err(),
            Some(if cancelled {
                MemoryStoreError::Cancelled
            } else {
                MemoryStoreError::Deadline
            })
        );
        assert!(control.work_used() > 20_000_000);
        assert_eq!(clock.1.borrow().calls, before + 2);
    }
}
