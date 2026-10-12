//! Actual macOS child-process abrupt-exit/restart. No physical power-cut or
//! external effect; owned task memory and trusted host enrollment fixture only.
#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod support;
use eylema_contracts::*;
use eylema_host::*;
use eylema_journal::{Journal, Limits as JournalLimits, NativeStorage, Sha256, Storage};
use eylema_platform::{IdKind, random_id, sha256};
use eylema_runtime::{HostServices, Limits};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};
struct NativeServices;
impl HostServices for NativeServices {
    fn now(&mut self) -> eylema_runtime::Result<Timestamp> {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrorCode::CapabilityUnavailable)?
            .as_millis();
        Timestamp::new(u64::try_from(n).map_err(|_| ErrorCode::CapabilityUnavailable)?)
            .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn execution_id(&mut self) -> eylema_runtime::Result<ExecutionId> {
        ExecutionId::new(
            random_id(IdKind::Execution).map_err(|_| ErrorCode::CapabilityUnavailable)?,
        )
        .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn event_id(&mut self) -> eylema_runtime::Result<EventId> {
        EventId::new(random_id(IdKind::Event).map_err(|_| ErrorCode::CapabilityUnavailable)?)
            .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn sha256(&mut self, b: &[u8]) -> eylema_runtime::Result<[u8; 32]> {
        sha256(b).map_err(|_| ErrorCode::CapabilityUnavailable)
    }
}
struct Hash;
impl Sha256 for Hash {
    fn digest(&mut self, b: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        sha256(b).map_err(|_| eylema_journal::JournalError::HashUnavailable)
    }
}
fn full_sync(f: &File) -> io::Result<()> {
    eylema_platform::full_sync_file(f).map_err(io::Error::other)
}
struct CrashStorage {
    storage: NativeStorage,
    phase: Arc<AtomicU8>,
    last_tag: String,
}
impl Read for CrashStorage {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.storage.read(b)
    }
}
impl Seek for CrashStorage {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.storage.seek(p)
    }
}
impl Write for CrashStorage {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if b.len() >= eylema_journal::FRAME_HEADER_BYTES
            && let Ok(codec::Value::List(v)) =
                codec::decode(&b[eylema_journal::FRAME_HEADER_BYTES..])
            && let Some(codec::Value::Text(tag)) = v.first()
        {
            self.last_tag = tag.clone();
            let phase = self.phase.load(Ordering::Relaxed);
            if (phase == 1 && tag == "intent") || (phase == 3 && tag == "commit") {
                std::process::exit(86)
            }
        }
        self.storage.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.storage.flush()
    }
}
impl Storage for CrashStorage {
    fn byte_len(&mut self) -> io::Result<u64> {
        self.storage.byte_len()
    }
    fn synchronize(&mut self) -> io::Result<()> {
        self.storage.synchronize()?;
        let phase = self.phase.load(Ordering::Relaxed);
        if (phase == 2 && self.last_tag == "intent") || (phase == 4 && self.last_tag == "commit") {
            std::process::exit(86)
        }
        Ok(())
    }
}
fn error<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
fn child(dir: &Path, case: &str) -> std::result::Result<(), String> {
    let mut services = NativeServices;
    let now = services.now().map_err(|e| e.as_str().to_string())?;
    let mut draft = support::task("task:native-recovery", 20, 80)
        .fields()
        .clone();
    draft.context.request_id =
        RequestId::new(random_id(IdKind::Request).map_err(error)?).map_err(error)?;
    draft.context.correlation_id =
        CorrelationId::new(random_id(IdKind::Correlation).map_err(error)?).map_err(error)?;
    draft.budget = Budget::new(
        20,
        80,
        1024,
        1,
        Timestamp::new(now.unix_ms().checked_add(60000).ok_or("clock overflow")?).map_err(error)?,
    )
    .map_err(error)?;
    let task = TaskSpec::new(draft).map_err(error)?;
    let proposal = support::proposal(&task, &[2, 3]);
    let phase = Arc::new(AtomicU8::new(0));
    let storage = CrashStorage {
        storage: NativeStorage::create_new(&dir.join("journal.eylj"), full_sync).map_err(error)?,
        phase: phase.clone(),
        last_tag: String::new(),
    };
    let journal = Journal::create_new(storage, Hash, JournalLimits::default()).map_err(error)?;
    let (mut host, owner) =
        DurableHost::create_new(journal, services, Limits::default()).map_err(error)?;
    let session = host.enroll(&owner, support::grant()).map_err(error)?;
    host.create_task(&session, task.clone()).map_err(error)?;
    host.admit(&session, task.clone(), proposal)
        .map_err(error)?;
    host.step().map_err(error)?;
    assert_eq!(
        host.task(&session, &task.fields().task_id)
            .map_err(error)?
            .value,
        2
    );
    phase.store(
        match case {
            "before-intent" => 1,
            "after-intent" => 2,
            "after-effect" => 3,
            "after-commit" => 4,
            _ => return Err("unknown crash phase".into()),
        },
        Ordering::Relaxed,
    );
    host.step().map_err(error)?;
    Err("crash hook was not reached".into())
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let id = random_id(IdKind::Request).map_err(io::Error::other)?;
        let path = std::env::temp_dir().join(format!("eylema-recovery-{id}"));
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn parent() -> std::result::Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    let root = Directory::new().map_err(error)?;
    let executable = std::env::current_exe().map_err(error)?;
    for case in [
        "before-intent",
        "after-intent",
        "after-effect",
        "after-commit",
    ] {
        let directory = root.0.join(case);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(error)?;
        let child = std::process::Command::new(&executable)
            .arg("--child")
            .arg(&directory)
            .arg(case)
            .output()
            .map_err(error)?;
        if child.status.code() != Some(86) {
            return Err(format!(
                "scoped child did not reach {case}: exit={:?}, diagnostic={}",
                child.status.code(),
                String::from_utf8_lossy(&child.stderr)
            ));
        }
        let path = directory.join("journal.eylj");
        let journal = Journal::open(
            NativeStorage::open_existing(&path, full_sync).map_err(error)?,
            Hash,
            JournalLimits::default(),
        )
        .map_err(error)?;
        let (mut host, owner) = DurableHost::open(journal, NativeServices).map_err(error)?;
        let session = host.enroll(&owner, support::grant()).map_err(error)?;
        let task_id = TaskId::new("task:native-recovery").map_err(error)?;
        let snapshot = host.task(&session, &task_id).map_err(error)?;
        let events = host.events(&session, &task_id, None, 256).map_err(error)?;
        let id = events
            .iter()
            .find(|e| e.fields().kind == EventKind::ExecutionAccepted)
            .and_then(|e| e.fields().execution_id.clone())
            .ok_or("missing retained execution identity")?;
        let execution = host.execution(&session, &id).map_err(error)?;
        assert_eq!(snapshot.value, if case == "after-commit" { 5 } else { 2 });
        assert_eq!(
            execution.fields().state,
            if case == "after-commit" {
                ExecutionState::Succeeded
            } else {
                ExecutionState::OutcomeUnknown
            }
        );
        assert_eq!(
            execution
                .fields()
                .effects
                .iter()
                .filter(|e| e.fields().outcome == EffectOutcome::Applied)
                .count(),
            if case == "after-commit" { 2 } else { 1 }
        );
        assert_eq!(
            execution
                .fields()
                .effects
                .iter()
                .filter(|e| e.fields().outcome == EffectOutcome::Unknown)
                .count(),
            if case == "after-intent" || case == "after-effect" {
                1
            } else {
                0
            }
        );
        assert_eq!(
            host.recovery_report()
                .map_err(error)?
                .abandoned_attempt
                .is_some(),
            case == "after-intent" || case == "after-effect"
        );
        let mut original = snapshot.task.fields().clone();
        original.revision = Revision::new(2).map_err(error)?;
        let original = TaskSpec::new(original).map_err(error)?;
        let proposal = support::proposal(&original, &[2, 3]);
        assert_eq!(host.admit(&session, original, proposal).map_err(error)?, id);
        assert_eq!(host.step().map_err(error)?, None);
        for event in &events {
            for evidence in &event.fields().evidence {
                host.observation(&session, evidence).map_err(error)?;
            }
        }
        assert_eq!(host.execution(&session, &id).map_err(error)?, execution);
        assert_eq!(
            host.events(&session, &task_id, None, 256).map_err(error)?,
            events
        );
        host.close();
        let journal = Journal::open(
            NativeStorage::open_existing(&path, full_sync).map_err(error)?,
            Hash,
            JournalLimits::default(),
        )
        .map_err(error)?;
        let (mut host, owner) = DurableHost::open(journal, NativeServices).map_err(error)?;
        let fresh = host.enroll(&owner, support::grant()).map_err(error)?;
        assert_eq!(host.execution(&fresh, &id).map_err(error)?, execution);
        assert_eq!(
            host.events(&fresh, &task_id, None, 256).map_err(error)?,
            events
        );
        host.close();
        println!(
            "Native restart proof passed: phase={case}, child_exit=86, value={}, execution={}, retained_events={}, replay_same_identity=true, confirmed_effects_not_repeated=true, second_restart_exact=true",
            snapshot.value,
            id.as_str(),
            events.len()
        );
    }
    println!(
        "Actual child processes/macOS locked files/file+parent+F_FULLFSYNC/CommonCrypto/Security/SystemTime. Owned memory reconstruction only; enrollment fixture is trusted host, no person-login proof. Physical power-loss/OS-crash/external exactly-once/browser/GPU/model/training/performance not_run."
    );
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 4 && args[1] == "--child" {
        child(Path::new(&args[2]), &args[3])
    } else if args.len() == 1 {
        parent()
    } else {
        Err("invalid proof invocation".into())
    };
    if let Err(e) = result {
        eprintln!("recovery-proof: {e}");
        std::process::exit(1)
    }
}
