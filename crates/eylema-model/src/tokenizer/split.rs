//! Fixed ordered Qwen split semantics, no regex engine.
use super::{TokenizeError as E, budget::Counters, unicode};
use std::ops::Range;
pub(super) fn pieces(s: &str, c: &mut Counters) -> Result<Vec<Range<usize>>, E> {
    if s.len() > 65536 {
        return Err(E::SizeLimit);
    }
    unicode::ensure(c)?;
    let mut chars = Vec::new();
    for (i, ch) in s.char_indices() {
        let f = unicode::flags(ch, c)?;
        c.push(&mut chars, (i, ch, f))?;
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        c.charge(1)?;
        let mut end = None;
        if chars[i].1 == '\'' {
            for suffix in [b"s".as_slice(), b"t", b"re", b"ve", b"m", b"ll", b"d"] {
                if i + 1 + suffix.len() <= chars.len() {
                    let mut matched = true;
                    for (j, want) in suffix.iter().enumerate() {
                        if unicode::fold(chars[i + 1 + j].1, c)? != *want {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        end = Some(i + 1 + suffix.len());
                        break;
                    }
                }
            }
        }
        if end.is_none() {
            let mut j = i;
            if chars[i].2 & 1 == 0
                && chars[i].2 & 2 == 0
                && chars[i].2 & 8 == 0
                && i + 1 < chars.len()
                && chars[i + 1].2 & 1 != 0
            {
                j += 1
            }
            if chars[j].2 & 1 != 0 {
                while j < chars.len() && chars[j].2 & 1 != 0 {
                    c.charge(1)?;
                    j += 1
                }
                end = Some(j)
            }
        }
        if end.is_none() && chars[i].2 & 2 != 0 {
            end = Some(i + 1)
        }
        if end.is_none() {
            let mut j = i;
            if chars[i].1 == ' ' {
                j += 1
            }
            let begin = j;
            while j < chars.len() && chars[j].2 & 7 == 0 {
                c.charge(1)?;
                j += 1
            }
            if j > begin {
                while j < chars.len() && chars[j].2 & 8 != 0 {
                    c.charge(1)?;
                    j += 1
                }
                end = Some(j)
            }
        }
        if end.is_none() && chars[i].2 & 4 != 0 {
            let mut j = i;
            let mut newline = None;
            while j < chars.len() && chars[j].2 & 4 != 0 {
                c.charge(1)?;
                if chars[j].2 & 8 != 0 {
                    newline = Some(j + 1)
                }
                j += 1
            }
            end = newline.or_else(|| {
                if j == chars.len() {
                    Some(j)
                } else if j - i > 1 {
                    Some(j - 1)
                } else {
                    Some(j)
                }
            })
        }
        let end = end.ok_or(E::InvalidInput)?;
        let stop = if end == chars.len() {
            s.len()
        } else {
            chars[end].0
        };
        c.push(&mut out, chars[i].0..stop)?;
        i = end;
    }
    c.drop_vec(chars);
    Ok(out)
}
