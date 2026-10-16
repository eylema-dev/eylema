//! Representation-only owned-Rust CPU execution for the exact accepted G68 profile.
//! It returns vectors and bounded receipts; it grants no retrieval or action authority.
pub(crate) mod profile;

use crate::{
    LoadedWeights, WeightLayout,
    cpu::{CpuControl, CpuError, session::Engine},
    tokenizer::{EmbeddingTokenizer, TextBudget, TokenizeError},
};
use eylema_contracts::model_contract::ModelManifest;

const TOKENIZER_WORKSPACE_RESERVATION: u64 = 134_217_728;
const HEADER_WORKSPACE_RESERVATION: u64 = 4_194_304;
const MODEL_METADATA_RESERVATION: u64 = 131_072;
const MAX_RESIDENT_BYTES: u64 = 2_684_354_560;
const MAX_WORK: u64 = 2_500_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingBudget {
    pub context: usize,
    pub max_resident_bytes: u64,
    pub max_work: u64,
}

impl EmbeddingBudget {
    pub fn validate(self) -> Result<Self, CpuError> {
        if !(1..=profile::CONTEXT_LIMIT).contains(&self.context) {
            return Err(CpuError::ContextLimit);
        }
        if self.max_resident_bytes == 0 || self.max_resident_bytes > MAX_RESIDENT_BYTES {
            return Err(CpuError::ResidentLimit);
        }
        if self.max_work == 0 || self.max_work > MAX_WORK {
            return Err(CpuError::WorkLimit);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingStopReason {
    Complete,
}

pub struct EmbeddingReceipt {
    manifest_sha256: [u8; 32],
    artifact_sha256: [u8; 32],
    tokenizer_sha256: [u8; 32],
    config_sha256: [u8; 32],
    input_tokens: usize,
    work: u64,
    peak_logical_bytes: u64,
    stop_reason: EmbeddingStopReason,
}

impl EmbeddingReceipt {
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    pub fn artifact_sha256(&self) -> [u8; 32] {
        self.artifact_sha256
    }
    pub fn tokenizer_sha256(&self) -> [u8; 32] {
        self.tokenizer_sha256
    }
    pub fn config_sha256(&self) -> [u8; 32] {
        self.config_sha256
    }
    pub fn input_tokens(&self) -> usize {
        self.input_tokens
    }
    pub fn work(&self) -> u64 {
        self.work
    }
    pub fn peak_logical_bytes(&self) -> u64 {
        self.peak_logical_bytes
    }
    pub fn stop_reason(&self) -> EmbeddingStopReason {
        self.stop_reason
    }
}

impl std::fmt::Debug for EmbeddingReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingReceipt")
            .field("input_tokens", &self.input_tokens)
            .field("work", &self.work)
            .field("peak_logical_bytes", &self.peak_logical_bytes)
            .field("stop_reason", &self.stop_reason)
            .finish_non_exhaustive()
    }
}

pub struct EmbeddingCpuOutput {
    vector: Vec<f32>,
    receipt: EmbeddingReceipt,
}

impl EmbeddingCpuOutput {
    pub fn vector(&self) -> &[f32] {
        &self.vector
    }
    pub fn receipt(&self) -> &EmbeddingReceipt {
        &self.receipt
    }
}

impl std::fmt::Debug for EmbeddingCpuOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingCpuOutput")
            .field("dimensions", &self.vector.len())
            .field("receipt", &self.receipt)
            .finish()
    }
}

pub struct EmbeddingCpuModel<'a> {
    weights: crate::cpu::weights::BoundWeights<'a>,
    tokenizer: &'a EmbeddingTokenizer,
    manifest_sha256: [u8; 32],
    artifact_sha256: [u8; 32],
    base_bytes: u64,
    admission_work: u64,
}

impl std::fmt::Debug for EmbeddingCpuModel<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingCpuModel")
            .field("admission_work", &self.admission_work)
            .field("tokenizer_sha256", &self.tokenizer.tokenizer_sha256())
            .finish_non_exhaustive()
    }
}

pub fn embedding_layout(manifest: &ModelManifest) -> Result<WeightLayout, crate::LoadError> {
    profile::layout(manifest)
}

