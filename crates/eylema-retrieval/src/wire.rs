use crate::{Control, RetrievalError as E, RetrievalResult as R};
use eylema_contracts::codec::{self, Value};
pub(crate) const MAX_SNAPSHOT: usize = 1024 * 1024;
pub(crate) fn preflight(b: &[u8]) -> R<()> {
    if b.len() > MAX_SNAPSHOT || !b.starts_with(codec::HEADER) {
        return Err(E::CorruptSnapshot);
    }
    fn take<'a>(b: &'a [u8], p: &mut usize, n: usize) -> R<&'a [u8]> {
        let end = p.checked_add(n).ok_or(E::CorruptSnapshot)?;
        let s = b.get(*p..end).ok_or(E::CorruptSnapshot)?;
        *p = end;
        Ok(s)
    }
    fn scan(b: &[u8], p: &mut usize, depth: usize, nodes: &mut usize) -> R<()> {
        if depth > 8 || *nodes >= 32768 {
            return Err(E::CorruptSnapshot);
        }
        *nodes += 1;
        let t = take(b, p, 1)?[0];
        match t {
            0..=2 => {}
            3 | 4 => {
                take(b, p, 8)?;
            }
            5..=7 => {
                let n =
                    u32::from_be_bytes(take(b, p, 4)?.try_into().map_err(|_| E::CorruptSnapshot)?)
                        as usize;
                let cap = match t {
                    5 => 16384,
                    6 => 1024,
                    _ => 128,
                };
                if n > cap {
                    return Err(E::CorruptSnapshot);
                }
                if t == 7 {
                    for _ in 0..n {
                        scan(b, p, depth + 1, nodes)?;
                    }
                } else {
                    let s = take(b, p, n)?;
                    if t == 6 && std::str::from_utf8(s).is_err() {
                        return Err(E::CorruptSnapshot);
                    }
                }
            }
            _ => return Err(E::CorruptSnapshot),
        }
        Ok(())
    }
    let mut p = codec::HEADER.len();
    scan(b, &mut p, 0, &mut 0)?;
    if p != b.len() {
        return Err(E::CorruptSnapshot);
    }
    Ok(())
}
pub(crate) fn extent(v: &Value, c: &mut Control) -> R<usize> {
    c.check()?;
    fn inner(v: &Value, depth: usize, nodes: &mut usize, c: &mut Control) -> R<usize> {
        c.charge(1)?;
        if depth > 8 || *nodes >= 32768 {
            return Err(E::CorruptSnapshot);
        }
        *nodes += 1;
        match v {
            Value::Null | Value::Bool(_) => Ok(1),
            Value::Signed(_) | Value::Unsigned(_) => Ok(9),
            Value::Bytes(b) => {
                if b.len() > 16384 {
                    return Err(E::LimitExceeded);
                }
                Ok(5 + b.len())
            }
            Value::Text(s) => {
                if s.len() > 1024 {
                    return Err(E::LimitExceeded);
                }
                Ok(5 + s.len())
            }
            Value::List(l) => {
                if l.len() > 128 {
                    return Err(E::LimitExceeded);
                }
                let mut size = 5usize;
                for v in l {
                    size = size
                        .checked_add(inner(v, depth + 1, nodes, c)?)
                        .ok_or(E::LimitExceeded)?;
                }
                Ok(size)
            }
            _ => Err(E::CorruptSnapshot),
        }
    }
    let n = inner(v, 0, &mut 0, c)?
        .checked_add(codec::HEADER.len())
        .ok_or(E::LimitExceeded)?;
    if n > MAX_SNAPSHOT {
        return Err(E::LimitExceeded);
    }
    Ok(n)
}
pub(crate) fn encoded(v: &Value, max: usize, c: &mut Control) -> R<Vec<u8>> {
    let n = extent(v, c)?;
    if n > max {
        return Err(E::LimitExceeded);
    }
    c.charge(n * 3)?;
    c.reserve(n * 2)?;
    codec::encode(v).map_err(|_| E::CorruptSnapshot)
}
pub(crate) fn text(s: &str) -> Value {
    Value::Text(s.into())
}
pub(crate) fn list(v: &Value, n: usize) -> R<&[Value]> {
    match v {
        Value::List(l) if l.len() == n => Ok(l),
        _ => Err(E::CorruptSnapshot),
    }
}
pub(crate) fn array(v: &Value, max: usize) -> R<&[Value]> {
    match v {
        Value::List(l) if l.len() <= max => Ok(l),
        _ => Err(E::CorruptSnapshot),
    }
}
pub(crate) fn string(v: &Value) -> R<&str> {
    match v {
        Value::Text(s) => Ok(s),
        _ => Err(E::CorruptSnapshot),
    }
}
pub(crate) fn number(v: &Value) -> R<u64> {
    match v {
        Value::Unsigned(n) => Ok(*n),
        _ => Err(E::CorruptSnapshot),
    }
}
pub(crate) fn digest(v: &Value) -> R<[u8; 32]> {
    match v {
        Value::Bytes(b) => b.as_slice().try_into().map_err(|_| E::CorruptSnapshot),
        _ => Err(E::CorruptSnapshot),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_blocks_claimed_container_bombs_before_decoder() {
        let mut b = codec::HEADER.to_vec();
        for _ in 0..33 {
            b.push(7);
            b.extend_from_slice(&65500u32.to_be_bytes());
        }
        assert!(preflight(&b).is_err());
        for v in [
            Value::Map(vec![]),
            Value::List(vec![Value::Null; 129]),
            Value::Text("x".repeat(1025)),
            Value::Bytes(vec![0; 16385]),
        ] {
            let b = codec::encode(&v).unwrap();
            let mut decoder_called = false;
            if preflight(&b).is_ok() {
                decoder_called = true;
                let _ = codec::decode(&b);
            }
            assert!(!decoder_called);
        }
    }
}
