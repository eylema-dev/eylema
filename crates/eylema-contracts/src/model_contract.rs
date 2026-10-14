//! Bounded declared model provenance and role compatibility; never execution authority.
use crate::{
    Revision, ValidationError,
    codec::{self, CodecError, Value},
    platform::*,
    primitives::require,
};
use std::collections::{BTreeMap, BTreeSet};
pub const MAX_MANIFEST_BYTES: usize = 32768;
const QWEN_OPERATORS: [&str; 16] = [
    "qwen3.embedding_gather.v1",
    "qwen3.linear_no_bias.v1",
    "qwen3.rmsnorm_fp32.v1",
    "qwen3.silu.v1",
    "qwen3.swiglu.v1",
    "qwen3.residual_add.v1",
    "qwen3.rope_default.v1",
    "qwen3.reshape_transpose.v1",
    "qwen3.grouped_kv_repeat.v1",
    "qwen3.causal_mask.v1",
    "qwen3.scaled_attention.v1",
    "qwen3.softmax_fp32.v1",
    "qwen3.kv_cache_append.v1",
    "qwen3.tied_logits.v1",
    "qwen3.greedy_bounded.v1",
    "qwen3.dtype_and_layout.v1",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestSource {
    PublisherAdvertised,
    AuditMeasuredSmallBytes,
}
impl DigestSource {
    fn name(self) -> &'static str {
        match self {
            Self::PublisherAdvertised => "publisher_advertised",
            Self::AuditMeasuredSmallBytes => "audit_measured_small_bytes",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactComponent {
    pub name: String,
    pub size: u64,
    pub sha256: [u8; 32],
    pub digest_source: DigestSource,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorRequirement {
    pub id: SemanticProfileId,
    pub spec_sha256: [u8; 32],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RightsStatus {
    PublisherTermsPinned,
    Unresolved,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RightsRecord {
    pub publisher: String,
    pub source_repository: String,
    pub source_commit: [u8; 20],
    pub license_id: String,
    pub license_component: String,
    pub status: RightsStatus,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationRecord {
    pub config_component: String,
    pub weights_component: String,
    pub tokenizer_component: Option<String>,
    pub tokenizer_config_component: Option<String>,
    pub template_sha256: Option<[u8; 32]>,
    pub preprocessing_component: String,
    pub operators_component: String,
    pub dimensions: Vec<(String, u64)>,
    pub batch_size: u64,
    pub context_limit: u64,
    pub max_new_tokens: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelManifestDraft {
    pub id: ModelId,
    pub revision: Revision,
    pub source_repository: String,
    pub source_commit: [u8; 20],
    pub architecture: String,
    pub artifact_format: String,
    pub numeric_format: String,
    pub quantization: String,
    pub components: Vec<ArtifactComponent>,
    pub operators: Vec<OperatorRequirement>,
    pub rights: RightsRecord,
    pub configuration: ConfigurationRecord,
    pub profiles: Vec<RoleInterface>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelManifest(ModelManifestDraft);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedReason {
    Architecture,
    Format,
    Precision,
    Operators,
    Tokenizer,
    ModelExecutionNotImplemented,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignmentStatus {
    UnsupportedLocal(UnsupportedReason),
    ExternalConfigurationRequired,
}
fn text_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control)
}
fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn source_ok(s: &str) -> bool {
    let path = s
        .strip_prefix("https://huggingface.co/")
        .or_else(|| s.strip_prefix("https://github.com/"));
    path.is_some_and(|p| {
        let v: Vec<_> = p.split('/').collect();
        v.len() == 2 && v.iter().all(|x| name_ok(x))
    })
}
impl ModelManifest {
    pub fn new(d: ModelManifestDraft) -> Result<Self, ValidationError> {
        require(
            source_ok(&d.source_repository),
            "source_repository",
            "expected an exact supported publisher repository",
        )?;
        require(
            d.source_commit != [0; 20],
            "source_commit",
            "missing immutable commit",
        )?;
        require(
            [
                &d.architecture,
                &d.artifact_format,
                &d.numeric_format,
                &d.quantization,
            ]
            .iter()
            .all(|s| text_ok(s)),
            "profile",
            "missing or oversized format identity",
        )?;
        require(
            !d.components.is_empty() && d.components.len() <= 64,
            "components",
            "expected1..64 components",
        )?;
        let mut names = BTreeSet::new();
        let mut total = 0u64;
        for c in &d.components {
            require(
                name_ok(&c.name)
                    && names.insert(c.name.as_str())
                    && c.size > 0
                    && c.sha256 != [0; 32],
                "components",
                "invalid/duplicate component or empty identity",
            )?;
            total = total.checked_add(c.size).ok_or(ValidationError {
                field: "components",
                message: "component byte sum overflow",
            })?;
        }
        require(
            !d.operators.is_empty() && d.operators.len() <= 64,
            "operators",
            "expected1..64 operators",
        )?;
        let mut ops = BTreeSet::new();
        for o in &d.operators {
            require(
                o.spec_sha256 != [0; 32] && ops.insert(o.id.as_str()),
                "operators",
                "missing or duplicate operator identity",
            )?;
        }
        let r = &d.rights;
        require(
            text_ok(&r.publisher)
                && text_ok(&r.license_id)
                && r.source_repository == d.source_repository
                && r.source_commit == d.source_commit
                && r.status == RightsStatus::PublisherTermsPinned
                && names.contains(r.license_component.as_str()),
            "rights",
            "unresolved or foreign publisher terms",
        )?;
        let c = &d.configuration;
        require(
            [
                &c.config_component,
                &c.weights_component,
                &c.preprocessing_component,
                &c.operators_component,
            ]
            .iter()
            .all(|n| names.contains(n.as_str())),
            "configuration",
            "dangling component reference",
        )?;
        let tokenizer = [
            c.tokenizer_component.is_some(),
            c.tokenizer_config_component.is_some(),
            c.template_sha256.is_some(),
        ];
        require(
            tokenizer.iter().all(|x| *x) || tokenizer.iter().all(|x| !*x),
            "configuration",
            "partial tokenizer contract",
        )?;
        require(
            c.tokenizer_component
                .iter()
                .chain(c.tokenizer_config_component.iter())
                .all(|n| names.contains(n.as_str()))
                && c.template_sha256.is_none_or(|h| h != [0; 32]),
            "configuration",
            "dangling or empty tokenizer identity",
        )?;
        let op_component = d
            .components
            .iter()
            .find(|x| x.name == c.operators_component)
            .ok_or(ValidationError {
                field: "configuration",
                message: "missing operator component",
            })?;
        require(
            d.operators
                .iter()
                .all(|o| o.spec_sha256 == op_component.sha256),
            "operators",
            "operator spec must bind its exact component",
        )?;
        require(
            !c.dimensions.is_empty()
                && c.dimensions.len() <= 32
                && c.batch_size > 0
                && c.context_limit > 0
                && c.max_new_tokens > 0
                && c.max_new_tokens <= c.context_limit,
            "configuration",
            "invalid dimension/workload bounds",
        )?;
        let mut dims = BTreeMap::new();
        for (k, v) in &c.dimensions {
            require(
                name_ok(k) && *v > 0 && dims.insert(k.as_str(), *v).is_none(),
                "configuration",
                "invalid/duplicate dimension",
            )?;
        }
        require(
            c.batch_size.checked_mul(c.context_limit).is_some(),
            "configuration",
            "generic workload extent overflow",
        )?;
        if d.architecture == "qwen3-dense.v1" {
            require(
                [
                    "hidden_size",
                    "intermediate_size",
                    "layers",
                    "attention_heads",
                    "kv_heads",
                    "head_dim",
                    "vocab_size",
                    "context_capacity",
                ]
                .iter()
                .all(|k| dims.contains_key(k)),
                "configuration",
                "missing Qwen dimension",
            )?;
            require(
                qwen_extents_fit(&dims, c) && c.context_limit <= dims["context_capacity"],
                "configuration",
                "invalid grouped heads or tensor/cache/model/workload extent overflow",
            )?;
            require(
                c.tokenizer_component.is_some(),
                "configuration",
                "language profile needs exact tokenizer/template",
            )?;
        }
        require(
            !d.profiles.is_empty() && d.profiles.len() <= 128,
            "profiles",
            "expected1..128 optional intended profiles",
        )?;
        for (i, p) in d.profiles.iter().enumerate() {
            require(
                !d.profiles[..i].contains(p),
                "profiles",
                "duplicate intended role interface",
            )?;
        }
        let m = Self(d);
        require(
            m.to_bytes().is_ok(),
            "manifest",
            "encoded metadata exceeds closed payload bound",
        )?;
        Ok(m)
    }
    pub fn fields(&self) -> &ModelManifestDraft {
        &self.0
    }
    pub fn attention_projection_width(&self) -> Option<u64> {
        let c = &self.0.configuration;
        let head = c.dimensions.iter().find(|(k, _)| k == "attention_heads")?.1;
        let dim = c.dimensions.iter().find(|(k, _)| k == "head_dim")?.1;
        head.checked_mul(dim)
    }
    pub fn to_bytes(&self) -> codec::Result<Vec<u8>> {
        let b = codec::encode(&self.value())?;
        if b.len() > MAX_MANIFEST_BYTES {
            return Err(CodecError::TooLarge);
        }
        Ok(b)
    }
    pub fn from_bytes(b: &[u8]) -> codec::Result<Self> {
        if b.len() > MAX_MANIFEST_BYTES {
            return Err(CodecError::TooLarge);
        }
        let v = codec::decode(b)?;
        Self::parse(&v)
    }
    fn value(&self) -> Value {
        let d = &self.0;
        let c = &d.configuration;
        let r = &d.rights;
        map(vec![
            ("format", txt("model-profile.v1")),
            ("id", txt(d.id.as_str())),
            ("revision", num(d.revision.get())),
            ("source_repository", txt(&d.source_repository)),
            ("source_commit", blob(&d.source_commit)),
            ("architecture", txt(&d.architecture)),
            ("artifact_format", txt(&d.artifact_format)),
            ("numeric_format", txt(&d.numeric_format)),
            ("quantization", txt(&d.quantization)),
            (
                "components",
                Value::List(
                    d.components
                        .iter()
                        .map(|c| {
                            map(vec![
                                ("name", txt(&c.name)),
                                ("size", num(c.size)),
                                ("sha256", blob(&c.sha256)),
                                ("digest_source", txt(c.digest_source.name())),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "operators",
                Value::List(
                    d.operators
                        .iter()
                        .map(|o| {
                            map(vec![
                                ("id", txt(o.id.as_str())),
                                ("spec_sha256", blob(&o.spec_sha256)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "rights",
                map(vec![
                    ("publisher", txt(&r.publisher)),
                    ("source_repository", txt(&r.source_repository)),
                    ("source_commit", blob(&r.source_commit)),
                    ("license_id", txt(&r.license_id)),
                    ("license_component", txt(&r.license_component)),
                    ("status", txt("publisher_terms_pinned")),
                ]),
            ),
            (
                "configuration",
                map(vec![
                    ("config_component", txt(&c.config_component)),
                    ("weights_component", txt(&c.weights_component)),
                    ("tokenizer_component", opt_text(&c.tokenizer_component)),
                    (
                        "tokenizer_config_component",
                        opt_text(&c.tokenizer_config_component),
                    ),
                    (
                        "template_sha256",
                        c.template_sha256.map_or(Value::Null, |b| blob(&b)),
                    ),
                    ("preprocessing_component", txt(&c.preprocessing_component)),
                    ("operators_component", txt(&c.operators_component)),
                    (
                        "dimensions",
                        Value::Map(
                            c.dimensions
                                .iter()
                                .map(|(k, v)| (k.clone(), num(*v)))
                                .collect(),
                        ),
                    ),
                    ("batch_size", num(c.batch_size)),
                    ("context_limit", num(c.context_limit)),
                    ("max_new_tokens", num(c.max_new_tokens)),
                ]),
            ),
            (
                "profiles",
                Value::List(d.profiles.iter().map(role_value).collect()),
            ),
        ])
    }
    fn parse(v: &Value) -> codec::Result<Self> {
        let d = obj(
            v,
            &[
                "format",
                "id",
                "revision",
                "source_repository",
                "source_commit",
                "architecture",
                "artifact_format",
                "numeric_format",
                "quantization",
                "components",
                "operators",
                "rights",
                "configuration",
                "profiles",
            ],
        )?;
        if string(d["format"])? != "model-profile.v1" {
            return Err(CodecError::UnsupportedVersion);
        }
        let components = list(d["components"])?;
        if components.is_empty() || components.len() > 64 {
            return Err(CodecError::InvalidData);
        }
        let components = components
            .iter()
            .map(|v| {
                let c = obj(v, &["name", "size", "sha256", "digest_source"])?;
                Ok(ArtifactComponent {
                    name: string(c["name"])?,
                    size: uint(c["size"])?,
                    sha256: bytes(c["sha256"])?,
                    digest_source: match string(c["digest_source"])?.as_str() {
                        "publisher_advertised" => DigestSource::PublisherAdvertised,
                        "audit_measured_small_bytes" => DigestSource::AuditMeasuredSmallBytes,
                        _ => return Err(CodecError::InvalidData),
                    },
                })
            })
            .collect::<codec::Result<Vec<_>>>()?;
        let operators = list(d["operators"])?;
        if operators.is_empty() || operators.len() > 64 {
            return Err(CodecError::InvalidData);
        }
        let operators = operators
            .iter()
            .map(|v| {
                let o = obj(v, &["id", "spec_sha256"])?;
                Ok(OperatorRequirement {
                    id: SemanticProfileId::new(string(o["id"])?)?,
                    spec_sha256: bytes(o["spec_sha256"])?,
                })
            })
            .collect::<codec::Result<Vec<_>>>()?;
        let r = obj(
            d["rights"],
            &[
                "publisher",
                "source_repository",
                "source_commit",
                "license_id",
                "license_component",
                "status",
            ],
        )?;
        let rights = RightsRecord {
            publisher: string(r["publisher"])?,
            source_repository: string(r["source_repository"])?,
            source_commit: bytes(r["source_commit"])?,
            license_id: string(r["license_id"])?,
            license_component: string(r["license_component"])?,
            status: match string(r["status"])?.as_str() {
                "publisher_terms_pinned" => RightsStatus::PublisherTermsPinned,
                "unresolved" => RightsStatus::Unresolved,
                _ => return Err(CodecError::InvalidData),
            },
        };
        let c = obj(
            d["configuration"],
            &[
                "config_component",
                "weights_component",
                "tokenizer_component",
                "tokenizer_config_component",
                "template_sha256",
                "preprocessing_component",
                "operators_component",
                "dimensions",
                "batch_size",
                "context_limit",
                "max_new_tokens",
            ],
        )?;
        let Value::Map(dims) = c["dimensions"] else {
            return Err(CodecError::InvalidData);
        };
        if dims.len() > 32 {
            return Err(CodecError::InvalidData);
        }
        let configuration = ConfigurationRecord {
            config_component: string(c["config_component"])?,
            weights_component: string(c["weights_component"])?,
            tokenizer_component: optional_text(c["tokenizer_component"])?,
            tokenizer_config_component: optional_text(c["tokenizer_config_component"])?,
            template_sha256: if c["template_sha256"] == &Value::Null {
                None
            } else {
                Some(bytes(c["template_sha256"])?)
            },
            preprocessing_component: string(c["preprocessing_component"])?,
            operators_component: string(c["operators_component"])?,
            dimensions: dims
                .iter()
                .map(|(k, v)| Ok((k.clone(), uint(v)?)))
                .collect::<codec::Result<Vec<_>>>()?,
            batch_size: uint(c["batch_size"])?,
            context_limit: uint(c["context_limit"])?,
            max_new_tokens: uint(c["max_new_tokens"])?,
        };
        let profiles = list(d["profiles"])?;
        if profiles.is_empty() || profiles.len() > 128 {
            return Err(CodecError::InvalidData);
        }
        let profiles = profiles
            .iter()
            .map(parse_role)
            .collect::<codec::Result<Vec<_>>>()?;
        Self::new(ModelManifestDraft {
            id: ModelId::new(string(d["id"])?)?,
            revision: Revision::new(uint(d["revision"])?)?,
            source_repository: string(d["source_repository"])?,
            source_commit: bytes(d["source_commit"])?,
            architecture: string(d["architecture"])?,
            artifact_format: string(d["artifact_format"])?,
            numeric_format: string(d["numeric_format"])?,
            quantization: string(d["quantization"])?,
            components,
            operators,
            rights,
            configuration,
            profiles,
        })
        .map_err(Into::into)
    }
}
// Structural element extents only, not a resident-memory or parameter estimate.
fn qwen_extents_fit(d: &BTreeMap<&str, u64>, c: &ConfigurationRecord) -> bool {
    let calculate = || -> Option<u64> {
        let h = d["hidden_size"];
        let intermediate = d["intermediate_size"];
        let layers = d["layers"];
        let q = d["attention_heads"].checked_mul(d["head_dim"])?;
        let kv = d["kv_heads"].checked_mul(d["head_dim"])?;
        if !d["attention_heads"].is_multiple_of(d["kv_heads"]) {
            return None;
        }
        let attention = h.checked_mul(q.checked_mul(2)?.checked_add(kv.checked_mul(2)?)?)?;
        let mlp = h.checked_mul(intermediate)?.checked_mul(3)?;
        let norms = h
            .checked_mul(2)?
            .checked_add(d["head_dim"].checked_mul(2)?)?;
        let embedding = d["vocab_size"].checked_mul(h)?;
        let all_layers = attention
            .checked_add(mlp)?
            .checked_add(norms)?
            .checked_mul(layers)?;
        all_layers.checked_add(embedding)?.checked_add(h)?;
        let tokens = c.batch_size.checked_mul(c.context_limit)?;
        tokens
            .checked_mul(d["attention_heads"])?
            .checked_mul(c.context_limit)?;
        tokens
            .checked_mul(kv)?
            .checked_mul(2)?
            .checked_mul(layers)?;
        tokens.checked_mul(q)?;
        tokens.checked_mul(h)?;
        tokens.checked_mul(intermediate)?;
        tokens.checked_mul(d["vocab_size"])
    };
    calculate().is_some()
}
pub fn check_assignment(
    m: &ModelManifest,
    p: &RoleInterface,
    deployment: DeploymentKind,
    mode: LoopMode,
) -> Result<AssignmentStatus, ValidationError> {
    require(
        m.0.profiles.contains(p),
        "profile",
        "role/model/semantic interface mismatch",
    )?;
    if deployment == DeploymentKind::ExternalConfigured {
        require(
            mode == LoopMode::ExplicitExternal,
            "deployment",
            "local-only cannot route externally",
        )?;
        return Ok(AssignmentStatus::ExternalConfigurationRequired);
    }
    let d = &m.0;
    let why = if d.architecture != "qwen3-dense.v1" {
        UnsupportedReason::Architecture
    } else if d.artifact_format != "safetensors.v1" {
        UnsupportedReason::Format
    } else if d.numeric_format != "bfloat16" || d.quantization != "none" {
        UnsupportedReason::Precision
    } else if d.configuration.tokenizer_component.is_none() {
        UnsupportedReason::Tokenizer
    } else if d.operators.len() != QWEN_OPERATORS.len()
        || d.operators
            .iter()
            .any(|o| !QWEN_OPERATORS.contains(&o.id.as_str()))
    {
        UnsupportedReason::Operators
    } else {
        UnsupportedReason::ModelExecutionNotImplemented
    };
    Ok(AssignmentStatus::UnsupportedLocal(why))
}
fn map(v: Vec<(&str, Value)>) -> Value {
    Value::Map(v.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}
fn txt(s: &str) -> Value {
    Value::Text(s.to_owned())
}
fn num(n: u64) -> Value {
    Value::Unsigned(n)
}
fn blob(b: &[u8]) -> Value {
    Value::Bytes(b.to_vec())
}
fn opt_text(s: &Option<String>) -> Value {
    s.as_ref().map_or(Value::Null, |s| txt(s))
}
fn obj<'a>(v: &'a Value, keys: &[&str]) -> codec::Result<BTreeMap<&'a str, &'a Value>> {
    let Value::Map(v) = v else {
        return Err(CodecError::InvalidData);
    };
    if v.len() != keys.len() {
        return Err(CodecError::InvalidData);
    }
    let mut m = BTreeMap::new();
    for (k, v) in v {
        if !keys.contains(&k.as_str()) || m.insert(k.as_str(), v).is_some() {
            return Err(CodecError::InvalidData);
        }
    }
    Ok(m)
}
fn string(v: &Value) -> codec::Result<String> {
    match v {
        Value::Text(s) if text_ok(s) => Ok(s.clone()),
        _ => Err(CodecError::InvalidData),
    }
}
fn uint(v: &Value) -> codec::Result<u64> {
    if let Value::Unsigned(n) = v {
        Ok(*n)
    } else {
        Err(CodecError::InvalidData)
    }
}
fn bytes<const N: usize>(v: &Value) -> codec::Result<[u8; N]> {
    if let Value::Bytes(b) = v {
        b.as_slice().try_into().map_err(|_| CodecError::InvalidData)
    } else {
        Err(CodecError::InvalidData)
    }
}
fn list(v: &Value) -> codec::Result<&[Value]> {
    if let Value::List(v) = v {
        Ok(v)
    } else {
        Err(CodecError::InvalidData)
    }
}
fn optional_text(v: &Value) -> codec::Result<Option<String>> {
    if v == &Value::Null {
        Ok(None)
    } else {
        Ok(Some(string(v)?))
    }
}
fn role_value(p: &RoleInterface) -> Value {
    map(vec![
        ("role", txt(p.role.as_str())),
        ("version", num(p.version.get())),
        ("input_schema", txt(p.input_schema.as_str())),
        ("output_schema", txt(p.output_schema.as_str())),
        ("input_units", txt(p.input_units.as_str())),
        ("output_units", txt(p.output_units.as_str())),
        ("preprocessing", txt(p.preprocessing.as_str())),
        ("action_semantics", txt(p.action_semantics.as_str())),
        ("state_reset", txt(p.state_reset.as_str())),
    ])
}
fn parse_role(v: &Value) -> codec::Result<RoleInterface> {
    let p = obj(
        v,
        &[
            "role",
            "version",
            "input_schema",
            "output_schema",
            "input_units",
            "output_units",
            "preprocessing",
            "action_semantics",
            "state_reset",
        ],
    )?;
    let name = string(p["role"])?;
    let role = RoleKind::ALL
        .into_iter()
        .find(|r| r.as_str() == name)
        .ok_or(CodecError::InvalidData)?;
    Ok(RoleInterface {
        role,
        version: Revision::new(uint(p["version"])?)?,
        input_schema: SemanticProfileId::new(string(p["input_schema"])?)?,
        output_schema: SemanticProfileId::new(string(p["output_schema"])?)?,
        input_units: SemanticProfileId::new(string(p["input_units"])?)?,
        output_units: SemanticProfileId::new(string(p["output_units"])?)?,
        preprocessing: SemanticProfileId::new(string(p["preprocessing"])?)?,
        action_semantics: SemanticProfileId::new(string(p["action_semantics"])?)?,
        state_reset: SemanticProfileId::new(string(p["state_reset"])?)?,
    })
}
