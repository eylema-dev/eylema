use crate::tensor::{copied, extent, finite, zeroed};
use crate::{Tensor, TensorError};

fn axes4(input: &Tensor) -> Result<[usize; 4], TensorError> {
    if input.shape().len() != 4 {
        return Err(TensorError::InvalidShape);
    }
    let shape = [
        input.shape()[0],
        input.shape()[1],
        input.shape()[2],
        input.shape()[3],
    ];
    bounded_axes(shape)?;
    Ok(shape)
}

fn bounded_axes(shape: [usize; 4]) -> Result<usize, TensorError> {
    let total = extent(&shape)?;
    if shape[0] > 2 || shape[1] > 4 || shape[2] > 16 || shape[3] > 16 {
        return Err(TensorError::ExtentLimit);
    }
    Ok(total)
}

#[derive(Debug)]
pub struct RopeSpec {
    pub positions: Vec<u32>,
    pub theta: f32,
    pub rotary_dim: usize,
}

pub fn rope(input: &Tensor, spec: &RopeSpec) -> Result<Tensor, TensorError> {
    let [batch, heads, tokens, width] = axes4(input)?;
    if spec.positions.len() != tokens {
        return Err(TensorError::InvalidShape);
    }
    if !spec.theta.is_finite()
        || spec.theta <= 0.
        || spec.rotary_dim == 0
        || spec.rotary_dim > width
        || !spec.rotary_dim.is_multiple_of(2)
        || spec.positions.iter().any(|&p| p > 4095)
    {
        return Err(TensorError::InvalidParameter);
    }
    let mut result = copied(input.values())?;
    let half = spec.rotary_dim / 2;
    for b in 0..batch {
        for h in 0..heads {
            for (t, &position) in spec.positions.iter().enumerate() {
                let start = ((b * heads + h) * tokens + t) * width;
                for i in 0..half {
                    let exponent = -((2 * i) as f32) / spec.rotary_dim as f32;
                    let frequency = finite(spec.theta.powf(exponent))?;
                    let angle = finite(position as f32 * frequency)?;
                    let c = finite(angle.cos())?;
                    let s = finite(angle.sin())?;
                    let first = input.values()[start + i];
                    let second = input.values()[start + i + half];
                    result[start + i] = finite(finite(first * c)? - finite(second * s)?)?;
                    result[start + i + half] = finite(finite(second * c)? + finite(first * s)?)?;
                }
            }
        }
    }
    Tensor::new(input.shape(), &result)
}

pub fn repeat_kv(input: &Tensor, query_heads: usize) -> Result<Tensor, TensorError> {
    let [batch, heads, tokens, width] = axes4(input)?;
    if query_heads == 0 || !query_heads.is_multiple_of(heads) {
        return Err(TensorError::InvalidParameter);
    }
    let shape = [batch, query_heads, tokens, width];
    let mut result = zeroed(bounded_axes(shape)?)?;
    let ratio = query_heads / heads;
    let block = tokens * width;
    for b in 0..batch {
        for qh in 0..query_heads {
            let source = (b * heads + qh / ratio) * block;
            let target = (b * query_heads + qh) * block;
            result[target..target + block].copy_from_slice(&input.values()[source..source + block]);
        }
    }
    Tensor::new(&shape, &result)
}

#[derive(Debug, PartialEq)]
pub struct AttentionMask {
    shape: [usize; 4],
    allowed: Vec<bool>,
}

impl AttentionMask {
    pub fn new(shape: [usize; 4], allowed: &[bool]) -> Result<Self, TensorError> {
        if bounded_axes(shape)? != allowed.len() {
            return Err(TensorError::InvalidShape);
        }
        Ok(Self {
            shape,
            allowed: copied(allowed)?,
        })
    }
    pub fn shape(&self) -> &[usize; 4] {
        &self.shape
    }
    pub fn allowed(&self) -> &[bool] {
        &self.allowed
    }
}

