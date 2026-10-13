use eylema_contracts::{
    Revision,
    codec::{self, Value},
    memory::*,
    platform::*,
    project_data::*,
};
use eylema_host::{Owner, memory::*, native::NativeHash, project_data::*};
use eylema_journal::{Journal, Storage};
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
struct Clock(Rc<Cell<u64>>, Rc<Cell<u64>>);
impl DataClock for Clock {
    fn now(&mut self) -> DataResult<u64> {
        let now = self.0.get();
        self.0.set(
            now.checked_add(self.1.get())
                .ok_or(DataError::ClockUnavailable)?,
        );
        Ok(now)
    }
}
type Store = ProjectStore<Memory, NativeHash, Clock>;
fn sid(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn space() -> EmbeddingSpace {
    EmbeddingSpace {
        id: sid("space:v1"),
        model: ModelProfileId::new("model:v1").unwrap(),
        preprocessing: sid("pre:v1"),
        revision: Revision::new(1).unwrap(),
        dimensions: 16,
    }
}
fn p() -> ProjectId {
    ProjectId::new("project:a").unwrap()
}
fn reference(s: &str) -> DataRef {
    DataRef {
        project: p(),
        id: DataId::new(s).unwrap(),
    }
}
fn profile(backend: MemoryBackend) -> MemoryProfile {
    MemoryProfile::new(MemoryProfileDraft {
        id: sid("memory:v1"),
        revision: Revision::new(1).unwrap(),
        schema: sid("utf8:v1"),
        backend,
        kinds: MemoryKind::ALL.to_vec(),
        roles: vec![RoleKind::Reasoning],
        embedding: Some(space()),
    })
    .unwrap()
}
fn fixture() -> (Store, Owner, DataSession, Clock) {
    let m = Memory(Rc::new(RefCell::new(Cursor::new(vec![]))));
    let c = Clock(Rc::new(Cell::new(1000)), Rc::new(Cell::new(0)));
    let j = Journal::create_new(m, NativeHash, project_journal_limits()).unwrap();
    let (mut s, o) = Store::create_new(j, p(), c.clone()).unwrap();
    let session = s
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::new(DataUse::ALL.to_vec()).unwrap(),
                scope: vec![
                    DataId::new("source").unwrap(),
                    DataId::new("second").unwrap(),
                ],
                expires_at: 10000,
            },
        )
        .unwrap();
    (s, o, session, c)
}
fn add(s: &mut Store, o: &Owner, a: &DataSession, id: &str, body: &[u8]) {
    s.collect(
        a,
        DataDraft {
            reference: reference(id),
            kind: DataKind::Artifact,
            parents: vec![],
            payload: body.to_vec(),
            expires_at: 10000,
        },
    )
    .unwrap();
    s.set_policy(
        o,
        &reference(id),
        Revision::new(1).unwrap(),
        DataPermissions::new(vec![DataUse::Execute, DataUse::Collect]).unwrap(),
        false,
    )
    .unwrap();
}
fn source(id: &str, kind: MemoryKind, status: Verification) -> MemorySourceDraft {
    MemorySourceDraft {
        reference: reference(id),
        kind,
        profile: sid("memory:v1"),
        schema: sid("utf8:v1"),
        observed_at: 1000,
        valid_from: 1000,
        expires_at: 9000,
        verification: status,
        confidence_bps: Some(10000),
        embedding: if kind == MemoryKind::Semantic {
            Some(space())
        } else {
            None
        },
    }
}
fn query(kind: MemoryKind) -> MemoryQueryDraft {
    MemoryQueryDraft {
        project: p(),
        profile: sid("memory:v1"),
        revision: Revision::new(1).unwrap(),
        role: RoleKind::Reasoning,
        purpose: DataUse::Execute,
        kinds: vec![kind],
        sources: vec![DataId::new("source").unwrap()],
        embedding: if kind == MemoryKind::Semantic {
            Some(space())
        } else {
            None
        },
        verified_only: true,
        max_bytes: 65536,
        max_tokens: None,
    }
}
fn registry() -> (MemoryRegistry, Owner) {
    MemoryRegistry::new(p(), vec![profile(MemoryBackend::OwnedReferences)]).unwrap()
}
#[test]
fn all_six_kinds_use_source_gates_and_labeled_serialized_context() {
    for kind in MemoryKind::ALL {
        let (mut data, owner, session, _) = fixture();
        add(
            &mut data,
            &owner,
            &session,
            "source",
            "kaynaklı synthetic document".as_bytes(),
        );
        let (mut memory, mo) = registry();
        memory
            .register(
                &mo,
                &mut data,
                &owner,
                source("source", kind, Verification::Observed),
            )
            .unwrap();
        let q = MemoryQuery::new(query(kind)).unwrap();
        let context = memory.assemble(&mut data, &session, &q).unwrap();
        assert_eq!(context.sources, 1);
        assert_eq!(context.tokens, None);
        assert!(context.bytes.len() <= q.fields().max_bytes);
        let v = codec::decode(&context.bytes).unwrap();
        assert!(matches!(v, Value::List(_)));
    }
}
#[test]
fn cached_requests_recheck_revocation_expiry_and_never_store_grants() {
    let (mut d, o, s, c) = fixture();
    add(&mut d, &o, &s, "source", b"source body");
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Episodic, Verification::Observed),
    )
    .unwrap();
    let q = MemoryQuery::new(query(MemoryKind::Episodic)).unwrap();
    let cache = m.cache(&q).unwrap();
    assert!(m.assemble_cached(&mut d, &s, &cache).is_ok());
    d.set_policy(
        &o,
        &reference("source"),
        Revision::new(2).unwrap(),
        DataPermissions::NONE,
        false,
    )
    .unwrap();
    assert!(matches!(
        m.assemble_cached(&mut d, &s, &cache),
        Err(MemoryError::Data(DataError::Forbidden))
    ));
    d.set_policy(
        &o,
        &reference("source"),
        Revision::new(3).unwrap(),
        DataPermissions::new(vec![DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
    c.0.set(9000);
    assert_eq!(
        m.assemble_cached(&mut d, &s, &cache),
        Err(MemoryError::Expired)
    );
}
#[test]
fn owner_project_role_purpose_profile_and_prediction_denials() {
    let (mut d, o, s, _) = fixture();
    add(&mut d, &o, &s, "source", b"hypothesis");
    let (mut m, mo) = registry();
    let (_, foreign) = registry();
    assert!(matches!(
        m.register(
            &foreign,
            &mut d,
            &o,
            source("source", MemoryKind::Semantic, Verification::Predicted)
        ),
        Err(MemoryError::Forbidden)
    ));
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Semantic, Verification::Predicted),
    )
    .unwrap();
    let mut q = query(MemoryKind::Semantic);
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q.clone()).unwrap()),
        Err(MemoryError::Unverified)
    );
    q.verified_only = false;
    assert!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q.clone()).unwrap())
            .is_ok()
    );
    q.role = RoleKind::Decision;
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q.clone()).unwrap()),
        Err(MemoryError::Forbidden)
    );
    q = query(MemoryKind::Semantic);
    q.verified_only = false;
    q.purpose = DataUse::Train;
    assert!(matches!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap()),
        Err(MemoryError::Data(DataError::Forbidden))
    ));
    q = query(MemoryKind::Semantic);
    q.project = ProjectId::new("other").unwrap();
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap()),
        Err(MemoryError::WrongProject)
    );
    q = query(MemoryKind::Semantic);
    q.revision = Revision::new(2).unwrap();
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap()),
        Err(MemoryError::IncompatibleProfile)
    );
}
#[test]
fn stale_space_missing_provenance_and_invalid_context_reject() {
    let (mut d, o, s, _) = fixture();
    add(&mut d, &o, &s, "source", b"valid");
    let (mut m, mo) = registry();
    let mut missing = source("second", MemoryKind::Semantic, Verification::Observed);
    assert!(m.register(&mo, &mut d, &o, missing.clone()).is_err());
    missing = source("source", MemoryKind::Semantic, Verification::Observed);
    missing.embedding.as_mut().unwrap().model = ModelProfileId::new("other:model").unwrap();
    assert!(matches!(
        m.register(&mo, &mut d, &o, missing),
        Err(MemoryError::IncompatibleProfile)
    ));
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Semantic, Verification::Observed),
    )
    .unwrap();
    let mut q = query(MemoryKind::Semantic);
    q.embedding.as_mut().unwrap().preprocessing = sid("pre:v2");
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap()),
        Err(MemoryError::IncompatibleProfile)
    );
    q = query(MemoryKind::Semantic);
    q.max_bytes = 1;
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap()),
        Err(MemoryError::BudgetExceeded)
    );
}
#[test]
fn unsupported_engines_token_limits_and_empty_hits_are_explicit() {
    let (mut d, o, s, _) = fixture();
    for backend in [
        MemoryBackend::OwnedGraph,
        MemoryBackend::OwnedAnalytical,
        MemoryBackend::SqliteReference,
        MemoryBackend::DuckdbReference,
    ] {
        let (m, _) = MemoryRegistry::new(p(), vec![profile(backend)]).unwrap();
        assert_eq!(
            m.assemble(
                &mut d,
                &s,
                &MemoryQuery::new(query(MemoryKind::Working)).unwrap()
            ),
            Err(MemoryError::BackendUnavailable)
        );
    }
    add(&mut d, &o, &s, "source", b"body");
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Working, Verification::Observed),
    )
    .unwrap();
    let mut q = query(MemoryKind::Working);
    q.max_tokens = Some(1);
    assert_eq!(
        m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap()),
        Err(MemoryError::TokenCounterUnavailable)
    );
    q = query(MemoryKind::Working);
    q.sources.clear();
    let ctx = m
        .assemble(&mut d, &s, &MemoryQuery::new(q).unwrap())
        .unwrap();
    assert_eq!(ctx.sources, 0);
}
#[test]
fn equal_ids_and_bytes_in_another_live_store_cannot_bypass_source_withdrawal() {
    let (mut d, o, s, _) = fixture();
    add(&mut d, &o, &s, "source", b"identical");
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Working, Verification::Observed),
    )
    .unwrap();
    let (mut other, other_owner, other_session, _) = fixture();
    add(
        &mut other,
        &other_owner,
        &other_session,
        "source",
        b"identical",
    );
    d.withdraw(&o, &reference("source"), Revision::new(2).unwrap())
        .unwrap();
    let q = MemoryQuery::new(query(MemoryKind::Working)).unwrap();
    assert_eq!(
        m.assemble(&mut other, &other_session, &q),
        Err(MemoryError::Forbidden)
    );
}

