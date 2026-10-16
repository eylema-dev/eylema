use eylema_contracts::{
    Revision,
    memory::EmbeddingSpace,
    platform::{ModelProfileId, ProjectId, SemanticProfileId},
    project_data::{DataId, DataRef},
};
use eylema_retrieval::*;
use std::time::{Duration, Instant};
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
pub fn r(s: &str) -> DataRef {
    DataRef {
        project: p(),
        id: DataId::new(s).unwrap(),
    }
}
pub fn docs() -> Vec<DocumentInput> {
    [
        ("a", "apple red", 1., 0.),
        ("b", "apple green", 0., 1.),
        ("c", "ocean blue", -1., 0.),
        ("d", "apple red", 1., 0.),
    ]
    .into_iter()
    .map(|(id, s, x, y)| {
        let mut v = vec![0.; 1024];
        v[0] = x;
        v[1] = y;
        DocumentInput {
            reference: r(id),
            put_digest: [5; 32],
            body: s.as_bytes().to_vec(),
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
// Deliberately deterministic synthetic integrity fixture, not cryptographic evidence.
pub struct Hash;
impl eylema_journal::Sha256 for Hash {
    fn digest(&mut self, b: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        let mut h = [0u8; 32];
        for (i, &x) in b.iter().enumerate() {
            h[i % 32] = h[i % 32].wrapping_mul(31).wrapping_add(x);
        }
        Ok(h)
    }
}
