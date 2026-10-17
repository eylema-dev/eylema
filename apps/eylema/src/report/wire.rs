//! Predecode extent guard: no generic allocation from hostile counts.
use super::types::*;
use eylema_contracts::codec::{self, Value};
use std::io::{Read, Write};
pub fn decode_closed(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > 8192 || !bytes.starts_with(codec::HEADER) {
        return Err(ReportError::Limit);
    }
    fn scan(b: &[u8], p: &mut usize, depth: usize) -> Result<()> {
        if depth > 8 {
            return Err(ReportError::Limit);
        }
        let tag = *b.get(*p).ok_or(ReportError::InvalidInput)?;
        *p += 1;
        let n = match tag {
            0..=2 => 0,
            3 | 4 => 8,
            5..=7 => {
                let end = p.checked_add(4).ok_or(ReportError::Limit)?;
                let n = u32::from_be_bytes(
                    b.get(*p..end)
                        .ok_or(ReportError::InvalidInput)?
                        .try_into()
                        .map_err(|_| ReportError::InvalidInput)?,
                ) as usize;
                *p = end;
                if tag == 7 {
                    if n > 32 {
                        return Err(ReportError::Limit);
                    }
                    for _ in 0..n {
                        scan(b, p, depth + 1)?;
                    }
                    return Ok(());
                }
                n
            }
            _ => return Err(ReportError::InvalidInput),
        };
        *p = p
            .checked_add(n)
            .filter(|&n| n <= b.len())
            .ok_or(ReportError::InvalidInput)?;
        Ok(())
    }
    let mut p = codec::HEADER.len();
    scan(bytes, &mut p, 0)?;
    if p != bytes.len() {
        return Err(ReportError::InvalidInput);
    }
    codec::decode(bytes).map_err(|_| ReportError::InvalidInput)
}
#[derive(Default)]
pub struct FrameBudget {
    frames: u32,
    bytes: usize,
}
impl FrameBudget {
    fn reserve(&mut self, n: usize) -> Result<()> {
        if n > 8192
            || self.frames >= 256
            || self.bytes.checked_add(n + 4).is_none_or(|v| v > 1048576)
        {
            return Err(ReportError::Limit);
        }
        self.frames += 1;
        self.bytes += n + 4;
        Ok(())
    }
    pub fn read(&mut self, r: &mut impl Read) -> Result<Value> {
        let mut h = [0; 4];
        r.read_exact(&mut h)
            .map_err(|_| ReportError::WorkerFailed)?;
        let n = u32::from_be_bytes(h) as usize;
        self.reserve(n)?;
        let mut b = vec![0; n];
        r.read_exact(&mut b)
            .map_err(|_| ReportError::WorkerFailed)?;
        decode_closed(&b)
    }
    pub fn write(&mut self, w: &mut impl Write, b: &[u8]) -> Result<()> {
        self.reserve(b.len())?;
        decode_closed(b)?;
        w.write_all(&(b.len() as u32).to_be_bytes())
            .and_then(|()| w.write_all(b))
            .and_then(|()| w.flush())
            .map_err(|_| ReportError::WorkerFailed)
    }
}
pub fn list(v: &Value) -> Result<&[Value]> {
    if let Value::List(x) = v {
        Ok(x)
    } else {
        Err(ReportError::InvalidInput)
    }
}
pub fn text(v: &Value) -> Result<&str> {
    if let Value::Text(x) = v {
        Ok(x)
    } else {
        Err(ReportError::InvalidInput)
    }
}
pub fn number(v: &Value) -> Result<u64> {
    if let Value::Unsigned(x) = v {
        Ok(*x)
    } else {
        Err(ReportError::InvalidInput)
    }
}
pub fn hash(v: &Value) -> Result<[u8; 32]> {
    if let Value::Bytes(x) = v {
        x.as_slice()
            .try_into()
            .map_err(|_| ReportError::InvalidInput)
    } else {
        Err(ReportError::InvalidInput)
    }
}
