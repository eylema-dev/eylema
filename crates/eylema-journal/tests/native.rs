#![cfg(target_os = "macos")]
use eylema_contracts::codec::Value;
use eylema_journal::*;
use std::cell::Cell;
use std::fs::File;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
thread_local! { static FAIL_SYNC: Cell<bool> = const { Cell::new(false) }; }
struct Hash;
impl Sha256 for Hash {
    fn digest(&mut self, bytes: &[u8]) -> Result<[u8; 32]> {
        eylema_platform::sha256(bytes).map_err(|_| JournalError::HashUnavailable)
    }
}
fn sync(file: &File) -> io::Result<()> {
    if FAIL_SYNC.get() {
        return Err(io::ErrorKind::Other.into());
    }
    eylema_platform::full_sync_file(file).map_err(io::Error::other)
}
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let id = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
        let path = std::env::temp_dir().join(format!("eylema-journal-test-{id}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn actual_native_lock_permissions_sync_failure_and_reopen_have_explicit_outcomes() {
    let dir = Directory::new();
    let path = dir.0.join("journal.eylj");
    let store = NativeStorage::create_new(&path, sync).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut journal = Journal::create_new(store, Hash, Limits::default()).unwrap();
    assert!(
        matches!(NativeStorage::open_existing(&path, sync), Err(e) if e.kind() == io::ErrorKind::WouldBlock)
    );
    assert!(
        matches!(NativeStorage::create_new(&path, sync), Err(e) if e.kind() == io::ErrorKind::AlreadyExists)
    );
    journal.append(&Value::Unsigned(1)).unwrap();
    FAIL_SYNC.set(true);
    let failed = journal.append(&Value::Unsigned(2));
    FAIL_SYNC.set(false);
    assert_eq!(
        failed,
        Err(JournalError::WriteUncertain {
            stage: IoStage::Synchronize,
            kind: io::ErrorKind::Other
        })
    );
    assert!(journal.is_poisoned());
    assert_eq!(
        journal.append(&Value::Unsigned(2)),
        Err(JournalError::NeedsRecovery)
    );
    journal.close();
    let mut reopened = Journal::open(
        NativeStorage::open_existing(&path, sync).unwrap(),
        Hash,
        Limits::default(),
    )
    .unwrap();
    // Readable completed bytes cannot retroactively acknowledge the failed append.
    assert_eq!(reopened.read_records().unwrap().len(), 2);
    assert_eq!(reopened.append(&Value::Unsigned(3)).unwrap().sequence, 3);
    reopened.close();
}
#[test]
fn actual_full_sync_rejects_non_file_device_and_existing_empty_journal_is_truncated() {
    assert!(eylema_platform::full_sync_file(&File::open("/dev/null").unwrap()).is_err());
    let dir = Directory::new();
    let path = dir.0.join("empty.eylj");
    File::create(&path).unwrap();
    assert!(matches!(
        Journal::open(
            NativeStorage::open_existing(&path, sync).unwrap(),
            Hash,
            Limits::default()
        ),
        Err(JournalError::Truncated { offset: 0 })
    ));
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
}
