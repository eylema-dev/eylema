//! Bounded streaming JSON spans; no recursive value DOM.
use super::{
    TokenizeError as E,
    budget::{Counters, compare, sort},
};
use std::ops::Range;
pub(super) type Span = Range<usize>;
pub(super) struct Parser<'a, 'b> {
    pub data: &'a [u8],
    pub at: usize,
    pub c: &'b mut Counters,
    pub string_limit: usize,
    pub key_limit: usize,
}
impl<'a, 'b> Parser<'a, 'b> {
    pub fn new(data: &'a [u8], c: &'b mut Counters, limit: usize) -> Result<Self, E> {
        std::str::from_utf8(data).map_err(|_| E::InvalidJson)?;
        c.nodes = 0;
        Ok(Self {
            data,
            at: 0,
            c,
            string_limit: limit,
            key_limit: 128,
        })
    }
    fn take(&mut self) -> Result<u8, E> {
        self.c.charge(1)?;
        let b = *self.data.get(self.at).ok_or(E::InvalidJson)?;
        self.at += 1;
        Ok(b)
    }
    fn ws(&mut self) -> Result<(), E> {
        while self
            .data
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.take()?;
        }
        Ok(())
    }
    fn require(&mut self, b: u8) -> Result<(), E> {
        self.ws()?;
        if self.take()? == b {
            Ok(())
        } else {
            Err(E::InvalidJson)
        }
    }
    fn hex(&mut self) -> Result<u16, E> {
        let mut n = 0;
        for _ in 0..4 {
            let d = (self.take()? as char).to_digit(16).ok_or(E::InvalidJson)?;
            n = n * 16 + d as u16;
        }
        Ok(n)
    }
    pub fn text(&mut self) -> Result<String, E> {
        self.ws()?;
        self.require(b'"')?;
        let mut out = Vec::new();
        loop {
            let b = self.take()?;
            if b == b'"' {
                break;
            }
            if b < 32 {
                return Err(E::InvalidJson);
            }
            let mut buf = [0u8; 4];
            let bytes: &[u8] = if b == b'\\' {
                match self.take()? {
                    b'"' => b"\"",
                    b'\\' => b"\\",
                    b'/' => b"/",
                    b'b' => b"\x08",
                    b'f' => b"\x0c",
                    b'n' => b"\n",
                    b'r' => b"\r",
                    b't' => b"\t",
                    b'u' => {
                        let hi = self.hex()?;
                        let cp = if (0xd800..=0xdbff).contains(&hi) {
                            if self.take()? != b'\\' || self.take()? != b'u' {
                                return Err(E::InvalidJson);
                            }
                            let lo = self.hex()?;
                            if !(0xdc00..=0xdfff).contains(&lo) {
                                return Err(E::InvalidJson);
                            }
                            0x10000 + ((hi as u32 - 0xd800) << 10) + (lo as u32 - 0xdc00)
                        } else {
                            hi as u32
                        };
                        char::from_u32(cp)
                            .ok_or(E::InvalidJson)?
                            .encode_utf8(&mut buf)
                            .as_bytes()
                    }
                    _ => return Err(E::InvalidJson),
                }
            } else {
                buf[0] = b;
                &buf[..1]
            };
            if out.len() + bytes.len() > self.string_limit {
                return Err(E::SizeLimit);
            }
            self.c.charge(bytes.len())?;
            self.c.bytes(&mut out, bytes)?;
        }
        String::from_utf8(out).map_err(|_| E::InvalidJson)
    }
    fn fields(&mut self, depth: usize) -> Result<Vec<(String, Span)>, E> {
        self.require(b'{')?;
        let mut v = Vec::new();
        self.ws()?;
        if self.data.get(self.at) == Some(&b'}') {
            self.take()?;
            return Ok(v);
        }
        loop {
            let key = self.text()?;
            if key.len() > self.key_limit && depth == 0 {
                return Err(E::SizeLimit);
            }
            self.require(b':')?;
            self.ws()?;
            let start = self.at;
            if key == "vocab" && self.data.get(self.at) == Some(&b'{') {
                self.vocab_span(depth + 1)?;
            } else {
                self.skip(depth + 1)?;
            }
            let end = self.at;
            self.c.push(&mut v, (key, start..end))?;
            self.ws()?;
            match self.take()? {
                b'}' => break,
                b',' => (),
                _ => return Err(E::InvalidJson),
            }
        }
        sort(&mut v, self.c, |a, b, c| compare(c, &a.0, &b.0))?;
        for pair in v.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(E::InvalidJson);
            }
        }
        Ok(v)
    }
    fn vocab_span(&mut self, depth: usize) -> Result<(), E> {
        if depth > 16 {
            return Err(E::SizeLimit);
        }
        self.require(b'{')?;
        self.ws()?;
        if self.data.get(self.at) == Some(&b'}') {
            self.take()?;
            return Ok(());
        }
        loop {
            let key = self.text()?;
            let n = key.capacity();
            drop(key);
            self.c.release(n);
            self.require(b':')?;
            self.skip(depth + 1)?;
            self.ws()?;
            match self.take()? {
                b'}' => break,
                b',' => (),
                _ => return Err(E::InvalidJson),
            }
        }
        Ok(())
    }
    fn skip(&mut self, depth: usize) -> Result<(), E> {
        if depth > 16 {
            return Err(E::SizeLimit);
        }
        self.c.nodes = self.c.nodes.checked_add(1).ok_or(E::WorkLimit)?;
        if self.c.nodes > 1000000 {
            return Err(E::WorkLimit);
        }
        self.c.charge(1)?;
        self.ws()?;
        match self.data.get(self.at).copied().ok_or(E::InvalidJson)? {
            b'"' => {
                let t = self.text()?;
                let n = t.capacity();
                drop(t);
                self.c.release(n);
            }
            b'{' => {
                let v = self.fields(depth)?;
                free_fields(self.c, v);
            }
            b'[' => {
                self.take()?;
                self.ws()?;
                if self.data.get(self.at) == Some(&b']') {
                    self.take()?;
                } else {
                    loop {
                        self.skip(depth + 1)?;
                        self.ws()?;
                        match self.take()? {
                            b']' => break,
                            b',' => (),
                            _ => return Err(E::InvalidJson),
                        }
                    }
                }
            }
            b't' => self.literal(b"true")?,
            b'f' => self.literal(b"false")?,
            b'n' => self.literal(b"null")?,
            b'-' | b'0'..=b'9' => self.number()?,
            _ => return Err(E::InvalidJson),
        }
        Ok(())
    }
    fn literal(&mut self, t: &[u8]) -> Result<(), E> {
        for b in t {
            if self.take()? != *b {
                return Err(E::InvalidJson);
            }
        }
        Ok(())
    }
    fn number(&mut self) -> Result<(), E> {
        if self.data.get(self.at) == Some(&b'-') {
            self.take()?;
        }
        match self.take()? {
            b'0' => {
                if self.data.get(self.at).is_some_and(u8::is_ascii_digit) {
                    return Err(E::InvalidJson);
                }
            }
            b'1'..=b'9' => {
                while self.data.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.take()?;
                }
            }
            _ => return Err(E::InvalidJson),
        }
        if self.data.get(self.at) == Some(&b'.') {
            self.take()?;
            self.digits()?;
        }
        if self
            .data
            .get(self.at)
            .is_some_and(|b| matches!(b, b'e' | b'E'))
        {
            self.take()?;
            if self
                .data
                .get(self.at)
                .is_some_and(|b| matches!(b, b'+' | b'-'))
            {
                self.take()?;
            }
            self.digits()?;
        }
        Ok(())
    }
    fn digits(&mut self) -> Result<(), E> {
        let begin = self.at;
        while self.data.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.take()?;
        }
        if self.at == begin {
            Err(E::InvalidJson)
        } else {
            Ok(())
        }
    }
    pub fn object(&mut self) -> Result<Vec<(String, Span)>, E> {
        let fields = self.fields(0)?;
        self.ws()?;
        if self.at != self.data.len() {
            return Err(E::InvalidJson);
        }
        Ok(fields)
    }
    pub fn array(&mut self) -> Result<Vec<Span>, E> {
        self.require(b'[')?;
        self.ws()?;
        let mut v = Vec::new();
        if self.data.get(self.at) == Some(&b']') {
            self.take()?;
        } else {
            loop {
                self.ws()?;
                let start = self.at;
                self.skip(1)?;
                self.c.push(&mut v, start..self.at)?;
                self.ws()?;
                match self.take()? {
                    b']' => break,
                    b',' => (),
                    _ => return Err(E::InvalidJson),
                }
            }
        }
        self.ws()?;
        if self.at != self.data.len() {
            return Err(E::InvalidJson);
        }
        Ok(v)
    }
}
pub(super) fn get<'a>(data: &'a [u8], v: &[(String, Span)], key: &str) -> Result<&'a [u8], E> {
    let r = v.iter().find(|(k, _)| k == key).ok_or(E::InvalidConfig)?;
    Ok(&data[r.1.clone()])
}
pub(super) fn free_fields(c: &mut Counters, v: Vec<(String, Span)>) {
    let n = v.iter().map(|x| x.0.capacity()).sum::<usize>()
        + v.capacity() * std::mem::size_of::<(String, Span)>();
    drop(v);
    c.release(n)
}
pub(super) fn uint(data: &[u8]) -> Result<u32, E> {
    if data.is_empty()
        || !data.iter().all(u8::is_ascii_digit)
        || (data.len() > 1 && data[0] == b'0')
    {
        return Err(E::InvalidJson);
    }
    std::str::from_utf8(data)
        .map_err(|_| E::InvalidJson)?
        .parse()
        .map_err(|_| E::InvalidVocabulary)
}
