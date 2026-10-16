use eylema_contracts::{
    Revision,
    memory::{EmbeddingSpace, durable::*},
    platform::*,
    project_data::*,
};
use eylema_host::{Owner, memory_store::*, native::NativeHash, project_data::*};
use eylema_journal::{Journal, Storage};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct Mem {
    pub bytes: Rc<RefCell<Cursor<Vec<u8>>>>,
    pub reads: Rc<Cell<usize>>,
    pub writes: Rc<Cell<usize>>,
    pub fragment: Rc<Cell<bool>>,
    pub fail: Rc<Cell<u8>>,
    pub sync_clock: Rc<RefCell<Option<Rc<Cell<u64>>>>>,
}
impl Read for Mem {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        let n = if self.fragment.get() {
            b.len().min(1)
        } else {
            b.len()
        };
        self.bytes.borrow_mut().read(&mut b[..n])
    }
}
impl Write for Mem {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.writes.set(self.writes.get() + 1);
        if self.fail.get() == 1 {
            return Err(io::ErrorKind::Other.into());
        }
        let n = if self.fragment.get() {
            b.len().min(1)
        } else {
            b.len()
        };
        self.bytes.borrow_mut().write(&b[..n])
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail.get() == 2 {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
impl Seek for Mem {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.bytes.borrow_mut().seek(p)
    }
}
impl Storage for Mem {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.bytes.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        if let Some(clock) = self.sync_clock.borrow().as_ref() {
            clock.set(1001)
        }
        if self.fail.get() == 3 {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone)]
pub struct Clock(pub Rc<Cell<u64>>);
impl DataClock for Clock {
    fn now(&mut self) -> DataResult<u64> {
        Ok(self.0.get())
    }
}
pub type Data = ProjectStore<Mem, NativeHash, Clock>;
pub fn id(s: &str) -> DataId {
    DataId::new(s).unwrap()
}
pub fn r(s: &str) -> DataRef {
    DataRef {
        project: ProjectId::new("memory:alpha").unwrap(),
        id: id(s),
    }
}
pub fn rev(n: u64) -> Revision {
    Revision::new(n).unwrap()
}
pub fn profile() -> StoreProfile {
    StoreProfile {
        project: r("x").project,
        id: SemanticProfileId::new("memory:v1").unwrap(),
        schema: SemanticProfileId::new("records:v1").unwrap(),
        revision: rev(1),
        predicates: vec![PredicateProfile {
            id: SemanticProfileId::new("owner").unwrap(),
            cardinality: Cardinality::SingleValue,
        }],
        representation: RepresentationProfile {
            space: EmbeddingSpace {
                id: SemanticProfileId::new("fixture-space").unwrap(),
                model: ModelProfileId::new("fixture-model").unwrap(),
                preprocessing: SemanticProfileId::new("fixture-prep").unwrap(),
                revision: rev(1),
                dimensions: 1024,
            },
            manifest_sha256: [1; 32],
            artifact_sha256: [2; 32],
            tokenizer_sha256: [3; 32],
            preprocessing_sha256: [4; 32],
        },
    }
}
pub fn c() -> MemoryControl {
    MemoryControl::new(
        MemoryBudget {
            max_work: 100_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(5),
        },
        MemoryCancel::new(),
    )
    .unwrap()
}
pub fn data() -> (Data, Owner, DataSession, Clock) {
    let clock = Clock(Rc::new(Cell::new(1)));
    let j = Journal::create_new(Mem::default(), NativeHash, project_journal_limits()).unwrap();
    let (mut d, o) = ProjectStore::create_new(j, r("x").project, clock.clone()).unwrap();
    let session = d
        .enroll(
            &o,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: [
                    "source", "review", "entity:1", "entity:2", "claim:1", "claim:2", "claim:3",
                    "bad:1",
                ]
                .iter()
                .map(|s| id(s))
                .collect(),
                expires_at: 1000,
            },
        )
        .unwrap();
    for (n, t) in [
        ("source", "Pump A owner Mira; checklist C7.\n"),
        (
            "review",
            "Owner reviewed the attributed synthetic claims.\n",
        ),
    ] {
        d.collect(
            &session,
            DataDraft {
                reference: r(n),
                kind: DataKind::Artifact,
                parents: vec![],
                payload: t.as_bytes().to_vec(),
                expires_at: 1000,
            },
        )
        .unwrap();
        d.set_policy(
            &o,
            &r(n),
            rev(1),
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
    }
    (d, o, session, clock)
}
pub fn pin(d: &mut Data, s: &DataSession, n: &str) -> SourcePin {
    let m = d.authorized_metadata(s, &r(n), DataUse::Collect).unwrap();
    SourcePin {
        reference: r(n),
        put_digest: m.digest,
    }
}
pub fn entity(source: SourcePin) -> RecordDraft {
    let p = profile();
    RecordDraft {
        project: p.project,
        id: id("pump-a"),
        revision: rev(1),
        profile: p.id,
        schema: p.schema,
        kind: RecordKind::Entity,
        origin: Origin::DocumentClaim,
        observed_at: 1,
        valid_from: 1,
        expires_at: 100,
        confidence_bps: None,
        subject: Some(id("pump-a")),
        predicate: None,
        object: None,
        episode_status: None,
        episode_text: None,
        sources: vec![source],
    }
}
pub fn claim(source: SourcePin) -> RecordDraft {
    let mut d = entity(source);
    d.id = id("owner-claim");
    d.kind = RecordKind::Fact;
    d.predicate = Some(SemanticProfileId::new("owner").unwrap());
    d.object = Some(TypedObject::Text("Mira".into()));
    d
}
