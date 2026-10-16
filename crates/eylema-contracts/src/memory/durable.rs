//! Portable attributed-memory values. Constructing these grants no authority.
use super::EmbeddingSpace;
use crate::{
    Revision,
    platform::{ProjectId, RoleKind, SemanticProfileId},
    project_data::{DataId, DataRef, DataUse},
};
use std::collections::BTreeSet;
pub type RecordId = DataId;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordError {
    Invalid,
    Limit,
    Incompatible,
}
pub type RecordResult<T> = Result<T, RecordError>;
macro_rules! closed_enum {($name:ident {$($variant:ident => $text:literal),+ $(,)?})=>{
 #[derive(Clone,Copy,Debug,Eq,PartialEq,Ord,PartialOrd)] pub enum $name {$($variant),+}
 impl $name {pub const fn as_str(self)-> &'static str {match self {$(Self::$variant=>$text),+}} pub fn parse(s:&str)->RecordResult<Self>{match s {$($text=>Ok(Self::$variant),)+ _=>Err(RecordError::Invalid)}}}
};}
closed_enum!(RecordKind {Entity=>"entity",Fact=>"fact",Relation=>"relation",Episode=>"episode"});
closed_enum!(Origin {Observation=>"observation",DocumentClaim=>"document_claim",Estimate=>"estimate",Prediction=>"prediction"});
closed_enum!(Admission {Proposed=>"proposed",ExplicitlyReviewed=>"explicitly_reviewed",VerifiedOutcome=>"verified_outcome"});
closed_enum!(EpisodeStatus {Completed=>"completed",Failed=>"failed",Partial=>"partial",Stopped=>"stopped",Unknown=>"unknown",Pending=>"pending"});
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cardinality {
    SingleValue,
    SetValue,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeMode {
    Reusable,
    Evidence,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedObject {
    Entity(RecordId),
    Text(String),
    Integer(i64),
    Boolean(bool),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePin {
    pub reference: DataRef,
    pub put_digest: [u8; 32],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateProfile {
    pub id: SemanticProfileId,
    pub cardinality: Cardinality,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepresentationProfile {
    pub space: EmbeddingSpace,
    pub manifest_sha256: [u8; 32],
    pub artifact_sha256: [u8; 32],
    pub tokenizer_sha256: [u8; 32],
    pub preprocessing_sha256: [u8; 32],
}
impl RepresentationProfile {
    pub fn validate(&self) -> RecordResult<()> {
        if self.space.dimensions != 1024
            || self.space.validate().is_err()
            || [
                self.manifest_sha256,
                self.artifact_sha256,
                self.tokenizer_sha256,
                self.preprocessing_sha256,
            ]
            .contains(&[0; 32])
        {
            Err(RecordError::Incompatible)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreProfile {
    pub project: ProjectId,
    pub id: SemanticProfileId,
    pub schema: SemanticProfileId,
    pub revision: Revision,
    pub predicates: Vec<PredicateProfile>,
    pub representation: RepresentationProfile,
}
impl StoreProfile {
    pub fn validate(&self) -> RecordResult<()> {
        bounded(self.predicates.len(), 1, 32)?;
        unique(self.predicates.iter().map(|p| p.id.as_str()))?;
        self.representation.validate()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordDraft {
    pub project: ProjectId,
    pub id: RecordId,
    pub revision: Revision,
    pub profile: SemanticProfileId,
    pub schema: SemanticProfileId,
    pub kind: RecordKind,
    pub origin: Origin,
    pub observed_at: u64,
    pub valid_from: u64,
    pub expires_at: u64,
    pub confidence_bps: Option<u16>,
    pub subject: Option<RecordId>,
    pub predicate: Option<SemanticProfileId>,
    pub object: Option<TypedObject>,
    pub episode_status: Option<EpisodeStatus>,
    pub episode_text: Option<String>,
    pub sources: Vec<SourcePin>,
}
impl RecordDraft {
    pub fn validate(&self, p: &StoreProfile) -> RecordResult<()> {
        bounded(self.sources.len(), 1, 8)?;
        if self.episode_text.as_ref().is_some_and(|s| s.len() > 256)
            || matches!(&self.object,Some(TypedObject::Text(s)) if s.len()>256)
        {
            return Err(RecordError::Limit);
        }
        if self.project != p.project || self.profile != p.id || self.schema != p.schema {
            return Err(RecordError::Incompatible);
        }
        if self.observed_at == 0
            || self.valid_from == 0
            || self.expires_at <= self.valid_from
            || self.expires_at <= self.observed_at
            || self.confidence_bps.is_some_and(|n| n > 10000)
        {
            return Err(RecordError::Invalid);
        }
        if self
            .sources
            .iter()
            .any(|s| s.reference.project != self.project || s.put_digest == [0; 32])
        {
            return Err(RecordError::Invalid);
        }
        unique(self.sources.iter().map(|s| s.reference.id.as_str()))?;
        if let Some(predicate) = &self.predicate {
            if !p.predicates.iter().any(|x| &x.id == predicate) {
                return Err(RecordError::Incompatible);
            }
        }
        let valid = match self.kind {
            RecordKind::Entity => {
                self.subject.as_ref() == Some(&self.id)
                    && self.predicate.is_none()
                    && self.object.is_none()
                    && self.episode_status.is_none()
                    && self.episode_text.is_none()
            }
            RecordKind::Fact => {
                self.subject.is_some()
                    && self.predicate.is_some()
                    && self.object.is_some()
                    && self.episode_status.is_none()
                    && self.episode_text.is_none()
            }
            RecordKind::Relation => {
                self.subject.is_some()
                    && self.predicate.is_some()
                    && matches!(self.object, Some(TypedObject::Entity(_)))
                    && self.episode_status.is_none()
                    && self.episode_text.is_none()
            }
            RecordKind::Episode => {
                self.subject.is_none()
                    && self.predicate.is_none()
                    && self.object.is_none()
                    && self.episode_status.is_some()
                    && self.episode_text.is_some()
            }
        };
        if valid {
            Ok(())
        } else {
            Err(RecordError::Invalid)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub draft: RecordDraft,
    pub admission: Admission,
    pub projection: SourcePin,
}
impl Record {
    pub const fn eligible_knowledge(&self) -> bool {
        matches!(
            (self.draft.origin, self.admission),
            (Origin::DocumentClaim, Admission::ExplicitlyReviewed)
                | (Origin::Observation, Admission::VerifiedOutcome)
        )
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum RankMode {
    Graph,
    Lexical(String),
    Vector(Vec<f32>),
    Hybrid {
        text: String,
        vector: Vec<f32>,
        vector_weight: u32,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct QueryDraft {
    pub project: ProjectId,
    pub profile: SemanticProfileId,
    pub revision: Revision,
    pub generation: u64,
    pub role: RoleKind,
    pub purpose: DataUse,
    pub kinds: Vec<RecordKind>,
    pub origins: Vec<Origin>,
    pub knowledge: KnowledgeMode,
    pub effective_at: u64,
    pub entities: Vec<RecordId>,
    pub edge_predicates: Vec<SemanticProfileId>,
    pub result_predicates: Vec<SemanticProfileId>,
    pub seeds: Vec<RecordId>,
    pub hops: u8,
    pub rank: RankMode,
    pub representation: Option<RepresentationProfile>,
    pub max_hits: usize,
    pub max_sources: usize,
    pub max_bytes: usize,
    pub max_tokens: Option<u32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Query(QueryDraft);
impl Query {
    pub fn new(d: QueryDraft) -> RecordResult<Self> {
        bounded(d.kinds.len(), 1, 4)?;
        bounded(d.origins.len(), 1, 4)?;
        bounded(d.entities.len(), 0, 32)?;
        bounded(d.seeds.len(), 0, 8)?;
        bounded(d.edge_predicates.len(), 0, 32)?;
        bounded(d.result_predicates.len(), 0, 32)?;
        bounded(d.max_hits, 1, 32)?;
        bounded(d.max_sources, 1, 32)?;
        bounded(d.max_bytes, 1, 65536)?;
        if d.hops > 4 || d.effective_at == 0 || d.purpose.is_egress() {
            return Err(RecordError::Invalid);
        }
        unique(d.kinds.iter().map(|k| k.as_str()))?;
        unique(d.origins.iter().map(|o| o.as_str()))?;
        unique(d.entities.iter().map(|s| s.as_str()))?;
        unique(d.seeds.iter().map(|s| s.as_str()))?;
        unique(d.edge_predicates.iter().map(|s| s.as_str()))?;
        unique(d.result_predicates.iter().map(|s| s.as_str()))?;
        let (text, vector) = match &d.rank {
            RankMode::Graph => (None, None),
            RankMode::Lexical(s) => (Some(s), None),
            RankMode::Vector(v) => (None, Some(v)),
            RankMode::Hybrid {
                text,
                vector,
                vector_weight,
            } => {
                if !(1..1000000).contains(vector_weight) {
                    return Err(RecordError::Invalid);
                }
                (Some(text), Some(vector))
            }
        };
        if text.is_some_and(|s| s.len() > 8192)
            || vector.is_some_and(|v| v.len() != 1024 || v.iter().any(|n| !n.is_finite()))
        {
            return Err(RecordError::Limit);
        }
        if !matches!(d.rank, RankMode::Graph) {
            d.representation
                .as_ref()
                .ok_or(RecordError::Incompatible)?
                .validate()?;
        }
        Ok(Self(d))
    }
    pub fn fields(&self) -> &QueryDraft {
        &self.0
    }
}
fn bounded(n: usize, min: usize, max: usize) -> RecordResult<()> {
    if n < min || n > max {
        Err(RecordError::Limit)
    } else {
        Ok(())
    }
}
fn unique<'a>(items: impl Iterator<Item = &'a str>) -> RecordResult<()> {
    let mut seen = BTreeSet::new();
    for x in items {
        if !seen.insert(x) {
            return Err(RecordError::Invalid);
        }
    }
    Ok(())
}
