use eylema_tensor::{OperatorKey, PrimitiveCoverage, TensorError, reference_coverage};

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

#[test]
fn identity_distinguishes_complete_partial_and_invalid_primitive_sets() {
    let keys: Vec<_> = IDS
        .iter()
        .map(|id| OperatorKey {
            id,
            spec_sha256: SPEC,
        })
        .collect();
    assert_eq!(reference_coverage(&keys), Ok(PrimitiveCoverage::Complete));
    assert_eq!(
        reference_coverage(&keys[..15]),
        Ok(PrimitiveCoverage::Partial { present: 15 })
    );
    assert_eq!(
        reference_coverage(&[]),
        Ok(PrimitiveCoverage::Partial { present: 0 })
    );
    assert_eq!(
        reference_coverage(&[keys[0], keys[0]]),
        Err(TensorError::DuplicateOperator)
    );
    assert_eq!(
        reference_coverage(&[keys[0]; 17]),
        Err(TensorError::ExtentLimit)
    );
    assert_eq!(
        reference_coverage(&[OperatorKey {
            id: IDS[0],
            spec_sha256: [0; 32]
        }]),
        Err(TensorError::SpecMismatch)
    );
    assert_eq!(
        reference_coverage(&[OperatorKey {
            id: "other.architecture.v1",
            spec_sha256: SPEC
        }]),
        Err(TensorError::UnsupportedOperator)
    );
}