pub fn causal_mask(
    batch: usize,
    heads: usize,
    query_positions: &[u32],
    key_positions: &[u32],
    allowed_keys: &[bool],
) -> Result<AttentionMask, TensorError> {
    let shape = [batch, heads, query_positions.len(), key_positions.len()];
    let total = bounded_axes(shape)?;
    if allowed_keys.len() != key_positions.len() {
        return Err(TensorError::InvalidShape);
    }
    if query_positions
        .iter()
        .chain(key_positions)
        .any(|&p| p > 4095)
    {
        return Err(TensorError::InvalidParameter);
    }
    let mut allowed = Vec::new();
    allowed
        .try_reserve_exact(total)
        .map_err(|_| TensorError::AllocationFailed)?;
    for _ in 0..batch * heads {
        for &query in query_positions {
            for (&key, &valid) in key_positions.iter().zip(allowed_keys) {
                allowed.push(valid && key <= query);
            }
        }
    }
    AttentionMask::new(shape, &allowed)
}

pub fn softmax(input: &Tensor, allowed: &[bool]) -> Result<Tensor, TensorError> {
    if allowed.len() != input.values().len() {
        return Err(TensorError::InvalidShape);
    }
    let width = *input.shape().last().ok_or(TensorError::InvalidShape)?;
    let mut result = zeroed(input.values().len())?;
    for ((source, mask), target) in input
        .values()
        .chunks_exact(width)
        .zip(allowed.chunks_exact(width))
        .zip(result.chunks_exact_mut(width))
    {
        let mut max: Option<f32> = None;
        for (&score, &valid) in source.iter().zip(mask) {
            if valid {
                max = Some(max.map_or(score, |current| current.max(score)));
            }
        }
        let max = max.ok_or(TensorError::FullyMasked)?;
        let mut total = 0_f32;
        for ((&score, &valid), weight) in source.iter().zip(mask).zip(target.iter_mut()) {
            if valid {
                *weight = finite(finite(score - max)?.exp())?;
                total = finite(total + *weight)?;
            }
        }
        if total <= 0. {
            return Err(TensorError::NonFinite);
        }
        for value in target {
            *value = finite(*value / total)?;
        }
    }
    Tensor::new(input.shape(), &result)
}

pub fn attention(
    q: &Tensor,
    k: &Tensor,
    v: &Tensor,
    mask: &AttentionMask,
    scale: f32,
) -> Result<Tensor, TensorError> {
    if !scale.is_finite() || scale <= 0. {
        return Err(TensorError::InvalidParameter);
    }
    let [batch, heads, queries, width] = axes4(q)?;
    let [kb, kh, keys, kw] = axes4(k)?;
    let [vb, vh, vkeys, value_width] = axes4(v)?;
    let score_shape = [batch, heads, queries, keys];
    if batch != kb
        || batch != vb
        || heads != kh
        || heads != vh
        || width != kw
        || keys != vkeys
        || mask.shape() != &score_shape
    {
        return Err(TensorError::InvalidShape);
    }
    let mut scores = zeroed(bounded_axes(score_shape)?)?;
    for b in 0..batch {
        for h in 0..heads {
            for qi in 0..queries {
                for ki in 0..keys {
                    let mut sum = 0_f32;
                    for d in 0..width {
                        let product = finite(
                            q.values()[((b * heads + h) * queries + qi) * width + d]
                                * k.values()[((b * heads + h) * keys + ki) * width + d],
                        )?;
                        sum = finite(sum + product)?;
                    }
                    scores[((b * heads + h) * queries + qi) * keys + ki] = finite(sum * scale)?;
                }
            }
        }
    }
    let probabilities = softmax(&Tensor::new(&score_shape, &scores)?, mask.allowed())?;
    let shape = [batch, heads, queries, value_width];
    let mut result = zeroed(bounded_axes(shape)?)?;
    for b in 0..batch {
        for h in 0..heads {
            for qi in 0..queries {
                for d in 0..value_width {
                    let mut sum = 0_f32;
                    for ki in 0..keys {
                        let product = finite(
                            probabilities.values()[((b * heads + h) * queries + qi) * keys + ki]
                                * v.values()[((b * heads + h) * keys + ki) * value_width + d],
                        )?;
                        sum = finite(sum + product)?;
                    }
                    result[((b * heads + h) * queries + qi) * value_width + d] = sum;
                }
            }
        }
    }
    Tensor::new(&shape, &result)
}

