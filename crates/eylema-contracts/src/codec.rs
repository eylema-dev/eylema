//! Owned EYL codec v1, runtime contract 0.1.0. No cryptography or execution.
use crate::*;
use std::collections::BTreeMap;

pub const MAX_WIRE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DEPTH: usize = 32;
pub const MAX_ITEMS: usize = 65536;
pub const HEADER: &[u8; 7] = b"EYL\x01\x00\x01\x00";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Bytes(Vec<u8>),
    Text(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    Truncated,
    TooLarge,
    TooDeep,
    InvalidData,
    NonCanonical,
    UnsupportedVersion,
    InvalidContract(ValidationError),
}
impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CodecError {}
impl From<ValidationError> for CodecError {
    fn from(value: ValidationError) -> Self {
        Self::InvalidContract(value)
    }
}
pub type Result<T> = std::result::Result<T, CodecError>;

struct Writer {
    bytes: Vec<u8>,
    items: usize,
}
impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > MAX_WIRE_BYTES)
        {
            return Err(CodecError::TooLarge);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn blob(&mut self, bytes: &[u8], limit: usize) -> Result<()> {
        if bytes.len() > limit {
            return Err(CodecError::TooLarge);
        }
        self.put(&(bytes.len() as u32).to_be_bytes())?;
        self.put(bytes)
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(CodecError::TooDeep);
        }
        self.items += 1;
        if self.items > MAX_ITEMS {
            return Err(CodecError::TooLarge);
        }
        match value {
            Value::Null => self.put(&[0]),
            Value::Bool(false) => self.put(&[1]),
            Value::Bool(true) => self.put(&[2]),
            Value::Signed(n) => {
                self.put(&[3])?;
                self.put(&n.to_be_bytes())
            }
            Value::Unsigned(n) => {
                self.put(&[4])?;
                self.put(&n.to_be_bytes())
            }
            Value::Bytes(b) => {
                self.put(&[5])?;
                self.blob(b, MAX_PAYLOAD_BYTES)
            }
            Value::Text(s) => {
                self.put(&[6])?;
                self.blob(s.as_bytes(), MAX_TEXT_BYTES)
            }
            Value::List(items) => {
                if items.len() > MAX_ITEMS {
                    return Err(CodecError::TooLarge);
                }
                self.put(&[7])?;
                self.put(&(items.len() as u32).to_be_bytes())?;
                for item in items {
                    self.value(item, depth + 1)?;
                }
                Ok(())
            }
            Value::Map(items) => {
                if items.len() > MAX_ITEMS {
                    return Err(CodecError::TooLarge);
                }
                let mut sorted: Vec<_> = items.iter().collect();
                sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                if sorted.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                    return Err(CodecError::InvalidData);
                }
                self.put(&[8])?;
                self.put(&(items.len() as u32).to_be_bytes())?;
                for (key, item) in sorted {
                    if key.is_empty() || key.chars().any(char::is_control) {
                        return Err(CodecError::InvalidData);
                    }
                    self.blob(key.as_bytes(), MAX_ID_BYTES)?;
                    self.value(item, depth + 1)?;
                }
                Ok(())
            }
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    items: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(CodecError::TooLarge)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(CodecError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }
    fn count(&mut self, limit: usize) -> Result<usize> {
        let count = u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| CodecError::Truncated)?,
        ) as usize;
        if count > limit {
            Err(CodecError::TooLarge)
        } else {
            Ok(count)
        }
    }
    fn blob(&mut self, limit: usize) -> Result<&'a [u8]> {
        let count = self.count(limit)?;
        self.take(count)
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(CodecError::TooDeep);
        }
        self.items += 1;
        if self.items > MAX_ITEMS {
            return Err(CodecError::TooLarge);
        }
        match self.take(1)?[0] {
            0 => Ok(Value::Null),
            1 => Ok(Value::Bool(false)),
            2 => Ok(Value::Bool(true)),
            3 => Ok(Value::Signed(i64::from_be_bytes(
                self.take(8)?
                    .try_into()
                    .map_err(|_| CodecError::Truncated)?,
            ))),
            4 => Ok(Value::Unsigned(u64::from_be_bytes(
                self.take(8)?
                    .try_into()
                    .map_err(|_| CodecError::Truncated)?,
            ))),
            5 => Ok(Value::Bytes(self.blob(MAX_PAYLOAD_BYTES)?.to_vec())),
            6 => Ok(Value::Text(
                std::str::from_utf8(self.blob(MAX_TEXT_BYTES)?)
                    .map_err(|_| CodecError::InvalidData)?
                    .to_owned(),
            )),
            7 | 8 => {
                let tag = self.bytes[self.position - 1];
                let count = self.count(MAX_ITEMS)?;
                let minimum = if tag == 7 { 1 } else { 6 };
                if count > (self.bytes.len() - self.position) / minimum
                    || count > MAX_ITEMS - self.items
                {
                    return Err(CodecError::Truncated);
                }
                if tag == 7 {
                    let mut values = Vec::with_capacity(count);
                    for _ in 0..count {
                        values.push(self.value(depth + 1)?);
                    }
                    Ok(Value::List(values))
                } else {
                    let mut values: Vec<(String, Value)> = Vec::with_capacity(count);
                    for _ in 0..count {
                        let key = std::str::from_utf8(self.blob(MAX_ID_BYTES)?)
                            .map_err(|_| CodecError::InvalidData)?
                            .to_owned();
                        if key.is_empty() || key.chars().any(char::is_control) {
                            return Err(CodecError::InvalidData);
                        }
                        if values
                            .last()
                            .is_some_and(|last| last.0.as_bytes() >= key.as_bytes())
                        {
                            return Err(CodecError::NonCanonical);
                        }
                        values.push((key, self.value(depth + 1)?));
                    }
                    Ok(Value::Map(values))
                }
            }
            _ => Err(CodecError::InvalidData),
        }
    }
}

/// Sorts maps bytewise; list order, signed/unsigned kinds and UTF8 bytes remain exact.
pub fn encode(value: &Value) -> Result<Vec<u8>> {
    let mut writer = Writer {
        bytes: Vec::new(),
        items: 0,
    };
    writer.put(HEADER)?;
    writer.value(value, 0)?;
    Ok(writer.bytes)
}
/// Rejects noncanonical order, duplicates, unknown tags/versions and trailing data.
pub fn decode(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(CodecError::TooLarge);
    }
    let mut reader = Reader {
        bytes,
        position: 0,
        items: 0,
    };
    if reader.take(HEADER.len())? != HEADER {
        return Err(CodecError::UnsupportedVersion);
    }
    let value = reader.value(0)?;
    if reader.position != bytes.len() {
        return Err(CodecError::NonCanonical);
    }
    Ok(value)
}

pub(crate) fn map(fields: Vec<(&str, Value)>) -> Value {
    Value::Map(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub(crate) struct Fields(BTreeMap<String, Value>);
impl Fields {
    pub(crate) fn new(value: Value) -> Result<Self> {
        if let Value::Map(values) = value {
            let count = values.len();
            let values: BTreeMap<_, _> = values.into_iter().collect();
            if count != values.len() {
                return Err(CodecError::InvalidData);
            }
            Ok(Self(values))
        } else {
            Err(CodecError::InvalidData)
        }
    }
    pub(crate) fn take(&mut self, key: &str) -> Result<Value> {
        self.0.remove(key).ok_or(CodecError::InvalidData)
    }
    pub(crate) fn finish(self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(CodecError::InvalidData)
        }
    }
}

mod typed;
pub use typed::*;
