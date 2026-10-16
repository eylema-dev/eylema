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
pub struct Clock(
    pub Rc<Cell<u64>>,
    pub Rc<Cell<u64>>,
    pub Rc<Cell<usize>>,
    pub Rc<Cell<Option<(usize, u64)>>>,
    pub Rc<RefCell<ClockHooks>>,
);
#[derive(Default)]
pub struct ClockHooks {
    pub cancel: Option<(usize, CancelToken)>,
    pub delay: Option<(usize, Duration)>,
}
impl DataClock for Clock {
    fn now(&mut self) -> DataResult<u64> {
        let call = self.2.get() + 1;
        self.2.set(call);
        {
            let mut hooks = self.4.borrow_mut();
            if hooks
                .cancel
                .as_ref()
                .is_some_and(|(target, _)| *target == call)
            {
                let (_, token) = hooks.cancel.take().unwrap();
                token.cancel();
            }
            if hooks
                .delay
                .as_ref()
                .is_some_and(|(target, _)| *target == call)
            {
                let (_, delay) = hooks.delay.take().unwrap();
                std::thread::sleep(delay);
            }
        }
        if let Some((target, time)) = self.3.get() {
            if call == target {
                self.0.set(time);
                self.3.set(None);
            }
        }
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
    let clock = Clock(
        Rc::new(Cell::new(1000)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(None)),
        Rc::new(RefCell::new(ClockHooks::default())),
    );
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
pub fn request() -> Request {
    Request {
        project: reference("source:a").project,
        registry: pin("registry:a"),
        policy: pin("policy:a"),
        resource: pin("capacity:a"),
        quality: pin("quality:a"),
        intent: Intent::Retrieval,
        candidates: vec![id("route:a")],
        local_only: true,
        context: 4,
        response: 0,
        verified_tokens_required: false,
    }
}
pub fn assessment(source: SourcePin) -> Assessment {
    Assessment {
        route: id("route:a"),
        utility: 700000,
        score_profile: pin("score:a"),
        provenance: Some(source),
    }
}
pub fn representation() -> eylema_retrieval::RepresentationBinding {
    use eylema_contracts::{memory::EmbeddingSpace, platform::ModelProfileId};
    eylema_retrieval::RepresentationBinding {
        space: EmbeddingSpace {
            id: id("space:a"),
            model: ModelProfileId::new("model:a").unwrap(),
            preprocessing: id("pre:a"),
            revision: rev(1),
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
pub type Retrieval = eylema_host::retrieval::RetrievalHost<Mem, NativeHash>;
pub fn retrieval_fixture() -> (
    Data,
    Owner,
    DataSession,
    Clock,
    Vec<SourcePin>,
    Retrieval,
    eylema_retrieval::SearchQuery,
) {
    use eylema_host::retrieval::*;
    use eylema_retrieval::*;
    let (mut data, owner, _, clock, a) = fixture(b"apple red".to_vec());
    let refs: Vec<_> = ["source:a", "source:b", "source:c", "source:d"]
        .iter()
        .map(|s| reference(s))
        .collect();
    let s = data
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: refs.iter().map(|r| r.id.clone()).collect(),
                expires_at: 10000,
            },
        )
        .unwrap();
    let mut pins = vec![a];
    for (r, body) in refs
        .iter()
        .skip(1)
        .zip(["apple green", "ocean blue", "apple red"])
    {
        let m = data
            .collect(
                &s,
                DataDraft {
                    reference: r.clone(),
                    kind: DataKind::Artifact,
                    parents: vec![],
                    payload: body.as_bytes().to_vec(),
                    expires_at: 5000,
                },
            )
            .unwrap();
        data.set_policy(
            &owner,
            r,
            m.revision,
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
        pins.push(SourcePin {
            reference: r.clone(),
            digest: m.digest,
        });
    }
    // Collection of the original immutable source remains a distinct explicit right.
    let m = data.inspect(&owner, &refs[0]).unwrap();
    data.set_policy(
        &owner,
        &refs[0],
        m.revision,
        DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
    let vectors = [(1., 0.), (0., 1.), (-1., 0.), (1., 0.)];
    let sources = refs
        .iter()
        .zip(vectors)
        .map(|(r, (x, y))| {
            let mut v = vec![0.; 1024];
            v[0] = x;
            v[1] = y;
            SourceVectors {
                reference: r.clone(),
                vectors: vec![v],
            }
        })
        .collect();
    let mut c = Control::new(
        RetrievalBudget {
            max_work: 100_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(30),
        },
        CancelToken::new(),
    )
    .unwrap();
    let (index, _) = RetrievalHost::create(
        Mem::default(),
        NativeHash,
        NativeHash,
        &mut data,
        &owner,
        &s,
        &representation(),
        sources,
        &mut c,
    )
    .unwrap();
    let mut vector = vec![0.; 1024];
    vector[0] = 1.;
    let query = SearchQuery {
        project: refs[0].project.clone(),
        binding: representation(),
        generation: 1,
        sources: refs,
        input: QueryInput::Vector(vector),
        top_k: 1,
        max_hits: 4,
        max_bytes: 65536,
        max_tokens: None,
    };
    (data, owner, s, clock, pins, index, query)
}
#[path = "../../../eylema-runtime/tests/support/mod.rs"]
pub mod support;
pub type Tools = eylema_host::tool::ToolHost<Mem, NativeHash, support::Services>;
pub struct SkillFixture {
    pub data: Data,
    pub owner: Owner,
    pub session: DataSession,
    pub clock: Clock,
    pub config: RegistryConfig<'static>,
    pub tools: Tools,
    pub actor: eylema_host::DurableSession,
    pub human: eylema_host::DurableHuman,
    pub plan: eylema_contracts::tool::ToolPlan,
    pub definition: eylema_contracts::skill::SkillLoop,
    pub pins: Vec<SourcePin>,
}
pub fn skill_fixture() -> SkillFixture {
    use eylema_contracts::{platform::LoopId, skill::*, tool::*, *};
    use eylema_host::DurableHost;
    use eylema_runtime::{Limits, tool::ToolRegistry};
    let (mut data, owner, _, clock, parent) = fixture(b"reviewed route policy".to_vec());
    let m = data.inspect(&owner, &parent.reference).unwrap();
    data.set_policy(
        &owner,
        &parent.reference,
        m.revision,
        DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
    let session = data
        .enroll(
            &owner,
            DataGrant {
                permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute])
                    .unwrap(),
                scope: vec![
                    reference("source:a").id,
                    reference("package:add").id,
                    reference("package:read").id,
                ],
                expires_at: 10000,
            },
        )
        .unwrap();
    let mut pins = vec![parent];
    let mut profiles = vec![];
    let mut calls = vec![];
    for (name, op) in [
        ("add", OwnedTool::AddTaskValue),
        ("read", OwnedTool::ReadTaskValue),
    ] {
        let profile = ToolProfile::new(id(&format!("tool:{name}")), rev(1), op);
        let skill = SkillId::new(format!("skill:{name}")).unwrap();
        let definition = SkillDefinition::new(
            skill.clone(),
            rev(1),
            profile.clone(),
            id(OWNED_SKILL_ADAPTER),
            None,
            id("acceptance:synthetic"),
        )
        .unwrap();
        let reference = reference(&format!("package:{name}"));
        let m = data
            .collect(
                &session,
                DataDraft {
                    reference: reference.clone(),
                    kind: DataKind::Derived,
                    parents: vec![pins[0].reference.clone()],
                    payload: codec::encode(&definition.to_value()).unwrap(),
                    expires_at: 5000,
                },
            )
            .unwrap();
        data.set_policy(
            &owner,
            &reference,
            m.revision,
            DataPermissions::new(vec![DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
        pins.push(SourcePin {
            reference: reference.clone(),
            digest: m.digest,
        });
        calls.push(SkillCall {
            reference,
            skill,
            revision: rev(1),
        });
        profiles.push(profile);
    }
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
    let mut tools =
        eylema_host::tool::ToolHost::new(host, ToolRegistry::new(profiles.clone()).unwrap())
            .unwrap();
    let actor = tools.enroll(&host_owner, support::grant()).unwrap();
    let mut hg = support::grant();
    hg.principal = PrincipalRef::new("principal:controller").unwrap();
    hg.permissions.push(Permission::RequestCancellation);
    let human = tools.enroll_verified_human(&host_owner, hg).unwrap();
    let task = support::task("task:routed", 30, 100);
    tools.create_task(&actor, task.clone()).unwrap();
    let mut proposal = support::proposal(&task, &[2, 0]).fields().clone();
    let mut bindings = vec![];
    for (n, action) in proposal.actions.iter_mut().enumerate() {
        let profile = &profiles[n];
        let mut f = action.fields().clone();
        f.operation_id = OperationId::new(profile.operation().operation_id()).unwrap();
        f.side_effect_class = profile.operation().effect();
        if n == 1 {
            f.payload = vec![];
        }
        f.authority = AuthorityRequirement::new(
            f.authority.principal_ref().clone(),
            f.authority.policy_revision(),
            f.authority.resource_scope().clone(),
            f.authority.permissions().to_vec(),
            OversightRequirements::new(vec![], true, true, None).unwrap(),
        )
        .unwrap();
        *action = ActionSpec::new(f).unwrap();
        bindings.push(ToolBinding {
            action_id: action.fields().action_id.clone(),
            profile: profile.id().clone(),
            revision: profile.revision(),
            input_schema: profile.input_schema(),
            output_schema: profile.output_schema(),
            source: None,
        });
    }
    let plan = ToolPlan::new(
        task.clone(),
        ActionProposal::new(proposal, &task).unwrap(),
        bindings,
    )
    .unwrap();
    let definition = SkillLoop::new(LoopId::new("loop:routed").unwrap(), rev(1), calls, 1).unwrap();
    let mut cfg = config(pins[0].clone());
    cfg.entries[0].intent = Intent::TypedSkill;
    cfg.entries[0].backend = RouteBackend::Skill;
    cfg.entries[0].sources = pins.clone();
    SkillFixture {
        data,
        owner,
        session,
        clock,
        config: cfg,
        tools,
        actor,
        human,
        plan,
        definition,
        pins,
    }
}
