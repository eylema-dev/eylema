use super::{CpuError as E, budget::Meter, weights::Tensor};
pub(crate) fn finite(x: f32) -> Result<f32, E> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(E::NonFinite)
    }
}
pub(crate) fn matvec(
    w: Tensor<'_>,
    input: &[f32],
    out: &mut [f32],
    m: &mut Meter,
) -> Result<(), E> {
    if input.is_empty() || out.len().checked_mul(input.len()) != Some(w.len()) {
        return Err(E::InvalidLayout);
    }
    m.preflight(
        (w.len() as u64)
            .checked_mul(3)
            .and_then(|n| n.checked_add(out.len() as u64))
            .ok_or(E::WorkLimit)?,
    )?;
    for (row, o) in out.iter_mut().enumerate() {
        let mut sum = 0.;
        for (block, xs) in input.chunks(1024).enumerate() {
            m.charge((xs.len() * 3) as u64)?;
            for (j, x) in xs.iter().enumerate() {
                sum += w.at(row * input.len() + block * 1024 + j) * x;
            }
        }
        m.charge(1)?;
        *o = finite(sum)?;
    }
    Ok(())
}
pub(crate) fn norm(input: &[f32], w: Tensor<'_>, out: &mut [f32], m: &mut Meter) -> Result<(), E> {
    if input.is_empty() || input.len() != w.len() || input.len() != out.len() {
        return Err(E::InvalidLayout);
    }
    let mut sum = 0.;
    for xs in input.chunks(1024) {
        m.charge((xs.len() * 2) as u64)?;
        for x in xs {
            sum += x * x;
        }
    }
    // Division, epsilon addition, square root and finite predicate.
    m.charge(4)?;
    let scale = finite((sum / input.len() as f32 + 1e-6).sqrt())?;
    for (block, (xs, ys)) in input.chunks(1024).zip(out.chunks_mut(1024)).enumerate() {
        // Division, weight read conversion, multiplication and finite predicate.
        m.charge((xs.len() * 4) as u64)?;
        for (j, (x, y)) in xs.iter().zip(ys).enumerate() {
            *y = finite(x / scale * w.at(block * 1024 + j))?;
        }
    }
    Ok(())
}
pub(crate) fn norm_head(x: &mut [f32], w: Tensor<'_>, m: &mut Meter) -> Result<(), E> {
    if x.is_empty() || x.len() != w.len() || x.len() > 128 {
        return Err(E::InvalidLayout);
    }
    // Two reduction operations, four output operations and four scale operations.
    m.charge((x.len() * 6 + 4) as u64)?;
    let mut sum = 0.;
    for v in x.iter() {
        sum += v * v;
    }
    let div = finite((sum / x.len() as f32 + 1e-6).sqrt())?;
    for (i, v) in x.iter_mut().enumerate() {
        *v = finite(*v / div * w.at(i))?;
    }
    Ok(())
}
pub(crate) fn rope(x: &mut [f32], pos: usize, m: &mut Meter) -> Result<(), E> {
    if x.is_empty() || !x.len().is_multiple_of(2) || x.len() > 128 {
        return Err(E::InvalidLayout);
    }
    m.charge((x.len() * 9) as u64)?;
    let half = x.len() / 2;
    for j in 0..half {
        let angle = pos as f32 * 1_000_000f32.powf(-2. * j as f32 / x.len() as f32);
        let (s, c) = angle.sin_cos();
        let a = x[j];
        let b = x[j + half];
        x[j] = finite(a * c - b * s)?;
        x[j + half] = finite(b * c + a * s)?;
    }
    Ok(())
}
pub(crate) fn add(a: &mut [f32], b: &[f32], m: &mut Meter) -> Result<(), E> {
    if a.len() != b.len() {
        return Err(E::InvalidLayout);
    }
    for (xs, ys) in a.chunks_mut(1024).zip(b.chunks(1024)) {
        m.charge((xs.len() * 2) as u64)?; // Addition and finite predicate.
        for (x, y) in xs.iter_mut().zip(ys) {
            *x = finite(*x + y)?;
        }
    }
    Ok(())
}
pub(crate) fn greedy(x: &[f32]) -> Result<u32, E> {
    if x.is_empty() {
        return Err(E::InvalidOutput);
    }
    let mut best = 0;
    for (i, &v) in x.iter().enumerate() {
        finite(v)?;
        if v > x[best] {
            best = i;
        }
    }
    u32::try_from(best).map_err(|_| E::InvalidOutput)
}
