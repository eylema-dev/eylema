//! Thin adapters to unchanged accepted SDKs; no model-specific math.
use super::types::*;
pub fn decision_query(query: &str, proposal: &str) -> Result<String> {
    if query.len() > 1024 || proposal.is_empty() || proposal.len() > 8192 || proposal.contains('\0')
    {
        return Err(ReportError::InvalidInput);
    }
    Ok(format!("Request: {query}\nPlanning proposal: {proposal}"))
}
use eylema_contracts::{
    codec::{self, Value},
    model_contract::ModelManifest,
};
use eylema_model::{
    LoadBudget,
    cpu::{CpuBudget, CpuControl, CpuModel, CpuRoleOutput, CpuRoleRequest, CpuWorkGroup},
    decision::{
        DecisionBatch, DecisionBudget, DecisionModel, DecisionOption, DecisionQuestion,
        decision_layout,
    },
    embedding::{EmbeddingBudget, EmbeddingCpuModel, embedding_layout},
    load,
    tokenizer::{
        AdmissionBudget, DecisionTokenizer, EmbeddingTokenizer, PromptMessage, PromptRole,
        TextBudget, Tokenizer,
    },
};
use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::PathBuf,
    time::Instant,
};
const PLANNING: &[u8] =
    include_bytes!("../../../../crates/eylema-model/tests/fixtures/g30/cpu-profile.v1.eyl");
const PARENT: &[u8] =
    include_bytes!("../../../../crates/eylema-contracts/tests/fixtures/qwen3-profile.v1.eyl");
const DECISION: &[u8] =
    include_bytes!("../../../../crates/eylema-model/tests/fixtures/g57/cpu-profile.v1.eyl");
const EMBEDDING: &[u8] =
    include_bytes!("../../../../crates/eylema-model/tests/fixtures/g68/embedding-profile.v1.eyl");
