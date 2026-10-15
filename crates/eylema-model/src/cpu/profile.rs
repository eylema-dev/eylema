use super::CpuError as E;
use crate::{StoredDtype, TensorSpec, WeightLayout};
use eylema_contracts::model_contract::ModelManifest;
use std::fmt::Write;
pub(crate) const CPU_SHA: [u8; 32] =
    hex(b"abc9669adb66de6ca55e86f5a6793e6b6f2a694eee260f63dcc7120794b7fcc5");
pub(crate) const PARENT_SHA: [u8; 32] =
    hex(b"c087a033d4d193acd9912d0f555c99f14f9b968c9f308aca8acb52b07af85af2");
pub(crate) const fn hex(s: &[u8; 64]) -> [u8; 32] {
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
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dimensions {
    pub h: usize,
    pub inter: usize,
    pub qh: usize,
    pub kh: usize,
    pub w: usize,
    pub layers: usize,
    pub vocab: usize,
}
pub(crate) const QWEN: Dimensions = Dimensions {
    h: 1024,
    inter: 3072,
    qh: 16,
    kh: 8,
    w: 128,
    layers: 28,
    vocab: 151936,
};
impl Dimensions {
    pub fn validate(&self) -> Result<(), E> {
        if self.h == 0
            || self.h > 1024
            || self.inter == 0
            || self.inter > 3072
            || self.qh == 0
            || self.qh > 16
            || self.kh == 0
            || self.kh > self.qh
            || !self.qh.is_multiple_of(self.kh)
            || self.w == 0
            || self.w > 128
            || !self.w.is_multiple_of(2)
            || self.layers == 0
            || self.layers > 28
            || self.vocab == 0
            || self.vocab > 151936
        {
            return Err(E::InvalidLayout);
        }
        Ok(())
    }
    pub fn cache(&self, context: usize) -> Result<usize, E> {
        self.layers
            .checked_mul(context)
            .and_then(|v| v.checked_mul(self.kh))
            .and_then(|v| v.checked_mul(self.w))
            .ok_or(E::ResidentLimit)
    }
}
pub(crate) fn name(layer: usize, suffix: &str) -> Result<String, E> {
    let mut s = String::new();
    s.try_reserve_exact(128).map_err(|_| E::AllocationFailed)?;
    if layer >= 28 || suffix.len() > 80 {
        return Err(E::InvalidLayout);
    }
    write!(&mut s, "model.layers.{layer}.{suffix}").map_err(|_| E::AllocationFailed)?;
    Ok(s)
}
pub fn qwen_layout(manifest: &ModelManifest) -> Result<WeightLayout, E> {
    let wire = manifest.to_bytes().map_err(|_| E::UnsupportedProfile)?;
    if eylema_platform::sha256(&wire).map_err(|_| E::PlatformUnavailable)? != CPU_SHA {
        return Err(E::UnsupportedProfile);
    }
    let mut specs = Vec::new();
    specs
        .try_reserve_exact(311)
        .map_err(|_| E::AllocationFailed)?;
    for (n, shape) in [
        ("model.embed_tokens.weight", &[151936, 1024][..]),
        ("lm_head.weight", &[151936, 1024][..]),
        ("model.norm.weight", &[1024][..]),
    ] {
        specs.push(TensorSpec::new(n, StoredDtype::BF16, shape).map_err(|_| E::InvalidLayout)?);
    }
    for l in 0..28 {
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
            specs.push(
                TensorSpec::new(&name(l, suffix)?, StoredDtype::BF16, shape)
                    .map_err(|_| E::InvalidLayout)?,
            );
        }
    }
    WeightLayout::new(manifest, &specs).map_err(|_| E::InvalidLayout)
}
