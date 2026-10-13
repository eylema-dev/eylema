use eylema_contracts::{Revision, platform::ProjectId, project_data::*};
use eylema_host::Owner;
use eylema_host::project_data::*;
use eylema_journal::{Journal, Limits, Sha256, Storage};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
#[derive(Clone)]
struct Memory {
    bytes: Rc<RefCell<Cursor<Vec<u8>>>>,
    fail_sync: Rc<Cell<bool>>,
}
impl Memory {
    fn new() -> Self {
        Self {
            bytes: Rc::new(RefCell::new(Cursor::new(vec![]))),
            fail_sync: Rc::new(Cell::new(false)),
        }
    }
}
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.bytes.borrow_mut().read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.bytes.borrow_mut().write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for Memory {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.bytes.borrow_mut().seek(p)
    }
}
impl Storage for Memory {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.bytes.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        if self.fail_sync.get() {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
struct Hash;
impl Sha256 for Hash {
    fn digest(&mut self, b: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        eylema_platform::sha256(b).map_err(|_| eylema_journal::JournalError::HashUnavailable)
    }
}
#[derive(Clone)]
struct Clock {
    value: Rc<Cell<u64>>,
    fail: Rc<Cell<bool>>,
}
impl DataClock for Clock {
    fn now(&mut self) -> DataResult<u64> {
        if self.fail.get() {
            Err(DataError::ClockUnavailable)
        } else {
            Ok(self.value.get())
        }
    }
}
type Store = ProjectStore<Memory, Hash, Clock>;
fn project(s: &str) -> ProjectId {
    ProjectId::new(s).unwrap()
}
fn reference(s: &str) -> DataRef {
    DataRef {
        project: project("project:a"),
        id: DataId::new(s).unwrap(),
    }
}
fn permissions(p: &[DataUse]) -> DataPermissions {
    DataPermissions::new(p.to_vec()).unwrap()
}
fn fixture() -> (Store, Owner, Memory, Clock) {
    let m = Memory::new();
    let c = Clock {
        value: Rc::new(Cell::new(1000)),
        fail: Rc::new(Cell::new(false)),
    };
    let j = Journal::create_new(m.clone(), Hash, project_journal_limits()).unwrap();
    let (s, o) = Store::create_new(j, project("project:a"), c.clone()).unwrap();
    (s, o, m, c)
}
fn session(s: &mut Store, o: &Owner, ids: &[&str], uses: &[DataUse]) -> DataSession {
    s.enroll(
        o,
        DataGrant {
            permissions: permissions(uses),
            scope: ids.iter().map(|id| DataId::new(*id).unwrap()).collect(),
            expires_at: 100_000,
        },
    )
    .unwrap()
}
fn draft(id: &str, parents: Vec<DataRef>) -> DataDraft {
    DataDraft {
        reference: reference(id),
        kind: if parents.is_empty() {
            DataKind::Artifact
        } else {
            DataKind::Derived
        },
        parents,
        payload: b"synthetic project payload".to_vec(),
        expires_at: 10_000,
    }
}
fn allow(s: &mut Store, o: &Owner, id: &str, uses: &[DataUse], reviewed: bool) -> DataMetadata {
    let rev = s.inspect(o, &reference(id)).unwrap().revision;
    s.set_policy(o, &reference(id), rev, permissions(uses), reviewed)
        .unwrap()
}
#[test]
fn default_and_five_purposes_are_independent() {
    for purpose in DataUse::ALL {
        let (mut s, o, _, _) = fixture();
        let collector = session(&mut s, &o, &["root"], &[DataUse::Collect]);
        s.collect(&collector, draft("root", vec![])).unwrap();
        let empty = session(&mut s, &o, &["root"], &[]);
        assert_eq!(
            s.read(&empty, &reference("root"), DataUse::Execute),
            Err(DataError::Forbidden)
        );
        let active = session(&mut s, &o, &["root"], &[purpose]);
        allow(&mut s, &o, "root", &[purpose], true);
        for candidate in DataUse::ALL {
            if matches!(
                candidate,
                DataUse::ExternalDisclosure | DataUse::EcosystemExport
            ) {
                assert_eq!(
                    s.export_manifest(&active, &reference("root"), candidate)
                        .is_ok(),
                    candidate == purpose
                );
            } else {
                assert_eq!(
                    s.read(&active, &reference("root"), candidate).is_ok(),
                    candidate == purpose
                );
            }
        }
    }
}
#[test]
fn private_egress_is_denied_and_projection_has_no_payload() {
    let (mut s, o, _, _) = fixture();
    let a = session(&mut s, &o, &["root"], &DataUse::ALL);
    s.collect(&a, draft("root", vec![])).unwrap();
    allow(&mut s, &o, "root", &DataUse::ALL, false);
    assert_eq!(
        s.export_manifest(&a, &reference("root"), DataUse::EcosystemExport),
        Err(DataError::Forbidden)
    );
    allow(&mut s, &o, "root", &DataUse::ALL, true);
    let m = s
        .export_manifest(&a, &reference("root"), DataUse::EcosystemExport)
        .unwrap();
    assert_eq!(m.reference, reference("root"));
    assert_eq!(m.kind, DataKind::Artifact);
    assert!(m.parents.is_empty());
    assert!(!format!("{m:?}").contains("synthetic project payload"));
    assert_eq!(
        s.read(&a, &reference("root"), DataUse::ExternalDisclosure),
        Err(DataError::Forbidden)
    );
    for kind in ["raw_reasoning", "credentials", "unknown"] {
        assert!(DataKind::parse(kind).is_err());
    }
    assert!(DataPermissions::from_bits(32).is_err());
    assert!(DataPermissions::new(vec![DataUse::Execute, DataUse::Execute]).is_err());
}
#[test]
fn foreign_owner_session_scope_and_cross_project_import_fail() {
    let (mut s, o, _, _) = fixture();
    let (mut other, foreign, _, _) = fixture();
    let a = session(&mut s, &o, &["root", "child"], &DataUse::ALL);
    let alien = session(&mut other, &foreign, &["root"], &DataUse::ALL);
    s.collect(&a, draft("root", vec![])).unwrap();
    allow(&mut s, &o, "root", &DataUse::ALL, true);
    assert!(matches!(
        s.enroll(
            &foreign,
            DataGrant {
                permissions: DataPermissions::NONE,
                scope: vec![],
                expires_at: 10000
            }
        ),
        Err(DataError::Forbidden)
    ));
    assert_eq!(
        s.read(&alien, &reference("root"), DataUse::Execute),
        Err(DataError::Forbidden)
    );
    assert!(matches!(
        s.set_policy(
            &foreign,
            &reference("root"),
            Revision::new(2).unwrap(),
            DataPermissions::NONE,
            false
        ),
        Err(DataError::Forbidden)
    ));
    let mut imported = draft("child", vec![]);
    imported.reference.project = project("project:b");
    assert!(matches!(
        s.collect(&a, imported),
        Err(DataError::WrongProject)
    ));
    let mut p = reference("root");
    p.project = project("project:b");
    assert!(matches!(
        s.collect(&a, draft("child", vec![p.clone()])),
        Err(DataError::WrongProject)
    ));
    assert_eq!(
        s.read(&a, &p, DataUse::Execute),
        Err(DataError::WrongProject)
    );
    let no_scope = session(&mut s, &o, &[], &DataUse::ALL);
    assert_eq!(
        s.read(&no_scope, &reference("root"), DataUse::Execute),
        Err(DataError::Forbidden)
    );
    s.revoke(&o, &a).unwrap();
    assert_eq!(
        s.read(&a, &reference("root"), DataUse::Execute),
        Err(DataError::Forbidden)
    );
}
#[test]
fn derivatives_and_trained_candidates_inherit_live_sources() {
    let (mut s, o, _, _) = fixture();
    let a = session(
        &mut s,
        &o,
        &["source", "second", "child", "candidate"],
        &DataUse::ALL,
    );
    for id in ["source", "second"] {
        s.collect(&a, draft(id, vec![])).unwrap();
        allow(&mut s, &o, id, &DataUse::ALL, id == "source");
    }
    s.collect(
        &a,
        draft("child", vec![reference("source"), reference("second")]),
    )
    .unwrap();
    allow(&mut s, &o, "child", &DataUse::ALL, true);
    assert!(s.read(&a, &reference("child"), DataUse::Execute).is_ok());
    assert_eq!(
        s.export_manifest(&a, &reference("child"), DataUse::EcosystemExport),
        Err(DataError::Forbidden)
    );
    allow(&mut s, &o, "second", &DataUse::ALL, true);
    let mut candidate = draft("candidate", vec![reference("child")]);
    candidate.kind = DataKind::TrainingCandidate;
    s.collect(&a, candidate).unwrap();
    allow(&mut s, &o, "candidate", &DataUse::ALL, true);
    assert!(s.read(&a, &reference("candidate"), DataUse::Train).is_ok());
    allow(
        &mut s,
        &o,
        "source",
        &[DataUse::Collect, DataUse::Execute],
        true,
    );
    assert_eq!(
        s.read(&a, &reference("candidate"), DataUse::Train),
        Err(DataError::Forbidden)
    );
    let rev = s.inspect(&o, &reference("source")).unwrap().revision;
    s.withdraw(&o, &reference("source"), rev).unwrap();
    assert_eq!(
        s.read(&a, &reference("candidate"), DataUse::Execute),
        Err(DataError::Forbidden)
    );
    assert!(matches!(
        s.set_policy(
            &o,
            &reference("source"),
            Revision::new(5).unwrap(),
            permissions(&DataUse::ALL),
            true
        ),
        Err(DataError::Forbidden)
    ));
}
#[test]
fn expiry_watermark_reopen_and_clock_fail_closed() {
    let (mut s, o, m, c) = fixture();
    let a = session(&mut s, &o, &["root"], &DataUse::ALL);
    s.collect(&a, draft("root", vec![])).unwrap();
    allow(&mut s, &o, "root", &DataUse::ALL, true);
    c.value.set(9999);
    assert!(s.read(&a, &reference("root"), DataUse::Execute).is_ok());
    c.value.set(10000);
    assert_eq!(
        s.read(&a, &reference("root"), DataUse::Execute),
        Err(DataError::Expired)
    );
    s.close();
    c.value.set(9999);
    let j = Journal::open(m.clone(), Hash, project_journal_limits()).unwrap();
    assert!(matches!(
        Store::open(j, project("project:a"), c.clone()),
        Err(DataError::ClockRollback)
    ));
    c.value.set(10000);
    let j = Journal::open(m, Hash, project_journal_limits()).unwrap();
    let (mut fresh, new) = Store::open(j, project("project:a"), c.clone()).unwrap();
    assert_eq!(
        fresh.read(&a, &reference("root"), DataUse::Execute),
        Err(DataError::Forbidden)
    );
    let b = session(&mut fresh, &new, &["root"], &DataUse::ALL);
    assert_eq!(
        fresh.read(&b, &reference("root"), DataUse::Execute),
        Err(DataError::Expired)
    );
    c.fail.set(true);
    assert_eq!(
        fresh.read(&b, &reference("root"), DataUse::Execute),
        Err(DataError::ClockUnavailable)
    );
}
#[test]
fn malformed_replay_conflicts_and_io_uncertainty_never_grant_access() {
    let (mut s, o, m, _) = fixture();
    let a = session(&mut s, &o, &["root"], &DataUse::ALL);
    s.collect(&a, draft("root", vec![])).unwrap();
    assert!(matches!(
        s.set_policy(
            &o,
            &reference("root"),
            Revision::new(2).unwrap(),
            permissions(&DataUse::ALL),
            true
        ),
        Err(DataError::RevisionConflict)
    ));
    m.fail_sync.set(true);
    assert!(matches!(
        s.set_policy(
            &o,
            &reference("root"),
            Revision::new(1).unwrap(),
            permissions(&DataUse::ALL),
            true
        ),
        Err(DataError::Journal(_))
    ));
    assert_eq!(
        s.read(&a, &reference("root"), DataUse::Execute),
        Err(DataError::Poisoned)
    );
    s.close();
    m.fail_sync.set(false);
    let j = Journal::open(m.clone(), Hash, project_journal_limits()).unwrap();
    assert!(matches!(
        Store::open(
            j,
            project("project:b"),
            Clock {
                value: Rc::new(Cell::new(1000)),
                fail: Rc::new(Cell::new(false))
            }
        ),
        Err(DataError::WrongProject)
    ));
    let mut j = Journal::open(m, Hash, project_journal_limits()).unwrap();
    j.append(&eylema_contracts::codec::Value::List(vec![
        eylema_contracts::codec::Value::Text("unknown".into()),
    ]))
    .unwrap();
    let c = Clock {
        value: Rc::new(Cell::new(1000)),
        fail: Rc::new(Cell::new(false)),
    };
    assert!(matches!(
        Store::open(j, project("project:a"), c),
        Err(DataError::InvalidRecord)
    ));
}
#[test]
fn structural_payload_parent_depth_scope_and_session_bounds() {
    let (mut s, o, _, _) = fixture();
    let ids: Vec<String> = (0..=32).map(|n| format!("r{n}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let a = session(&mut s, &o, &refs, &DataUse::ALL);
    let mut d = draft("r0", vec![]);
    d.payload = vec![];
    assert!(s.collect(&a, d).is_err());
    let mut d = draft("r0", vec![]);
    d.payload = vec![0; MAX_DATA_PAYLOAD_BYTES + 1];
    assert!(s.collect(&a, d).is_err());
    let mut d = draft("r0", vec![]);
    d.expires_at = 1000;
    assert!(s.collect(&a, d).is_err());
    let mut d = draft("r0", vec![]);
    d.expires_at = 1001 + MAX_DATA_TTL_MS;
    assert!(s.collect(&a, d).is_err());
    let mut d = draft("r0", vec![]);
    d.kind = DataKind::TrainingCandidate;
    assert!(s.collect(&a, d).is_err());
    s.collect(&a, draft("r0", vec![])).unwrap();
    allow(&mut s, &o, "r0", &DataUse::ALL, true);
    assert!(s.collect(&a, draft("r0", vec![])).is_err());
    let mut d = draft("r1", vec![reference("r0"), reference("r0")]);
    assert!(s.collect(&a, d).is_err());
    d = draft("r1", vec![reference("missing")]);
    assert!(s.collect(&a, d).is_err());
    d = draft("r1", vec![reference("r1")]);
    assert!(s.collect(&a, d).is_err());
    for i in 1..32 {
        s.collect(&a, draft(&ids[i], vec![reference(&ids[i - 1])]))
            .unwrap();
        allow(&mut s, &o, &ids[i], &DataUse::ALL, true);
    }
    assert!(matches!(
        s.collect(&a, draft(&ids[32], vec![reference(&ids[31])])),
        Err(DataError::BudgetExceeded)
    ));
    assert!(matches!(
        s.enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::NONE,
                scope: vec![DataId::new("same").unwrap(); 2],
                expires_at: 10000
            }
        ),
        Err(DataError::InvalidRecord)
    ));
    assert!(matches!(
        s.enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::NONE,
                scope: (0..65)
                    .map(|n| DataId::new(format!("s{n}")).unwrap())
                    .collect(),
                expires_at: 10000
            }
        ),
        Err(DataError::BudgetExceeded)
    ));
    for _ in 1..64 {
        session(&mut s, &o, &[], &[]);
    }
    assert!(matches!(
        s.enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::NONE,
                scope: vec![],
                expires_at: 10000
            }
        ),
        Err(DataError::BudgetExceeded)
    ));
}
#[test]
fn reopen_preserves_metadata_policy_payload_and_terminal_withdrawal() {
    let (mut s, o, m, c) = fixture();
    let a = session(&mut s, &o, &["source", "child"], &DataUse::ALL);
    s.collect(&a, draft("source", vec![])).unwrap();
    allow(&mut s, &o, "source", &DataUse::ALL, true);
    s.collect(&a, draft("child", vec![reference("source")]))
        .unwrap();
    let before = allow(&mut s, &o, "child", &DataUse::ALL, true);
    let payload = s.read(&a, &reference("child"), DataUse::Train).unwrap();
    s.close();
    let j = Journal::open(m.clone(), Hash, project_journal_limits()).unwrap();
    let (mut s, o) = Store::open(j, project("project:a"), c.clone()).unwrap();
    assert_eq!(s.inspect(&o, &reference("child")).unwrap(), before);
    let b = session(&mut s, &o, &["source", "child"], &DataUse::ALL);
    assert_eq!(
        s.read(&b, &reference("child"), DataUse::Train).unwrap(),
        payload
    );
    assert_eq!(
        s.read(&a, &reference("child"), DataUse::Train),
        Err(DataError::Forbidden)
    );
    let rev = s.inspect(&o, &reference("source")).unwrap().revision;
    s.withdraw(&o, &reference("source"), rev).unwrap();
    s.close();
    let j = Journal::open(m, Hash, project_journal_limits()).unwrap();
    let (mut s, o) = Store::open(j, project("project:a"), c).unwrap();
    let b = session(&mut s, &o, &["child"], &DataUse::ALL);
    assert_eq!(
        s.read(&b, &reference("child"), DataUse::Train),
        Err(DataError::Forbidden)
    );
}
#[test]
fn aggregate_payload_and_item_ceilings_are_real_admission_limits() {
    let (mut s, o, _, _) = fixture();
    for group in 0..3 {
        let names: Vec<String> = (group * 64..((group + 1) * 64).min(129))
            .map(|n| format!("p{n}"))
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let a = session(&mut s, &o, &refs, &[DataUse::Collect]);
        for name in names {
            let mut d = draft(&name, vec![]);
            d.payload = vec![7; MAX_DATA_PAYLOAD_BYTES];
            let r = s.collect(&a, d);
            if name == "p128" {
                assert!(matches!(r, Err(DataError::BudgetExceeded)));
            } else {
                r.unwrap();
            }
        }
    }
    let (mut s, o, _, _) = fixture();
    for group in 0..16 {
        let names: Vec<String> = (group * 64..(group + 1) * 64)
            .map(|n| format!("i{n}"))
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let a = session(&mut s, &o, &refs, &[DataUse::Collect]);
        for name in names {
            let mut d = draft(&name, vec![]);
            d.payload = vec![1];
            s.collect(&a, d).unwrap();
        }
    }
    let a = session(&mut s, &o, &["extra"], &[DataUse::Collect]);
    assert!(matches!(
        s.collect(&a, draft("extra", vec![])),
        Err(DataError::BudgetExceeded)
    ));
}

