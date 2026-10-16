#![cfg(target_os = "macos")]
#[path = "memory_store/briefs.rs"]
mod briefs;
#[path = "memory_store/fixtures.rs"]
mod frozen;
#[path = "memory_store/recall.rs"]
mod recall;
#[path = "memory_store/records.rs"]
mod records;
use eylema_host::{memory_store::*, native::NativeHash, project_data::*};
use eylema_journal::{Journal, NativeStorage};
use records::*;
use std::{
    fs::{DirBuilder, File},
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
fn sync(file: &File) -> io::Result<()> {
    eylema_platform::full_sync_file(file).map_err(io::Error::other)
}
fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("G88_PRIVATE_TEST_DIR").expect("owned test directory only"))
}
#[test]
#[ignore = "G88 helper: only the bounded parent may spend the writer phase"]
fn native_writer() {
    let dir = directory();
    let clock = Clock::new(1);
    let j = Journal::create_new(
        NativeStorage::create_new(&dir.join("data.eylj"), sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut data, owner) = ProjectStore::create_new(j, r("x").project, clock).unwrap();
    let ds = data.enroll(&owner, briefs::grant()).unwrap();
    let (mut memory, mo) = MemoryStore::create_new(
        NativeStorage::create_new(&dir.join("memory.eylj"), sync).unwrap(),
        NativeHash,
        &mut data,
        &owner,
        briefs::corpus_profile(),
        &mut c(),
    )
    .unwrap();
    briefs::seed_corpus(&mut data, &owner, &ds, &mut memory, &mo);
    for name in ["data.eylj", "memory.eylj"] {
        let path = dir.join(name);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            matches!(NativeStorage::open_existing(&path,sync),Err(e)if e.kind()==io::ErrorKind::WouldBlock)
        );
    }
    assert_eq!(memory.generation(), 32);
    memory.close();
    data.close();
    println!("G88 writer: synchronized; exclusive locks and0600files verified");
}
#[test]
#[ignore = "G88 helper: only the bounded parent may spend the fresh-reader phase"]
fn native_reader() {
    let dir = directory();
    let clock = Clock::new(80);
    let j = Journal::open(
        NativeStorage::open_existing(&dir.join("data.eylj"), sync).unwrap(),
        NativeHash,
        project_journal_limits(),
    )
    .unwrap();
    let (mut data, owner) = ProjectStore::open(j, r("x").project, clock.clone()).unwrap();
    let ds = data.enroll(&owner, briefs::grant()).unwrap();
    let (memory, mo) = MemoryStore::open(
        NativeStorage::open_existing(&dir.join("memory.eylj"), sync).unwrap(),
        NativeHash,
        &mut data,
        &owner,
        briefs::corpus_profile(),
        &mut c(),
    )
    .unwrap();
    let session = memory.enroll(&mo, &mut data, &ds, &mut c()).unwrap();
    let corpus = briefs::reopen_corpus(&data, &owner);
    for report in briefs::run_jobs(&mut data, &ds, &memory, &session, &corpus) {
        assert_eq!(report.memory_passes, report.cases);
        println!(
            "G88 native J{} memory={}/{} flat={}/{} work={}/{} peak={}",
            report.job,
            report.memory_passes,
            report.cases,
            report.flat_matches,
            report.cases,
            report.memory_work,
            report.flat_work,
            report.peak_bytes
        );
    }
    let query = briefs::job_query(1, false, 0, memory.generation());
    let cached = memory.cache(&query).unwrap();
    let current = data.inspect(&owner, &r("en-owner")).unwrap().revision;
    data.withdraw(&owner, &r("en-owner"), current).unwrap();
    assert!(
        memory
            .query_cached(&mut data, &session, &cached, &mut c())
            .is_err()
    );
    memory.close();
    let (mut memory, new_owner) = MemoryStore::open(
        NativeStorage::open_existing(&dir.join("memory.eylj"), sync).unwrap(),
        NativeHash,
        &mut data,
        &owner,
        briefs::corpus_profile(),
        &mut c(),
    )
    .unwrap();
    assert!(memory.enroll(&mo, &mut data, &ds, &mut c()).is_err());
    let fresh = memory.enroll(&new_owner, &mut data, &ds, &mut c()).unwrap();
    assert!(
        memory
            .query_cached(&mut data, &fresh, &cached, &mut c())
            .is_err()
    );
    assert!(memory.query(&mut data, &fresh, &query, &mut c()).is_err());
    let generation = memory.generation();
    memory
        .tombstone(
            &new_owner,
            &mut data,
            &fresh,
            generation,
            &id("tr-owner"),
            rev(2),
            &mut c(),
        )
        .unwrap();
    let result = memory
        .query(
            &mut data,
            &fresh,
            &briefs::job_query(2, false, 0, memory.generation()),
            &mut c(),
        )
        .unwrap();
    assert_eq!(result.status(), MemoryStatus::NoEvidence);
    clock.0.set(101);
    assert!(
        memory
            .query(
                &mut data,
                &fresh,
                &briefs::job_query(2, false, 0, memory.generation()),
                &mut c()
            )
            .is_err()
    );
    memory.close();
    data.close();
    println!(
        "G88 reader: fresh capabilities; cached/reopened withdrawal, deletion and TTL verified"
    );
}
fn exceeded(elapsed: Duration, bytes: usize) -> bool {
    elapsed > Duration::from_secs(10) || bytes > 4096
}
#[test]
fn parent_limit_boundaries() {
    assert!(!exceeded(Duration::from_secs(10), 4096));
    assert!(exceeded(
        Duration::from_secs(10) + Duration::from_nanos(1),
        4096
    ));
    assert!(exceeded(Duration::ZERO, 4097));
}
fn phase(name: &str, dir: &PathBuf) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", name, "--nocapture"])
        .env("G88_PRIVATE_TEST_DIR", dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let overflow = Arc::new(AtomicBool::new(false));
    let pipe =
        |mut input: Box<dyn Read + Send>, count: Arc<AtomicUsize>, overflow: Arc<AtomicBool>| {
            std::thread::spawn(move || {
                let mut buf = [0; 256];
                let mut bytes = vec![];
                loop {
                    let n = input.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    let previous = count.fetch_add(n, Ordering::SeqCst);
                    if previous.saturating_add(n) > 4096 {
                        overflow.store(true, Ordering::SeqCst);
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                }
                bytes
            })
        };
    let out = pipe(
        Box::new(child.stdout.take().unwrap()),
        count.clone(),
        overflow.clone(),
    );
    let err = pipe(
        Box::new(child.stderr.take().unwrap()),
        count.clone(),
        overflow.clone(),
    );
    let started = Instant::now();
    let status = loop {
        if exceeded(started.elapsed(), count.load(Ordering::SeqCst))
            || overflow.load(Ordering::SeqCst)
        {
            let _ = child.kill();
            let _ = child.wait();
            panic!("G88 phase exceeded fixed time/output bounds")
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = out.join().unwrap();
    let _stderr = err.join().unwrap();
    assert!(
        !overflow.load(Ordering::SeqCst) && count.load(Ordering::SeqCst) <= 4096,
        "G88 phase output limit"
    );
    assert!(status.success(), "G88 phase failed; no automatic retry");
    println!("{}", String::from_utf8_lossy(&stdout));
}
#[test]
#[ignore = "G88 mandatory explicit acceptance: at most two authorized native child phases"]
fn native_restart_and_eight_briefs() {
    let token = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
    let dir = std::env::temp_dir().join(format!("eyleme-g88-{token}"));
    DirBuilder::new().mode(0o700).create(&dir).unwrap();
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let cleanup = Cleanup(dir);
    phase("native_writer", &cleanup.0);
    phase("native_reader", &cleanup.0);
}
