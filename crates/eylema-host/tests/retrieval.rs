use eylema_contracts::{
    Revision,
    codec::{self, Value},
    memory::EmbeddingSpace,
    platform::{ModelProfileId, ProjectId, SemanticProfileId},
    project_data::*,
};
use eylema_host::{Owner, native::NativeHash, project_data::*, retrieval::*};
use eylema_journal::{Journal, Storage};
use eylema_retrieval::*;
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Mem(
    pub Rc<RefCell<Cursor<Vec<u8>>>>,
    pub Rc<Cell<bool>>,
    pub Option<Rc<Cell<u64>>>,
);
impl Mem {
    pub fn new() -> Self {
        Self(
            Rc::new(RefCell::new(Cursor::new(vec![]))),
            Rc::new(Cell::new(false)),
            None,
        )
    }
}
impl Read for Mem {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.borrow_mut().read(b)
    }
}
impl Write for Mem {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for Mem {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.0.borrow_mut().seek(p)
    }
}
impl Storage for Mem {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.0.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        if self.1.replace(false) {
            if let Some(clock) = &self.2 {
                clock.set(20000);
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Clock(pub Rc<Cell<u64>>, pub Rc<Cell<u64>>);
impl DataClock for Clock {
    fn now(&mut self) -> DataResult<u64> {
        let n = self.0.get();
        self.0.set(n + self.1.get());
        Ok(n)
    }
}
pub fn c() -> Control {
    Control::new(
        RetrievalBudget {
            max_work: 100_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(30),
        },
        CancelToken::new(),
    )
    .unwrap()
}
pub fn p() -> ProjectId {
    ProjectId::new("project:a").unwrap()
}
pub fn r(s: &str) -> DataRef {
    DataRef {
        project: p(),
        id: DataId::new(s).unwrap(),
    }
}
pub fn binding() -> RepresentationBinding {
    RepresentationBinding {
        space: EmbeddingSpace {
            id: SemanticProfileId::new("space:a").unwrap(),
            model: ModelProfileId::new("model:a").unwrap(),
            preprocessing: SemanticProfileId::new("pre:a").unwrap(),
            revision: Revision::new(1).unwrap(),
            dimensions: 1024,
        },
        manifest_sha256: [1; 32],
        artifact_sha256: [2; 32],
        tokenizer_sha256: [3; 32],
        preprocessing_sha256: [4; 32],
        chunk_profile: "utf8-window1024-v1".into(),
        lexical_profile: "unicode-alnum-lower-rust199-v1".into(),
        score_profile: "scalar-micro-half-even-v1".into(),
    }
}
pub fn grant() -> DataGrant {
    DataGrant {
        permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
        scope: ["a", "b", "c", "d", "e", "missing"]
            .into_iter()
            .map(|s| r(s).id)
            .collect(),
        expires_at: 10000,
    }
}
pub fn sources() -> Vec<SourceVectors> {
    [("a", 1., 0.), ("b", 0., 1.), ("c", -1., 0.), ("d", 1., 0.)]
        .into_iter()
        .map(|(id, x, y)| {
            let mut v = vec![0.; 1024];
            v[0] = x;
            v[1] = y;
            SourceVectors {
                reference: r(id),
                vectors: vec![v],
            }
        })
        .collect()
}
pub fn q() -> SearchQuery {
    let mut v = vec![0.; 1024];
    v[0] = 1.;
    SearchQuery {
        project: p(),
        binding: binding(),
        generation: 1,
        sources: ["a", "b", "c", "d"].into_iter().map(r).collect(),
        input: QueryInput::Vector(v),
        top_k: 1,
        max_hits: 2,
        max_bytes: 65536,
        max_tokens: None,
    }
}
pub type Store = ProjectStore<Mem, NativeHash, Clock>;
pub fn fixture() -> (Store, Owner, DataSession, Clock, Mem) {
    let m = Mem::new();
    let clock = Clock(Rc::new(Cell::new(1000)), Rc::new(Cell::new(0)));
    let j = Journal::create_new(m.clone(), NativeHash, project_journal_limits()).unwrap();
    let (mut d, o) = ProjectStore::create_new(j, p(), clock.clone()).unwrap();
    let s = d.enroll(&o, grant()).unwrap();
    for (id, text) in [
        ("a", "apple red"),
        ("b", "apple green"),
        ("c", "ocean blue"),
        ("d", "apple red"),
    ] {
        d.collect(
            &s,
            DataDraft {
                reference: r(id),
                kind: DataKind::Artifact,
                parents: vec![],
                payload: text.as_bytes().to_vec(),
                expires_at: 10000,
            },
        )
        .unwrap();
        d.set_policy(
            &o,
            &r(id),
            Revision::new(1).unwrap(),
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
    }
    (d, o, s, clock, m)
}
// Removing live identity or current permission checks must fail these assertions.
#[test]
fn current_authority_allowlist_revocation_and_reopen() {
    let (mut d, o, s, clock, m) = fixture();
    let index = Mem::new();
    let (mut h, owner) = RetrievalHost::create(
        index.clone(),
        NativeHash,
        NativeHash,
        &mut d,
        &o,
        &s,
        &binding(),
        sources(),
        &mut c(),
    )
    .unwrap();
    assert_eq!(h.search(&mut d, &s, &q(), &mut c()).unwrap().hit_count(), 2);
    let mut query = q();
    query.sources = vec![r("a")];
    assert_eq!(
        h.search(&mut d, &s, &query, &mut c()).unwrap().hit_count(),
        1
    );
    query.generation = 99;
    d.revoke(&o, &s).unwrap();
    assert!(matches!(
        h.search(&mut d, &s, &query, &mut c()),
        Err(HostRetrievalError::Data(DataError::Forbidden))
    ));
    let s = d.enroll(&o, grant()).unwrap();
    let (mut other, oo, os, _, _) = fixture();
    assert!(matches!(
        h.search(&mut other, &os, &q(), &mut c()),
        Err(HostRetrievalError::Forbidden)
    ));
    let (_, foreign) = RetrievalHost::create(
        Mem::new(),
        NativeHash,
        NativeHash,
        &mut other,
        &oo,
        &os,
        &binding(),
        sources(),
        &mut c(),
    )
    .unwrap();
    assert!(
        h.replace(&mut d, &o, &foreign, &s, 1, &binding(), vec![], &mut c())
            .is_err()
    );
    drop(h);
    drop(d);
    let j = Journal::open(m, NativeHash, project_journal_limits()).unwrap();
    let (mut d, o2) = ProjectStore::open(j, p(), clock).unwrap();
    let s2 = d.enroll(&o2, grant()).unwrap();
    let (h, _) = RetrievalHost::open(
        index,
        NativeHash,
        NativeHash,
        &mut d,
        &o2,
        &s2,
        &binding(),
        &mut c(),
    )
    .unwrap();
    assert!(h.search(&mut d, &s, &q(), &mut c()).is_err());
    assert_eq!(
        h.search(&mut d, &s2, &q(), &mut c()).unwrap().hit_count(),
        2
    );
    let _ = (owner, oo);
}
#[test]
fn synchronized_but_expired_replacement_has_no_ack() {
    let (mut d, o, s, clock, _) = fixture();
    let mut m = Mem::new();
    m.2 = Some(clock.0.clone());
    let (mut h, owner) = RetrievalHost::create(
        m.clone(),
        NativeHash,
        NativeHash,
        &mut d,
        &o,
        &s,
        &binding(),
        sources(),
        &mut c(),
    )
    .unwrap();
    m.1.set(true);
    assert!(
        h.replace(&mut d, &o, &owner, &s, 1, &binding(), sources(), &mut c())
            .is_err()
    );
    clock.0.set(20000);
    assert!(h.search(&mut d, &s, &q(), &mut c()).is_err());
}
#[test]
fn encoded_provenance_scores_are_complete() {
    let (mut d, o, s, _, _) = fixture();
    let (h, _) = RetrievalHost::create(
        Mem::new(),
        NativeHash,
        NativeHash,
        &mut d,
        &o,
        &s,
        &binding(),
        sources(),
        &mut c(),
    )
    .unwrap();
    let resp = h.search(&mut d, &s, &q(), &mut c()).unwrap();
    let Value::List(e) = codec::decode(resp.bytes()).unwrap() else {
        panic!()
    };
    let Value::List(hits) = &e[7] else { panic!() };
    assert_eq!(hits.len(), 2);
    for (id, h) in ["a", "d"].into_iter().zip(hits) {
        let Value::List(v) = h else { panic!() };
        assert_eq!(v[1], Value::Text(id.into()));
        assert_eq!(v[9], Value::Signed(1_000_000));
        let metadata = d.authorized_metadata(&s, &r(id), DataUse::Execute).unwrap();
        assert_eq!(v[2], Value::Bytes(metadata.digest.to_vec()));
    }
}
#[test]
fn final_common_time_rejects_earlier_sources_expired_during_staging() {
    let (mut d, o, s, clock, _) = fixture();
    let (h, _) = RetrievalHost::create(
        Mem::new(),
        NativeHash,
        NativeHash,
        &mut d,
        &o,
        &s,
        &binding(),
        sources(),
        &mut c(),
    )
    .unwrap();
    clock.1.set(2500);
    assert!(matches!(
        h.search(&mut d, &s, &q(), &mut c()),
        Err(HostRetrievalError::Data(DataError::Expired))
    ));
}
#[test]
fn ancestor_policy_and_terminal_withdrawal_are_current() {
    let (mut d, o, s, _, _) = fixture();
    d.collect(
        &s,
        DataDraft {
            reference: r("e"),
            kind: DataKind::Derived,
            parents: vec![r("a")],
            payload: b"apple red".to_vec(),
            expires_at: 10000,
        },
    )
    .unwrap();
    d.set_policy(
        &o,
        &r("e"),
        Revision::new(1).unwrap(),
        DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
    let mut src = sources();
    let mut e = src.remove(0);
    e.reference = r("e");
    let (h, _) = RetrievalHost::create(
        Mem::new(),
        NativeHash,
        NativeHash,
        &mut d,
        &o,
        &s,
        &binding(),
        vec![e],
        &mut c(),
    )
    .unwrap();
    let mut query = q();
    query.sources = vec![r("e")];
    assert_eq!(
        h.search(&mut d, &s, &query, &mut c()).unwrap().hit_count(),
        1
    );
    d.set_policy(
        &o,
        &r("a"),
        Revision::new(2).unwrap(),
        DataPermissions::NONE,
        false,
    )
    .unwrap();
    assert!(matches!(
        h.search(&mut d, &s, &query, &mut c()),
        Err(HostRetrievalError::Data(DataError::Forbidden))
    ));
    let m = d.inspect(&o, &r("e")).unwrap();
    d.withdraw(&o, &r("e"), m.revision).unwrap();
    assert!(h.search(&mut d, &s, &query, &mut c()).is_err());
}
#[test]
fn denial_precedes_index_status_and_no_partial_results() {
    let (mut d, o, s, _, _) = fixture();
    let (h, _) = RetrievalHost::create(
        Mem::new(),
        NativeHash,
        NativeHash,
        &mut d,
        &o,
        &s,
        &binding(),
        sources(),
        &mut c(),
    )
    .unwrap();
    let denied = d
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect]).unwrap(),
                scope: grant().scope,
                expires_at: 10000,
            },
        )
        .unwrap();
    for id in ["a", "missing"] {
        let mut query = q();
        query.sources = vec![r(id)];
        query.generation = 99;
        assert!(matches!(
            h.search(&mut d, &denied, &query, &mut c()),
            Err(HostRetrievalError::Data(DataError::Forbidden))
        ));
    }
    let mut query = q();
    query.max_tokens = Some(1);
    assert!(matches!(
        h.search(&mut d, &s, &query, &mut c()),
        Err(HostRetrievalError::Core(
            RetrievalError::TokenCounterUnavailable
        ))
    ));
    query = q();
    query.max_bytes = 1;
    assert!(matches!(
        h.search(&mut d, &s, &query, &mut c()),
        Err(HostRetrievalError::Core(RetrievalError::LimitExceeded))
    ));
}
