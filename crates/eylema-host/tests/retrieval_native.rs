#![cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "retrieval.rs"]
mod f;
use eylema_contracts::{
    Revision,
    codec::{self, Value},
    project_data::*,
};
use eylema_host::{native::NativeHash, project_data::*, retrieval::*};
use eylema_journal::{Journal, NativeStorage};
use eylema_retrieval::*;
use std::{
    fs::{DirBuilder, File},
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
};
fn sync(f: &File) -> io::Result<()> {
    eylema_platform::full_sync_file(f).map_err(io::Error::other)
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let id = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
        let p = std::env::temp_dir().join(format!("eylema-retrieval-{id}"));
        DirBuilder::new().mode(0o700).create(&p).unwrap();
        Self(p)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn actual_native_files_reopen_fresh_authority_update_delete_and_lock() {
    let dir = Directory::new();
    assert_eq!(
        std::fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let path = dir.0.join("data.eylj");
    let index = dir.0.join("index.eylj");
    let journal = Journal::create_new(
        NativeStorage::create_new(&path, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut d, owner) = ProjectStore::create_new(journal, f::p(), NativeDataClock).unwrap();
    let expires = NativeDataClock.now().unwrap() + 60_000;
    let grant = || {
        let mut g = f::grant();
        g.expires_at = expires;
        g
    };
    let session = d.enroll(&owner, grant()).unwrap();
    for (id, text) in [
        ("a", "apple red"),
        ("b", "apple green"),
        ("c", "ocean blue"),
        ("d", "apple red"),
    ] {
        d.collect(
            &session,
            DataDraft {
                reference: f::r(id),
                kind: DataKind::Artifact,
                parents: vec![],
                payload: text.as_bytes().to_vec(),
                expires_at: expires,
            },
        )
        .unwrap();
        d.set_policy(
            &owner,
            &f::r(id),
            Revision::new(1).unwrap(),
            DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
            false,
        )
        .unwrap();
    }
    let (h, old_owner) = RetrievalHost::create(
        NativeStorage::create_new(&index, sync).unwrap(),
        NativeHash,
        NativeHash,
        &mut d,
        &owner,
        &session,
        &f::binding(),
        f::sources(),
        &mut f::c(),
    )
    .unwrap();
    for p in [&path, &index] {
        assert_eq!(
            std::fs::metadata(p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            matches!(NativeStorage::open_existing(p,sync),Err(e)if e.kind()==io::ErrorKind::WouldBlock)
        );
    }
    assert_eq!(
        h.search(&mut d, &session, &f::q(), &mut f::c())
            .unwrap()
            .hit_count(),
        2
    );
    drop(d);
    let j = Journal::open(
        NativeStorage::open_existing(&path, sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut d, owner) = ProjectStore::open(j, f::p(), NativeDataClock).unwrap();
    let fresh = d.enroll(&owner, grant()).unwrap();
    assert!(h.search(&mut d, &fresh, &f::q(), &mut f::c()).is_err());
    drop(h);
    let (mut h, new_owner) = RetrievalHost::open(
        NativeStorage::open_existing(&index, sync).unwrap(),
        NativeHash,
        NativeHash,
        &mut d,
        &owner,
        &fresh,
        &f::binding(),
        &mut f::c(),
    )
    .unwrap();
    assert!(h.search(&mut d, &session, &f::q(), &mut f::c()).is_err());
    assert!(
        h.replace(
            &mut d,
            &owner,
            &old_owner,
            &fresh,
            1,
            &f::binding(),
            vec![],
            &mut f::c()
        )
        .is_err()
    );
    for (mode, wants) in [
        (0, vec![1_000_000, 1_000_000, 0, -1_000_000]),
        (1, vec![1_000_000, 1_000_000, 333333, 0]),
        (2, vec![1_000_000, 1_000_000, 166666, -500000]),
    ] {
        let mut q = f::q();
        q.top_k = 4;
        q.max_hits = 4;
        let mut v = vec![0.; 1024];
        v[0] = 1.;
        q.input = match mode {
            0 => QueryInput::Vector(v),
            1 => QueryInput::Lexical("apple red".into()),
            _ => QueryInput::Hybrid {
                text: "apple red".into(),
                vector: v,
                vector_weight: 500000,
            },
        };
        let mut control = f::c();
        let response = h.search(&mut d, &fresh, &q, &mut control).unwrap();
        let Value::List(e) = codec::decode(response.bytes()).unwrap() else {
            panic!()
        };
        let Value::List(hits) = &e[7] else { panic!() };
        for (i, (hit, score)) in hits.iter().zip(wants).enumerate() {
            let Value::List(hit) = hit else { panic!() };
            assert_eq!(hit[1], Value::Text(["a", "d", "b", "c"][i].into()));
            assert_eq!(hit[9], Value::Signed(score));
            assert_eq!(hit[3], Value::Unsigned(0));
            assert_eq!(hit[4], Value::Unsigned([9, 9, 11, 10][i]));
        }
        assert_eq!(response.hit_count(), 4);
        println!(
            "native mode={mode} hits=4 work={} peak_logical_bytes={}",
            control.work_used(),
            control.peak_live_bytes()
        );
    }
    // New immutable source e replaces a; d is explicitly deleted from the index.
    d.collect(
        &fresh,
        DataDraft {
            reference: f::r("e"),
            kind: DataKind::Artifact,
            parents: vec![],
            payload: b"apple green".to_vec(),
            expires_at: expires,
        },
    )
    .unwrap();
    d.set_policy(
        &owner,
        &f::r("e"),
        Revision::new(1).unwrap(),
        DataPermissions::new(vec![DataUse::Collect, DataUse::Execute]).unwrap(),
        false,
    )
    .unwrap();
    let mut replacement = f::sources();
    replacement.retain(|s| s.reference.id.as_str() == "b" || s.reference.id.as_str() == "c");
    let mut e = f::sources().remove(1);
    e.reference = f::r("e");
    replacement.push(e);
    assert_eq!(
        h.replace(
            &mut d,
            &owner,
            &new_owner,
            &fresh,
            1,
            &f::binding(),
            replacement,
            &mut f::c()
        )
        .unwrap()
        .generation,
        2
    );
    drop(h);
    let (mut h, owner2) = RetrievalHost::open(
        NativeStorage::open_existing(&index, sync).unwrap(),
        NativeHash,
        NativeHash,
        &mut d,
        &owner,
        &fresh,
        &f::binding(),
        &mut f::c(),
    )
    .unwrap();
    let mut q = f::q();
    q.generation = 2;
    q.sources = vec![f::r("d")];
    assert!(matches!(
        h.search(&mut d, &fresh, &q, &mut f::c()),
        Err(HostRetrievalError::Core(RetrievalError::NotIndexed))
    ));
    q.sources = vec![f::r("b"), f::r("c"), f::r("e")];
    q.max_hits = 3;
    assert_eq!(
        h.search(&mut d, &fresh, &q, &mut f::c())
            .unwrap()
            .hit_count(),
        2
    );
    h.replace(
        &mut d,
        &owner,
        &owner2,
        &fresh,
        2,
        &f::binding(),
        vec![],
        &mut f::c(),
    )
    .unwrap();
    drop(h);
    let (h, _) = RetrievalHost::open(
        NativeStorage::open_existing(&index, sync).unwrap(),
        NativeHash,
        NativeHash,
        &mut d,
        &owner,
        &fresh,
        &f::binding(),
        &mut f::c(),
    )
    .unwrap();
    q.generation = 3;
    assert!(h.search(&mut d, &fresh, &q, &mut f::c()).is_err());
    drop(h);
    let mut wrong = f::binding();
    wrong.space.dimensions = 2;
    assert!(
        RetrievalHost::open(
            NativeStorage::open_existing(&index, sync).unwrap(),
            NativeHash,
            NativeHash,
            &mut d,
            &owner,
            &fresh,
            &wrong,
            &mut f::c()
        )
        .is_err()
    );
}
