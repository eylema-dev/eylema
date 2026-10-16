use crate::{Control, RetrievalError as E, RetrievalResult as R};
pub fn unit_norm(v: &[f32], d: usize, c: &mut Control) -> R<f64> {
    if !(1..=4096).contains(&d) || v.len() != d {
        return Err(E::InvalidVector);
    }
    c.charge(d * 3)?;
    let mut sum = 0.;
    for &x in v {
        c.check()?;
        if !x.is_finite() {
            return Err(E::InvalidVector);
        }
        sum += f64::from(x) * f64::from(x);
    }
    let n = sum.sqrt();
    if (n - 1.).abs() > 1e-5 {
        return Err(E::InvalidVector);
    }
    Ok(n)
}
fn raw_dot(a: &[f32], b: &[f32], d: usize, c: &mut Control) -> R<f64> {
    unit_norm(a, d, c)?;
    unit_norm(b, d, c)?;
    c.charge(d * 2)?;
    let mut sum = 0.;
    for (&a, &b) in a.iter().zip(b) {
        c.check()?;
        sum += f64::from(a) * f64::from(b);
    }
    if !sum.is_finite() || sum.abs() > 1.00003 {
        return Err(E::InvalidVector);
    }
    Ok(sum)
}
pub fn vector_micro(a: &[f32], b: &[f32], d: usize, c: &mut Control) -> R<i64> {
    let sum = raw_dot(a, b, d, c)?;
    Ok((sum.clamp(-1., 1.) * 1_000_000.).round_ties_even() as i64)
}
fn ratio(n: i128, d: i128) -> R<i64> {
    if d <= 0 {
        return Err(E::InvalidInput);
    }
    let a = n.checked_abs().ok_or(E::LimitExceeded)?;
    let q = a / d;
    let r = a % d;
    let twice = r.checked_mul(2).ok_or(E::LimitExceeded)?;
    let v = q + i128::from(twice > d || (twice == d && q % 2 != 0));
    i64::try_from(if n < 0 { -v } else { v }).map_err(|_| E::LimitExceeded)
}
pub fn jaccard_micro(a: &[String], b: &[String], c: &mut Control) -> R<i64> {
    c.check()?;
    if a.is_empty() || b.is_empty() || a.len() > 128 || b.len() > 128 {
        return Err(E::InvalidInput);
    }
    c.charge((a.len() + b.len()) * 128 + a.len() * b.len() * 64)?;
    if a.iter().chain(b).any(|s| s.is_empty() || s.len() > 64)
        || a.windows(2).any(|w| w[0] >= w[1])
        || b.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(E::InvalidInput);
    }
    let i = a.iter().filter(|x| b.contains(x)).count();
    ratio(i as i128 * 1_000_000, (a.len() + b.len() - i) as i128)
}
pub fn hybrid_micro(v: i64, l: i64, w: u32) -> R<i64> {
    if !(1..1_000_000).contains(&w)
        || !(-1_000_000..=1_000_000).contains(&v)
        || !(0..=1_000_000).contains(&l)
    {
        return Err(E::InvalidInput);
    }
    ratio(
        i128::from(w) * i128::from(v) + (1_000_000 - i128::from(w)) * i128::from(l),
        1_000_000,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CancelToken, RetrievalBudget};
    use std::time::{Duration, Instant};
    // Independent literal F32 rational references from the approved plan.
    // Rounding a wrong dot to the same micro score must not hide scalar error.
    #[test]
    fn raw_dot_matches_frozen_scalar_references() {
        for d in [2, 1024, 4096] {
            for (a, b, want) in [
                (0x3f800000, 0, 1.),
                (0, 0x3f800000, 0.),
                (0xbf800000, 0, -1.),
                (0x3f19999a, 0x3f4ccccd, 0.6000000238418579),
                (0x3c000000, 0x3f7ffe00, 0.0078125),
                (0x3cc00000, 0x3f7fedff, 0.0234375),
                (0xbc000000, 0x3f7ffe00, -0.0078125),
                (0xbcc00000, 0x3f7fedff, -0.0234375),
            ] {
                let mut q = vec![0.; d];
                q[0] = 1.;
                let mut v = vec![0.; d];
                v[0] = f32::from_bits(a);
                v[1] = f32::from_bits(b);
                let mut c = Control::new(
                    RetrievalBudget {
                        max_work: 100_000_000,
                        max_live_bytes: 64 * 1024 * 1024,
                        deadline: Instant::now() + Duration::from_secs(30),
                    },
                    CancelToken::new(),
                )
                .unwrap();
                assert!((raw_dot(&q, &v, d, &mut c).unwrap() - want).abs() <= 2e-12);
            }
        }
    }
}