#[derive(Debug)]
pub struct KvCache {
    key: Option<Tensor>,
    value: Option<Tensor>,
    capacity: usize,
}

impl KvCache {
    pub fn empty(capacity: usize) -> Result<Self, TensorError> {
        if capacity == 0 || capacity > 16 {
            return Err(TensorError::InvalidParameter);
        }
        Ok(Self {
            key: None,
            value: None,
            capacity,
        })
    }
    pub fn len(&self) -> usize {
        self.key.as_ref().map_or(0, |k| k.shape()[2])
    }
    pub fn is_empty(&self) -> bool {
        self.key.is_none()
    }
    pub fn key(&self) -> Option<&Tensor> {
        self.key.as_ref()
    }
    pub fn value(&self) -> Option<&Tensor> {
        self.value.as_ref()
    }
    pub fn append(&self, key: &Tensor, value: &Tensor) -> Result<Self, TensorError> {
        let ks = axes4(key)?;
        let vs = axes4(value)?;
        if ks[..3] != vs[..3] {
            return Err(TensorError::InvalidShape);
        }
        for (old, new) in [(self.key.as_ref(), key), (self.value.as_ref(), value)] {
            if let Some(old) = old {
                let shape = old.shape();
                if shape[0] != new.shape()[0]
                    || shape[1] != new.shape()[1]
                    || shape[3] != new.shape()[3]
                {
                    return Err(TensorError::InvalidShape);
                }
            }
        }
        let total_tokens = self
            .len()
            .checked_add(ks[2])
            .ok_or(TensorError::ExtentOverflow)?;
        if total_tokens > self.capacity {
            return Err(TensorError::ExtentLimit);
        }
        fn join(old: Option<&Tensor>, new: &Tensor, total: usize) -> Result<Tensor, TensorError> {
            let [b, h, t, d] = axes4(new)?;
            let previous = old.map_or(0, |x| x.shape()[2]);
            let shape = [b, h, total, d];
            let mut values = zeroed(bounded_axes(shape)?)?;
            for group in 0..b * h {
                let target = group * total * d;
                if let Some(old) = old {
                    let source = group * previous * d;
                    values[target..target + previous * d]
                        .copy_from_slice(&old.values()[source..source + previous * d]);
                }
                let source = group * t * d;
                values[target + previous * d..target + total * d]
                    .copy_from_slice(&new.values()[source..source + t * d]);
            }
            Tensor::new(&shape, &values)
        }
        Ok(Self {
            key: Some(join(self.key.as_ref(), key, total_tokens)?),
            value: Some(join(self.value.as_ref(), value, total_tokens)?),
            capacity: self.capacity,
        })
    }
}

pub fn greedy(scores: &[f32], original_ids: &[u32]) -> Result<u32, TensorError> {
    if scores.is_empty() || scores.len() != original_ids.len() {
        return Err(TensorError::InvalidShape);
    }
    if scores.len() > 64 {
        return Err(TensorError::ExtentLimit);
    }
    for (i, &score) in scores.iter().enumerate() {
        finite(score)?;
        if original_ids[..i].contains(&original_ids[i]) {
            return Err(TensorError::InvalidParameter);
        }
    }
    let mut best = 0;
    for i in 1..scores.len() {
        if scores[i] > scores[best]
            || (scores[i] == scores[best] && original_ids[i] < original_ids[best])
        {
            best = i;
        }
    }
    Ok(original_ids[best])
}
