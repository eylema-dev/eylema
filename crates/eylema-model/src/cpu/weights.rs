use super::{
    CpuError as E,
    budget::Meter,
    profile::{Dimensions, name},
};
use crate::LoadedWeights;
pub(crate) fn bf16(bits: u16) -> Result<f32, E> {
    let x = f32::from_bits(u32::from(bits) << 16);
    if x.is_finite() {
        Ok(x)
    } else {
        Err(E::NonFinite)
    }
}
#[derive(Clone, Copy)]
enum Storage<'a> {
    Bf16(&'a [u8]),
    #[cfg(test)]
    F32(&'a [f32]),
}
#[derive(Clone, Copy)]
pub(crate) struct Tensor<'a>(Storage<'a>);
impl Tensor<'_> {
    pub fn len(self) -> usize {
        match self.0 {
            Storage::Bf16(b) => b.len() / 2,
            #[cfg(test)]
            Storage::F32(v) => v.len(),
        }
    }
    pub fn at(self, i: usize) -> f32 {
        match self.0 {
            Storage::Bf16(b) => {
                f32::from_bits(u32::from(u16::from_le_bytes([b[2 * i], b[2 * i + 1]])) << 16)
            }
            #[cfg(test)]
            Storage::F32(v) => v[i],
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Layer<'a> {
    pub input: Tensor<'a>,
    pub post: Tensor<'a>,
    pub q: Tensor<'a>,
    pub k: Tensor<'a>,
    pub v: Tensor<'a>,
    pub o: Tensor<'a>,
    pub qnorm: Tensor<'a>,
    pub knorm: Tensor<'a>,
    pub gate: Tensor<'a>,
    pub up: Tensor<'a>,
    pub down: Tensor<'a>,
}
#[derive(Clone, Copy)]
pub(crate) struct BoundWeights<'a> {
    pub embedding: Tensor<'a>,
    pub norm: Tensor<'a>,
    pub layers: [Option<Layer<'a>>; 28],
}
impl<'a> BoundWeights<'a> {
    fn bind<F>(d: Dimensions, mut get: F) -> Result<Self, E>
    where
        F: FnMut(&str, usize) -> Result<Tensor<'a>, E>,
    {
        d.validate()?;
        let embedding = get("model.embed_tokens.weight", d.vocab * d.h)?;
        let norm = get("model.norm.weight", d.h)?;
        let mut layers = [None; 28];
        for (l, slot) in layers.iter_mut().enumerate().take(d.layers) {
            *slot = Some(Layer {
                input: get(&name(l, "input_layernorm.weight")?, d.h)?,
                post: get(&name(l, "post_attention_layernorm.weight")?, d.h)?,
                q: get(&name(l, "self_attn.q_proj.weight")?, d.qh * d.w * d.h)?,
                k: get(&name(l, "self_attn.k_proj.weight")?, d.kh * d.w * d.h)?,
                v: get(&name(l, "self_attn.v_proj.weight")?, d.kh * d.w * d.h)?,
                o: get(&name(l, "self_attn.o_proj.weight")?, d.h * d.qh * d.w)?,
                qnorm: get(&name(l, "self_attn.q_norm.weight")?, d.w)?,
                knorm: get(&name(l, "self_attn.k_norm.weight")?, d.w)?,
                gate: get(&name(l, "mlp.gate_proj.weight")?, d.inter * d.h)?,
                up: get(&name(l, "mlp.up_proj.weight")?, d.inter * d.h)?,
                down: get(&name(l, "mlp.down_proj.weight")?, d.h * d.inter)?,
            });
        }
        Ok(Self {
            embedding,
            norm,
            layers,
        })
    }
    pub fn loaded(d: Dimensions, w: &'a LoadedWeights) -> Result<Self, E> {
        Self::bind(d, |name, n| {
            let b = w.tensor_bytes(name).ok_or(E::InvalidLayout)?;
            if b.len() != 2 * n {
                return Err(E::InvalidLayout);
            }
            Ok(Tensor(Storage::Bf16(b)))
        })
    }
    #[cfg(test)]
    pub fn synthetic(
        d: Dimensions,
        map: &'a std::collections::BTreeMap<String, Vec<f32>>,
    ) -> Result<Self, E> {
        Self::bind(d, |name, n| {
            let x = map.get(name).ok_or(E::InvalidLayout)?;
            if x.len() != n || x.iter().any(|x| !x.is_finite()) {
                return Err(E::InvalidLayout);
            }
            Ok(Tensor(Storage::F32(x)))
        })
    }
}
pub(crate) fn scan(
    w: &LoadedWeights,
    layout: &crate::WeightLayout,
    m: &mut Meter,
) -> Result<(), E> {
    scan_stored(w, layout, m)?;
    let a = w
        .tensor_bytes("model.embed_tokens.weight")
        .ok_or(E::InvalidLayout)?;
    let b = w.tensor_bytes("lm_head.weight").ok_or(E::InvalidLayout)?;
    if a.len() != b.len() {
        return Err(E::InvalidLayout);
    }
    m.preflight(a.len() as u64)?;
    for (x, y) in a.chunks(4096).zip(b.chunks(4096)) {
        m.charge(x.len() as u64)?;
        if x != y {
            return Err(E::InvalidLayout);
        }
    }
    Ok(())
}

pub(crate) fn scan_stored(
    w: &LoadedWeights,
    layout: &crate::WeightLayout,
    m: &mut Meter,
) -> Result<(), E> {
    m.preflight(w.artifact_len().checked_mul(2).ok_or(E::WorkLimit)?)?;
    for t in layout.tensors() {
        let data = w.tensor_bytes(t.name()).ok_or(E::InvalidLayout)?;
        for chunk in data.chunks(2048) {
            m.charge((chunk.len() * 2) as u64)?;
            for pair in chunk.as_chunks::<2>().0 {
                bf16(u16::from_le_bytes([pair[0], pair[1]]))?;
            }
        }
    }
    Ok(())
}