#[test]
fn unauthorized_session_cannot_probe_private_verification_status() {
    let (mut d, o, s, _) = fixture();
    add(&mut d, &o, &s, "source", b"predicted private source");
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Working, Verification::Predicted),
    )
    .unwrap();
    let no = d
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::NONE,
                scope: vec![],
                expires_at: 10000,
            },
        )
        .unwrap();
    let q = MemoryQuery::new(query(MemoryKind::Working)).unwrap();
    assert_eq!(
        m.assemble(&mut d, &no, &q),
        Err(MemoryError::Data(DataError::Forbidden))
    );
}
#[test]
fn source_ttl_is_rechecked_as_trusted_time_advances_during_assembly() {
    let (mut d, o, s, c) = fixture();
    add(&mut d, &o, &s, "source", b"short lived");
    let (mut m, mo) = registry();
    let mut item = source("source", MemoryKind::Working, Verification::Observed);
    item.expires_at = 3000;
    m.register(&mo, &mut d, &o, item).unwrap();
    c.1.set(1000);
    let q = MemoryQuery::new(query(MemoryKind::Working)).unwrap();
    assert_eq!(m.assemble(&mut d, &s, &q), Err(MemoryError::Expired));
}
#[test]
fn invalid_utf8_foreign_sessions_future_validity_and_cache_identity_fail() {
    let (mut d, o, s, c) = fixture();
    add(&mut d, &o, &s, "source", &[0xff]);
    let (mut m, mo) = registry();
    let mut item = source("source", MemoryKind::Working, Verification::Observed);
    item.valid_from = 2000;
    m.register(&mo, &mut d, &o, item.clone()).unwrap();
    let q = MemoryQuery::new(query(MemoryKind::Working)).unwrap();
    assert_eq!(m.assemble(&mut d, &s, &q), Err(MemoryError::NotYetValid));
    c.0.set(2000);
    assert_eq!(m.assemble(&mut d, &s, &q), Err(MemoryError::InvalidPayload));
    let (_, _, foreign, _) = fixture();
    assert_eq!(
        m.assemble(&mut d, &foreign, &q),
        Err(MemoryError::Data(DataError::Forbidden))
    );
    let cache = m.cache(&q).unwrap();
    let (other, _) = registry();
    assert_eq!(
        other.assemble_cached(&mut d, &s, &cache),
        Err(MemoryError::Forbidden)
    );
    assert!(matches!(
        m.register(&mo, &mut d, &o, item),
        Err(MemoryError::InvalidRequest)
    ));
    let mut future = source("second", MemoryKind::Working, Verification::Observed);
    future.observed_at = 3000;
    add(&mut d, &o, &s, "second", b"future");
    assert_eq!(
        m.register(&mo, &mut d, &o, future),
        Err(MemoryError::NotYetValid)
    );
}
#[test]
fn serialized_overhead_and_provenance_are_counted_and_debug_is_opaque() {
    let (mut d, o, s, _) = fixture();
    let body = vec![b'a'; 65536];
    add(&mut d, &o, &s, "source", &body);
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Working, Verification::Reviewed),
    )
    .unwrap();
    let q = MemoryQuery::new(query(MemoryKind::Working)).unwrap();
    assert_eq!(m.assemble(&mut d, &s, &q), Err(MemoryError::BudgetExceeded));
    let (mut d, o, s, _) = fixture();
    add(
        &mut d,
        &o,
        &s,
        "source",
        b"private content must not be a debug dump",
    );
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Working, Verification::Reviewed),
    )
    .unwrap();
    let context = m.assemble(&mut d, &s, &q).unwrap();
    assert!(!format!("{context:?}").contains("private content"));
    let Value::List(root) = codec::decode(&context.bytes).unwrap() else {
        panic!("context envelope")
    };
    assert_eq!(root[5], Value::Text("reasoning".into()));
    assert_eq!(root[6], Value::Text("execute".into()));
    let Value::List(entries) = &root[8] else {
        panic!("source entries")
    };
    let Value::List(entry) = &entries[0] else {
        panic!("source entry")
    };
    assert_eq!(entry[0], Value::Text("project:a".into()));
    assert_eq!(entry[1], Value::Text("source".into()));
    assert!(matches!(&entry[2],Value::Bytes(v) if v.len()==32));
    assert_eq!(entry[10], Value::Text("reviewed".into()));
    assert_eq!(entry[13], Value::List(vec![]));
    assert_eq!(entry[14], Value::Unsigned(2));
}
#[test]
fn profile_and_registered_source_limits_hold_at_the_boundary() {
    let profiles: Vec<MemoryProfile> = (0..128)
        .map(|n| {
            let mut p = profile(MemoryBackend::OwnedReferences).fields().clone();
            p.id = sid(&format!("p{n}"));
            MemoryProfile::new(p).unwrap()
        })
        .collect();
    assert!(MemoryRegistry::new(p(), profiles.clone()).is_ok());
    let mut extra = profiles;
    extra.push(profile(MemoryBackend::OwnedReferences));
    assert!(matches!(
        MemoryRegistry::new(p(), extra),
        Err(MemoryError::BudgetExceeded)
    ));
    assert!(matches!(
        MemoryRegistry::new(p(), vec![profile(MemoryBackend::OwnedReferences); 2]),
        Err(MemoryError::InvalidRequest)
    ));
    let (mut d, o, _, _) = fixture();
    let (mut m, mo) = registry();
    for group in 0..3 {
        let ids: Vec<DataId> = (group * 64..((group + 1) * 64).min(129))
            .map(|n| DataId::new(format!("item{n}")).unwrap())
            .collect();
        let a = d
            .enroll(
                &o,
                DataGrant {
                    permissions: DataPermissions::new(DataUse::ALL.to_vec()).unwrap(),
                    scope: ids.clone(),
                    expires_at: 10000,
                },
            )
            .unwrap();
        for id in ids {
            let r = DataRef {
                project: p(),
                id: id.clone(),
            };
            d.collect(
                &a,
                DataDraft {
                    reference: r.clone(),
                    kind: DataKind::Artifact,
                    parents: vec![],
                    payload: b"x".to_vec(),
                    expires_at: 10000,
                },
            )
            .unwrap();
            let mut item = source("source", MemoryKind::Working, Verification::Observed);
            item.reference = r;
            let result = m.register(&mo, &mut d, &o, item);
            if id.as_str() == "item128" {
                assert_eq!(result, Err(MemoryError::BudgetExceeded));
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn multi_source_expiry_uses_common_handoff_time() {
    let (mut d, o, s, c) = fixture();
    add(&mut d, &o, &s, "source", b"first");
    add(&mut d, &o, &s, "second", b"second");
    let (mut m, mo) = registry();
    let mut first = source("source", MemoryKind::Working, Verification::Observed);
    first.expires_at = 1900;
    m.register(&mo, &mut d, &o, first).unwrap();
    m.register(
        &mo,
        &mut d,
        &o,
        source("second", MemoryKind::Working, Verification::Observed),
    )
    .unwrap();
    let mut q = query(MemoryKind::Working);
    q.sources.push(DataId::new("second").unwrap());
    c.1.set(100);
    let result = m.assemble(&mut d, &s, &MemoryQuery::new(q).unwrap());
    assert_eq!(
        result,
        Err(MemoryError::Expired),
        "last observed time={}, first TTL=1900",
        c.0.get() - 100
    );
}
#[test]
fn session_expiry_uses_common_handoff_time() {
    let (mut d, o, s, c) = fixture();
    add(&mut d, &o, &s, "source", b"private");
    let a = d
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Execute]).unwrap(),
                scope: vec![DataId::new("source").unwrap()],
                expires_at: 1500,
            },
        )
        .unwrap();
    let (mut m, mo) = registry();
    m.register(
        &mo,
        &mut d,
        &o,
        source("source", MemoryKind::Working, Verification::Observed),
    )
    .unwrap();
    c.1.set(100);
    let result = m.assemble(
        &mut d,
        &a,
        &MemoryQuery::new(query(MemoryKind::Working)).unwrap(),
    );
    assert_eq!(
        result,
        Err(MemoryError::Data(DataError::Expired)),
        "last observed time={}, session TTL=1500",
        c.0.get() - 100
    );
}

