use eylema_contracts::{
    Revision,
    platform::{ProjectId, SemanticProfileId},
    project_data::*,
};
use eylema_host::{Owner, native::NativeHash, project_data::*, routing::*};
use eylema_journal::{Journal, Storage};
use eylema_retrieval::{CancelToken, RetrievalBudget};
use eylema_runtime::{resource_estimate::*, routing::*};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct Mem(pub Rc<RefCell<Cursor<Vec<u8>>>>);
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
pub type Data = ProjectStore<Mem, NativeHash, Clock>;
pub fn id(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
pub fn rev(n: u64) -> Revision {
    Revision::new(n).unwrap()
}
pub fn pin(s: &str) -> Pin {
    Pin {
        id: id(s),
        revision: rev(1),
    }
}
pub fn stamp(s: &str) -> Stamp {
    Stamp {
        pin: pin(s),
        valid_from: 500,
        valid_until: 5000,
    }
}
pub fn reference(s: &str) -> DataRef {
    DataRef {
        project: ProjectId::new("project:a").unwrap(),
        id: DataId::new(s).unwrap(),
    }
}
pub fn control() -> RouteControl {
    RouteControl::new(
        RetrievalBudget {
            max_work: 10_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(5),
        },
        CancelToken::new(),
    )
    .unwrap()
}
pub fn fixture(body: Vec<u8>) -> (Data, Owner, DataSession, Clock, SourcePin) {
    let clock = Clock(Rc::new(Cell::new(1000)), Rc::new(Cell::new(0)));
    let (mut data, owner) = ProjectStore::create_new(
        Journal::create_new(Mem::default(), NativeHash, project_journal_limits()).unwrap(),
        reference("source:a").project.clone(),
        clock.clone(),
    )
    .unwrap();
    let session = data
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: vec![reference("source:a").id],
                expires_at: 10000,
            },
        )
        .unwrap();
    let m = data
        .collect(
            &session,
            DataDraft {
                reference: reference("source:a"),
                kind: DataKind::Artifact,
                parents: vec![],
                payload: body,
                expires_at: 5000,
            },
        )
        .unwrap();
    data.set_policy(
        &owner,
        &m.reference,
        m.revision,
        DataPermissions::new(vec![DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
    let source = SourcePin {
        reference: m.reference,
        digest: m.digest,
    };
    (data, owner, session, clock, source)
}
const EV: Evidence<'static> = Evidence {
    origin: Origin::SuppliedAssumption,
    source: "source:fixture",
    revision: 1,
    valid_from: 500,
    valid_until: 5000,
};
const fn known(n: u64) -> Claim<'static, u64> {
    Claim::Known {
        value: n,
        evidence: EV,
    }
}
static POOLS: [Pool<'static>; 1] = [Pool {
    id: "pool:a",
    physical_id: "physical:a",
    kind: PoolKind::Unified,
    capacity: known(1024 * 1024 * 1024),
    available: known(1024 * 1024 * 1024),
    reserve: known(0),
}];
static AUX: [Auxiliary<'static>; 1] = [Auxiliary {
    id: "aux:a",
    pool: "pool:a",
    kind: AuxiliaryKind::RagWorking,
    lifetime: Lifetime::Persistent,
    size: Claim::Known {
        value: Formula::Bytes(ByteInterval {
            lower: 65536,
            upper: 65536,
        }),
        evidence: EV,
    },
}];
static SNAP: Snapshot<'static> = Snapshot {
    id: "capacity:a",
    expected_id: "capacity:a",
    revision: 1,
    expected_revision: 1,
    pools: &POOLS,
    residents: &[],
    invocations: &[],
    auxiliary: &AUX,
    overlaps: Some(&[]),
};
pub fn config(source: SourcePin) -> RegistryConfig<'static> {
    RegistryConfig {
        registry: stamp("registry:a"),
        policy: stamp("policy:a"),
        entries: vec![Entry {
            id: id("route:a"),
            revision: rev(1),
            intent: Intent::Retrieval,
            backend: RouteBackend::Retrieval,
            sources: vec![source],
            models: vec![],
            input_schema: id("schema:query"),
            output_schema: id("schema:hits"),
            quality: stamp("quality:a"),
            context_limit: 4096,
            resource: pin("capacity:a"),
            grounding: SourceStatus::DeclaredConsistent,
        }],
        resources: vec![ResourceView {
            snapshot: &SNAP,
            required_invocations: &[],
            required_auxiliary: &["aux:a"],
            valid_from: 500,
            valid_until: 5000,
        }],
    }
}
