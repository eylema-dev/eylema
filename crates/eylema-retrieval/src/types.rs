use crate::{RetrievalError as E, RetrievalResult as R};
use eylema_contracts::{memory::EmbeddingSpace, platform::ProjectId, project_data::DataRef};
#[derive(Clone, Eq, PartialEq)]
pub struct RepresentationBinding {
    pub space: EmbeddingSpace,
    pub manifest_sha256: [u8; 32],
    pub artifact_sha256: [u8; 32],
    pub tokenizer_sha256: [u8; 32],
    pub preprocessing_sha256: [u8; 32],
    pub chunk_profile: String,
    pub lexical_profile: String,
    pub score_profile: String,
}
impl RepresentationBinding {
    pub fn validate(&self) -> R<()> {
        if self.space.validate().is_err()
            || [
                self.manifest_sha256,
                self.artifact_sha256,
                self.tokenizer_sha256,
                self.preprocessing_sha256,
            ]
            .contains(&[0; 32])
            || self.chunk_profile != "utf8-window1024-v1"
            || self.lexical_profile != "unicode-alnum-lower-rust199-v1"
            || self.score_profile != "scalar-micro-half-even-v1"
        {
            Err(E::IncompatibleBinding)
        } else {
            Ok(())
        }
    }
}
pub struct DocumentInput {
    pub reference: DataRef,
    pub put_digest: [u8; 32],
    pub body: Vec<u8>,
    pub vectors: Vec<Vec<f32>>,
}
pub enum QueryInput {
    Vector(Vec<f32>),
    Lexical(String),
    Hybrid {
        text: String,
        vector: Vec<f32>,
        vector_weight: u32,
    },
}
pub struct SearchQuery {
    pub project: ProjectId,
    pub binding: RepresentationBinding,
    pub generation: u64,
    pub sources: Vec<DataRef>,
    pub input: QueryInput,
    pub top_k: usize,
    pub max_hits: usize,
    pub max_bytes: usize,
    pub max_tokens: Option<u32>,
}
impl SearchQuery {
    pub fn validate(&self) -> R<()> {
        if self.sources.is_empty()
            || self.sources.len() > 16
            || !(1..=16).contains(&self.top_k)
            || !(self.top_k..=32).contains(&self.max_hits)
            || !(1..=65536).contains(&self.max_bytes)
        {
            return Err(E::InvalidInput);
        }
        for (i, s) in self.sources.iter().enumerate() {
            if s.project != self.project {
                return Err(E::WrongProject);
            }
            if self.sources[..i].contains(s) {
                return Err(E::InvalidInput);
            }
        }
        self.binding.validate()?;
        if self.max_tokens.is_some() {
            return Err(E::TokenCounterUnavailable);
        }
        Ok(())
    }
}