impl<'a> EmbeddingCpuModel<'a> {
    pub fn admit(
        manifest: &'a ModelManifest,
        weights: &'a LoadedWeights,
        tokenizer: &'a EmbeddingTokenizer,
        budget: EmbeddingBudget,
        control: &CpuControl,
    ) -> Result<Self, CpuError> {
        let budget = budget.validate()?;
        control.check()?;
        let layout = embedding_layout(manifest).map_err(map_load_error)?;
        let wire = manifest
            .to_bytes()
            .map_err(|_| CpuError::UnsupportedProfile)?;
        let manifest_sha256 =
            eylema_platform::sha256(&wire).map_err(|_| CpuError::PlatformUnavailable)?;
        if manifest_sha256 != profile::MANIFEST_SHA
            || tokenizer.manifest_sha256() != profile::MANIFEST_SHA
            || tokenizer.tokenizer_sha256() != profile::TOKENIZER_SHA
            || tokenizer.config_sha256() != profile::TOKENIZER_CONFIG_SHA
            || weights.manifest_sha256() != profile::MANIFEST_SHA
            || weights.layout_sha256() != layout.layout_sha256()
            || weights.artifact_sha256() != profile::ARTIFACT_SHA
        {
            return Err(CpuError::IdentityMismatch);
        }
        let base_bytes = weights
            .resident_content_bytes()
            .checked_add(tokenizer.resident_bytes() as u64)
            .and_then(|n| n.checked_add(TOKENIZER_WORKSPACE_RESERVATION))
            .and_then(|n| n.checked_add(HEADER_WORKSPACE_RESERVATION))
            .and_then(|n| n.checked_add(MODEL_METADATA_RESERVATION))
            .ok_or(CpuError::ResidentLimit)?;
        let mut meter = super::cpu::budget::Meter::embedding(
            budget.context,
            budget.max_resident_bytes,
            budget.max_work,
            control.clone(),
            base_bytes,
        )?;
        meter.preflight(MODEL_METADATA_RESERVATION)?;
        crate::cpu::weights::scan_stored(weights, &layout, &mut meter)?;
        let bound = crate::cpu::weights::BoundWeights::loaded_embedding(profile::D, weights)?;
        control.check()?;
        Ok(Self {
            weights: bound,
            tokenizer,
            manifest_sha256,
            artifact_sha256: weights.artifact_sha256(),
            base_bytes,
            admission_work: control.observed_work(),
        })
    }

    pub fn admission_work(&self) -> u64 {
        self.admission_work
    }

    pub fn embed_query(
        &self,
        text: &str,
        budget: EmbeddingBudget,
        control: CpuControl,
    ) -> Result<EmbeddingCpuOutput, CpuError> {
        self.embed(text, true, budget, control)
    }

    pub fn embed_document(
        &self,
        text: &str,
        budget: EmbeddingBudget,
        control: CpuControl,
    ) -> Result<EmbeddingCpuOutput, CpuError> {
        self.embed(text, false, budget, control)
    }

