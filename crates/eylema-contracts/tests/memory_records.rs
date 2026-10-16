use eylema_contracts::{
    Revision,
    memory::{EmbeddingSpace, durable::*},
    platform::*,
    project_data::*,
};
fn rev(n: u64) -> Revision {
    Revision::new(n).unwrap()
}
fn id(s: &str) -> DataId {
    DataId::new(s).unwrap()
}
fn pin(s: &str) -> SourcePin {
    SourcePin {
        reference: DataRef {
            project: ProjectId::new("memory:alpha").unwrap(),
            id: id(s),
        },
        put_digest: [1; 32],
    }
}
fn profile() -> StoreProfile {
    StoreProfile {
        project: ProjectId::new("memory:alpha").unwrap(),
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
fn draft() -> RecordDraft {
    let p = profile();
    RecordDraft {
        project: p.project,
        id: id("claim"),
        revision: rev(1),
        profile: p.id,
        schema: p.schema,
        kind: RecordKind::Fact,
        origin: Origin::DocumentClaim,
        observed_at: 1,
        valid_from: 1,
        expires_at: 100,
        confidence_bps: None,
        subject: Some(id("pump-a")),
        predicate: Some(SemanticProfileId::new("owner").unwrap()),
        object: Some(TypedObject::Text("Mira".into())),
        episode_status: None,
        episode_text: None,
        sources: vec![pin("source")],
    }
}
fn query() -> QueryDraft {
    let p = profile();
    QueryDraft {
        project: p.project,
        profile: p.id,
        revision: rev(1),
        generation: 1,
        role: RoleKind::Reasoning,
        purpose: DataUse::Execute,
        kinds: vec![RecordKind::Fact],
        origins: vec![Origin::DocumentClaim],
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
#[test]
fn record_shapes_and_bounds() {
    let p = profile();
    assert!(p.validate().is_ok());
    assert!(DataId::new("a".repeat(96)).is_ok());
    assert!(DataId::new("a".repeat(97)).is_err());
    assert!(DataId::new("a".repeat(128)).is_err());
    assert!(DataId::new("a".repeat(129)).is_err());
    let mut d = draft();
    d.object = Some(TypedObject::Text("x".repeat(256)));
    assert!(d.validate(&p).is_ok());
    d.object = Some(TypedObject::Text("x".repeat(257)));
    assert_eq!(d.validate(&p), Err(RecordError::Limit));
    d = draft();
    d.sources = (0..8).map(|i| pin(&format!("s{i}"))).collect();
    assert!(d.validate(&p).is_ok());
    d.sources.push(pin("s8"));
    assert_eq!(d.validate(&p), Err(RecordError::Limit));
    d = draft();
    d.sources.push(pin("source"));
    assert_eq!(d.validate(&p), Err(RecordError::Invalid));
    for (observed, start, end) in [
        (0, 1, 100),
        (1, 0, 100),
        (100, 1, 100),
        (1, 100, 100),
        (1, 2, 1),
    ] {
        d = draft();
        d.observed_at = observed;
        d.valid_from = start;
        d.expires_at = end;
        assert!(d.validate(&p).is_err());
    }
    d = draft();
    d.confidence_bps = Some(10001);
    assert!(d.validate(&p).is_err());
    let mut p2 = p.clone();
    p2.predicates.push(p2.predicates[0].clone());
    assert!(p2.validate().is_err());
    d = draft();
    d.kind = RecordKind::Relation;
    assert!(d.validate(&p).is_err());
    d.object = Some(TypedObject::Entity(id("other")));
    assert!(d.validate(&p).is_ok());
    d = draft();
    d.kind = RecordKind::Entity;
    d.subject = Some(d.id.clone());
    d.predicate = None;
    d.object = None;
    assert!(d.validate(&p).is_ok());
}
#[test]
fn prediction_review_keeps_origin() {
    for origin in [Origin::Prediction, Origin::Estimate] {
        let mut d = draft();
        d.origin = origin;
        d.confidence_bps = Some(10000);
        let r = Record {
            draft: d,
            admission: Admission::ExplicitlyReviewed,
            projection: pin("projection"),
        };
        assert!(!r.eligible_knowledge());
        assert_eq!(r.draft.origin, origin);
    }
    let mut r = Record {
        draft: draft(),
        admission: Admission::Proposed,
        projection: pin("projection"),
    };
    assert!(!r.eligible_knowledge());
    r.admission = Admission::ExplicitlyReviewed;
    assert!(r.eligible_knowledge());
    r.draft.origin = Origin::Observation;
    assert!(!r.eligible_knowledge());
    r.admission = Admission::VerifiedOutcome;
    assert!(r.eligible_knowledge());
    assert!(Origin::parse("guessed").is_err());
    assert!(Admission::parse("trusted").is_err());
}
#[test]
fn query_caps_and_representation() {
    assert!(Query::new(query()).is_ok());
    let mut q = query();
    q.hops = 5;
    assert!(Query::new(q).is_err());
    let mut q = query();
    q.seeds = (0..9).map(|i| id(&format!("s{i}"))).collect();
    assert!(Query::new(q).is_err());
    for (hits, sources, bytes) in [
        (0, 32, 65536),
        (33, 32, 65536),
        (1, 33, 65536),
        (1, 1, 65537),
        (1, 1, 0),
    ] {
        let mut q = query();
        q.max_hits = hits;
        q.max_sources = sources;
        q.max_bytes = bytes;
        assert!(Query::new(q).is_err());
    }
    let mut q = query();
    q.purpose = DataUse::EcosystemExport;
    assert!(Query::new(q).is_err());
    let mut q = query();
    q.rank = RankMode::Vector(vec![0.; 1024]);
    assert!(Query::new(q.clone()).is_err());
    q.representation = Some(profile().representation);
    assert!(Query::new(q.clone()).is_ok());
    q.representation.as_mut().unwrap().space.dimensions = 512;
    assert!(Query::new(q).is_err());
    let mut q = query();
    q.edge_predicates = vec![SemanticProfileId::new("owner").unwrap(); 2];
    assert!(Query::new(q).is_err());
}
