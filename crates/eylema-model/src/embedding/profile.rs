use crate::{StoredDtype, TensorSpec, WeightLayout, cpu::profile::Dimensions};

pub(super) const MANIFEST_SHA: [u8; 32] =
    hex(b"ca3cde459437692c9ae877b66cf4d15bab58c0852ec2f0fc7dc08e7003fdc71f");
pub(super) const TOKENIZER_SHA: [u8; 32] =
    hex(b"def76fb086971c7867b829c23a26261e38d9d74e02139253b38aeb9df8b4b50a");
pub(super) const TOKENIZER_CONFIG_SHA: [u8; 32] =
    hex(b"253153d0738ceb4c668d2eff957714dd2bea0b56de772a9fdccd96cbf517e6a0");
pub(super) const ARTIFACT_SHA: [u8; 32] =
    hex(b"0437e45c94563b09e13cb7a64478fc406947a93cb34a7e05870fc8dcd48e23fd");
pub(super) const ARTIFACT_SIZE: u64 = 1_191_586_416;
#[cfg(test)]
pub(super) const TOKENIZER_SIZE: usize = 11_423_705;
#[cfg(test)]
pub(super) const TOKENIZER_CONFIG_SIZE: usize = 9_706;
pub(super) const CONTEXT_LIMIT: usize = 4_096;
pub(crate) const QUERY_PREFIX: &str =
    "Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery:";

pub(super) const D: Dimensions = Dimensions {
    h: 1024,
    inter: 3072,
    qh: 16,
    kh: 8,
    w: 128,
    layers: 28,
    vocab: 151_669,
};

pub fn layout(
    manifest: &eylema_contracts::model_contract::ModelManifest,
) -> Result<WeightLayout, crate::LoadError> {
    let wire = manifest
        .to_bytes()
        .map_err(|_| crate::LoadError::InvalidProfile)?;
    if eylema_platform::sha256(&wire)? != MANIFEST_SHA {
        return Err(crate::LoadError::InvalidProfile);
    }
    let fields = manifest.fields();
    if fields.id.as_str() != "qwen3-embedding-0.6b"
        || fields.revision.get() != 1
        || fields.architecture.as_str() != "qwen3-dense.v1"
        || fields.numeric_format.as_str() != "bfloat16"
        || fields.quantization.as_str() != "none"
        || fields.configuration.batch_size != 1
        || fields.configuration.context_limit != 4096
        || fields.configuration.weights_component.as_str() != "model.safetensors"
        || fields.configuration.tokenizer_component.as_deref() != Some("tokenizer.json")
        || fields.configuration.tokenizer_config_component.as_deref()
            != Some("tokenizer_config.json")
    {
        return Err(crate::LoadError::InvalidProfile);
    }
    let dims = &fields.configuration.dimensions;
    for (name, expected) in [
        ("hidden_size", 1024),
        ("intermediate_size", 3072),
        ("attention_heads", 16),
        ("kv_heads", 8),
        ("head_dim", 128),
        ("layers", 28),
        ("vocab_size", 151_669),
    ] {
        if dims.iter().find(|(n, _)| n == name).map(|(_, v)| *v) != Some(expected) {
            return Err(crate::LoadError::InvalidProfile);
        }
    }
    let artifact = fields
        .components
        .iter()
        .find(|c| c.name == "model.safetensors")
        .ok_or(crate::LoadError::InvalidProfile)?;
    if artifact.size != ARTIFACT_SIZE || artifact.sha256 != ARTIFACT_SHA {
        return Err(crate::LoadError::InvalidProfile);
    }

    let mut specs = Vec::new();
    specs
        .try_reserve_exact(310)
        .map_err(|_| crate::LoadError::AllocationFailed)?;
    for (name, shape) in [
        ("embed_tokens.weight", &[151_669, 1024][..]),
        ("norm.weight", &[1024][..]),
    ] {
        specs.push(TensorSpec::new(name, StoredDtype::BF16, shape)?);
    }
    for layer in 0..28 {
        for (suffix, shape) in [
            ("input_layernorm.weight", &[1024][..]),
            ("post_attention_layernorm.weight", &[1024][..]),
            ("self_attn.q_proj.weight", &[2048, 1024][..]),
            ("self_attn.k_proj.weight", &[1024, 1024][..]),
            ("self_attn.v_proj.weight", &[1024, 1024][..]),
            ("self_attn.o_proj.weight", &[1024, 2048][..]),
            ("self_attn.q_norm.weight", &[128][..]),
            ("self_attn.k_norm.weight", &[128][..]),
            ("mlp.gate_proj.weight", &[3072, 1024][..]),
            ("mlp.up_proj.weight", &[3072, 1024][..]),
            ("mlp.down_proj.weight", &[1024, 3072][..]),
        ] {
            let name = crate::cpu::profile::name(layer, suffix)
                .map_err(|_| crate::LoadError::AllocationFailed)?;
            specs.push(TensorSpec::new(
                name.strip_prefix("model.")
                    .ok_or(crate::LoadError::InvalidLayout)?,
                StoredDtype::BF16,
                shape,
            )?);
        }
    }
    WeightLayout::new(manifest, &specs)
}

const fn hex(s: &[u8; 64]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut i = 0;
    while i < 32 {
        let a = s[i * 2];
        let b = s[i * 2 + 1];
        out[i] = ((if a <= b'9' { a - b'0' } else { a - b'a' + 10 }) << 4)
            | (if b <= b'9' { b - b'0' } else { b - b'a' + 10 });
        i += 1;
    }
    out
}
