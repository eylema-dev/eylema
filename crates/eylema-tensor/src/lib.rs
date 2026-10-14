#![forbid(unsafe_code)]

mod attention;
mod identity;
mod ops;
mod tensor;

pub use attention::{
    AttentionMask, KvCache, RopeSpec, attention, causal_mask, greedy, repeat_kv, rope, softmax,
};
pub use identity::{OperatorKey, PrimitiveCoverage, reference_coverage};
pub use ops::{
    embedding, linear, reshape, residual_add, rms_norm, silu, swiglu, tied_logits, transpose,
};
pub use tensor::{Layout, Tensor, TensorError, validate_layout};
