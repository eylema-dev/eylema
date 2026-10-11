//! Fault fixtures model IO decisions, not physical media failures.
use eylema_contracts::codec::{self, Value};
use eylema_journal::*;
use std::cell::{Cell, RefCell};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::rc::Rc;

#[derive(Default)]
struct Faults {
    write_after: Option<usize>,
    written: usize,
    max_write: Option<usize>,
    zero: bool,
    interrupt_write: bool,
    interrupt_read: bool,
    read: bool,
    seek: bool,
    metadata: bool,
    metadata_after_write: bool,
    flush: bool,
    sync_stage: Option<usize>,
    sync_calls: usize,
    writes: usize,
}
#[derive(Clone)]
struct Memory {
    bytes: Rc<RefCell<Cursor<Vec<u8>>>>,
    faults: Rc<RefCell<Faults>>,
}
impl Memory {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(Cursor::new(bytes))),
            faults: Rc::new(RefCell::new(Faults::default())),
        }
    }
    fn data(&self) -> Vec<u8> {
        self.bytes.borrow().get_ref().clone()
    }
}
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let f = self.faults.borrow();
        if f.interrupt_read {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if f.read {
            return Err(io::ErrorKind::Other.into());
        }
        drop(f);
        self.bytes.borrow_mut().read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let mut f = self.faults.borrow_mut();
        f.writes += 1;
        if f.interrupt_write {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if f.zero {
            return Ok(0);
        }
        let remaining = f
            .write_after
            .map_or(b.len(), |n| n.saturating_sub(f.written));
        if remaining == 0 {
            return Err(io::Error::from_raw_os_error(28));
        }
        let n = b.len().min(remaining).min(f.max_write.unwrap_or(b.len()));
        let n = self.bytes.borrow_mut().write(&b[..n])?;
        f.written += n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.faults.borrow().flush {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
impl Seek for Memory {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if self.faults.borrow().seek {
            Err(io::ErrorKind::Other.into())
        } else {
            self.bytes.borrow_mut().seek(pos)
        }
    }
}
impl Storage for Memory {
    fn byte_len(&mut self) -> io::Result<u64> {
        let f = self.faults.borrow();
        if f.metadata || (f.metadata_after_write && f.written > 0) {
            return Err(io::ErrorKind::Other.into());
        }
        Ok(self.bytes.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        let mut f = self.faults.borrow_mut();
        for stage in 0..3 {
            f.sync_calls += 1;
            if f.sync_stage == Some(stage) {
                return Err(io::ErrorKind::Other.into());
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Hash(Rc<Cell<bool>>);
impl Hash {
    fn new() -> Self {
        Self(Rc::new(Cell::new(false)))
    }
}
impl Sha256 for Hash {
    fn digest(&mut self, b: &[u8]) -> Result<[u8; 32]> {
        if self.0.get() {
            return Err(JournalError::HashUnavailable);
        }
        eylema_platform::sha256(b).map_err(|_| JournalError::HashUnavailable)
    }
}
fn fixture(limits: Limits) -> (Journal<Memory, Hash>, Memory, Hash) {
    let store = Memory::new(vec![]);
    let hash = Hash::new();
    let j = Journal::create_new(store.clone(), hash.clone(), limits).unwrap();
    {
        let mut f = store.faults.borrow_mut();
        f.written = 0;
        f.writes = 0;
    }
    (j, store, hash)
}
fn open(bytes: Vec<u8>) -> Result<Journal<Memory, Hash>> {
    Journal::open(Memory::new(bytes), Hash::new(), Limits::default())
}
fn valid() -> Vec<u8> {
    let (mut j, s, _) = fixture(Limits::default());
    j.append(&Value::Text("first".into())).unwrap();
    j.append(&Value::Unsigned(42)).unwrap();
    s.data()
}
#[test]
fn synchronized_records_close_and_reopen_keep_sequence_values_and_digest_chain() {
    let (mut j, s, _) = fixture(Limits::default());
    assert_eq!(s.data(), HEADER);
    let first = j.append(&Value::Unsigned(2)).unwrap();
    let second = j.append(&Value::Unsigned(5)).unwrap();
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    assert_ne!(first.digest, second.digest);
    j.close();
    let mut reopened = open(s.data()).unwrap();
    let records = reopened.read_records().unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| r.payload.clone())
            .collect::<Vec<_>>(),
        vec![Value::Unsigned(2), Value::Unsigned(5)]
    );
    assert_eq!(records[1].digest, second.digest);
    assert_eq!(reopened.append(&Value::Null).unwrap().sequence, 3);
}
#[test]
fn every_cut_inside_header_or_frame_is_explicit_and_never_silently_repaired() {
    let bytes = valid();
    let first_len = u32::from_be_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let boundary = 8 + FRAME_HEADER_BYTES + first_len;
    for cut in 0..=bytes.len() {
        let sliced = bytes[..cut].to_vec();
        let store = Memory::new(sliced.clone());
        let result = Journal::open(store.clone(), Hash::new(), Limits::default());
        if [8, boundary, bytes.len()].contains(&cut) {
            assert!(result.is_ok(), "valid boundary {cut}");
        } else {
            assert!(
                matches!(result, Err(JournalError::Truncated { .. })),
                "cut{cut}: unexpected outcome"
            );
        }
        assert_eq!(store.data(), sliced);
        assert_eq!(store.faults.borrow().writes, 0);
    }
}
#[test]
fn corrupted_header_version_flags_sequence_length_chain_and_digest_are_distinct() {
    let bytes = valid();
    for (offset, error) in [
        (
            0,
            JournalError::Corrupt {
                offset: 0,
                reason: Corruption::Header,
            },
        ),
        (4, JournalError::IncompatibleFormat),
        (7, JournalError::IncompatibleFormat),
        (
            8,
            JournalError::Corrupt {
                offset: 8,
                reason: Corruption::Sequence,
            },
        ),
        (
            20,
            JournalError::Corrupt {
                offset: 8,
                reason: Corruption::Chain,
            },
        ),
        (
            52,
            JournalError::Corrupt {
                offset: 8,
                reason: Corruption::Digest,
            },
        ),
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        assert!(matches!(open(bad),Err(e) if e==error));
    }
    let mut bad = bytes.clone();
    bad[16..20].copy_from_slice(&0u32.to_be_bytes());
    assert!(matches!(
        open(bad),
        Err(JournalError::Corrupt {
            reason: Corruption::Length,
            ..
        })
    ));
    let mut bad = bytes;
    bad[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(open(bad), Err(JournalError::BudgetExceeded)));
}
fn rewrite_first_hash(bytes: &mut [u8]) {
    let len = u32::from_be_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let mut input = HEADER.to_vec();
    input.extend_from_slice(&bytes[8..52]);
    input.extend_from_slice(&bytes[84..84 + len]);
    let hash = eylema_platform::sha256(&input).unwrap();
    bytes[52..84].copy_from_slice(&hash);
}
#[test]
fn canonical_payload_and_codec_versions_are_checked_after_a_valid_digest() {
    let (mut j, s, _) = fixture(Limits::default());
    j.append(&Value::Null).unwrap();
    let mut unsupported = s.data();
    unsupported[84 + 3] = 2;
    rewrite_first_hash(&mut unsupported);
    assert!(matches!(
        open(unsupported),
        Err(JournalError::IncompatibleFormat)
    ));
    let mut invalid = s.data();
    invalid[84 + 7] = 255;
    rewrite_first_hash(&mut invalid);
    assert!(matches!(
        open(invalid),
        Err(JournalError::Corrupt {
            reason: Corruption::Payload,
            ..
        })
    ));
    let mut j = open(s.data()).unwrap();
    assert_eq!(j.read_records().unwrap()[0].payload, Value::Null);
}
#[test]
fn short_writes_finish_but_partial_disk_failure_has_no_receipt_and_poisoned_writer() {
    let (mut j, s, _) = fixture(Limits::default());
    s.faults.borrow_mut().max_write = Some(3);
    j.append(&Value::Unsigned(42)).unwrap();
    assert!(s.faults.borrow().writes > 1);
    let threshold = s.faults.borrow().written + 10;
    s.faults.borrow_mut().write_after = Some(threshold);
    let result = j.append(&Value::Unsigned(99));
    assert!(matches!(
        result,
        Err(JournalError::WriteUncertain {
            stage: IoStage::Write,
            ..
        })
    ));
    assert!(j.is_poisoned());
    assert_eq!(j.append(&Value::Null), Err(JournalError::NeedsRecovery));
    assert_eq!(j.read_records(), Err(JournalError::NeedsRecovery));
    assert!(matches!(
        open(s.data()),
        Err(JournalError::Truncated { .. })
    ));
}
#[test]
fn zero_and_repeated_interrupted_writes_have_finite_failure_not_acknowledgement() {
    for interrupt in [false, true] {
        let (mut j, s, _) = fixture(Limits::default());
        {
            let mut f = s.faults.borrow_mut();
            f.interrupt_write = interrupt;
            f.zero = !interrupt;
        }
        let outcome = j.append(&Value::Null);
        assert!(matches!(
            outcome,
            Err(JournalError::WriteUncertain {
                stage: IoStage::Write,
                ..
            })
        ));
        assert!(j.is_poisoned());
        assert_eq!(s.data(), HEADER);
        assert!(s.faults.borrow().writes <= 4096);
    }
}
#[test]
fn flush_and_each_file_parent_media_sync_failure_do_not_ack_complete_bytes() {
    for stage in 0..4 {
        let (mut j, s, _) = fixture(Limits::default());
        {
            let mut f = s.faults.borrow_mut();
            if stage == 0 {
                f.flush = true;
            } else {
                f.sync_stage = Some(stage - 1);
            }
        }
        let result = j.append(&Value::Unsigned(7));
        assert!(matches!(
            result,
            Err(JournalError::WriteUncertain {
                stage: IoStage::Flush | IoStage::Synchronize,
                ..
            })
        ));
        assert!(j.is_poisoned());
        assert_eq!(j.append(&Value::Null), Err(JournalError::NeedsRecovery));
        let mut reopened = open(s.data()).unwrap();
        assert_eq!(
            reopened.read_records().unwrap()[0].payload,
            Value::Unsigned(7)
        ); /* parsed complete bytes do not recover an old client receipt */
    }
}
#[test]
fn failed_new_header_write_flush_or_sync_never_creates_an_accepted_journal() {
    for stage in 0..3 {
        let s = Memory::new(vec![]);
        {
            let mut f = s.faults.borrow_mut();
            match stage {
                0 => f.zero = true,
                1 => f.flush = true,
                _ => f.sync_stage = Some(2),
            }
        }
        assert!(matches!(
            Journal::create_new(s.clone(), Hash::new(), Limits::default()),
            Err(JournalError::WriteUncertain { .. })
        ));
    }
    let s = Memory::new(vec![1, 2, 3]);
    assert!(matches!(
        Journal::create_new(s.clone(), Hash::new(), Limits::default()),
        Err(JournalError::NotEmpty)
    ));
    assert_eq!(s.data(), vec![1, 2, 3]);
}
#[test]
fn read_seek_metadata_and_finite_interruption_failures_are_explicit() {
    for stage in 0..4 {
        let s = Memory::new(valid());
        {
            let mut f = s.faults.borrow_mut();
            match stage {
                0 => f.read = true,
                1 => f.seek = true,
                2 => f.metadata = true,
                _ => f.interrupt_read = true,
            }
        }
        assert!(matches!(
            Journal::open(s, Hash::new(), Limits::default()),
            Err(JournalError::Io { .. })
        ));
    }
    let (mut j, s, _) = fixture(Limits::default());
    j.append(&Value::Null).unwrap();
    s.faults.borrow_mut().read = true;
    assert!(matches!(
        j.read_records(),
        Err(JournalError::Io {
            stage: IoStage::Read,
            ..
        })
    ));
    assert!(j.is_poisoned());
    assert_eq!(j.append(&Value::Null), Err(JournalError::NeedsRecovery));
}
#[test]
fn post_write_metadata_failure_marks_outcome_uncertain() {
    let (mut j, s, _) = fixture(Limits::default());
    s.faults.borrow_mut().metadata_after_write = true;
    assert!(matches!(
        j.append(&Value::Null),
        Err(JournalError::WriteUncertain {
            stage: IoStage::Metadata,
            ..
        })
    ));
    assert!(j.is_poisoned());
    assert_eq!(j.append(&Value::Null), Err(JournalError::NeedsRecovery));
}
#[test]
fn payload_record_file_and_configuration_limits_reject_before_writes() {
    for limits in [
        Limits {
            records: 0,
            ..Limits::default()
        },
        Limits {
            payload_bytes: 7,
            ..Limits::default()
        },
        Limits {
            payload_bytes: MAX_PAYLOAD_BYTES + 1,
            ..Limits::default()
        },
        Limits {
            file_bytes: 7,
            ..Limits::default()
        },
    ] {
        let s = Memory::new(vec![]);
        assert!(matches!(
            Journal::create_new(s.clone(), Hash::new(), limits),
            Err(JournalError::InvalidLimits)
        ));
        assert_eq!(s.faults.borrow().writes, 0);
    }
    let (mut j, s, _) = fixture(Limits {
        records: 1,
        payload_bytes: 16,
        file_bytes: 128,
    });
    assert_eq!(
        j.append(&Value::Text("too long for this frame".into())),
        Err(JournalError::BudgetExceeded)
    );
    assert_eq!(s.data(), HEADER);
    j.append(&Value::Null).unwrap();
    let before = s.data();
    assert_eq!(j.append(&Value::Null), Err(JournalError::BudgetExceeded));
    assert_eq!(s.data(), before);
    let (mut j, s, _) = fixture(Limits {
        file_bytes: 80,
        ..Limits::default()
    });
    assert_eq!(j.append(&Value::Null), Err(JournalError::BudgetExceeded));
    assert_eq!(s.data(), HEADER);
}
#[test]
fn hash_unavailability_and_invalid_payload_have_no_write_or_poison_side_effect() {
    let (mut j, s, h) = fixture(Limits::default());
    h.0.set(true);
    assert_eq!(j.append(&Value::Null), Err(JournalError::HashUnavailable));
    assert!(!j.is_poisoned());
    assert_eq!(s.data(), HEADER);
    h.0.set(false);
    let mut deep = Value::Null;
    for _ in 0..40 {
        deep = Value::List(vec![deep]);
    }
    assert_eq!(j.append(&deep), Err(JournalError::InvalidPayload));
    assert_eq!(s.data(), HEADER);
    j.append(&Value::Null).unwrap();
    h.0.set(true);
    assert!(matches!(
        Journal::open(s, h, Limits::default()),
        Err(JournalError::HashUnavailable)
    ));
}
#[test]
fn external_growth_and_replacement_do_not_silently_rebind_a_live_writer() {
    let (mut j, s, _) = fixture(Limits::default());
    s.bytes.borrow_mut().get_mut().push(99);
    assert_eq!(j.append(&Value::Null), Err(JournalError::ExternalChange));
    assert!(j.is_poisoned());
    let (mut j, s, _) = fixture(Limits::default());
    j.append(&Value::Unsigned(2)).unwrap();
    let mut replacement = s.data();
    let last = replacement.len() - 1;
    replacement[last] = 3;
    rewrite_first_hash(&mut replacement);
    *s.bytes.borrow_mut() = Cursor::new(replacement);
    assert_eq!(j.read_records(), Err(JournalError::ExternalChange));
    assert!(j.is_poisoned());
}
#[test]
fn claimed_authority_remains_inert_stored_data() {
    let (mut j, _, _) = fixture(Limits::default());
    let value = Value::Map(vec![
        ("status".into(), Value::Text("succeeded".into())),
        ("claimed_human".into(), Value::Bool(true)),
    ]);
    j.append(&value).unwrap();
    let records = j.read_records().unwrap();
    assert_eq!(
        codec::encode(&records[0].payload).unwrap(),
        codec::encode(&value).unwrap()
    ); // storage treats claims as inert data
}