    fn embed(
        &self,
        text: &str,
        query: bool,
        budget: EmbeddingBudget,
        control: CpuControl,
    ) -> Result<EmbeddingCpuOutput, CpuError> {
        let budget = budget.validate()?;
        control.check()?;
        let start_work = control.observed_work();
        let text_budget = TextBudget {
            max_resident_bytes: TOKENIZER_WORKSPACE_RESERVATION as usize,
            max_tokens: budget.context,
            max_work: 2_000_000,
        };
        let tokens = if query {
            self.tokenizer.encode_query(text, text_budget, &control)
        } else {
            self.tokenizer.encode_document(text, text_budget, &control)
        }
        .map_err(map_tokenize_error)?;
        if tokens.manifest_sha256() != self.manifest_sha256 {
            return Err(CpuError::IdentityMismatch);
        }
        if tokens.ids().is_empty() || tokens.ids().len() > budget.context {
            return Err(CpuError::ContextLimit);
        }
        let required = crate::cpu::profile::embedding_work_bound(profile::D, tokens.ids().len())?;
        if control
            .observed_work()
            .checked_add(required)
            .is_none_or(|n| n > budget.max_work)
        {
            return Err(CpuError::WorkLimit);
        }
        let mut engine = Engine::new_embedding(
            profile::D,
            self.weights,
            budget.context,
            budget.max_resident_bytes,
            budget.max_work,
            control.clone(),
            self.base_bytes,
        )?;
        engine.prefill_embedding(tokens.ids())?;
        let vector = engine.normalized_embedding()?;
        control.check()?;
        let end_work = control.observed_work();
        let receipt = EmbeddingReceipt {
            manifest_sha256: self.manifest_sha256,
            artifact_sha256: self.artifact_sha256,
            tokenizer_sha256: self.tokenizer.tokenizer_sha256(),
            config_sha256: self.tokenizer.config_sha256(),
            input_tokens: tokens.ids().len(),
            work: end_work.saturating_sub(start_work),
            peak_logical_bytes: engine.meter.peak,
            stop_reason: EmbeddingStopReason::Complete,
        };
        Ok(EmbeddingCpuOutput { vector, receipt })
    }
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32, CpuError> {
    if left.len() != 1024 || right.len() != 1024 {
        return Err(CpuError::InvalidLayout);
    }
    let mut sum = 0.0f64;
    for (&a, &b) in left.iter().zip(right) {
        if !a.is_finite() || !b.is_finite() {
            return Err(CpuError::NonFinite);
        }
        sum += f64::from(a) * f64::from(b);
    }
    let result = sum as f32;
    if !result.is_finite() || result.abs() > 1.0001 {
        return Err(CpuError::InvalidOutput);
    }
    Ok(result)
}

fn map_tokenize_error(error: TokenizeError) -> CpuError {
    match error {
        TokenizeError::Cancelled => CpuError::Cancelled,
        TokenizeError::Deadline => CpuError::Deadline,
        TokenizeError::UnsupportedProfile | TokenizeError::InvalidProfile => {
            CpuError::UnsupportedProfile
        }
        TokenizeError::HashMismatch => CpuError::IdentityMismatch,
        TokenizeError::InvalidInput
        | TokenizeError::InvalidConfig
        | TokenizeError::InvalidJson
        | TokenizeError::InvalidVocabulary
        | TokenizeError::InvalidMerge
        | TokenizeError::UnsupportedUnicode => CpuError::InvalidInput,
        TokenizeError::TokenLimit => CpuError::ContextLimit,
        TokenizeError::SizeLimit => CpuError::InvalidInput,
        TokenizeError::ResidentLimit => CpuError::ResidentLimit,
        TokenizeError::WorkLimit => CpuError::WorkLimit,
        TokenizeError::AllocationFailed => CpuError::AllocationFailed,
        TokenizeError::PlatformUnavailable | TokenizeError::PlatformFailed | TokenizeError::Io => {
            CpuError::PlatformUnavailable
        }
    }
}

fn map_load_error(error: crate::LoadError) -> CpuError {
    match error {
        crate::LoadError::InvalidProfile => CpuError::UnsupportedProfile,
        crate::LoadError::HashMismatch => CpuError::IdentityMismatch,
        crate::LoadError::SizeLimit | crate::LoadError::ResidentLimit => CpuError::ResidentLimit,
        crate::LoadError::WorkLimit => CpuError::WorkLimit,
        crate::LoadError::AllocationFailed => CpuError::AllocationFailed,
        crate::LoadError::PlatformUnavailable => CpuError::PlatformUnavailable,
        _ => CpuError::InvalidLayout,
    }
}

#[cfg(test)]
fn pool_last_valid(
    rows: &[f32],
    positions: usize,
    width: usize,
    mask: &[u8],
) -> Result<Vec<f32>, CpuError> {
    if width == 0
        || positions == 0
        || mask.len() != positions
        || rows.len()
            != positions
                .checked_mul(width)
                .ok_or(CpuError::InvalidLayout)?
        || mask.iter().any(|&x| x > 1)
    {
        return Err(CpuError::InvalidLayout);
    }
    let first = mask
        .iter()
        .position(|&x| x == 1)
        .ok_or(CpuError::InvalidInput)?;
    let last = mask
        .iter()
        .rposition(|&x| x == 1)
        .ok_or(CpuError::InvalidInput)?;
    if mask[first..=last].iter().any(|&x| x != 1) {
        return Err(CpuError::InvalidInput);
    }
    let start = last.checked_mul(width).ok_or(CpuError::InvalidLayout)?;
    Ok(rows[start..start + width].to_vec())
}

#[cfg(test)]
mod tests;
