#[path = "memory_store/briefs.rs"]
mod briefs;
#[path = "memory_store/fixtures.rs"]
mod frozen;
#[path = "memory_store/journal.rs"]
mod journal;
#[path = "memory_store/recall.rs"]
mod recall;
#[path = "memory_store/records.rs"]
mod records;
#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod support;
#[path = "memory_store/writes.rs"]
mod writes;

mod matrix {
    use super::{briefs::*, records::*};
    use eylema_contracts::{memory::durable::*, platform::*, project_data::*};
    use eylema_host::{Owner, memory_store::*, native::NativeHash, project_data::*};
    use eylema_journal::Journal;
    struct Fixture {
        data: Data,
        owner: Owner,
        ds: DataSession,
        clock: Clock,
        memory: MemoryStore<Mem, NativeHash>,
        memory_owner: MemoryOwner,
        session: MemorySession,
        backing: Mem,
    }
    fn fixture() -> Fixture {
        fixture_with_edges(Cardinality::SetValue)
    }
    fn fixture_with_edges(cardinality: Cardinality) -> Fixture {
        let mut profile = corpus_profile();
        profile
            .predicates
            .iter_mut()
            .find(|p| p.id.as_str() == "contains")
            .unwrap()
            .cardinality = cardinality;
        let clock = Clock::new(1);
        let (mut data, owner) = ProjectStore::create_new(
            Journal::create_new(Mem::default(), NativeHash, project_journal_limits()).unwrap(),
            r("x").project,
            clock.clone(),
        )
        .unwrap();
        let ds = data.enroll(&owner, grant()).unwrap();
        let backing = Mem::default();
        let (mut memory, memory_owner) = MemoryStore::create_new(
            backing.clone(),
            NativeHash,
            &mut data,
            &owner,
            profile,
            &mut c(),
        )
        .unwrap();
        let (session, _) = seed_corpus(&mut data, &owner, &ds, &mut memory, &memory_owner);
        clock.0.set(80);
        Fixture {
            data,
            owner,
            ds,
            clock,
            memory,
            memory_owner,
            session,
            backing,
        }
    }
    fn add(f: &mut Fixture, index: usize, mut draft: RecordDraft) {
        let a = r(&format!("extra:{index}:1"));
        let b = r(&format!("extra:{index}:2"));
        draft.revision = rev(1);
        let id = draft.id.clone();
        let mut vector = vec![0.; 1024];
        vector[0] = 1.;
        let generation = f.memory.generation();
        f.memory
            .propose(
                &mut f.data,
                &f.session,
                generation,
                None,
                draft,
                Some(&vector),
                a,
                &mut c(),
            )
            .unwrap();
        let review = pin(&mut f.data, &f.ds, "review");
        let generation = f.memory.generation();
        f.memory
            .review(
                &f.memory_owner,
                &mut f.data,
                &f.owner,
                &f.session,
                generation,
                &id,
                rev(1),
                review,
                b.clone(),
                &mut c(),
            )
            .unwrap();
        f.data
            .set_policy(
                &f.owner,
                &b,
                rev(1),
                DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
                false,
            )
            .unwrap();
    }
    #[test]
    fn conflict_group_is_atomic_and_set_values_stay_attributed() {
        let mut f = fixture();
        let query = job_query(5, false, 0, f.memory.generation());
        let mut small = query.fields().clone();
        small.max_hits = 1;
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(small).unwrap(),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::IncompleteConflict)
        );
        let mut small = query.fields().clone();
        small.max_bytes = 32;
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(small).unwrap(),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::IncompleteConflict)
        );
        let mut d = claim(pin(&mut f.data, &f.ds, "en-owner"));
        d.id = id("second-checklist");
        d.predicate = Some(SemanticProfileId::new("checklist").unwrap());
        d.object = Some(TypedObject::Text("C8".into()));
        add(&mut f, 0, d);
        let mut q = job_query(1, false, 0, f.memory.generation())
            .fields()
            .clone();
        q.result_predicates = vec![SemanticProfileId::new("checklist").unwrap()];
        let result = f
            .memory
            .query(&mut f.data, &f.session, &Query::new(q).unwrap(), &mut c())
            .unwrap();
        assert_eq!(result.status(), MemoryStatus::Evidence);
        assert_eq!(result.record_count(), 2);
    }
    #[test]
    fn join_filter_precedes_rank_and_seed_probes_match() {
        let mut f = fixture();
        let query = job_query(3, false, 2, f.memory.generation());
        let result = f
            .memory
            .query(&mut f.data, &f.session, &query, &mut c())
            .unwrap();
        let ids: Vec<_> = result
            .records()
            .iter()
            .map(|r| r.draft.id.as_str())
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"en-owner") && ids.contains(&"en-check") && ids.contains(&"en-link"));
        assert!(!ids.contains(&"irrelevant"));
        let current = f.data.inspect(&f.owner, &r("en-site")).unwrap().revision;
        f.data.withdraw(&f.owner, &r("en-site"), current).unwrap();
        let denied = f
            .memory
            .query(&mut f.data, &f.session, &query, &mut c())
            .err();
        let mut absent = query.fields().clone();
        absent.seeds = vec![id("absent")];
        assert_eq!(denied, Some(MemoryStoreError::Forbidden));
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(absent).unwrap(),
                    &mut c()
                )
                .err(),
            denied
        );
    }
    #[test]
    fn graph_cycles_terminate_and_dangling_writes_have_no_effect() {
        let mut f = fixture();
        let mut d = claim(pin(&mut f.data, &f.ds, "en-owner"));
        d.id = id("cycle");
        d.kind = RecordKind::Relation;
        d.predicate = Some(SemanticProfileId::new("contains").unwrap());
        d.object = Some(TypedObject::Entity(id("north")));
        add(&mut f, 0, d);
        let mut q = job_query(3, false, 0, f.memory.generation())
            .fields()
            .clone();
        q.hops = 4;
        let result = f
            .memory
            .query(&mut f.data, &f.session, &Query::new(q).unwrap(), &mut c())
            .unwrap();
        assert!(result.record_count() <= 5);
        let mut bad = claim(pin(&mut f.data, &f.ds, "en-owner"));
        bad.id = id("dangling");
        bad.subject = Some(id("missing"));
        let before = f.backing.writes.get();
        let generation = f.memory.generation();
        assert_eq!(
            f.memory
                .propose(
                    &mut f.data,
                    &f.session,
                    generation,
                    None,
                    bad,
                    None,
                    r("extra:1:1"),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::InvalidRequest)
        );
        assert_eq!(before, f.backing.writes.get());
    }
    #[test]
    fn pins_store_purpose_profile_generation_and_current_ttl_fail_closed() {
        let mut f = fixture();
        let query = job_query(1, false, 0, f.memory.generation());
        let mut wrong = query.fields().clone();
        wrong.purpose = DataUse::Train;
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(wrong).unwrap(),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::Forbidden)
        );
        let mut wrong = query.fields().clone();
        wrong.generation += 1;
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(wrong).unwrap(),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::Conflict)
        );
        let mut wrong = job_query(1, false, 2, f.memory.generation())
            .fields()
            .clone();
        wrong.representation.as_mut().unwrap().artifact_sha256 = [9; 32];
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(wrong).unwrap(),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::IncompatibleProfile)
        );
        let (mut other, _, _, _) = data();
        assert_eq!(
            f.memory
                .query(&mut other, &f.session, &query, &mut c())
                .err(),
            Some(MemoryStoreError::Forbidden)
        );
        let mut bad = claim(pin(&mut f.data, &f.ds, "en-owner"));
        bad.id = id("changed-pin");
        bad.sources[0].put_digest = [8; 32];
        let before = f.backing.writes.get();
        let generation = f.memory.generation();
        assert_eq!(
            f.memory
                .propose(
                    &mut f.data,
                    &f.session,
                    generation,
                    None,
                    bad,
                    None,
                    r("extra:0:1"),
                    &mut c()
                )
                .err(),
            Some(MemoryStoreError::Forbidden)
        );
        assert_eq!(before, f.backing.writes.get());
        f.clock.0.set(101);
        assert!(
            f.memory
                .query(&mut f.data, &f.session, &query, &mut c())
                .is_err()
        );
    }
    #[test]
    fn current_deletion_invalidates_cached_query_and_cannot_rewrite_a_conflict() {
        let mut f = fixture();
        let query = job_query(1, false, 0, f.memory.generation());
        let cached = f.memory.cache(&query).unwrap();
        let generation = f.memory.generation();
        f.memory
            .tombstone(
                &f.memory_owner,
                &mut f.data,
                &f.session,
                generation,
                &id("en-owner"),
                rev(2),
                &mut c(),
            )
            .unwrap();
        assert_eq!(
            f.memory
                .query_cached(&mut f.data, &f.session, &cached, &mut c())
                .err(),
            Some(MemoryStoreError::Conflict)
        );
        let result = f
            .memory
            .query(
                &mut f.data,
                &f.session,
                &job_query(1, false, 0, f.memory.generation()),
                &mut c(),
            )
            .unwrap();
        assert_eq!(result.status(), MemoryStatus::NoEvidence);
    }
    #[test]
    fn over_sixteen_eligible_indexed_projections_is_explicit_limit() {
        let mut f = fixture();
        let mut ids: Vec<_> = [
            "en-owner",
            "en-alt",
            "review",
            "m:pump-a:2",
            "m:en-owner:2",
            "m:en-alt:2",
            "m:en-check:2",
        ]
        .iter()
        .map(|s| id(s))
        .collect();
        for i in 0..14 {
            ids.extend([id(&format!("extra:{i}:1")), id(&format!("extra:{i}:2"))]);
        }
        f.ds = f
            .data
            .enroll(
                &f.owner,
                DataGrant {
                    permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                        .unwrap(),
                    scope: ids,
                    expires_at: 1000,
                },
            )
            .unwrap();
        f.session = f
            .memory
            .enroll(&f.memory_owner, &mut f.data, &f.ds, &mut c())
            .unwrap();
        for i in 0..14 {
            let mut d = claim(pin(&mut f.data, &f.ds, "en-owner"));
            d.id = id(&format!("extra-claim:{i}"));
            d.predicate = Some(SemanticProfileId::new("checklist").unwrap());
            d.object = Some(TypedObject::Text(format!("X{i}")));
            add(&mut f, i, d);
        }
        let mut query = job_query(1, false, 2, f.memory.generation())
            .fields()
            .clone();
        query.result_predicates.clear();
        query.effective_at = 80;
        let mut control = c();
        assert_eq!(
            f.memory
                .query(
                    &mut f.data,
                    &f.session,
                    &Query::new(query).unwrap(),
                    &mut control
                )
                .err(),
            Some(MemoryStoreError::Limit)
        );
        assert!(
            control.work_used() < 95_000_000,
            "index cardinality must reject before payload copying exhausts the work budget"
        );
    }

    #[test]
    fn entity_object_join_requires_current_target_authority() {
        let mut f = fixture();
        let mut draft = claim(pin(&mut f.data, &f.ds, "en-owner"));
        draft.id = id("entity-object");
        draft.predicate = Some(SemanticProfileId::new("checklist").unwrap());
        draft.object = Some(TypedObject::Entity(id("pompa-b")));
        add(&mut f, 0, draft);
        let target = r(&projection_id("pompa-b", 2));
        let revision = f.data.inspect(&f.owner, &target).unwrap().revision;
        f.data.withdraw(&f.owner, &target, revision).unwrap();
        let mut q = job_query(1, false, 0, f.memory.generation())
            .fields()
            .clone();
        q.result_predicates = vec![SemanticProfileId::new("checklist").unwrap()];
        let result = f
            .memory
            .query(&mut f.data, &f.session, &Query::new(q).unwrap(), &mut c())
            .unwrap();
        assert_eq!(result.record_count(), 1);
        assert_eq!(result.records()[0].draft.id.as_str(), "en-check");
    }
    #[test]
    fn path_conflicts_include_links_outside_result_predicates() {
        use eylema_contracts::{memory::durable::*, platform::SemanticProfileId};
        use eylema_host::memory_store::*;

        for cardinality in [Cardinality::SingleValue, Cardinality::SetValue] {
            let mut f = fixture_with_edges(cardinality);
            let mut draft = claim(pin(&mut f.data, &f.ds, "en-site"));
            draft.id = id("second-link");
            draft.kind = RecordKind::Relation;
            draft.subject = Some(id("north"));
            draft.predicate = Some(SemanticProfileId::new("contains").unwrap());
            draft.object = Some(TypedObject::Entity(id("north")));
            add(&mut f, 0, draft);
            for mode in 0..4 {
                let mut query = super::briefs::job_query(3, false, mode, f.memory.generation())
                    .fields()
                    .clone();
                query.result_predicates = vec![SemanticProfileId::new("owner").unwrap()];
                let context = f
                    .memory
                    .query(
                        &mut f.data,
                        &f.session,
                        &Query::new(query.clone()).unwrap(),
                        &mut c(),
                    )
                    .unwrap();
                assert_eq!(
                    context.status(),
                    if cardinality == Cardinality::SingleValue {
                        MemoryStatus::Conflict
                    } else {
                        MemoryStatus::Evidence
                    }
                );
                let ids: std::collections::BTreeSet<_> = context
                    .records()
                    .iter()
                    .map(|r| r.draft.id.as_str())
                    .collect();
                assert_eq!(
                    ids,
                    ["en-link", "second-link", "en-owner"].into_iter().collect()
                );
                query.max_hits = 2;
                assert_eq!(
                    f.memory
                        .query(
                            &mut f.data,
                            &f.session,
                            &Query::new(query.clone()).unwrap(),
                            &mut c()
                        )
                        .err(),
                    Some(if cardinality == Cardinality::SingleValue {
                        MemoryStoreError::IncompleteConflict
                    } else {
                        MemoryStoreError::Limit
                    })
                );
                query.max_hits = 16;
                query.max_bytes = 32;
                assert!(matches!(
                    f.memory
                        .query(
                            &mut f.data,
                            &f.session,
                            &Query::new(query).unwrap(),
                            &mut c()
                        )
                        .err(),
                    Some(MemoryStoreError::IncompleteConflict | MemoryStoreError::Limit)
                ));
            }
        }
    }
}