pub const RESIDENT: u64 = 2_684_354_560;
pub struct DecisionInput {
    pub query: String,
    pub options: Vec<(String, String)>,
}
pub enum ModelRequest {
    Documents(Vec<String>),
    Queries(Vec<String>),
    Plan(Vec<String>),
    Decide(Vec<DecisionInput>),
}
impl ModelRequest {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Documents(s) | Self::Queries(s) | Self::Plan(s) => {
                let limit = match self {
                    Self::Documents(_) => 9,
                    Self::Queries(_) => 4,
                    _ => 2,
                };
                if s.is_empty()
                    || s.len() > limit
                    || s.iter().any(|s| s.is_empty() || s.len() > 8192)
                {
                    return Err(ReportError::Limit);
                }
            }
            Self::Decide(q) => {
                if q.is_empty()
                    || q.len() > 2
                    || q.iter().any(|q| {
                        q.query.is_empty()
                            || q.query.len() > 4096
                            || q.options.is_empty()
                            || q.options.len() > 2
                            || q.options
                                .iter()
                                .any(|(id, s)| id.is_empty() || id.len() > 64 || s.len() > 4096)
                    })
                {
                    return Err(ReportError::Limit);
                }
            }
        }
        Ok(())
    }
    pub fn value(&self) -> Value {
        match self {
            Self::Documents(x) => Value::List(vec![Value::Unsigned(1), strings(x)]),
            Self::Queries(x) => Value::List(vec![Value::Unsigned(2), strings(x)]),
            Self::Plan(x) => Value::List(vec![Value::Unsigned(3), strings(x)]),
            Self::Decide(x) => Value::List(vec![
                Value::Unsigned(4),
                Value::List(
                    x.iter()
                        .map(|q| {
                            Value::List(vec![
                                Value::Text(q.query.clone()),
                                Value::List(
                                    q.options
                                        .iter()
                                        .map(|(i, s)| {
                                            Value::List(vec![
                                                Value::Text(i.clone()),
                                                Value::Text(s.clone()),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ]),
        }
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        use super::wire::{list, number, text};
        let v = list(v)?;
        if v.len() != 2 {
            return Err(ReportError::InvalidInput);
        }
        let rows = list(&v[1])?;
        let result = match number(&v[0])? {
            1 => Self::Documents(
                rows.iter()
                    .map(|s| text(s).map(str::to_owned))
                    .collect::<Result<_>>()?,
            ),
            2 => Self::Queries(
                rows.iter()
                    .map(|s| text(s).map(str::to_owned))
                    .collect::<Result<_>>()?,
            ),
            3 => Self::Plan(
                rows.iter()
                    .map(|s| text(s).map(str::to_owned))
                    .collect::<Result<_>>()?,
            ),
            4 => Self::Decide(
                rows.iter()
                    .map(|q| {
                        let q = list(q)?;
                        if q.len() != 2 {
                            return Err(ReportError::InvalidInput);
                        }
                        Ok(DecisionInput {
                            query: text(&q[0])?.into(),
                            options: list(&q[1])?
                                .iter()
                                .map(|o| {
                                    let o = list(o)?;
                                    if o.len() != 2 {
                                        return Err(ReportError::InvalidInput);
                                    }
                                    Ok((text(&o[0])?.into(), text(&o[1])?.into()))
                                })
                                .collect::<Result<_>>()?,
                        })
                    })
                    .collect::<Result<_>>()?,
            ),
            _ => return Err(ReportError::Unsupported),
        };
        result.validate()?;
        Ok(result)
    }
    pub fn count(&self) -> usize {
        match self {
            Self::Documents(v) | Self::Queries(v) | Self::Plan(v) => v.len(),
            Self::Decide(_) => 1,
        }
    }
}
fn strings(v: &[String]) -> Value {
    Value::List(v.iter().cloned().map(Value::Text).collect())
}
pub struct AssetPaths {
    pub paths: Vec<PathBuf>,
}
impl AssetPaths {
    pub fn from_value(v: &Value) -> Result<Self> {
        let rows = super::wire::list(v)?;
        if rows.len() != 9 {
            return Err(ReportError::InvalidInput);
        }
        Ok(Self {
            paths: rows
                .iter()
                .map(|v| super::wire::text(v).map(PathBuf::from))
                .collect::<Result<_>>()?,
        })
    }
    pub fn value(&self) -> Result<Value> {
        if self.paths.len() != 9 {
            return Err(ReportError::InvalidInput);
        }
        Ok(Value::List(
            self.paths
                .iter()
                .map(|p| {
                    p.to_str()
                        .filter(|s| s.len() <= 1024)
                        .map(|s| Value::Text(s.into()))
                        .ok_or(ReportError::InvalidInput)
                })
                .collect::<Result<_>>()?,
        ))
    }
}
struct RawFile {
    file: File,
    calls: u32,
    bytes: u64,
    limit: u64,
    deadline: Instant,
}
impl Read for RawFile {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if self.calls >= 1024 || Instant::now() >= self.deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.calls += 1;
        let allowance = self.limit.saturating_sub(self.bytes).min(b.len() as u64) as usize;
        if allowance == 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let n = self.file.read(&mut b[..allowance])?;
        self.bytes += n as u64;
        if Instant::now() >= self.deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(n)
    }
}
fn file(path: &std::path::Path, limit: u64, deadline: Instant) -> Result<BufReader<RawFile>> {
    let meta = fs::symlink_metadata(path).map_err(|_| ReportError::Unsupported)?;
    if !meta.file_type().is_file() || meta.len() > limit {
        return Err(ReportError::InvalidInput);
    }
    let f = File::open(path).map_err(|_| ReportError::Unsupported)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = f.metadata().map_err(|_| ReportError::Unsupported)?;
        if actual.dev() != meta.dev() || actual.ino() != meta.ino() {
            return Err(ReportError::InvalidInput);
        }
    }
    Ok(BufReader::with_capacity(
        2 * 1024 * 1024,
        RawFile {
            file: f,
            calls: 0,
            bytes: 0,
            limit: limit + 1,
            deadline,
        },
    ))
}
fn admission() -> AdmissionBudget {
    AdmissionBudget {
        max_resident_bytes: 134217728,
        max_read_calls: 1024,
        max_interruptions: 16,
        max_work_per_artifact: 134217728,
    }
}
fn text_budget(max_tokens: usize) -> TextBudget {
    TextBudget {
        max_resident_bytes: 134217728,
        max_tokens,
        max_work: 2_000_000,
    }
}
fn cpu_budget() -> CpuBudget {
    CpuBudget {
        context: 64,
        max_new_tokens: 8,
        max_resident_bytes: RESIDENT,
        max_work: 80_000_000_000,
    }
}
fn manifest(bytes: &[u8]) -> Result<ModelManifest> {
    ModelManifest::from_bytes(bytes).map_err(|_| ReportError::Unsupported)
}
fn hash(v: &Value) -> Result<[u8; 32]> {
    eylema_platform::sha256(&codec::encode(v).map_err(|_| ReportError::InvalidInput)?)
        .map_err(|_| ReportError::Unsupported)
}
pub struct ModelReply {
    pub output: Value,
    pub receipt: Value,
}
fn reply(
    output: Value,
    m: &ModelManifest,
    input: &Value,
    tokens: usize,
    work: u64,
    peak: u64,
) -> Result<ModelReply> {
    let identity = eylema_platform::sha256(&m.to_bytes().map_err(|_| ReportError::Unsupported)?)
        .map_err(|_| ReportError::Unsupported)?;
    let weight = m
        .fields()
        .components
        .iter()
        .find(|c| c.name == "model.safetensors")
        .ok_or(ReportError::Unsupported)?;
    let receipt = Value::List(vec![
        Value::Bytes(identity.to_vec()),
        Value::Bytes(weight.sha256.to_vec()),
        Value::Bytes(hash(input)?.to_vec()),
        Value::Bytes(hash(&output)?.to_vec()),
        Value::Unsigned(tokens as u64),
        Value::Unsigned(work),
        Value::Unsigned(peak),
    ]);
    Ok(ModelReply { output, receipt })
}
/// This function is reached only in the owned worker, never in ordinary tests.
pub fn compute(
    request: &ModelRequest,
    assets: &AssetPaths,
    group: &CpuWorkGroup,
    deadline: Instant,
    controls: &super::control::Controls,
) -> Result<Vec<ModelReply>> {
    request.validate()?;
    if assets.paths.len() != 9 {
        return Err(ReportError::InvalidInput);
    }
    let make = |limit| {
        controls.check()?;
        let c =
            CpuControl::new_job(deadline, limit, group.clone()).map_err(|_| ReportError::Limit)?;
        controls.register(c.clone())?;
        Ok(c)
    };
    let mut replies = vec![];
    match request {
        ModelRequest::Documents(texts) | ModelRequest::Queries(texts) => {
            let m = manifest(EMBEDDING)?;
            let layout = embedding_layout(&m).map_err(|_| ReportError::Unsupported)?;
            let tok = EmbeddingTokenizer::load(
                &mut file(&assets.paths[7], 11_423_705, deadline)?,
                &mut file(&assets.paths[8], 9706, deadline)?,
                &m,
                admission(),
            )
            .map_err(|_| ReportError::InvalidInput)?;
            let weights = load(
                &mut file(&assets.paths[6], 1_191_586_416, deadline)?,
                &m,
                &layout,
                LoadBudget {
                    max_artifact_bytes: 1_191_586_416,
                    max_resident_content_bytes: 1_191_586_416 + 4194304,
                    max_read_calls: 65568,
                    max_interruptions: 16,
                },
            )
            .map_err(|_| ReportError::InvalidInput)?;
            let b = EmbeddingBudget {
                context: 4096,
                max_resident_bytes: RESIDENT,
                max_work: 650_000_000_000,
            };
            let c = make(b.max_work)?;
            let model = EmbeddingCpuModel::admit(&m, &weights, &tok, b, &c)
                .map_err(|_| ReportError::Limit)?;
            for text in texts {
                let control = make(b.max_work)?;
                let output = if matches!(request, ModelRequest::Queries(_)) {
                    model.embed_query(text, b, control)
                } else {
                    model.embed_document(text, b, control)
                }
                .map_err(|_| ReportError::WorkerFailed)?;
                let mut bytes = Vec::with_capacity(4096);
                for x in output.vector() {
                    bytes.extend_from_slice(&x.to_le_bytes());
                }
                replies.push(reply(
                    Value::Bytes(bytes),
                    &m,
                    &Value::Text(text.clone()),
                    output.receipt().input_tokens(),
                    output.receipt().work(),
                    output.receipt().peak_logical_bytes(),
                )?);
            }
        }
        ModelRequest::Plan(texts) => {
            let m = manifest(PLANNING)?;
            let parent = manifest(PARENT)?;
            let tok = Tokenizer::load(
                &mut file(&assets.paths[1], 16_777_216, deadline)?,
                &mut file(&assets.paths[2], 65536, deadline)?,
                &parent,
                admission(),
            )
            .map_err(|_| ReportError::InvalidInput)?;
            for text in texts {
                tok.encode_prompt(
                    &[PromptMessage {
                        role: PromptRole::User,
                        content: text,
                    }],
                    text_budget(56),
                )
                .map_err(|_| ReportError::Limit)?;
            }
            let layout =
                eylema_model::cpu::qwen_layout(&m).map_err(|_| ReportError::Unsupported)?;
            let weights = load(
                &mut file(&assets.paths[0], 1_503_300_328, deadline)?,
                &m,
                &layout,
                LoadBudget {
                    max_artifact_bytes: 1_503_300_328,
                    max_resident_content_bytes: 1_503_300_328 + 4194304,
                    max_read_calls: 65568,
                    max_interruptions: 16,
                },
            )
            .map_err(|_| ReportError::InvalidInput)?;
            let c = make(80_000_000_000)?;
            let model = CpuModel::admit(&m, &weights, &tok, cpu_budget(), &c)
                .map_err(|_| ReportError::Limit)?;
            for text in texts {
                let output = model
                    .generate(
                        CpuRoleRequest::Planning(&[PromptMessage {
                            role: PromptRole::User,
                            content: text,
                        }]),
                        cpu_budget(),
                        make(80_000_000_000)?,
                    )
                    .map_err(|_| ReportError::WorkerFailed)?;
                let CpuRoleOutput::PlanProposal(value) = output.output() else {
                    return Err(ReportError::InvalidWitness);
                };
                replies.push(reply(
                    Value::Text(value.clone()),
                    &m,
                    &Value::Text(text.clone()),
                    output.receipt().input_tokens(),
                    output.receipt().work(),
                    output.receipt().peak_logical_bytes(),
                )?);
            }
        }
        ModelRequest::Decide(inputs) => {
            let m = manifest(DECISION)?;
            let c = make(1_000_000_000_000)?;
            let tok = DecisionTokenizer::load(
                &mut file(&assets.paths[4], 16_777_216, deadline)?,
                &mut file(&assets.paths[5], 65536, deadline)?,
                &m,
                admission(),
                &c,
            )
            .map_err(|_| ReportError::InvalidInput)?;
            let layout = decision_layout(&m).map_err(|_| ReportError::Unsupported)?;
            let weights = load(
                &mut file(&assets.paths[3], 1_191_588_280, deadline)?,
                &m,
                &layout,
                LoadBudget {
                    max_artifact_bytes: 1_191_588_280,
                    max_resident_content_bytes: 1_191_588_280 + 4194304,
                    max_read_calls: 65568,
                    max_interruptions: 16,
                },
            )
            .map_err(|_| ReportError::InvalidInput)?;
            let budget = DecisionBudget {
                context: 4096,
                max_resident_bytes: RESIDENT,
                max_work: 1_000_000_000_000,
            };
            let model = DecisionModel::admit(&m, &weights, &tok, budget, &c)
                .map_err(|_| ReportError::Limit)?;
            let options: Vec<Vec<DecisionOption<'_>>> = inputs
                .iter()
                .map(|q| {
                    q.options
                        .iter()
                        .map(|(id, text)| DecisionOption {
                            id,
                            candidate: text,
                        })
                        .collect()
                })
                .collect();
            let ids: Vec<String> = (0..inputs.len()).map(|i| format!("question:{i}")).collect();
            let questions: Vec<DecisionQuestion<'_>> = inputs
                .iter()
                .enumerate()
                .map(|(i, q)| DecisionQuestion {
                    id: &ids[i],
                    instruction: "Judge whether the candidate directly answers the query.",
                    query: &q.query,
                    options: &options[i],
                })
                .collect();
            let batch = DecisionBatch::new(&questions).map_err(|_| ReportError::InvalidInput)?;
            let result = model
                .score(&batch, budget, c.clone())
                .map_err(|_| ReportError::WorkerFailed)?;
            let rows = result
                .questions()
                .iter()
                .map(|q| {
                    Value::List(vec![
                        Value::Text(q.question_id().into()),
                        q.choice().map_or(Value::Null, |s| Value::Text(s.into())),
                        Value::List(
                            q.scores()
                                .iter()
                                .map(|s| {
                                    Value::List(vec![
                                        Value::Text(s.option_id().into()),
                                        Value::Unsigned(s.score_micros() as u64),
                                    ])
                                })
                                .collect(),
                        ),
                    ])
                })
                .collect();
            replies.push(reply(
                Value::List(rows),
                &m,
                &request.value(),
                0,
                result.receipt().work(),
                result.receipt().peak_logical_bytes(),
            )?);
        }
    }
    Ok(replies)
}
