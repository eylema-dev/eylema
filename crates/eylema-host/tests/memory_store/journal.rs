use super::records::*;
use eylema_contracts::codec::{self, Value};
use eylema_host::{memory_store::*, native::NativeHash};
use eylema_journal::{Journal, Limits, Sha256};
use std::time::{Duration, Instant};
#[test]
fn replay_escrow_rejects_before_read() {
    let (mut d, o, _, _) = data();
    let m = Mem::default();
    let (s, _) =
        MemoryStore::create_new(m.clone(), NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    s.close();
    let reads = m.reads.get();
    let mut low = MemoryControl::new(
        MemoryBudget {
            max_work: 51_380_223,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(5),
        },
        MemoryCancel::new(),
    )
    .unwrap();
    assert_eq!(
        MemoryStore::open(m.clone(), NativeHash, &mut d, &o, profile(), &mut low).err(),
        Some(MemoryStoreError::Limit)
    );
    assert_eq!(m.reads.get(), reads);
}
#[test]
fn guard_precedes_codec_allocation() {
    let (mut d, o, _, _) = data();
    for payload in [
        vec![b'E', b'Y', b'L', 1, 0, 1, 0, 7, 255, 255, 255, 255],
        codec::encode(&Value::List(vec![Value::Null; 33])).unwrap(),
        codec::encode(&Value::Map(vec![("hostile".into(), Value::Null)])).unwrap(),
    ] {
        let m = Mem::default();
        let mut raw = Vec::from(*eylema_journal::HEADER);
        let mut hash = Vec::from(*eylema_journal::HEADER);
        hash.extend(1u64.to_be_bytes());
        hash.extend((payload.len() as u32).to_be_bytes());
        hash.extend([0; 32]);
        hash.extend(&payload);
        raw.extend(1u64.to_be_bytes());
        raw.extend((payload.len() as u32).to_be_bytes());
        raw.extend([0; 32]);
        raw.extend(NativeHash.digest(&hash).unwrap());
        raw.extend(payload);
        *m.bytes.borrow_mut().get_mut() = raw;
        assert!(MemoryStore::open(m, NativeHash, &mut d, &o, profile(), &mut c()).is_err());
    }
}
#[test]
fn fragmented_io_is_finite() {
    let (mut d, o, _, _) = data();
    let m = Mem::default();
    let mut j = Journal::create_new(
        m.clone(),
        NativeHash,
        Limits {
            records: 512,
            payload_bytes: 8192,
            file_bytes: 2 * 1024 * 1024,
        },
    )
    .unwrap();
    j.append(&Value::Bytes(vec![0; 6000])).unwrap();
    j.close();
    m.fragment.set(true);
    assert!(MemoryStore::open(m.clone(), NativeHash, &mut d, &o, profile(), &mut c()).is_err());
    assert!(m.reads.get() <= 4096);
}
#[test]
fn fragmented_append_stops_before_1025th_write() {
    let (mut d, o, ds, _) = data();
    let m = Mem::default();
    let (mut s, owner) =
        MemoryStore::create_new(m.clone(), NativeHash, &mut d, &o, profile(), &mut c()).unwrap();
    let session = s.enroll(&owner, &mut d, &ds, &mut c()).unwrap();
    let source = pin(&mut d, &ds, "source");
    let mut vector = vec![0.; 1024];
    vector[0] = 1.;
    m.fragment.set(true);
    let before = m.writes.get();
    assert_eq!(
        s.propose(
            &mut d,
            &session,
            0,
            None,
            entity(source),
            Some(&vector),
            r("entity:1"),
            &mut c()
        )
        .err(),
        Some(MemoryStoreError::WriteOutcomeUnknown)
    );
    assert_eq!(m.writes.get() - before, 1024);
    assert_eq!(s.generation(), 0);
}
