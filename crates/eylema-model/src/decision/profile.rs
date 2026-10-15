use crate::{
    StoredDtype, TensorSpec, WeightLayout,
    cpu::{
        CpuError as E,
        profile::{Dimensions, hex, name},
    },
};
use eylema_contracts::model_contract::ModelManifest;
pub(crate) const SHA: [u8; 32] =
    hex(b"c47cc3f8008e4735ab43acfde124b884cab99f9206913cbecfc79ab06e2d0cdc");
pub(crate) fn dimensions() -> Dimensions {
    Dimensions {
        h: 1024,
        inter: 3072,
        qh: 16,
        kh: 8,
        w: 128,
        layers: 28,
        vocab: 151669,
    }
}

pub(crate) fn workspace_reservation(context: usize) -> Result<(u64, u64), E> {
    let d = dimensions();
    d.validate()?;
    let cache = d.cache(context)?;
    let lengths = [
        d.h,
        d.h,
        d.qh * d.w,
        d.kh * d.w,
        d.kh * d.w,
        d.qh * d.w,
        d.h,
        d.h,
        d.inter,
        d.inter,
        d.inter,
        d.h,
        d.h,
        2,
        cache,
        cache,
        context,
        d.layers * d.h,
    ];
    let elements = lengths.iter().try_fold(0u64, |sum, n| {
        sum.checked_add(*n as u64).ok_or(E::ResidentLimit)
    })?;
    let bytes = elements.checked_mul(4).ok_or(E::ResidentLimit)?;
    Ok((bytes, elements))
}
pub fn decision_layout(manifest: &ModelManifest) -> Result<WeightLayout, E> {
    let wire = manifest.to_bytes().map_err(|_| E::UnsupportedProfile)?;
    if eylema_platform::sha256(&wire).map_err(|_| E::PlatformUnavailable)? != SHA {
        return Err(E::UnsupportedProfile);
    }
    let mut v = Vec::new();
    v.try_reserve_exact(310).map_err(|_| E::AllocationFailed)?;
    for (n, s) in [
        ("model.embed_tokens.weight", &[151669, 1024][..]),
        ("model.norm.weight", &[1024][..]),
    ] {
        v.push(TensorSpec::new(n, StoredDtype::BF16, s).map_err(|_| E::InvalidLayout)?)
    }
    for l in 0..28 {
        for (n, s) in [
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
            v.push(
                TensorSpec::new(&name(l, n)?, StoredDtype::BF16, s)
                    .map_err(|_| E::InvalidLayout)?,
            );
        }
    }
    WeightLayout::new(manifest, &v).map_err(|_| E::InvalidLayout)
}
