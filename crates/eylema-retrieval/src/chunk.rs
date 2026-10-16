use crate::{Control, RetrievalError as E, RetrievalResult as R};
use std::ops::Range;
pub fn ranges(body: &[u8], c: &mut Control) -> R<Vec<Range<usize>>> {
    if body.is_empty() || body.len() > 8192 {
        return Err(E::LimitExceeded);
    }
    c.charge(body.len() * 2)?;
    c.reserve(9 * std::mem::size_of::<Range<usize>>())?;
    let s = std::str::from_utf8(body).map_err(|_| E::InvalidInput)?;
    if s.chars()
        .any(|x| x.is_control() && !['\t', '\r', '\n'].contains(&x))
    {
        return Err(E::InvalidInput);
    }
    let mut out = Vec::with_capacity(9);
    let mut start = 0;
    while start < s.len() {
        let mut end = (start + 1024).min(s.len());
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        out.push(start..end);
        start = end;
    }
    Ok(out)
}
pub fn terms(s: &str, max_unique: usize, c: &mut Control) -> R<Vec<String>> {
    if s.len() > 2048 || max_unique == 0 || max_unique > 128 {
        return Err(E::LimitExceeded);
    }
    c.charge(s.len() * 12 + max_unique * 64)?;
    c.reserve(max_unique * (64 + std::mem::size_of::<String>()))?;
    let mut out = Vec::with_capacity(max_unique);
    for word in s
        .split(|x: char| !x.is_alphanumeric())
        .filter(|x| !x.is_empty())
    {
        let n = word
            .chars()
            .flat_map(char::to_lowercase)
            .map(char::len_utf8)
            .sum::<usize>();
        if n > 64 {
            return Err(E::LimitExceeded);
        }
        let word = word.to_lowercase();
        c.charge(out.len() * 64)?;
        if !out.contains(&word) {
            if out.len() == max_unique {
                return Err(E::LimitExceeded);
            }
            out.push(word);
        }
    }
    if out.is_empty() {
        return Err(E::InvalidInput);
    }
    c.charge(out.len() * out.len() * 64)?;
    out.sort();
    Ok(out)
}
