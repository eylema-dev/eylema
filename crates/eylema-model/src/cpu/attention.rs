use super::{CpuError as E, budget::Meter, ops::finite, profile::Dimensions};
pub(crate) struct Attention<'a> {
    pub q: &'a [f32],
    pub keys: &'a [f32],
    pub values: &'a [f32],
    pub scores: &'a mut [f32],
    pub out: &'a mut [f32],
}
pub(crate) fn apply(d: Dimensions, pos: usize, a: Attention<'_>, m: &mut Meter) -> Result<(), E> {
    let count = pos.checked_add(1).ok_or(E::ContextLimit)?;
    let kw = d.kh * d.w;
    if a.q.len() != d.qh * d.w
        || a.out.len() != a.q.len()
        || a.scores.len() < count
        || a.keys.len() < count * kw
        || a.values.len() < count * kw
    {
        return Err(E::InvalidLayout);
    }
    for head in 0..d.qh {
        let kv_head = head / (d.qh / d.kh);
        for t in 0..count {
            // Dot products/additions, square root, division and finite predicate.
            m.charge((d.w * 2 + 3) as u64)?;
            let mut dot = 0.;
            for j in 0..d.w {
                dot += a.q[head * d.w + j] * a.keys[t * kw + kv_head * d.w + j];
            }
            a.scores[t] = finite(dot / (d.w as f32).sqrt())?;
        }
        // Per position: max, subtract, exp, finite, sum, division; total finite.
        m.charge((count * 6 + 1) as u64)?;
        let maximum = a.scores[..count]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let mut total = 0.;
        for v in &mut a.scores[..count] {
            *v = finite((*v - maximum).exp())?;
            total += *v;
        }
        finite(total)?;
        for v in &mut a.scores[..count] {
            *v /= total;
        }
        for j in 0..d.w {
            m.charge((count * 2 + 1) as u64)?; // Products/additions and output finite.
            let mut sum = 0.;
            for t in 0..count {
                sum += a.scores[t] * a.values[t * kw + kv_head * d.w + j];
            }
            a.out[head * d.w + j] = finite(sum)?;
        }
    }
    Ok(())
}