#[test]
fn backing_source_and_ancestry_expiry_use_common_handoff_time() {
    let (mut d, o, session, clock) = fixture();
    for (id, parents) in [("second", vec![]), ("source", vec![reference("second")])] {
        d.collect(
            &session,
            DataDraft {
                reference: reference(id),
                kind: if parents.is_empty() {
                    DataKind::Artifact
                } else {
                    DataKind::Derived
                },
                parents,
                payload: b"eligible".to_vec(),
                expires_at: 1500,
            },
        )
        .unwrap();
        d.set_policy(
            &o,
            &reference(id),
            Revision::new(1).unwrap(),
            DataPermissions::new(vec![DataUse::Execute, DataUse::Collect]).unwrap(),
            false,
        )
        .unwrap();
    }
    let (mut m, owner) = registry();
    let mut draft = source("source", MemoryKind::Working, Verification::Observed);
    draft.expires_at = 1500;
    m.register(&owner, &mut d, &o, draft).unwrap();
    let query = MemoryQuery::new(query(MemoryKind::Working)).unwrap();
    assert!(m.assemble(&mut d, &session, &query).is_ok());
    clock.1.set(100);
    assert_eq!(
        m.assemble(&mut d, &session, &query),
        Err(MemoryError::Data(DataError::Expired))
    );
}
