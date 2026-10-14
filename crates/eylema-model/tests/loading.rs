mod support;
use eylema_model::*;
use std::io::{self, Cursor, Read};
use support::*;
fn budget(n: u64) -> LoadBudget {
    LoadBudget {
        max_artifact_bytes: n,
        max_resident_content_bytes: n + 4194304,
        max_read_calls: 65568,
        max_interruptions: 16,
    }
}
struct Chunked {
    data: Cursor<Vec<u8>>,
    width: usize,
    interruptions: u32,
}
impl Read for Chunked {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if self.interruptions > 0 {
            self.interruptions -= 1;
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = b.len().min(self.width);
        self.data.read(&mut b[..n])
    }
}
struct NoRead;
impl Read for NoRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("preallocation rejection consumed input")
    }
}
#[test]
fn exact_small_snapshot_and_read_chunking() {
    for dt in [StoredDtype::F32, StoredDtype::BF16] {
        let data = fixture(dt);
        let n = data.len() as u64;
        let m = manifest(dt, n, digest(dt));
        let l = layout(&m);
        for width in [1, 3, 7, 65536] {
            let mut r = Chunked {
                data: Cursor::new(data.clone()),
                width,
                interruptions: 16,
            };
            let got = load(&mut r, &m, &l, budget(n)).unwrap();
            assert_eq!(got.artifact_len(), n);
            assert_eq!(got.artifact_sha256(), digest(dt));
            assert_eq!(got.manifest_sha256(), l.manifest_sha256());
            assert_eq!(got.layout_sha256(), l.layout_sha256());
            if dt == StoredDtype::F32 {
                assert_eq!(
                    got.tensor_bytes("a"),
                    Some(&[0, 0, 128, 63, 0, 0, 0, 192][..])
                );
                assert_eq!(
                    got.tensor_bytes("b"),
                    Some(&[0, 0, 64, 64, 0, 0, 128, 64][..])
                );
            } else {
                assert_eq!(got.tensor_bytes("a"), Some(&[128, 63, 0, 192][..]));
                assert_eq!(got.tensor_bytes("b"), Some(&[64, 64, 128, 64][..]));
            }
            assert_eq!(got.tensor_spec("b").unwrap().shape(), &[1, 2]);
            assert!(got.tensor_bytes("missing").is_none());
            assert!(got.tensor_spec(&"x".repeat(129)).is_none());
            let debug = format!("{got:?}");
            assert!(!debug.contains("128") && !debug.contains("data_offsets"));
        }
    }
}
#[test]
fn changed_truncated_and_extra_input_reject() {
    let dt = StoredDtype::F32;
    let f = fixture(dt);
    let m = manifest(dt, 134, digest(dt));
    let l = layout(&m);
    let mut changed = f.clone();
    changed[133] ^= 1;
    assert_eq!(
        load(&mut Cursor::new(changed), &m, &l, budget(134)).unwrap_err(),
        LoadError::HashMismatch
    );
    for n in [0, 3, 8, 90, 118, 133] {
        assert_eq!(
            load(&mut Cursor::new(f[..n].to_vec()), &m, &l, budget(134)).unwrap_err(),
            LoadError::Io
        );
    }
    let mut extra = f;
    extra.push(9);
    assert_eq!(
        load(&mut Cursor::new(extra), &m, &l, budget(134)).unwrap_err(),
        LoadError::SizeLimit
    );
}
#[test]
fn budgets_and_manifest_mismatch_fail_before_read() {
    let dt = StoredDtype::F32;
    let m = manifest(dt, 134, digest(dt));
    let l = layout(&m);
    let mut b = budget(134);
    b.max_artifact_bytes = 133;
    assert_eq!(
        load(&mut NoRead, &m, &l, b).unwrap_err(),
        LoadError::SizeLimit
    );
    let mut b = budget(134);
    b.max_resident_content_bytes -= 1;
    assert_eq!(
        load(&mut NoRead, &m, &l, b).unwrap_err(),
        LoadError::ResidentLimit
    );
    let huge = manifest(dt, 2147483649, digest(dt));
    assert_eq!(
        load(&mut NoRead, &huge, &l, budget(2147483648)).unwrap_err(),
        LoadError::SizeLimit
    );
    let mut d = m.fields().clone();
    d.id = eylema_contracts::platform::ModelId::new("changed-model").unwrap();
    let changed = eylema_contracts::model_contract::ModelManifest::new(d).unwrap();
    assert_eq!(
        load(&mut NoRead, &changed, &l, budget(134)).unwrap_err(),
        LoadError::InvalidProfile
    );
    for b in [
        LoadBudget {
            max_read_calls: 0,
            ..budget(134)
        },
        LoadBudget {
            max_interruptions: 17,
            ..budget(134)
        },
        LoadBudget {
            max_resident_content_bytes: u64::MAX,
            ..budget(134)
        },
    ] {
        assert_eq!(
            load(&mut NoRead, &m, &l, b).unwrap_err(),
            LoadError::InvalidProfile
        );
    }
}
#[test]
fn reader_work_and_header_size_are_bounded() {
    let dt = StoredDtype::F32;
    let m = manifest(dt, 134, digest(dt));
    let l = layout(&m);
    let mut r = Chunked {
        data: Cursor::new(fixture(dt)),
        width: 1,
        interruptions: 17,
    };
    assert_eq!(
        load(&mut r, &m, &l, budget(134)).unwrap_err(),
        LoadError::WorkLimit
    );
    let mut b = budget(134);
    b.max_read_calls = 1;
    assert_eq!(
        load(&mut Cursor::new(fixture(dt)), &m, &l, b).unwrap_err(),
        LoadError::WorkLimit
    );
    assert_eq!(
        load(
            &mut Cursor::new(1048577u64.to_le_bytes()),
            &m,
            &l,
            budget(134)
        )
        .unwrap_err(),
        LoadError::SizeLimit
    );
    struct BadCount;
    impl Read for BadCount {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            Ok(b.len() + 1)
        }
    }
    assert_eq!(
        load(&mut BadCount, &m, &l, budget(134)).unwrap_err(),
        LoadError::Io
    );
    struct Failing;
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("private caller data"))
        }
    }
    let e = load(&mut Failing, &m, &l, budget(134)).unwrap_err();
    assert_eq!(e, LoadError::Io);
    assert!(!format!("{e:?}").contains("private"));
}
#[test]
fn owned_file_snapshot_survives_overwrite_and_removal() {
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    let p = std::env::temp_dir().join(format!(
        "eylema-g28-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)
        .unwrap();
    f.write_all(&fixture(StoredDtype::F32)).unwrap();
    drop(f);
    let m = manifest(StoredDtype::F32, 134, digest(StoredDtype::F32));
    let got = load(&mut File::open(&p).unwrap(), &m, &layout(&m), budget(134)).unwrap();
    std::fs::write(&p, b"changed").unwrap();
    std::fs::remove_file(&p).unwrap();
    assert_eq!(
        got.tensor_bytes("a"),
        Some(&[0, 0, 128, 63, 0, 0, 0, 192][..])
    );
}

#[test]
fn undersized_artifact_rejects_before_prefix_read() {
    struct NoRead;
    impl Read for NoRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("known undersized artifact consumed input")
        }
    }
    for dt in [StoredDtype::F32, StoredDtype::BF16] {
        for size in 1..8 {
            let m = manifest(dt, size, digest(dt));
            let l = layout(&m);
            assert_eq!(
                load(&mut NoRead, &m, &l, budget(size)).unwrap_err(),
                LoadError::SizeLimit
            );
        }
    }
}
