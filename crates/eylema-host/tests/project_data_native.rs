#![cfg(target_os = "macos")]
use eylema_contracts::{Revision, platform::ProjectId, project_data::*};
use eylema_host::{native::NativeHash, project_data::*};
use eylema_journal::{Journal, NativeStorage};
use std::{
    fs::{DirBuilder, File},
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
};
fn sync(file: &File) -> io::Result<()> {
    eylema_platform::full_sync_file(file).map_err(io::Error::other)
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let id = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
        let path = std::env::temp_dir().join(format!("eylema-project-data-test-{id}"));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn p(id: &str) -> ProjectId {
    ProjectId::new(id).unwrap()
}
#[test]
fn actual_two_project_files_full_sync_reopen_and_withdrawal() {
    let dir = Directory::new();
    assert_eq!(
        std::fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // Host-selected fixed names: logical project IDs never become file paths.
    let path_a = dir.0.join("a.eylj");
    let path_b = dir.0.join("b.eylj");
    let ja = Journal::create_new(
        NativeStorage::create_new(&path_a, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let jb = Journal::create_new(
        NativeStorage::create_new(&path_b, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut a, owner) = ProjectStore::create_new(ja, p(".."), NativeDataClock).unwrap();
    let (mut b, other) = ProjectStore::create_new(jb, p("project:b"), NativeDataClock).unwrap();
    for path in [&path_a, &path_b] {
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            matches!(NativeStorage::open_existing(path,sync),Err(e) if e.kind()==io::ErrorKind::WouldBlock)
        );
    }
    let expires = NativeDataClock.now().unwrap() + 60_000;
    let id = DataId::new("record:root").unwrap();
    let grant = || DataGrant {
        permissions: DataPermissions::new(DataUse::ALL.to_vec()).unwrap(),
        scope: vec![id.clone()],
        expires_at: expires,
    };
    let sa = a.enroll(&owner, grant()).unwrap();
    let sb = b.enroll(&other, grant()).unwrap();
    let r = DataRef {
        project: p(".."),
        id: id.clone(),
    };
    a.collect(
        &sa,
        DataDraft {
            reference: r.clone(),
            kind: DataKind::Episode,
            parents: vec![],
            payload: b"synthetic native episode".to_vec(),
            expires_at: expires,
        },
    )
    .unwrap();
    let meta = a
        .set_policy(
            &owner,
            &r,
            Revision::new(1).unwrap(),
            DataPermissions::new(DataUse::ALL.to_vec()).unwrap(),
            true,
        )
        .unwrap();
    assert_eq!(
        a.read(&sa, &r, DataUse::Execute).unwrap(),
        b"synthetic native episode"
    );
    assert_eq!(a.read(&sb, &r, DataUse::Execute), Err(DataError::Forbidden));
    assert_eq!(
        b.read(&sb, &r, DataUse::Execute),
        Err(DataError::WrongProject)
    );
    a.close();
    b.close();
    let ja = Journal::open(
        NativeStorage::open_existing(&path_a, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    assert!(matches!(
        ProjectStore::open(ja, p("project:b"), NativeDataClock),
        Err(DataError::WrongProject)
    ));
    let ja = Journal::open(
        NativeStorage::open_existing(&path_a, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut a, fresh) = ProjectStore::open(ja, p(".."), NativeDataClock).unwrap();
    assert_eq!(a.inspect(&fresh, &r).unwrap(), meta);
    assert_eq!(a.read(&sa, &r, DataUse::Execute), Err(DataError::Forbidden));
    let s = a.enroll(&fresh, grant()).unwrap();
    assert_eq!(
        a.read(&s, &r, DataUse::Execute).unwrap(),
        b"synthetic native episode"
    );
    a.withdraw(&fresh, &r, meta.revision).unwrap();
    assert_eq!(a.read(&s, &r, DataUse::Execute), Err(DataError::Forbidden));
    a.close();
    let ja = Journal::open(
        NativeStorage::open_existing(&path_a, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut a, fresh) = ProjectStore::open(ja, p(".."), NativeDataClock).unwrap();
    let s = a.enroll(&fresh, grant()).unwrap();
    assert_eq!(a.read(&s, &r, DataUse::Execute), Err(DataError::Forbidden));
    assert!(a.inspect(&fresh, &r).unwrap().withdrawn);
    a.close();
}
