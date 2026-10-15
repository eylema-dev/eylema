//! Authored Unicode16 NFC and classifications, with attributed immutable data.
use super::{
    TokenizeError as E,
    admission::{digest, hex},
    budget::Counters,
};
const DATA: &[u8] = include_bytes!("../../data/unicode16.bin");
const SHA: [u8; 32] = hex(b"6797aff88e12c5a56e4e28a7be08b66dfa7e2bcd9d37e6c834b7f7c407100fe6");
const CAT: usize = 33;
const CCC: usize = CAT + 4098 * 10;
const DEC: usize = CCC + 934 * 6;
const fn u16at(d: &[u8], p: usize) -> usize {
    d[p] as usize | ((d[p + 1] as usize) << 8)
}
const fn u32at(d: &[u8], p: usize) -> u32 {
    d[p] as u32 | ((d[p + 1] as u32) << 8) | ((d[p + 2] as u32) << 16) | ((d[p + 3] as u32) << 24)
}
const fn offsets() -> [usize; 13253] {
    let mut out = [0; 13253];
    let mut p = DEC;
    let mut i = 0;
    while i < out.len() {
        out[i] = p;
        p += 6 + u16at(DATA, p + 4) * 4;
        i += 1;
    }
    out
}
static OFF: [usize; 13253] = offsets();
const fn decomp_end() -> usize {
    let mut p = DEC;
    let mut i = 0;
    while i < 13253 {
        p += 6 + u16at(DATA, p + 4) * 4;
        i += 1;
    }
    p
}
const COMP: usize = decomp_end();
const FOLD: usize = COMP + 1360 * 12;
pub(super) fn validate(d: &[u8]) -> Result<(), E> {
    if d.len() != FOLD + 17 * 5
        || d.get(..9) != Some(b"EYLUNIC16")
        || u32at(d, 9) != 1
        || [
            u32at(d, 13),
            u32at(d, 17),
            u32at(d, 21),
            u32at(d, 25),
            u32at(d, 29),
        ] != [4098, 934, 13253, 1360, 17]
    {
        return Err(E::UnsupportedUnicode);
    }
    if digest(d)? != SHA {
        return Err(E::UnsupportedUnicode);
    }
    Ok(())
}
pub(super) fn ensure(c: &mut Counters) -> Result<(), E> {
    if !c.unicode_checked {
        c.hold(DATA.len() + std::mem::size_of_val(&OFF))?;
        c.charge(DATA.len())?;
        validate(DATA)?;
        c.unicode_checked = true;
    }
    Ok(())
}
fn range(cp: u32, c: &mut Counters) -> Result<usize, E> {
    c.charge(18)?;
    let (mut lo, mut hi) = (0, 4098);
    while lo < hi {
        let mid = (lo + hi) / 2;
        let p = CAT + mid * 10;
        if u32at(DATA, p + 4) < cp {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    if lo == 4098 {
        return Err(E::UnsupportedUnicode);
    }
    let p = CAT + lo * 10;
    if cp < u32at(DATA, p) || &DATA[p + 8..p + 10] == b"Cn" {
        return Err(E::UnsupportedUnicode);
    }
    Ok(p)
}
pub(super) fn flags(ch: char, c: &mut Counters) -> Result<u8, E> {
    let p = range(ch as u32, c)?;
    let category = DATA[p + 8];
    let cp = ch as u32;
    let white = matches!(cp,9..=13|32|133|160|5760|8192..=8202|8232|8233|8239|8287|12288);
    Ok(u8::from(category == b'L')
        | (u8::from(category == b'N') * 2)
        | (u8::from(white) * 4)
        | (u8::from(ch == '\r' || ch == '\n') * 8))
}
fn combining(cp: u32, c: &mut Counters) -> Result<usize, E> {
    c.charge(18)?;
    let (mut lo, mut hi) = (0, 934);
    while lo < hi {
        let mid = (lo + hi) / 2;
        let value = u32at(DATA, CCC + mid * 6);
        if value < cp { lo = mid + 1 } else { hi = mid }
    }
    Ok(if lo < 934 && u32at(DATA, CCC + lo * 6) == cp {
        u16at(DATA, CCC + lo * 6 + 4)
    } else {
        0
    })
}
pub(super) fn fold(ch: char, c: &mut Counters) -> Result<u8, E> {
    c.charge(5)?;
    let cp = ch as u32;
    let (mut lo, mut hi) = (0, 17);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if u32at(DATA, FOLD + mid * 5) < cp {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    Ok(if lo < 17 && u32at(DATA, FOLD + lo * 5) == cp {
        DATA[FOLD + lo * 5 + 4]
    } else {
        0
    })
}
fn decompose(cp: u32, out: &mut Vec<u32>, depth: usize, c: &mut Counters) -> Result<(), E> {
    if depth > 32 {
        return Err(E::WorkLimit);
    }
    c.charge(1)?;
    range(cp, c)?;
    if (0xac00..0xd7a4).contains(&cp) {
        let s = cp - 0xac00;
        c.push(out, 0x1100 + s / 588)?;
        c.push(out, 0x1161 + (s % 588) / 28)?;
        if !s.is_multiple_of(28) {
            c.push(out, 0x11a7 + s % 28)?;
        }
        return Ok(());
    }
    c.charge(18)?;
    let (mut lo, mut hi) = (0, OFF.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if u32at(DATA, OFF[mid]) < cp {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    if lo < OFF.len() && u32at(DATA, OFF[lo]) == cp {
        let p = OFF[lo];
        for i in 0..u16at(DATA, p + 4) {
            decompose(u32at(DATA, p + 6 + i * 4), out, depth + 1, c)?
        }
    } else {
        c.push(out, cp)?;
    }
    Ok(())
}
fn compose(a: u32, b: u32, c: &mut Counters) -> Result<Option<u32>, E> {
    c.charge(18)?;
    if (0x1100..0x1113).contains(&a) && (0x1161..0x1176).contains(&b) {
        return Ok(Some(0xac00 + (a - 0x1100) * 588 + (b - 0x1161) * 28));
    }
    if (0xac00..0xd7a4).contains(&a)
        && (a - 0xac00).is_multiple_of(28)
        && (0x11a8..0x11c3).contains(&b)
    {
        return Ok(Some(a + b - 0x11a7));
    }
    let (mut lo, mut hi) = (0, 1360);
    while lo < hi {
        let mid = (lo + hi) / 2;
        let p = COMP + mid * 12;
        let pair = (u32at(DATA, p), u32at(DATA, p + 4));
        if pair < (a, b) {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    let p = COMP + lo * 12;
    Ok(
        if lo < 1360 && (u32at(DATA, p), u32at(DATA, p + 4)) == (a, b) {
            Some(u32at(DATA, p + 8))
        } else {
            None
        },
    )
}
pub(super) fn normalize(s: &str, c: &mut Counters) -> Result<String, E> {
    normalize_profile(s, c, false)
}
pub(super) fn normalize_decision(s: &str, c: &mut Counters) -> Result<String, E> {
    normalize_profile(s, c, true)
}
fn normalize_profile(s: &str, c: &mut Counters, decision: bool) -> Result<String, E> {
    if s.len() > if decision { 32768 } else { 8192 } {
        return Err(E::SizeLimit);
    }
    ensure(c)?;
    let outer = c.max_resident;
    let scratch = c
        .resident
        .checked_add(if decision { 1048576 } else { 65536 })
        .ok_or(E::SizeLimit)?;
    c.max_resident = outer.min(scratch);
    let result = normalize_bounded(s, c, decision);
    c.max_resident = outer;
    result.map_err(|e| {
        if e == E::ResidentLimit && scratch < outer {
            E::SizeLimit
        } else {
            e
        }
    })
}
fn normalize_bounded(s: &str, c: &mut Counters, decision: bool) -> Result<String, E> {
    if s.len() > if decision { 32768 } else { 8192 } {
        return Err(E::SizeLimit);
    }
    ensure(c)?;
    let mut decomposed = Vec::new();
    for ch in s.chars() {
        decompose(ch as u32, &mut decomposed, 0, c)?;
        if decomposed.len() * 4 > if decision { 262144 } else { 65536 } {
            return Err(E::SizeLimit);
        }
    }
    for i in 1..decomposed.len() {
        let cc = combining(decomposed[i], c)?;
        if cc == 0 {
            continue;
        }
        let mut j = i;
        while j > 0 {
            c.charge(1)?;
            let previous = combining(decomposed[j - 1], c)?;
            if previous == 0 || previous <= cc {
                break;
            }
            decomposed.swap(j, j - 1);
            j -= 1;
        }
    }
    let mut composed: Vec<u32> = Vec::new();
    let mut starter: Option<usize> = None;
    let mut last_cc = 0;
    for cp in &decomposed {
        let cc = combining(*cp, c)?;
        if let Some(i) = starter
            && (last_cc < cc || last_cc == 0)
            && let Some(joined) = compose(composed[i], *cp, c)?
        {
            composed[i] = joined;
            continue;
        }
        if cc == 0 {
            starter = Some(composed.len());
        }
        c.push(&mut composed, *cp)?;
        last_cc = cc;
    }
    c.drop_vec(decomposed);
    let mut bytes = Vec::new();
    for cp in &composed {
        let ch = char::from_u32(*cp).ok_or(E::UnsupportedUnicode)?;
        let mut buf = [0; 4];
        let part = ch.encode_utf8(&mut buf).as_bytes();
        if bytes.len() + part.len() > 65536 {
            return Err(E::SizeLimit);
        }
        c.charge(part.len())?;
        c.bytes(&mut bytes, part)?;
    }
    c.drop_vec(composed);
    String::from_utf8(bytes).map_err(|_| E::UnsupportedUnicode)
}
