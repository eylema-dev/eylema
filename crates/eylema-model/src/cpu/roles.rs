use super::{CpuBudget, CpuControl, CpuError as E, CpuModel, ops, profile, session::Engine};
use crate::tokenizer::{PromptMessage, PromptRole, TextBudget, TokenizeError};
use eylema_contracts::platform::{RoleInterface, RoleKind, SemanticProfileId};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Eos,
    TokenLimit,
}
pub enum CpuRoleRequest<'a> {
    Reasoning(&'a [PromptMessage<'a>]),
    Planning(&'a [PromptMessage<'a>]),
    Specialist(&'a [PromptMessage<'a>]),
    Critic {
        candidate: &'a str,
        evidence: &'a str,
    },
}
pub enum CpuRoleOutput {
    ReasoningSummary(String),
    PlanProposal(String),
    DomainResult(String),
    EvaluationProposal(String),
}
impl std::fmt::Debug for CpuRoleOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, n) = match self {
            Self::ReasoningSummary(s) => ("ReasoningSummary", s.len()),
            Self::PlanProposal(s) => ("PlanProposal", s.len()),
            Self::DomainResult(s) => ("DomainResult", s.len()),
            Self::EvaluationProposal(s) => ("EvaluationProposal", s.len()),
        };
        f.debug_struct(kind).field("utf8_bytes", &n).finish()
    }
}
impl std::fmt::Debug for CpuRoleRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, n) = match self {
            Self::Reasoning(m) => ("Reasoning", m.len()),
            Self::Planning(m) => ("Planning", m.len()),
            Self::Specialist(m) => ("Specialist", m.len()),
            Self::Critic {
                candidate,
                evidence,
            } => ("Critic", candidate.len() + evidence.len()),
        };
        f.debug_struct(kind).field("input_count", &n).finish()
    }
}
pub struct CpuReceipt {
    role_interface: RoleInterface,
    artifact_sha256: [u8; 32],
    tokenizer_sha256: [u8; 32],
    input_tokens: usize,
    generated_tokens: usize,
    work: u64,
    peak_bytes: u64,
    stop: StopReason,
    context: usize,
}
impl CpuReceipt {
    pub fn interface(&self) -> &RoleInterface {
        &self.role_interface
    }
    pub fn manifest_sha256(&self) -> [u8; 32] {
        profile::CPU_SHA
    }
    pub fn tokenizer_parent_sha256(&self) -> [u8; 32] {
        profile::PARENT_SHA
    }
    pub fn artifact_sha256(&self) -> [u8; 32] {
        self.artifact_sha256
    }
    pub fn tokenizer_sha256(&self) -> [u8; 32] {
        self.tokenizer_sha256
    }
    pub fn input_tokens(&self) -> usize {
        self.input_tokens
    }
    pub fn generated_tokens(&self) -> usize {
        self.generated_tokens
    }
    pub fn work(&self) -> u64 {
        self.work
    }
    pub fn peak_logical_bytes(&self) -> u64 {
        self.peak_bytes
    }
    pub fn stop_reason(&self) -> StopReason {
        self.stop
    }
    pub fn context(&self) -> usize {
        self.context
    }
    pub fn numeric_profile(&self) -> &'static str {
        "qwen3.cpu-f32.v1"
    }
    pub fn proposal_only(&self) -> bool {
        true
    }
}
impl std::fmt::Debug for CpuReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuReceipt")
            .field("role", &self.role_interface.role)
            .field("input_tokens", &self.input_tokens)
            .field("generated_tokens", &self.generated_tokens)
            .field("work", &self.work)
            .field("stop", &self.stop)
            .finish_non_exhaustive()
    }
}
pub struct CpuProposal {
    output: CpuRoleOutput,
    receipt: CpuReceipt,
}
impl CpuProposal {
    pub fn role(&self) -> RoleKind {
        self.receipt.role_interface.role
    }
    pub fn output(&self) -> &CpuRoleOutput {
        &self.output
    }
    pub fn receipt(&self) -> &CpuReceipt {
        &self.receipt
    }
}
impl std::fmt::Debug for CpuProposal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpuProposal")
            .field("output", &self.output)
            .field("receipt", &self.receipt)
            .finish()
    }
}
fn text_error(e: TokenizeError, output: bool) -> E {
    match e {
        TokenizeError::Cancelled => E::Cancelled,
        TokenizeError::Deadline => E::Deadline,
        TokenizeError::WorkLimit => E::WorkLimit,
        TokenizeError::ResidentLimit => E::ResidentLimit,
        TokenizeError::AllocationFailed => E::AllocationFailed,
        TokenizeError::PlatformUnavailable => E::PlatformUnavailable,
        TokenizeError::TokenLimit => E::ContextLimit,
        _ => {
            if output {
                E::InvalidOutput
            } else {
                E::InvalidInput
            }
        }
    }
}
pub(super) fn validate_output(text: &str) -> Result<(), E> {
    if text.trim().is_empty()
        || text.contains("<|")
        || text.contains("<think>")
        || text.contains("</think>")
        || text.contains('\0')
    {
        Err(E::InvalidOutput)
    } else {
        Ok(())
    }
}
pub(super) fn make_output(role: RoleKind, text: String) -> Result<CpuRoleOutput, E> {
    validate_output(&text)?;
    match role {
        RoleKind::Reasoning => Ok(CpuRoleOutput::ReasoningSummary(text)),
        RoleKind::Planning => Ok(CpuRoleOutput::PlanProposal(text)),
        RoleKind::Specialist => Ok(CpuRoleOutput::DomainResult(text)),
        RoleKind::Critic => Ok(CpuRoleOutput::EvaluationProposal(text)),
        _ => Err(E::UnsupportedProfile),
    }
}
pub(super) fn evidence_json(
    candidate: &str,
    evidence: &str,
    control: &CpuControl,
) -> Result<String, E> {
    control.check()?;
    if candidate.len() > 1024 || evidence.len() > 1024 {
        return Err(E::InvalidInput);
    }
    let maximum = (candidate.len() + evidence.len()) * 6 + 32;
    control.charge(maximum as u64)?;
    let mut out = String::new();
    out.try_reserve_exact(maximum)
        .map_err(|_| E::AllocationFailed)?;
    fn quote(out: &mut String, s: &str, c: &CpuControl) -> Result<(), E> {
        out.push('"');
        for ch in s.chars() {
            c.check()?;
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                x if x < ' ' => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    out.push_str("\\u00");
                    out.push(HEX[(x as usize) >> 4] as char);
                    out.push(HEX[(x as usize) & 15] as char);
                }
                x => out.push(x),
            }
        }
        out.push('"');
        Ok(())
    }
    out.push_str("{\"candidate\":");
    quote(&mut out, candidate, control)?;
    out.push_str(",\"evidence\":");
    quote(&mut out, evidence, control)?;
    out.push('}');
    control.check()?;
    Ok(out)
}
pub(super) fn sample(
    engine: &mut Engine<'_>,
    prompt: &[u32],
    eos: u32,
) -> Result<(Vec<u32>, StopReason), E> {
    let result = (|| {
        engine.prefill_ids(prompt)?;
        let mut ids = Vec::new();
        engine
            .meter
            .hold((engine.meter.budget.max_new_tokens * 4) as u64)?;
        ids.try_reserve_exact(engine.meter.budget.max_new_tokens)
            .map_err(|_| E::AllocationFailed)?;
        let mut stop = StopReason::TokenLimit;
        for step in 0..engine.meter.budget.max_new_tokens {
            let mut best = 0usize;
            for (block, xs) in engine.ws.logits.chunks(1024).enumerate() {
                engine.meter.charge((xs.len() * 2 + 1) as u64)?;
                let local = ops::greedy(xs)? as usize + block * 1024;
                if engine.ws.logits[local] > engine.ws.logits[best] {
                    best = local;
                }
            }
            engine.meter.control.check()?;
            if best as u32 == eos {
                stop = StopReason::Eos;
                break;
            }
            ids.push(best as u32);
            if step + 1 < engine.meter.budget.max_new_tokens {
                engine.decode_id(best as u32)?;
            }
        }
        engine.meter.control.check()?;
        Ok((ids, stop))
    })();
    if result.is_err() {
        engine.valid = false;
    }
    result
}
fn copy_id(id: &SemanticProfileId) -> Result<SemanticProfileId, E> {
    let mut value = String::new();
    value
        .try_reserve_exact(id.as_str().len())
        .map_err(|_| E::AllocationFailed)?;
    value.push_str(id.as_str());
    SemanticProfileId::new(value).map_err(|_| E::IdentityMismatch)
}
fn copy_interface(i: &RoleInterface) -> Result<RoleInterface, E> {
    Ok(RoleInterface {
        role: i.role,
        version: i.version,
        input_schema: copy_id(&i.input_schema)?,
        output_schema: copy_id(&i.output_schema)?,
        input_units: copy_id(&i.input_units)?,
        output_units: copy_id(&i.output_units)?,
        preprocessing: copy_id(&i.preprocessing)?,
        action_semantics: copy_id(&i.action_semantics)?,
        state_reset: copy_id(&i.state_reset)?,
    })
}
impl CpuModel<'_> {
    pub fn generate(
        &self,
        request: CpuRoleRequest<'_>,
        budget: CpuBudget,
        control: CpuControl,
    ) -> Result<CpuProposal, E> {
        budget.validate()?;
        control.check()?;
        if self.base_bytes > budget.max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        if budget.max_work < profile::QWEN.minimum_forward_work()? {
            return Err(E::WorkLimit);
        }
        control.restrict_work(budget.max_work)?;
        match request {
            CpuRoleRequest::Reasoning(m) => {
                self.generate_messages(RoleKind::Reasoning, m, budget, control)
            }
            CpuRoleRequest::Planning(m) => {
                self.generate_messages(RoleKind::Planning, m, budget, control)
            }
            CpuRoleRequest::Specialist(m) => {
                self.generate_messages(RoleKind::Specialist, m, budget, control)
            }
            CpuRoleRequest::Critic {
                candidate,
                evidence,
            } => {
                let json = evidence_json(candidate, evidence, &control)?;
                let m = [PromptMessage {
                    role: PromptRole::User,
                    content: &json,
                }];
                self.generate_messages(RoleKind::Critic, &m, budget, control)
            }
        }
    }
    fn generate_messages(
        &self,
        role: RoleKind,
        messages: &[PromptMessage<'_>],
        budget: CpuBudget,
        control: CpuControl,
    ) -> Result<CpuProposal, E> {
        let interface = self
            .manifest
            .fields()
            .profiles
            .iter()
            .find(|i| i.role == role)
            .ok_or(E::UnsupportedProfile)?;
        let text_budget = TextBudget {
            max_resident_bytes: 134217728,
            max_tokens: 4096,
            max_work: 2_000_000,
        };
        let tokens = self
            .tokenizer
            .encode_prompt_controlled(messages, text_budget, &control)
            .map_err(|e| text_error(e, false))?;
        if tokens
            .ids()
            .len()
            .checked_add(budget.max_new_tokens)
            .is_none_or(|n| n > budget.context)
        {
            return Err(E::ContextLimit);
        }
        let mut session = self.begin(budget, control.clone())?;
        let (generated, stop) = sample(&mut session.engine, tokens.ids(), 151645)?;
        let text = self
            .tokenizer
            .decode_controlled(&generated, text_budget, &control)
            .map_err(|e| text_error(e, true))?;
        let output = make_output(role, text)?;
        let artifact = self
            .manifest
            .fields()
            .components
            .iter()
            .find(|c| c.name == "model.safetensors")
            .ok_or(E::IdentityMismatch)?
            .sha256;
        let receipt = CpuReceipt {
            role_interface: copy_interface(interface)?,
            artifact_sha256: artifact,
            tokenizer_sha256: self.tokenizer.tokenizer_sha256(),
            input_tokens: tokens.ids().len(),
            generated_tokens: generated.len(),
            work: control.observed_work(),
            peak_bytes: session.peak_logical_bytes(),
            stop,
            context: budget.context,
        };
        control.check()?;
        Ok(CpuProposal { output, receipt })
    }
}
