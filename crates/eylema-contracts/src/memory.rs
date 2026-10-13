//! Declared memory/query semantics. Confidence and metadata are never authority.
use crate::{
    Revision, ValidationError,
    codec::Value,
    platform::{ModelProfileId, ProjectId, RoleKind, SemanticProfileId},
    primitives::require,
    project_data::{DataId, DataRef, DataUse},
};
use std::collections::BTreeSet;
pub const MAX_MEMORY_ENTRIES: usize = 128;
pub const MAX_CONTEXT_SOURCES: usize = 32;
pub const MAX_CONTEXT_BYTES: usize = 65536;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryKind {
    Working,
    Episodic,
    Semantic,
    Procedural,
    KnowledgeGraph,
    Analytical,
}
impl MemoryKind {
    pub const ALL: [Self; 6] = [
        Self::Working,
        Self::Episodic,
        Self::Semantic,
        Self::Procedural,
        Self::KnowledgeGraph,
        Self::Analytical,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Procedural => "procedural",
            Self::KnowledgeGraph => "knowledge_graph",
            Self::Analytical => "analytical",
        }
    }
    pub fn parse(s: &str) -> Result<Self, ValidationError> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or(error("kind"))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verification {
    Observed,
    Reviewed,
    Estimated,
    Predicted,
}
impl Verification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Reviewed => "reviewed",
            Self::Estimated => "estimated",
            Self::Predicted => "predicted",
        }
    }
    pub const fn is_verified(self) -> bool {
        matches!(self, Self::Observed | Self::Reviewed)
    }
}
/// Declared intent; only OwnedReferences is implemented by the G87 adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryBackend {
    OwnedReferences,
    OwnedGraph,
    OwnedAnalytical,
    SqliteReference,
    DuckdbReference,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingSpace {
    pub id: SemanticProfileId,
    pub model: ModelProfileId,
    pub preprocessing: SemanticProfileId,
    pub revision: Revision,
    pub dimensions: u16,
}
impl EmbeddingSpace {
    pub fn validate(&self) -> Result<(), ValidationError> {
        require(
            (1..=4096).contains(&self.dimensions),
            "embedding",
            "invalid dimensions",
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryProfileDraft {
    pub id: SemanticProfileId,
    pub revision: Revision,
    pub schema: SemanticProfileId,
    pub backend: MemoryBackend,
    pub kinds: Vec<MemoryKind>,
    pub roles: Vec<RoleKind>,
    pub embedding: Option<EmbeddingSpace>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryProfile(MemoryProfileDraft);
impl MemoryProfile {
    pub fn new(d: MemoryProfileDraft) -> Result<Self, ValidationError> {
        unique(d.kinds.iter().map(|k| k.as_str()), 6, "kinds", false)?;
        unique(d.roles.iter().map(|k| k.as_str()), 9, "roles", false)?;
        space(d.kinds.contains(&MemoryKind::Semantic), &d.embedding)?;
        Ok(Self(d))
    }
    pub fn fields(&self) -> &MemoryProfileDraft {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySourceDraft {
    pub reference: DataRef,
    pub kind: MemoryKind,
    pub profile: SemanticProfileId,
    pub schema: SemanticProfileId,
    pub observed_at: u64,
    pub valid_from: u64,
    pub expires_at: u64,
    pub verification: Verification,
    pub confidence_bps: Option<u16>,
    pub embedding: Option<EmbeddingSpace>,
}
impl MemorySourceDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        require(
            self.observed_at > 0
                && self.valid_from > 0
                && self.expires_at > self.valid_from
                && self.expires_at > self.observed_at,
            "validity",
            "missing or invalid source time",
        )?;
        require(
            self.confidence_bps.is_none_or(|n| n <= 10000),
            "confidence",
            "invalid declared confidence",
        )?;
        space(self.kind == MemoryKind::Semantic, &self.embedding)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryQueryDraft {
    pub project: ProjectId,
    pub profile: SemanticProfileId,
    pub revision: Revision,
    pub role: RoleKind,
    pub purpose: DataUse,
    pub kinds: Vec<MemoryKind>,
    pub sources: Vec<DataId>,
    pub embedding: Option<EmbeddingSpace>,
    pub verified_only: bool,
    pub max_bytes: usize,
    pub max_tokens: Option<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryQuery(MemoryQueryDraft);
impl MemoryQuery {
    pub fn new(d: MemoryQueryDraft) -> Result<Self, ValidationError> {
        require(
            !d.purpose.is_egress(),
            "purpose",
            "context body egress unsupported",
        )?;
        unique(d.kinds.iter().map(|k| k.as_str()), 6, "kinds", false)?;
        unique(
            d.sources.iter().map(|id| id.as_str()),
            MAX_CONTEXT_SOURCES,
            "sources",
            true,
        )?;
        require(
            (1..=MAX_CONTEXT_BYTES).contains(&d.max_bytes),
            "max_bytes",
            "invalid context budget",
        )?;
        require(
            d.max_tokens.is_none_or(|n| n > 0),
            "max_tokens",
            "zero token budget",
        )?;
        space(d.kinds.contains(&MemoryKind::Semantic), &d.embedding)?;
        Ok(Self(d))
    }
    pub fn fields(&self) -> &MemoryQueryDraft {
        &self.0
    }
    pub fn to_value(&self) -> Value {
        let d = &self.0;
        Value::List(vec![
            text(d.project.as_str()),
            text(d.profile.as_str()),
            Value::Unsigned(d.revision.get()),
            text(d.role.as_str()),
            text(purpose_name(d.purpose)),
            Value::List(d.kinds.iter().map(|k| text(k.as_str())).collect()),
            Value::List(d.sources.iter().map(|id| text(id.as_str())).collect()),
            encode_space(&d.embedding),
            Value::Bool(d.verified_only),
            Value::Unsigned(d.max_bytes as u64),
            d.max_tokens
                .map_or(Value::Null, |n| Value::Unsigned(n as u64)),
        ])
    }
    pub fn from_value(v: &Value) -> Result<Self, ValidationError> {
        let Value::List(v) = v else {
            return Err(error("query"));
        };
        let [
            project,
            profile,
            rev,
            role,
            purpose,
            kinds,
            sources,
            embedding,
            verified,
            bytes,
            tokens,
        ]: &[Value; 11] = v.as_slice().try_into().map_err(|_| error("query"))?;
        let kinds = items(kinds, 6)?
            .iter()
            .map(|v| MemoryKind::parse(string(v)?))
            .collect::<Result<_, _>>()?;
        let sources = items(sources, 32)?
            .iter()
            .map(|v| DataId::new(string(v)?))
            .collect::<Result<_, _>>()?;
        let role = RoleKind::ALL
            .into_iter()
            .find(|r| Some(r.as_str()) == string(role).ok())
            .ok_or(error("role"))?;
        let purpose = match string(purpose)? {
            "execute" => DataUse::Execute,
            "collect" => DataUse::Collect,
            "train" => DataUse::Train,
            _ => return Err(error("purpose")),
        };
        let Value::Bool(verified) = verified else {
            return Err(error("verified_only"));
        };
        Self::new(MemoryQueryDraft {
            project: ProjectId::new(string(project)?)?,
            profile: SemanticProfileId::new(string(profile)?)?,
            revision: Revision::new(unsigned(rev)?)?,
            role,
            purpose,
            kinds,
            sources,
            embedding: decode_space(embedding)?,
            verified_only: *verified,
            max_bytes: usize::try_from(unsigned(bytes)?).map_err(|_| error("max_bytes"))?,
            max_tokens: if matches!(tokens, Value::Null) {
                None
            } else {
                Some(u32::try_from(unsigned(tokens)?).map_err(|_| error("max_tokens"))?)
            },
        })
    }
}
fn error(field: &'static str) -> ValidationError {
    ValidationError {
        field,
        message: "invalid memory contract",
    }
}
fn unique<'a>(
    values: impl Iterator<Item = &'a str>,
    max: usize,
    field: &'static str,
    allow_empty: bool,
) -> Result<(), ValidationError> {
    let values: Vec<_> = values.collect();
    require(
        (allow_empty || !values.is_empty()) && values.len() <= max,
        "collection",
        "invalid memory collection length",
    )?;
    require(
        values.iter().collect::<BTreeSet<_>>().len() == values.len(),
        field,
        "duplicate memory identity",
    )
}
fn space(semantic: bool, embedding: &Option<EmbeddingSpace>) -> Result<(), ValidationError> {
    require(
        semantic == embedding.is_some(),
        "embedding",
        "missing or unexpected embedding space",
    )?;
    if let Some(e) = embedding {
        e.validate()?;
    }
    Ok(())
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn string(v: &Value) -> Result<&str, ValidationError> {
    if let Value::Text(s) = v {
        Ok(s)
    } else {
        Err(error("text"))
    }
}
fn unsigned(v: &Value) -> Result<u64, ValidationError> {
    if let Value::Unsigned(n) = v {
        Ok(*n)
    } else {
        Err(error("number"))
    }
}
fn items(v: &Value, max: usize) -> Result<&[Value], ValidationError> {
    if let Value::List(v) = v
        && v.len() <= max
    {
        return Ok(v);
    }
    Err(error("list"))
}
fn purpose_name(p: DataUse) -> &'static str {
    match p {
        DataUse::Execute => "execute",
        DataUse::Collect => "collect",
        DataUse::Train => "train",
        DataUse::ExternalDisclosure => "external_disclosure",
        DataUse::EcosystemExport => "ecosystem_export",
    }
}
pub fn encode_space(space: &Option<EmbeddingSpace>) -> Value {
    space.as_ref().map_or(Value::Null, |s| {
        Value::List(vec![
            text(s.id.as_str()),
            text(s.model.as_str()),
            text(s.preprocessing.as_str()),
            Value::Unsigned(s.revision.get()),
            Value::Unsigned(s.dimensions as u64),
        ])
    })
}
fn decode_space(v: &Value) -> Result<Option<EmbeddingSpace>, ValidationError> {
    if matches!(v, Value::Null) {
        return Ok(None);
    }
    let [id, model, pre, rev, dims]: &[Value; 5] =
        items(v, 5)?.try_into().map_err(|_| error("embedding"))?;
    let s = EmbeddingSpace {
        id: SemanticProfileId::new(string(id)?)?,
        model: ModelProfileId::new(string(model)?)?,
        preprocessing: SemanticProfileId::new(string(pre)?)?,
        revision: Revision::new(unsigned(rev)?)?,
        dimensions: u16::try_from(unsigned(dims)?).map_err(|_| error("dimensions"))?,
    };
    s.validate()?;
    Ok(Some(s))
}
