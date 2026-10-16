//! Frozen evidence jobs; synthetic annotations are not extraction/model quality.
use super::{frozen, records::*};
use eylema_contracts::{memory::durable::*, platform::*, project_data::*};
use eylema_host::{Owner, memory_store::*, native::NativeHash, project_data::*};
use eylema_journal::{Journal, Sha256, Storage};
use eylema_retrieval::{
    CancelToken, Control, DocumentInput, QueryInput, RepresentationBinding, RetrievalBudget,
    SearchQuery, Snapshot,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};
pub fn corpus_profile() -> StoreProfile {
    let mut p = profile();
    for name in ["contains", "checklist"] {
        p.predicates.push(PredicateProfile {
            id: SemanticProfileId::new(name).unwrap(),
            cardinality: Cardinality::SetValue,
        });
    }
    p
}
pub const ENTITIES: &[(&str, &str)] = &[
    ("north", "en-site"),
    ("pump-a", "en-owner"),
    ("south", "tr-site"),
    ("pompa-b", "tr-owner"),
    ("pump-c", "forecast"),
];
pub const CLAIMS: &[(&str, &str, &str, &str, &str)] = &[
    ("en-link", "en-site", "north", "contains", "pump-a"),
    ("en-owner", "en-owner", "pump-a", "owner", "Mira"),
    ("en-check", "en-owner", "pump-a", "checklist", "C7"),
    ("tr-link", "tr-site", "south", "contains", "pompa-b"),
    ("tr-owner", "tr-owner", "pompa-b", "owner", "Ece"),
    ("tr-check", "tr-owner", "pompa-b", "checklist", "C9"),
    ("en-alt", "en-alt", "pump-a", "owner", "Leon"),
    ("tr-alt", "tr-alt", "pompa-b", "owner", "Deniz"),
    ("expired", "expired", "pump-a", "owner", "Nia"),
    ("forecast", "forecast", "pump-c", "owner", "Zoe"),
    ("irrelevant", "irrelevant", "pump-c", "checklist", "K"),
];
pub fn projection_id(key: &str, revision: u64) -> String {
    format!("m:{key}:{revision}")
}
pub fn scope() -> Vec<DataId> {
    let mut names: Vec<String> = frozen::SOURCES
        .iter()
        .filter(|(name, _)| *name != "foreign")
        .map(|(name, _)| name.to_string())
        .collect();
    for key in ENTITIES
        .iter()
        .map(|(id, _)| *id)
        .chain(CLAIMS.iter().map(|(id, ..)| *id))
    {
        names.extend([projection_id(key, 1), projection_id(key, 2)]);
    }
    for i in 0..8 {
        names.extend([format!("extra:{i}:1"), format!("extra:{i}:2")]);
    }
    names.into_iter().map(|s| id(&s)).collect()
}
pub fn grant() -> DataGrant {
    DataGrant {
        permissions: DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
        scope: scope(),
        expires_at: 1000,
    }
}
pub struct CorpusRecord {
    pub record: Record,
    pub vector: Vec<f32>,
}
fn hash_hex(b: &[u8]) -> String {
    NativeHash
        .digest(b)
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn expected_hash(key: &str) -> &'static str {
    frozen::PAYLOAD_SHA256
        .iter()
        .find(|(k, _)| *k == key)
        .unwrap()
        .1
}
pub fn seed_corpus<D: Storage, M: Storage>(
    data: &mut ProjectStore<D, NativeHash, Clock>,
    data_owner: &Owner,
    ds: &DataSession,
    memory: &mut MemoryStore<M, NativeHash>,
    owner: &MemoryOwner,
) -> (MemorySession, Vec<CorpusRecord>) {
    let mut pins = BTreeMap::new();
    for (name, body) in frozen::SOURCES {
        assert_eq!(
            hash_hex(body.as_bytes()),
            expected_hash(&format!("source/{name}"))
        );
        if *name == "foreign" {
            continue;
        }
        let m = data
            .collect(
                ds,
                DataDraft {
                    reference: r(name),
                    kind: DataKind::Artifact,
                    parents: vec![],
                    payload: body.as_bytes().to_vec(),
                    expires_at: 1000,
                },
            )
            .unwrap();
        pins.insert(
            *name,
            SourcePin {
                reference: r(name),
                put_digest: m.digest,
            },
        );
        data.set_policy(
            data_owner,
            &r(name),
            rev(1),
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
    }
    let session = memory.enroll(owner, data, ds, &mut c()).unwrap();
    let mut drafts = vec![];
    for (key, source) in ENTITIES {
        let mut draft = entity(pins[source].clone());
        draft.id = id(key);
        draft.subject = Some(id(key));
        drafts.push((draft, false));
    }
    for (key, source, subject, predicate, value) in CLAIMS {
        let mut draft = claim(pins[source].clone());
        draft.id = id(key);
        draft.subject = Some(id(subject));
        draft.predicate = Some(SemanticProfileId::new(*predicate).unwrap());
        draft.object = Some(if *predicate == "contains" {
            TypedObject::Entity(id(value))
        } else {
            TypedObject::Text((*value).into())
        });
        draft.kind = if *predicate == "contains" {
            RecordKind::Relation
        } else {
            RecordKind::Fact
        };
        if *key == "forecast" {
            draft.origin = Origin::Prediction;
            draft.confidence_bps = Some(10000)
        }
        if *key == "expired" {
            draft.expires_at = 10
        }
        if *key == "en-alt" || *key == "tr-alt" {
            draft.valid_from = 50
        }
        drafts.push((draft, true));
    }
    let mut corpus = vec![];
    for (mut draft, indexed) in drafts {
        let key = draft.id.as_str().to_string();
        let mut vector = vec![0.; 1024];
        vector[usize::from(key == "irrelevant")] = 1.;
        let a = r(&projection_id(&key, 1));
        let b = r(&projection_id(&key, 2));
        let generation = memory.generation();
        memory
            .propose(
                data,
                &session,
                generation,
                None,
                draft.clone(),
                indexed.then_some(vector.as_slice()),
                a,
                &mut c(),
            )
            .unwrap();
        let generation = memory.generation();
        memory
            .review(
                owner,
                data,
                data_owner,
                &session,
                generation,
                &draft.id,
                rev(1),
                pins["review"].clone(),
                b.clone(),
                &mut c(),
            )
            .unwrap();
        data.set_policy(
            data_owner,
            &b,
            rev(1),
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
        let meta = data.authorized_metadata(ds, &b, DataUse::Execute).unwrap();
        let body = data.read(ds, &b, DataUse::Execute).unwrap();
        let label = if indexed {
            format!("projection/{key}")
        } else {
            format!("projection/entity/{key}")
        };
        assert_eq!(hash_hex(&body), expected_hash(&label));
        draft.revision = rev(2);
        draft.sources.push(pins["review"].clone());
        draft.sources.sort_by(|a, b| a.reference.cmp(&b.reference));
        if indexed {
            corpus.push(CorpusRecord {
                record: Record {
                    draft,
                    admission: Admission::ExplicitlyReviewed,
                    projection: SourcePin {
                        reference: b,
                        put_digest: meta.digest,
                    },
                },
                vector,
            });
        }
    }
    (session, corpus)
}
pub struct ExpectedBrief {
    pub status: MemoryStatus,
    pub values: &'static [(&'static str, &'static str)],
    pub citations: &'static [&'static str],
}
fn expected(job: usize, historical: bool) -> ExpectedBrief {
    use MemoryStatus::*;
    match job {
        1 => ExpectedBrief {
            status: Evidence,
            values: &[("en-owner", "Mira")],
            citations: &["en-owner", "review"],
        },
        2 => ExpectedBrief {
            status: Evidence,
            values: &[("tr-owner", "Ece")],
            citations: &["review", "tr-owner"],
        },
        3 => ExpectedBrief {
            status: Evidence,
            values: &[
                ("en-link", "pump-a"),
                ("en-owner", "Mira"),
                ("en-check", "C7"),
            ],
            citations: &["en-owner", "en-site", "review"],
        },
        4 => ExpectedBrief {
            status: Evidence,
            values: &[
                ("tr-link", "pompa-b"),
                ("tr-owner", "Ece"),
                ("tr-check", "C9"),
            ],
            citations: &["review", "tr-owner", "tr-site"],
        },
        5 => ExpectedBrief {
            status: Conflict,
            values: &[("en-alt", "Leon"), ("en-owner", "Mira")],
            citations: &["en-alt", "en-owner", "review"],
        },
        6 => ExpectedBrief {
            status: Conflict,
            values: &[("tr-alt", "Deniz"), ("tr-owner", "Ece")],
            citations: &["review", "tr-alt", "tr-owner"],
        },
        7 => expected(if historical { 1 } else { 5 }, false),
        8 => ExpectedBrief {
            status: NoEvidence,
            values: &[],
            citations: &[],
        },
        _ => unreachable!(),
    }
}
pub fn job_query(job: usize, historical: bool, mode: usize, generation: u64) -> Query {
    let mut q = super::recall::q(generation);
    q.origins = vec![
        Origin::Observation,
        Origin::DocumentClaim,
        Origin::Prediction,
        Origin::Estimate,
    ];
    q.result_predicates = vec![SemanticProfileId::new("owner").unwrap()];
    q.effective_at = if job <= 4 || (job == 7 && historical) {
        20
    } else {
        80
    };
    q.entities = vec![id(if [2, 4, 6].contains(&job) {
        "pompa-b"
    } else if job == 8 {
        "pump-c"
    } else {
        "pump-a"
    })];
    if job == 3 || job == 4 {
        q.entities.clear();
        q.seeds = vec![id(if job == 3 { "north" } else { "south" })];
        q.hops = 2;
        q.edge_predicates = vec![SemanticProfileId::new("contains").unwrap()];
        q.result_predicates
            .push(SemanticProfileId::new("checklist").unwrap());
        q.kinds = vec![RecordKind::Fact, RecordKind::Relation];
    }
    let mut vector = vec![0.; 1024];
    vector[1] = 1.;
    let text = if [2, 4, 6].contains(&job) {
        "sorumlu"
    } else {
        "owner"
    };
    q.rank = match mode {
        0 => RankMode::Graph,
        1 => RankMode::Lexical(text.into()),
        2 => RankMode::Vector(vector),
        _ => RankMode::Hybrid {
            text: text.into(),
            vector,
            vector_weight: 500000,
        },
    };
    if mode > 0 {
        q.representation = Some(corpus_profile().representation);
    }
    q.max_hits = 16;
    Query::new(q).unwrap()
}
fn value(r: &Record) -> String {
    match &r.draft.object {
        Some(TypedObject::Entity(s)) => s.as_str().to_string(),
        Some(TypedObject::Text(s)) => s.clone(),
        Some(TypedObject::Integer(n)) => n.to_string(),
        Some(TypedObject::Boolean(b)) => b.to_string(),
        None => String::new(),
    }
}
// Shared deterministic brief renderer/verifier for both conditions. No model prose.
pub fn render_and_verify(
    status: MemoryStatus,
    records: &[Record],
    citations: &[MemoryCitation],
    expected: &ExpectedBrief,
) -> bool {
    let got: BTreeSet<_> = records
        .iter()
        .map(|r| (r.draft.id.as_str(), value(r)))
        .collect();
    let want: BTreeSet<_> = expected
        .values
        .iter()
        .map(|(id, value)| (*id, (*value).to_string()))
        .collect();
    let cited: BTreeSet<_> = citations
        .iter()
        .map(|c| c.pin.reference.id.as_str())
        .collect();
    status == expected.status
        && got == want
        && cited == expected.citations.iter().copied().collect()
}
fn baseline_control() -> Control {
    let mut c = Control::new(
        RetrievalBudget {
            max_work: 100000000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(5),
        },
        CancelToken::new(),
    )
    .unwrap();
    c.reserve(8 * 1024 * 1024).unwrap();
    c
}
fn binding() -> RepresentationBinding {
    let p = corpus_profile().representation;
    RepresentationBinding {
        space: p.space,
        manifest_sha256: p.manifest_sha256,
        artifact_sha256: p.artifact_sha256,
        tokenizer_sha256: p.tokenizer_sha256,
        preprocessing_sha256: p.preprocessing_sha256,
        chunk_profile: "utf8-window1024-v1".into(),
        lexical_profile: "unicode-alnum-lower-rust199-v1".into(),
        score_profile: "scalar-micro-half-even-v1".into(),
    }
}
fn baseline<D: Storage>(
    data: &mut ProjectStore<D, NativeHash, Clock>,
    ds: &DataSession,
    corpus: &[CorpusRecord],
    q: &Query,
    job: usize,
) -> (MemoryStatus, Vec<Record>, Vec<MemoryCitation>, u64, usize) {
    let mut c = baseline_control();
    let mut docs = vec![];
    let mut eligible = vec![];
    for row in corpus {
        c.charge(65536).unwrap();
        c.reserve(65536).unwrap();
        let meta = match data.authorized_metadata(
            ds,
            &row.record.projection.reference,
            DataUse::Execute,
        ) {
            Ok(m) => m,
            Err(DataError::Expired | DataError::Forbidden) => continue,
            Err(e) => panic!("baseline authority {e:?}"),
        };
        let body = data
            .read(ds, &row.record.projection.reference, DataUse::Execute)
            .unwrap();
        assert!(body.len() <= 1024);
        assert_eq!(meta.digest, row.record.projection.put_digest);
        docs.push(DocumentInput {
            reference: row.record.projection.reference.clone(),
            put_digest: meta.digest,
            body,
            vectors: vec![row.vector.clone()],
        });
        eligible.push(&row.record);
    }
    let refs = docs.iter().map(|d| d.reference.clone()).collect();
    let binding = binding();
    let snapshot = Snapshot::build(
        r("x").project,
        binding.clone(),
        q.fields().generation,
        docs,
        &mut NativeHash,
        &mut c,
    )
    .unwrap();
    let input = match &q.fields().rank {
        RankMode::Graph => QueryInput::Lexical(
            if [2, 4, 6].contains(&job) {
                "sorumlu"
            } else {
                "owner"
            }
            .into(),
        ),
        RankMode::Lexical(t) => QueryInput::Lexical(t.clone()),
        RankMode::Vector(v) => QueryInput::Vector(v.clone()),
        RankMode::Hybrid {
            text,
            vector,
            vector_weight,
        } => QueryInput::Hybrid {
            text: text.clone(),
            vector: vector.clone(),
            vector_weight: *vector_weight,
        },
    };
    let search = SearchQuery {
        project: r("x").project,
        binding,
        generation: q.fields().generation,
        sources: refs,
        input,
        top_k: 16,
        max_hits: 16,
        max_bytes: 65536,
        max_tokens: None,
    };
    let result = snapshot.search(&search, &mut c).unwrap();
    let bytes = result.encode(65536, &mut c).unwrap();
    let decoded = eylema_contracts::codec::decode(&bytes).unwrap();
    let eylema_contracts::codec::Value::List(fields) = decoded else {
        panic!("closed retrieval result")
    };
    let eylema_contracts::codec::Value::List(hits) = &fields[7] else {
        panic!("closed hits")
    };
    let mut records = vec![];
    for hit in hits {
        let eylema_contracts::codec::Value::List(h) = hit else {
            panic!("closed hit")
        };
        let eylema_contracts::codec::Value::Text(key) = &h[1] else {
            panic!("closed reference")
        };
        records.push(
            (*eligible
                .iter()
                .find(|r| r.projection.reference.id.as_str() == key)
                .unwrap())
            .clone(),
        );
    }
    let mut citations = BTreeMap::new();
    data.trusted_time().unwrap();
    for record in &records {
        c.charge(65536).unwrap();
        let meta = data
            .authorized_metadata(ds, &record.projection.reference, DataUse::Execute)
            .unwrap();
        assert_eq!(meta.digest, record.projection.put_digest);
        for pin in &record.draft.sources {
            c.charge(65536).unwrap();
            let meta = data
                .authorized_metadata(ds, &pin.reference, DataUse::Execute)
                .unwrap();
            assert_eq!(meta.digest, pin.put_digest);
            citations.insert(
                pin.reference.clone(),
                MemoryCitation {
                    pin: pin.clone(),
                    authorization_revision: meta.revision,
                },
            );
        }
    }
    (
        if records.is_empty() {
            MemoryStatus::NoEvidence
        } else {
            MemoryStatus::Evidence
        },
        records,
        citations.into_values().collect(),
        c.work_used(),
        c.peak_live_bytes(),
    )
}
#[derive(Default)]
pub struct JobReport {
    pub job: usize,
    pub memory_passes: usize,
    pub flat_matches: usize,
    pub cases: usize,
    pub memory_work: u64,
    pub flat_work: u64,
    pub peak_bytes: usize,
}
pub fn run_jobs<D: Storage, M: Storage>(
    data: &mut ProjectStore<D, NativeHash, Clock>,
    ds: &DataSession,
    memory: &MemoryStore<M, NativeHash>,
    session: &MemorySession,
    corpus: &[CorpusRecord],
) -> Vec<JobReport> {
    let mut reports = vec![];
    for job in 1..=8 {
        let mut report = JobReport {
            job,
            ..Default::default()
        };
        let times: Vec<bool> = if job == 7 {
            vec![true, false]
        } else {
            vec![false]
        };
        for historical in times {
            for mode in 0..4 {
                let query = job_query(job, historical, mode, memory.generation());
                let mut c = c();
                let context = memory
                    .query(data, session, &query, &mut c)
                    .unwrap_or_else(|e| {
                        panic!(
                            "J{job}/mode{mode} memory error {e:?};work={}",
                            c.work_used()
                        )
                    });
                let wanted = expected(job, historical);
                assert!(
                    render_and_verify(
                        context.status(),
                        context.records(),
                        context.citations(),
                        &wanted
                    ),
                    "J{job}/mode{mode} evidence mismatch"
                );
                for citation in context.citations() {
                    let m = data
                        .authorized_metadata(ds, &citation.pin.reference, DataUse::Execute)
                        .unwrap();
                    assert_eq!(m.digest, citation.pin.put_digest);
                    assert_ne!(citation.pin.reference.project.as_str(), "memory:beta");
                }
                let (status, records, citations, work, peak) =
                    baseline(data, ds, corpus, &query, job);
                report.flat_matches +=
                    usize::from(render_and_verify(status, &records, &citations, &wanted));
                report.memory_passes += 1;
                report.cases += 1;
                report.memory_work = report.memory_work.max(c.work_used());
                report.flat_work = report.flat_work.max(work);
                report.peak_bytes = report.peak_bytes.max(peak).max(c.peak_live_bytes());
            }
        }
        reports.push(report);
    }
    reports
}
#[test]
fn eight_briefs_and_equal_bound_flat_rag() {
    let clock = Clock::new(1);
    let (mut data, owner) = ProjectStore::create_new(
        Journal::create_new(Mem::default(), NativeHash, project_journal_limits()).unwrap(),
        r("x").project,
        clock.clone(),
    )
    .unwrap();
    let ds = data.enroll(&owner, grant()).unwrap();
    let (mut memory, mo) = MemoryStore::create_new(
        Mem::default(),
        NativeHash,
        &mut data,
        &owner,
        corpus_profile(),
        &mut c(),
    )
    .unwrap();
    let (session, corpus) = seed_corpus(&mut data, &owner, &ds, &mut memory, &mo);
    clock.0.set(80);
    for report in run_jobs(&mut data, &ds, &memory, &session, &corpus) {
        assert_eq!(report.memory_passes, report.cases);
        println!(
            "G88 J{} memory={}/{} flat={}/{} memory_work={} flat_work={} peak_bytes={}",
            report.job,
            report.memory_passes,
            report.cases,
            report.flat_matches,
            report.cases,
            report.memory_work,
            report.flat_work,
            report.peak_bytes
        );
    }
}
// Shared with the native restart target; unused by the portable test binary.
#[allow(dead_code)]
pub fn reopen_corpus<D: Storage>(
    data: &ProjectStore<D, NativeHash, Clock>,
    owner: &Owner,
) -> Vec<CorpusRecord> {
    let review = data.inspect(owner, &r("review")).unwrap();
    let review = SourcePin {
        reference: r("review"),
        put_digest: review.digest,
    };
    CLAIMS
        .iter()
        .map(|(key, source, subject, predicate, value)| {
            let m = data.inspect(owner, &r(source)).unwrap();
            let source = SourcePin {
                reference: r(source),
                put_digest: m.digest,
            };
            let mut draft = claim(source);
            draft.id = id(key);
            draft.revision = rev(2);
            draft.subject = Some(id(subject));
            draft.predicate = Some(SemanticProfileId::new(*predicate).unwrap());
            draft.object = Some(if *predicate == "contains" {
                TypedObject::Entity(id(value))
            } else {
                TypedObject::Text((*value).into())
            });
            draft.kind = if *predicate == "contains" {
                RecordKind::Relation
            } else {
                RecordKind::Fact
            };
            if *key == "forecast" {
                draft.origin = Origin::Prediction;
                draft.confidence_bps = Some(10000)
            }
            if *key == "expired" {
                draft.expires_at = 10
            }
            if *key == "en-alt" || *key == "tr-alt" {
                draft.valid_from = 50
            }
            draft.sources.push(review.clone());
            draft.sources.sort_by(|a, b| a.reference.cmp(&b.reference));
            let reference = r(&projection_id(key, 2));
            let meta = data.inspect(owner, &reference).unwrap();
            let mut vector = vec![0.; 1024];
            vector[usize::from(*key == "irrelevant")] = 1.;
            CorpusRecord {
                record: Record {
                    draft,
                    admission: Admission::ExplicitlyReviewed,
                    projection: SourcePin {
                        reference,
                        put_digest: meta.digest,
                    },
                },
                vector,
            }
        })
        .collect()
}
