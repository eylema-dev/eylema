use eylema_contracts::{Revision, codec::Value, memory::*, platform::*, project_data::*};
fn id(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn embedding() -> EmbeddingSpace {
    EmbeddingSpace {
        id: id("embedding:v1"),
        model: ModelProfileId::new("model:v1").unwrap(),
        preprocessing: id("pre:v1"),
        revision: Revision::new(1).unwrap(),
        dimensions: 32,
    }
}
fn profile() -> MemoryProfileDraft {
    MemoryProfileDraft {
        id: id("memory:v1"),
        revision: Revision::new(1).unwrap(),
        schema: id("utf8:v1"),
        backend: MemoryBackend::OwnedReferences,
        kinds: MemoryKind::ALL.to_vec(),
        roles: RoleKind::ALL.to_vec(),
        embedding: Some(embedding()),
    }
}
fn query() -> MemoryQueryDraft {
    MemoryQueryDraft {
        project: ProjectId::new("project:a").unwrap(),
        profile: id("memory:v1"),
        revision: Revision::new(1).unwrap(),
        role: RoleKind::Reasoning,
        purpose: DataUse::Execute,
        kinds: vec![MemoryKind::Semantic],
        sources: vec![DataId::new("source:a").unwrap()],
        embedding: Some(embedding()),
        verified_only: true,
        max_bytes: 65536,
        max_tokens: None,
    }
}
fn source() -> MemorySourceDraft {
    MemorySourceDraft {
        reference: DataRef {
            project: ProjectId::new("project:a").unwrap(),
            id: DataId::new("source:a").unwrap(),
        },
        kind: MemoryKind::Semantic,
        profile: id("memory:v1"),
        schema: id("utf8:v1"),
        observed_at: 1000,
        valid_from: 1000,
        expires_at: 10000,
        verification: Verification::Observed,
        confidence_bps: Some(5000),
        embedding: Some(embedding()),
    }
}
#[test]
fn six_optional_kinds_profiles_and_closed_query_roundtrip() {
    let p = MemoryProfile::new(profile()).unwrap();
    for k in MemoryKind::ALL {
        assert_eq!(MemoryKind::parse(k.as_str()).unwrap(), k);
    }
    let q = MemoryQuery::new(query()).unwrap();
    assert_eq!(MemoryQuery::from_value(&q.to_value()).unwrap(), q);
    assert_eq!(p.fields().kinds.len(), 6);
    let mut v = q.to_value();
    if let Value::List(ref mut fields) = v {
        fields.push(Value::Text("grant_all".into()));
    }
    assert!(MemoryQuery::from_value(&v).is_err());
    assert!(MemoryKind::parse("private_reasoning").is_err());
}
#[test]
fn missing_provenance_time_confidence_and_semantic_spaces_reject() {
    let mut s = source();
    s.observed_at = 0;
    assert!(s.validate().is_err());
    s = source();
    s.expires_at = s.valid_from;
    assert!(s.validate().is_err());
    s = source();
    s.confidence_bps = Some(10001);
    assert!(s.validate().is_err());
    s = source();
    s.embedding = None;
    assert!(s.validate().is_err());
    s = source();
    s.kind = MemoryKind::Working;
    assert!(s.validate().is_err());
    let mut e = embedding();
    e.dimensions = 0;
    assert!(e.validate().is_err());
    e = embedding();
    e.dimensions = 4097;
    assert!(e.validate().is_err());
}
#[test]
fn duplicate_empty_and_excessive_profile_query_limits_fail() {
    let mut p = profile();
    p.kinds.clear();
    assert!(MemoryProfile::new(p).is_err());
    p = profile();
    p.roles.push(RoleKind::Reasoning);
    assert!(MemoryProfile::new(p).is_err());
    p = profile();
    p.kinds.push(MemoryKind::Semantic);
    assert!(MemoryProfile::new(p).is_err());
    p = profile();
    p.embedding = None;
    assert!(MemoryProfile::new(p).is_err());
    for n in [0, 65537] {
        let mut q = query();
        q.max_bytes = n;
        assert!(MemoryQuery::new(q).is_err());
    }
    let mut q = query();
    q.sources = vec![DataId::new("same").unwrap(); 2];
    assert!(MemoryQuery::new(q).is_err());
    q = query();
    q.sources = (0..33)
        .map(|n| DataId::new(format!("s{n}")).unwrap())
        .collect();
    assert!(MemoryQuery::new(q).is_err());
    q = query();
    q.kinds.push(MemoryKind::Semantic);
    assert!(MemoryQuery::new(q).is_err());
    q = query();
    q.purpose = DataUse::ExternalDisclosure;
    assert!(MemoryQuery::new(q).is_err());
    q = query();
    q.max_tokens = Some(0);
    assert!(MemoryQuery::new(q).is_err());
}
#[test]
fn identical_dimensions_do_not_equal_embedding_semantics() {
    let e = embedding();
    for field in 0..4 {
        let mut changed = e.clone();
        match field {
            0 => changed.id = id("embedding:v2"),
            1 => changed.model = ModelProfileId::new("model:v2").unwrap(),
            2 => changed.preprocessing = id("pre:v2"),
            _ => changed.revision = Revision::new(2).unwrap(),
        }
        assert_ne!(e, changed);
    }
}
