use super::{
    TokenizeError as E,
    admission::{AdmittedData, Merge},
    budget::Counters,
    split, unicode,
};
fn pair<'a>(
    data: &'a AdmittedData,
    left: u32,
    right: u32,
    c: &mut Counters,
) -> Result<Option<&'a Merge>, E> {
    c.charge(18)?;
    let (mut lo, mut hi) = (0, data.merges.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        let m = &data.merges[mid];
        if (m.left, m.right) < (left, right) {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    Ok(data
        .merges
        .get(lo)
        .filter(|m| (m.left, m.right) == (left, right)))
}
pub(super) fn emit(out: &mut Vec<u32>, id: u32, c: &mut Counters, limit: usize) -> Result<(), E> {
    if out.len() >= limit {
        return Err(E::TokenLimit);
    }
    c.charge(1)?;
    c.push(out, id)
}
pub(super) fn encode_piece(
    data: &AdmittedData,
    input: &str,
    c: &mut Counters,
    out: &mut Vec<u32>,
    limit: usize,
) -> Result<(), E> {
    let mut parts = Vec::new();
    c.vec(&mut parts, input.len())?;
    for b in input.bytes() {
        c.charge(1)?;
        parts.push(data.byte_ids[b as usize]);
    }
    while parts.len() > 1 {
        let mut best: Option<&Merge> = None;
        for w in parts.windows(2) {
            if let Some(m) = pair(data, w[0], w[1], c)?
                && best.is_none_or(|old| m.rank < old.rank)
            {
                best = Some(m)
            }
        }
        let Some(best) = best else { break };
        let (mut read, mut write) = (0, 0);
        while read < parts.len() {
            c.charge(2)?;
            if read + 1 < parts.len() && parts[read] == best.left && parts[read + 1] == best.right {
                parts[write] = best.out;
                read += 2;
            } else {
                parts[write] = parts[read];
                read += 1;
            }
            write += 1;
        }
        parts.truncate(write);
    }
    for id in &parts {
        emit(out, *id, c, limit)?;
    }
    c.drop_vec(parts);
    Ok(())
}
pub(super) fn guard(data: &AdmittedData, text: &str, c: &mut Counters) -> Result<(), E> {
    for token in &data.added {
        let needle = token.text.as_bytes();
        let bytes = text.as_bytes();
        if bytes.len() < needle.len() {
            continue;
        }
        for i in 0..=bytes.len() - needle.len() {
            let mut yes = true;
            for (j, b) in needle.iter().enumerate() {
                c.charge(1)?;
                if bytes[i + j] != *b {
                    yes = false;
                    break;
                }
            }
            if yes {
                return Err(E::InvalidInput);
            }
        }
    }
    Ok(())
}
pub(super) fn encode_text(
    data: &AdmittedData,
    input: &str,
    c: &mut Counters,
    out: &mut Vec<u32>,
    limit: usize,
) -> Result<(), E> {
    if input.len() > 8192 {
        return Err(E::SizeLimit);
    }
    guard(data, input, c)?;
    let normalized = unicode::normalize(input, c)?;
    encode_normalized(data, &normalized, c, out, limit)?;
    let n = normalized.capacity();
    drop(normalized);
    c.release(n);
    Ok(())
}
// Only internal callers supply NFC text; typed prompts retain the public
// per-content input limit while including the fixed ASCII header in one span.
pub(super) fn encode_normalized(
    data: &AdmittedData,
    normalized: &str,
    c: &mut Counters,
    out: &mut Vec<u32>,
    limit: usize,
) -> Result<(), E> {
    guard(data, normalized, c)?;
    let ranges = split::pieces(normalized, c)?;
    for range in &ranges {
        encode_piece(data, &normalized[range.clone()], c, out, limit)?;
    }
    c.drop_vec(ranges);
    Ok(())
}
pub(super) fn byte(ch: char, c: &mut Counters) -> Result<u8, E> {
    c.charge(1)?;
    let cp = ch as u32;
    if (33..=126).contains(&cp) || (161..=172).contains(&cp) || (174..=255).contains(&cp) {
        return Ok(cp as u8);
    }
    let mut code = 256;
    for b in 0..256 {
        if (33..=126).contains(&b) || (161..=172).contains(&b) || (174..=255).contains(&b) {
            continue;
        }
        c.charge(1)?;
        if code == cp {
            return Ok(b as u8);
        }
        code += 1;
    }
    Err(E::InvalidVocabulary)
}
pub(super) fn decode(
    data: &AdmittedData,
    ids: &[u32],
    c: &mut Counters,
    limit: usize,
) -> Result<String, E> {
    if ids.len() > limit || ids.len() > 4096 {
        return Err(E::TokenLimit);
    }
    let mut out = Vec::new();
    for id in ids {
        c.charge(18)?;
        let (mut lo, mut hi) = (0, data.by_id.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if data.by_id[mid].id < *id {
                lo = mid + 1
            } else {
                hi = mid
            }
        }
        let index = data
            .by_id
            .get(lo)
            .filter(|x| x.id == *id)
            .ok_or(E::InvalidInput)?;
        if index.added {
            let bytes = data.added[index.index].text.as_bytes();
            if out.len() + bytes.len() > 65536 {
                return Err(E::SizeLimit);
            }
            c.charge(bytes.len())?;
            c.bytes(&mut out, bytes)?;
        } else {
            for ch in data.vocab[index.index].text.chars() {
                if out.len() >= 65536 {
                    return Err(E::SizeLimit);
                }
                let b = byte(ch, c)?;
                c.push(&mut out, b)?;
            }
        }
    }
    String::from_utf8(out).map_err(|_| E::InvalidInput)
}
