use crate::TensorError;

const SPEC: [u8; 32] = [
    0xac, 0xbb, 0xf6, 0x87, 0x55, 0xde, 0xe5, 0xca, 0x2b, 0x9d, 0x25, 0xbd, 0xcd, 0x3a, 0xa4, 0x20,
    0x6c, 0xe7, 0x15, 0x6e, 0x03, 0x2c, 0x38, 0x51, 0x8a, 0x1f, 0xc6, 0x6d, 0x78, 0x19, 0xfb, 0x06,
];
const IDS: [&str; 16] = [
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

#[derive(Debug, Clone, Copy)]
pub struct OperatorKey<'a> {
    pub id: &'a str,
    pub spec_sha256: [u8; 32],
}

/// Complete means primitive identity coverage, never model execution eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveCoverage {
    Complete,
    Partial { present: usize },
}

pub fn reference_coverage(keys: &[OperatorKey<'_>]) -> Result<PrimitiveCoverage, TensorError> {
    if keys.len() > IDS.len() {
        return Err(TensorError::ExtentLimit);
    }
    for (i, key) in keys.iter().enumerate() {
        if !IDS.contains(&key.id) {
            return Err(TensorError::UnsupportedOperator);
        }
        if key.spec_sha256 != SPEC {
            return Err(TensorError::SpecMismatch);
        }
        if keys[..i].iter().any(|previous| previous.id == key.id) {
            return Err(TensorError::DuplicateOperator);
        }
    }
    Ok(if keys.len() == IDS.len() {
        PrimitiveCoverage::Complete
    } else {
        PrimitiveCoverage::Partial {
            present: keys.len(),
        }
    })
}