#[test]
fn permissive_journal_profiles_and_exhaustion_fail_closed() {
    let m = Memory::new();
    let c = Clock {
        value: Rc::new(Cell::new(1000)),
        fail: Rc::new(Cell::new(false)),
    };
    let j = Journal::create_new(m, Hash, Limits::default()).unwrap();
    assert!(matches!(
        Store::create_new(j, project("project:a"), c),
        Err(DataError::InvalidRecord)
    ));
    let m = Memory::new();
    let c = Clock {
        value: Rc::new(Cell::new(1000)),
        fail: Rc::new(Cell::new(false)),
    };
    let j = Journal::create_new(
        m,
        Hash,
        Limits {
            records: 2,
            ..project_journal_limits()
        },
    )
    .unwrap();
    let (mut s, o) = Store::create_new(j, project("project:a"), c).unwrap();
    let a = session(&mut s, &o, &["root"], &DataUse::ALL);
    s.collect(&a, draft("root", vec![])).unwrap();
    assert!(matches!(
        s.set_policy(
            &o,
            &reference("root"),
            Revision::new(1).unwrap(),
            permissions(&DataUse::ALL),
            true
        ),
        Err(DataError::Journal(
            eylema_journal::JournalError::BudgetExceeded
        ))
    ));
    assert_eq!(
        s.read(&a, &reference("root"), DataUse::Execute),
        Err(DataError::Forbidden)
    );
}
#[test]
fn all_sources_and_parent_boundary_are_checked_before_derivation() {
    let (mut s, o, _, _) = fixture();
    let names: Vec<String> = (0..17)
        .map(|n| format!("s{n}"))
        .chain(["child".into(), "candidate".into()])
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let a = session(&mut s, &o, &refs, &DataUse::ALL);
    for name in names.iter().take(17) {
        s.collect(&a, draft(name, vec![])).unwrap();
        allow(&mut s, &o, name, &DataUse::ALL, true);
    }
    let parents: Vec<DataRef> = names.iter().take(16).map(|id| reference(id)).collect();
    s.collect(&a, draft("child", parents.clone())).unwrap();
    allow(&mut s, &o, "child", &DataUse::ALL, true);
    assert!(
        s.export_manifest(&a, &reference("child"), DataUse::EcosystemExport)
            .is_ok()
    );
    let extra: Vec<DataRef> = names.iter().take(17).map(|id| reference(id)).collect();
    assert!(matches!(
        s.collect(&a, draft("candidate", extra)),
        Err(DataError::InvalidRecord)
    ));
    allow(&mut s, &o, "s15", &[DataUse::Execute], true);
    assert!(matches!(
        s.collect(&a, draft("candidate", parents)),
        Err(DataError::Forbidden)
    ));
    assert_eq!(
        s.export_manifest(&a, &reference("child"), DataUse::EcosystemExport),
        Err(DataError::Forbidden)
    );
    let limited = session(&mut s, &o, &["candidate"], &DataUse::ALL);
    assert!(matches!(
        s.collect(&limited, draft("candidate", vec![reference("s0")])),
        Err(DataError::Forbidden)
    ));
    let mut d = draft("candidate", vec![reference("s0")]);
    d.expires_at = 10001;
    assert!(matches!(s.collect(&a, d), Err(DataError::InvalidRecord)));
}
#[test]
fn malformed_known_replay_events_and_configurations_fail_closed() {
    use eylema_contracts::codec::Value;
    let t = |s: &str| Value::Text(s.into());
    let u = Value::Unsigned;
    let l = Value::List;
    let put = |id: &str, kind: &str, parents: Vec<Value>, bytes: Vec<u8>, expires: u64| {
        l(vec![
            t("put"),
            t(id),
            t(kind),
            l(parents),
            Value::Bytes(bytes),
            u(expires),
        ])
    };
    let cases = vec![
        l(vec![t("clock"), u(1000)]),
        l(vec![t("clock"), u(999)]),
        l(vec![t("clock"), Value::Null]),
        l(vec![t("clock"), u(1001), Value::Null]),
        put("child", "raw_reasoning", vec![], vec![1], 10000),
        put("child", "credentials", vec![], vec![1], 10000),
        put("child", "derived", vec![], vec![1], 10000),
        put(
            "child",
            "training_candidate",
            vec![t("missing")],
            vec![1],
            10000,
        ),
        put(
            "child",
            "derived",
            vec![t("root"), t("root")],
            vec![1],
            10000,
        ),
        put("child", "derived", vec![t("root"); 17], vec![1], 10000),
        put("root", "artifact", vec![], vec![1], 10000),
        put("child", "artifact", vec![], vec![], 10000),
        put(
            "child",
            "artifact",
            vec![],
            vec![1; MAX_DATA_PAYLOAD_BYTES + 1],
            10000,
        ),
        put("child", "derived", vec![t("root")], vec![1], 10001),
        put("child", "artifact", vec![], vec![1], 1000),
        put("child", "artifact", vec![], vec![1], 1001 + MAX_DATA_TTL_MS),
        l(vec![t("put"), t("child")]),
        l(vec![
            t("policy"),
            t("missing"),
            u(1),
            u(0),
            Value::Bool(false),
        ]),
        l(vec![t("policy"), t("root"), u(2), u(1), Value::Bool(false)]),
        l(vec![t("policy"), t("root"), u(4), u(1), Value::Bool(false)]),
        l(vec![
            t("policy"),
            t("root"),
            u(3),
            u(32),
            Value::Bool(false),
        ]),
        l(vec![t("policy"), t("root"), u(3), u(1), u(0)]),
        l(vec![
            t("policy"),
            t("root"),
            u(3),
            u(1),
            Value::Bool(false),
            Value::Null,
        ]),
        l(vec![t("withdraw"), t("missing"), u(1)]),
        l(vec![t("withdraw"), t("root"), u(2)]),
        l(vec![t("withdraw"), t("root"), u(3), Value::Null]),
    ];
    let reopen_event = |event: Value| {
        let (mut s, o, m, c) = fixture();
        let a = session(&mut s, &o, &["root"], &DataUse::ALL);
        s.collect(&a, draft("root", vec![])).unwrap();
        allow(&mut s, &o, "root", &DataUse::ALL, true);
        s.close();
        let mut j = Journal::open(m, Hash, project_journal_limits()).unwrap();
        j.append(&event).unwrap();
        c.value.set(2000);
        Store::open(j, project("project:a"), c)
    };
    for (n, event) in cases.into_iter().enumerate() {
        assert!(reopen_event(event).is_err(), "malformed replay fixture {n}");
    }
    for event in [
        put("child", "derived", vec![t("root")], vec![1], 10000),
        l(vec![t("policy"), t("root"), u(3), u(1), Value::Bool(false)]),
        l(vec![t("withdraw"), t("root"), u(3)]),
        l(vec![t("clock"), u(1001)]),
    ] {
        assert!(reopen_event(event).is_ok(), "valid semantic control");
    }
    for config in [
        l(vec![t("project-data"), u(2), t("project:a"), u(1000)]),
        l(vec![t("project-data"), u(1), t("project:a"), u(0)]),
        l(vec![t("project-data"), u(1), t("bad/path"), u(1000)]),
        l(vec![
            t("project-data"),
            u(1),
            t("project:a"),
            u(1000),
            Value::Null,
        ]),
    ] {
        let m = Memory::new();
        let c = Clock {
            value: Rc::new(Cell::new(1000)),
            fail: Rc::new(Cell::new(false)),
        };
        let mut j = Journal::create_new(m, Hash, project_journal_limits()).unwrap();
        j.append(&config).unwrap();
        assert!(Store::open(j, project("project:a"), c).is_err());
    }
}
