//! Bounded canonical record storage. Parsed bytes do not establish authority,
//! prior acknowledgement, semantic recovery or physical power-loss evidence.
#![forbid(unsafe_code)]
use eylema_contracts::codec::{self, Value};
use std::io::{self, Read, Seek, SeekFrom, Write};

mod native;
pub use native::NativeStorage;

pub const HEADER: &[u8; 8] = b"EYLJ\x01\x00\x00\x00";
pub const FRAME_HEADER_BYTES: usize = 76;
pub const MAX_PAYLOAD_BYTES: usize = codec::MAX_WIRE_BYTES - 52;
const IO_ATTEMPTS: usize = 4096;
pub type Result<T> = std::result::Result<T, JournalError>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoStage {
    Metadata,
    Seek,
    Read,
    Write,
    Flush,
    Synchronize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Corruption {
    Header,
    Sequence,
    Chain,
    Digest,
    Payload,
    Length,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalError {
    InvalidLimits,
    BudgetExceeded,
    IncompatibleFormat,
    Truncated { offset: u64 },
    Corrupt { offset: u64, reason: Corruption },
    Io { stage: IoStage, kind: io::ErrorKind },
    WriteUncertain { stage: IoStage, kind: io::ErrorKind },
    HashUnavailable,
    InvalidPayload,
    NeedsRecovery,
    ExternalChange,
    NotEmpty,
}
impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for JournalError {}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub records: u64,
    pub payload_bytes: usize,
    pub file_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            records: 4096,
            payload_bytes: 1024 * 1024,
            file_bytes: 64 * 1024 * 1024,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<()> {
        if self.records == 0
            || self.records > 65536
            || self.payload_bytes < 8
            || self.payload_bytes > MAX_PAYLOAD_BYTES
            || self.file_bytes < HEADER.len() as u64
            || self.file_bytes > 1024 * 1024 * 1024
        {
            return Err(JournalError::InvalidLimits);
        }
        Ok(())
    }
}
/// Trusted storage must supply exclusive cooperative ownership and truthful IO.
/// NativeStorage owns its locked handle. Fault fixtures are not durable media.
pub trait Storage: Read + Write + Seek {
    fn byte_len(&mut self) -> io::Result<u64>;
    fn synchronize(&mut self) -> io::Result<()>;
}
/// Trusted host standard SHA256; input is at most codec::MAX_WIRE_BYTES.
/// Request data cannot replace the provider. Digest integrity is not a MAC.
pub trait Sha256 {
    fn digest(&mut self, bytes: &[u8]) -> Result<[u8; 32]>;
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub sequence: u64,
    pub digest: [u8; 32],
    pub payload: Value,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynchronizedReceipt {
    pub sequence: u64,
    pub digest: [u8; 32],
    pub end_offset: u64,
}
/// Receipt means this trusted storage's synchronization succeeded, not a claim
/// about actual device power loss, malicious writes or runtime effect recovery.
pub struct Journal<T: Storage, H: Sha256> {
    storage: T,
    hasher: H,
    limits: Limits,
    sequence: u64,
    digest: [u8; 32],
    end: u64,
    poisoned: bool,
}
fn io_error(stage: IoStage, error: io::Error) -> JournalError {
    JournalError::Io {
        stage,
        kind: error.kind(),
    }
}
fn read_exact<T: Read>(reader: &mut T, mut bytes: &mut [u8]) -> io::Result<()> {
    for _ in 0..IO_ATTEMPTS {
        if bytes.is_empty() {
            return Ok(());
        }
        match reader.read(bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) if n <= bytes.len() => {
                bytes = &mut bytes[n..];
            }
            Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    if bytes.is_empty() {
        Ok(())
    } else {
        Err(io::ErrorKind::TimedOut.into())
    }
}
fn write_all<T: Write>(writer: &mut T, mut bytes: &[u8]) -> io::Result<()> {
    for _ in 0..IO_ATTEMPTS {
        if bytes.is_empty() {
            return Ok(());
        }
        match writer.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) if n <= bytes.len() => {
                bytes = &bytes[n..];
            }
            Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    if bytes.is_empty() {
        Ok(())
    } else {
        Err(io::ErrorKind::TimedOut.into())
    }
}
fn hash_input(sequence: u64, previous: &[u8; 32], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(52 + payload.len());
    bytes.extend_from_slice(HEADER);
    bytes.extend_from_slice(&sequence.to_be_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(previous);
    bytes.extend_from_slice(payload);
    bytes
}
impl<T: Storage, H: Sha256> Journal<T, H> {
    /// Only for a newly allocated empty storage object. Existing empty files
    /// must be opened and reported as truncated, never automatically repaired.
    pub fn create_new(mut storage: T, hasher: H, limits: Limits) -> Result<Self> {
        limits.validate()?;
        if storage
            .byte_len()
            .map_err(|e| io_error(IoStage::Metadata, e))?
            != 0
        {
            return Err(JournalError::NotEmpty);
        }
        storage
            .seek(SeekFrom::Start(0))
            .map_err(|e| io_error(IoStage::Seek, e))?;
        write_all(&mut storage, HEADER).map_err(|e| JournalError::WriteUncertain {
            stage: IoStage::Write,
            kind: e.kind(),
        })?;
        storage.flush().map_err(|e| JournalError::WriteUncertain {
            stage: IoStage::Flush,
            kind: e.kind(),
        })?;
        storage
            .synchronize()
            .map_err(|e| JournalError::WriteUncertain {
                stage: IoStage::Synchronize,
                kind: e.kind(),
            })?;
        if storage
            .byte_len()
            .map_err(|e| JournalError::WriteUncertain {
                stage: IoStage::Metadata,
                kind: e.kind(),
            })?
            != HEADER.len() as u64
        {
            return Err(JournalError::ExternalChange);
        }
        Ok(Self {
            storage,
            hasher,
            limits,
            sequence: 0,
            digest: [0; 32],
            end: HEADER.len() as u64,
            poisoned: false,
        })
    }
    pub fn open(storage: T, hasher: H, limits: Limits) -> Result<Self> {
        limits.validate()?;
        let mut journal = Self {
            storage,
            hasher,
            limits,
            sequence: 0,
            digest: [0; 32],
            end: 0,
            poisoned: false,
        };
        let records = journal.scan()?;
        journal.sequence = records.last().map_or(0, |r| r.sequence);
        journal.digest = records.last().map_or([0; 32], |r| r.digest);
        journal.end = journal
            .storage
            .byte_len()
            .map_err(|e| io_error(IoStage::Metadata, e))?;
        Ok(journal)
    }
    fn scan(&mut self) -> Result<Vec<Record>> {
        let size = self
            .storage
            .byte_len()
            .map_err(|e| io_error(IoStage::Metadata, e))?;
        if size > self.limits.file_bytes {
            return Err(JournalError::BudgetExceeded);
        }
        if size < HEADER.len() as u64 {
            return Err(JournalError::Truncated { offset: 0 });
        }
        self.storage
            .seek(SeekFrom::Start(0))
            .map_err(|e| io_error(IoStage::Seek, e))?;
        let mut header = [0; 8];
        read_exact(&mut self.storage, &mut header).map_err(|e| io_error(IoStage::Read, e))?;
        if header[..4] != HEADER[..4] {
            return Err(JournalError::Corrupt {
                offset: 0,
                reason: Corruption::Header,
            });
        }
        if header != *HEADER {
            return Err(JournalError::IncompatibleFormat);
        }
        let mut offset = HEADER.len() as u64;
        let mut sequence = 0;
        let mut previous = [0; 32];
        let mut records = vec![];
        while offset < size {
            if records.len() as u64 >= self.limits.records {
                return Err(JournalError::BudgetExceeded);
            }
            if size - offset < FRAME_HEADER_BYTES as u64 {
                return Err(JournalError::Truncated { offset });
            }
            let mut frame = [0; FRAME_HEADER_BYTES];
            read_exact(&mut self.storage, &mut frame).map_err(|e| io_error(IoStage::Read, e))?;
            let next = u64::from_be_bytes(frame[..8].try_into().expect("fixed sequence bytes"));
            let length =
                u32::from_be_bytes(frame[8..12].try_into().expect("fixed length bytes")) as usize;
            if next != sequence + 1 {
                return Err(JournalError::Corrupt {
                    offset,
                    reason: Corruption::Sequence,
                });
            }
            if length < 8 {
                return Err(JournalError::Corrupt {
                    offset,
                    reason: Corruption::Length,
                });
            }
            if length > self.limits.payload_bytes {
                return Err(JournalError::BudgetExceeded);
            }
            if frame[12..44] != previous {
                return Err(JournalError::Corrupt {
                    offset,
                    reason: Corruption::Chain,
                });
            }
            let end = offset
                .checked_add(FRAME_HEADER_BYTES as u64)
                .and_then(|v| v.checked_add(length as u64))
                .ok_or(JournalError::BudgetExceeded)?;
            if end > size {
                return Err(JournalError::Truncated { offset });
            }
            let mut payload = vec![0; length];
            read_exact(&mut self.storage, &mut payload).map_err(|e| io_error(IoStage::Read, e))?;
            let hash = self.hasher.digest(&hash_input(next, &previous, &payload))?;
            if frame[44..] != hash {
                return Err(JournalError::Corrupt {
                    offset,
                    reason: Corruption::Digest,
                });
            }
            let value = codec::decode(&payload).map_err(|error| match error {
                codec::CodecError::UnsupportedVersion => JournalError::IncompatibleFormat,
                _ => JournalError::Corrupt {
                    offset,
                    reason: Corruption::Payload,
                },
            })?;
            if codec::encode(&value).map_err(|_| JournalError::InvalidPayload)? != payload {
                return Err(JournalError::Corrupt {
                    offset,
                    reason: Corruption::Payload,
                });
            }
            records.push(Record {
                sequence: next,
                digest: hash,
                payload: value,
            });
            sequence = next;
            previous = hash;
            offset = end;
        }
        Ok(records)
    }
    pub fn read_records(&mut self) -> Result<Vec<Record>> {
        if self.poisoned {
            return Err(JournalError::NeedsRecovery);
        }
        let result = self.scan().and_then(|records| {
            if records.last().map_or(0, |r| r.sequence) != self.sequence
                || records.last().map_or([0; 32], |r| r.digest) != self.digest
                || self
                    .storage
                    .byte_len()
                    .map_err(|e| io_error(IoStage::Metadata, e))?
                    != self.end
            {
                return Err(JournalError::ExternalChange);
            }
            Ok(records)
        });
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn uncertain(&mut self, stage: IoStage, error: io::Error) -> JournalError {
        self.poisoned = true;
        JournalError::WriteUncertain {
            stage,
            kind: error.kind(),
        }
    }
    pub fn append(&mut self, value: &Value) -> Result<SynchronizedReceipt> {
        if self.poisoned {
            return Err(JournalError::NeedsRecovery);
        }
        if self.sequence >= self.limits.records {
            return Err(JournalError::BudgetExceeded);
        }
        let payload = codec::encode(value).map_err(|_| JournalError::InvalidPayload)?;
        if payload.len() > self.limits.payload_bytes {
            return Err(JournalError::BudgetExceeded);
        }
        let end = self
            .end
            .checked_add(FRAME_HEADER_BYTES as u64)
            .and_then(|v| v.checked_add(payload.len() as u64))
            .ok_or(JournalError::BudgetExceeded)?;
        if end > self.limits.file_bytes {
            return Err(JournalError::BudgetExceeded);
        }
        let sequence = self.sequence + 1;
        let digest = self
            .hasher
            .digest(&hash_input(sequence, &self.digest, &payload))?;
        let size = self
            .storage
            .byte_len()
            .map_err(|e| io_error(IoStage::Metadata, e))?;
        if size != self.end {
            self.poisoned = true;
            return Err(JournalError::ExternalChange);
        }
        self.storage
            .seek(SeekFrom::Start(self.end))
            .map_err(|e| io_error(IoStage::Seek, e))?;
        let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
        frame.extend_from_slice(&sequence.to_be_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&self.digest);
        frame.extend_from_slice(&digest);
        frame.extend_from_slice(&payload);
        if let Err(e) = write_all(&mut self.storage, &frame) {
            return Err(self.uncertain(IoStage::Write, e));
        }
        if let Err(e) = self.storage.flush() {
            return Err(self.uncertain(IoStage::Flush, e));
        }
        if let Err(e) = self.storage.synchronize() {
            return Err(self.uncertain(IoStage::Synchronize, e));
        }
        match self.storage.byte_len() {
            Err(e) => return Err(self.uncertain(IoStage::Metadata, e)),
            Ok(n) if n != end => {
                self.poisoned = true;
                return Err(JournalError::ExternalChange);
            }
            _ => {}
        }
        self.sequence = sequence;
        self.digest = digest;
        self.end = end;
        Ok(SynchronizedReceipt {
            sequence,
            digest,
            end_offset: end,
        })
    }
    /// Trusted host composition preflights complete transaction envelopes. This
    /// does not write/reserve disk space and is not a filesystem capacity promise.
    /// Exclusive ownership must continue; append repeats all its own checks.
    pub fn check_append_capacity(&self, encoded_payload_lengths: &[usize]) -> Result<()> {
        if self.poisoned {
            return Err(JournalError::NeedsRecovery);
        }
        if encoded_payload_lengths.is_empty() || encoded_payload_lengths.len() > 65536 {
            return Err(JournalError::InvalidLimits);
        }
        let count = self
            .sequence
            .checked_add(encoded_payload_lengths.len() as u64)
            .ok_or(JournalError::BudgetExceeded)?;
        if count > self.limits.records {
            return Err(JournalError::BudgetExceeded);
        }
        let mut end = self.end;
        for &length in encoded_payload_lengths {
            if length < 8 || length > self.limits.payload_bytes {
                return Err(JournalError::BudgetExceeded);
            }
            end = end
                .checked_add(FRAME_HEADER_BYTES as u64)
                .and_then(|n| n.checked_add(length as u64))
                .ok_or(JournalError::BudgetExceeded)?;
        }
        if end > self.limits.file_bytes {
            return Err(JournalError::BudgetExceeded);
        }
        Ok(())
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
    /// Explicit close releases native lock; returns no raw file/storage handle.
    pub fn close(self) {
        drop(self)
    }
}
