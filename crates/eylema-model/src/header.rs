use crate::{LoadError, StoredDtype, TensorSpec, WeightLayout};
pub(crate) const AUX_BYTES: usize = 4_194_304;
pub(crate) const HEADER_BYTES: usize = 1_048_576;
pub(crate) struct TensorRange {
    pub(crate) spec: TensorSpec,
    pub(crate) start: u64,
    pub(crate) end: u64,
}
enum Json {
    Str(String),
    Num(u64),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}
struct Parser<'a> {
    text: &'a str,
    pos: usize,
    nodes: usize,
    allocated: usize,
}
impl Parser<'_> {
    fn charge(&mut self, n: usize) -> Result<(), LoadError> {
        self.allocated = self.allocated.checked_add(n).ok_or(LoadError::WorkLimit)?;
        if self.allocated > AUX_BYTES {
            return Err(LoadError::WorkLimit);
        }
        Ok(())
    }
    fn grow<T>(&mut self, v: &mut Vec<T>) -> Result<(), LoadError> {
        if v.len() == v.capacity() {
            let cap = (v.capacity() * 2).max(1);
            self.charge((cap - v.capacity()) * std::mem::size_of::<T>())?;
            v.try_reserve_exact(cap - v.len())
                .map_err(|_| LoadError::AllocationFailed)?;
        }
        Ok(())
    }
    fn ws(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.pos)
            .is_some_and(|c| b" \t\r\n".contains(c))
        {
            self.pos += 1
        }
    }
    fn byte(&mut self) -> Result<u8, LoadError> {
        let b = *self
            .text
            .as_bytes()
            .get(self.pos)
            .ok_or(LoadError::InvalidHeader)?;
        self.pos += 1;
        Ok(b)
    }
    fn expect(&mut self, b: u8) -> Result<(), LoadError> {
        if self.byte()? == b {
            Ok(())
        } else {
            Err(LoadError::InvalidHeader)
        }
    }
    fn hex(&mut self) -> Result<u16, LoadError> {
        let mut n = 0u16;
        for _ in 0..4 {
            n = n * 16
                + char::from(self.byte()?)
                    .to_digit(16)
                    .ok_or(LoadError::InvalidHeader)? as u16;
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String, LoadError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let b = self.byte()?;
            if b == b'"' {
                return Ok(out);
            }
            let c = if b == b'\\' {
                match self.byte()? {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'b' => '\x08',
                    b'f' => '\x0c',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'u' => {
                        let h = self.hex()?;
                        let n = if (0xd800..=0xdbff).contains(&h) {
                            self.expect(b'\\')?;
                            self.expect(b'u')?;
                            let l = self.hex()?;
                            if !(0xdc00..=0xdfff).contains(&l) {
                                return Err(LoadError::InvalidHeader);
                            }
                            0x10000 + ((h as u32 - 0xd800) << 10) + (l as u32 - 0xdc00)
                        } else {
                            h as u32
                        };
                        char::from_u32(n).ok_or(LoadError::InvalidHeader)?
                    }
                    _ => return Err(LoadError::InvalidHeader),
                }
            } else {
                if b < 32 {
                    return Err(LoadError::InvalidHeader);
                }
                self.pos -= 1;
                let c = self.text[self.pos..]
                    .chars()
                    .next()
                    .ok_or(LoadError::InvalidHeader)?;
                self.pos += c.len_utf8();
                c
            };
            let needed = out.len() + c.len_utf8();
            if needed > 4096 {
                return Err(LoadError::SizeLimit);
            }
            if needed > out.capacity() {
                let cap = (out.capacity() * 2).max(needed).min(4096);
                self.charge(cap - out.capacity())?;
                out.try_reserve_exact(cap - out.len())
                    .map_err(|_| LoadError::AllocationFailed)?;
            }
            out.push(c)
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, LoadError> {
        if depth > 6 || self.nodes >= 16384 {
            return Err(LoadError::WorkLimit);
        }
        self.nodes += 1;
        self.ws();
        match self.text.as_bytes().get(self.pos).copied() {
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.pos += 1;
                let mut a = Vec::new();
                self.ws();
                if self.text.as_bytes().get(self.pos) == Some(&b']') {
                    self.pos += 1;
                    return Ok(Json::Array(a));
                }
                loop {
                    let v = self.value(depth + 1)?;
                    self.grow(&mut a)?;
                    a.push(v);
                    self.ws();
                    match self.byte()? {
                        b']' => break,
                        b',' => {}
                        _ => return Err(LoadError::InvalidHeader),
                    }
                }
                Ok(Json::Array(a))
            }
            Some(b'{') => {
                self.pos += 1;
                let mut o: Vec<(String, Json)> = Vec::new();
                self.ws();
                if self.text.as_bytes().get(self.pos) == Some(&b'}') {
                    self.pos += 1;
                    return Ok(Json::Object(o));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    if k.is_empty() || k.len() > 128 {
                        return Err(LoadError::SizeLimit);
                    }
                    if o.iter().any(|(s, _)| s == &k) {
                        return Err(LoadError::InvalidHeader);
                    }
                    self.ws();
                    self.expect(b':')?;
                    let v = self.value(depth + 1)?;
                    self.grow(&mut o)?;
                    o.push((k, v));
                    self.ws();
                    match self.byte()? {
                        b'}' => break,
                        b',' => {}
                        _ => return Err(LoadError::InvalidHeader),
                    }
                }
                Ok(Json::Object(o))
            }
            Some(b'0'..=b'9') => {
                let first = self.byte()?;
                let mut n = (first - b'0') as u64;
                if first == b'0'
                    && self
                        .text
                        .as_bytes()
                        .get(self.pos)
                        .is_some_and(u8::is_ascii_digit)
                {
                    return Err(LoadError::InvalidHeader);
                }
                while self
                    .text
                    .as_bytes()
                    .get(self.pos)
                    .is_some_and(u8::is_ascii_digit)
                {
                    n = n
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((self.text.as_bytes()[self.pos] - b'0') as u64))
                        .ok_or(LoadError::InvalidHeader)?;
                    self.pos += 1
                }
                Ok(Json::Num(n))
            }
            _ => Err(LoadError::InvalidHeader),
        }
    }
}
fn numbers(v: Json) -> Result<Vec<u64>, LoadError> {
    if let Json::Array(a) = v {
        if a.len() > 4 {
            return Err(LoadError::InvalidLayout);
        }
        let mut out = crate::error::reserved(a.len())?;
        for x in a {
            if let Json::Num(n) = x {
                out.push(n)
            } else {
                return Err(LoadError::InvalidHeader);
            }
        }
        Ok(out)
    } else {
        Err(LoadError::InvalidHeader)
    }
}
pub(crate) fn validate_header(
    header: &[u8],
    payload_len: u64,
    layout: &WeightLayout,
) -> Result<Vec<TensorRange>, LoadError> {
    if header.len() > HEADER_BYTES {
        return Err(LoadError::SizeLimit);
    }
    if header.first() != Some(&b'{') {
        return Err(LoadError::InvalidHeader);
    }
    let text = std::str::from_utf8(header).map_err(|_| LoadError::InvalidHeader)?;
    let mut p = Parser {
        text,
        pos: 0,
        nodes: 0,
        allocated: header.len() + 65536 + layout.content_bytes(),
    };
    let root = p.value(0)?;
    if !text.as_bytes()[p.pos..].iter().all(|b| *b == b' ') {
        return Err(LoadError::InvalidHeader);
    }
    let Json::Object(entries) = root else {
        return Err(LoadError::InvalidHeader);
    };
    let mut out = crate::error::reserved(layout.tensors().len())?;
    p.charge(layout.content_bytes() + layout.tensors().len() * std::mem::size_of::<TensorRange>())?;
    for (name, value) in entries {
        if name == "__metadata__" {
            let Json::Object(meta) = value else {
                return Err(LoadError::InvalidHeader);
            };
            if meta.len() > 128 {
                return Err(LoadError::SizeLimit);
            }
            if meta.iter().any(|(_, v)| !matches!(v, Json::Str(_))) {
                return Err(LoadError::InvalidHeader);
            }
            continue;
        }
        if out.len() >= 1024 {
            return Err(LoadError::SizeLimit);
        }
        let expected = layout
            .tensors()
            .iter()
            .find(|t| t.name() == name)
            .ok_or(LoadError::ExtraTensor)?;
        let Json::Object(fields) = value else {
            return Err(LoadError::InvalidHeader);
        };
        if fields.len() != 3 {
            return Err(LoadError::InvalidHeader);
        }
        let (mut dt, mut shape, mut offsets) = (None, None, None);
        for (k, v) in fields {
            match k.as_str() {
                "dtype" => {
                    if let Json::Str(s) = v {
                        dt = Some(match s.as_str() {
                            "F32" => StoredDtype::F32,
                            "BF16" => StoredDtype::BF16,
                            _ => return Err(LoadError::InvalidLayout),
                        })
                    } else {
                        return Err(LoadError::InvalidHeader);
                    }
                }
                "shape" => shape = Some(numbers(v)?),
                "data_offsets" => offsets = Some(numbers(v)?),
                _ => return Err(LoadError::InvalidHeader),
            }
        }
        let spec = TensorSpec::new(
            &name,
            dt.ok_or(LoadError::InvalidHeader)?,
            &shape.ok_or(LoadError::InvalidHeader)?,
        )?;
        let offsets = offsets.ok_or(LoadError::InvalidHeader)?;
        if spec.dtype() != expected.dtype()
            || spec.shape() != expected.shape()
            || offsets.len() != 2
        {
            return Err(LoadError::InvalidLayout);
        }
        let (start, end) = (offsets[0], offsets[1]);
        if start >= end || end > payload_len || end - start != spec.bytes() {
            return Err(LoadError::InvalidLayout);
        }
        out.push(TensorRange { spec, start, end });
    }
    if out.len() != layout.tensors().len() {
        return Err(LoadError::MissingTensor);
    }
    out.sort_unstable_by_key(|x| x.start);
    let mut cursor = 0;
    for t in &out {
        if t.start != cursor {
            return Err(LoadError::InvalidLayout);
        }
        cursor = t.end
    }
    if cursor != payload_len {
        return Err(LoadError::InvalidLayout);
    }
    out.sort_unstable_by(|a, b| a.spec.name().cmp(b.spec.name()));
    Ok(out)
}
