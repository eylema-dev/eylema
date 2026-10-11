//! Actual native journal consumer; stored contracts are inert, not recovered authority.
#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod support;
use eylema_contracts::*;
use eylema_platform::{IdKind, random_id, sha256};
use eylema_runtime::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct NativeServices;
impl HostServices for NativeServices {
    fn now(&mut self) -> Result<Timestamp> {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrorCode::CapabilityUnavailable)?
            .as_millis();
        Timestamp::new(u64::try_from(ms).map_err(|_| ErrorCode::CapabilityUnavailable)?)
            .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn execution_id(&mut self) -> Result<ExecutionId> {
        ExecutionId::new(
            random_id(IdKind::Execution).map_err(|_| ErrorCode::CapabilityUnavailable)?,
        )
        .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn event_id(&mut self) -> Result<EventId> {
        EventId::new(random_id(IdKind::Event).map_err(|_| ErrorCode::CapabilityUnavailable)?)
            .map_err(|_| ErrorCode::CapabilityUnavailable)
    }
    fn sha256(&mut self, bytes: &[u8]) -> Result<[u8; 32]> {
        sha256(bytes).map_err(|_| ErrorCode::CapabilityUnavailable)
    }
}
fn checked<T>(result: Result<T>) -> std::result::Result<T, String> {
    result.map_err(|code| code.as_str().into())
}
fn run() -> std::result::Result<(), String> {
    let mut services = NativeServices;
    let now = checked(services.now())?;
    let id = random_id(IdKind::Task).map_err(|e| e.to_string())?;
    let mut draft = support::task(&id, 10, 20).fields().clone();
    draft.context.request_id =
        RequestId::new(random_id(IdKind::Request).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    draft.context.correlation_id =
        CorrelationId::new(random_id(IdKind::Correlation).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    draft.budget = Budget::new(
        10,
        20,
        1024,
        1,
        Timestamp::new(now.unix_ms().checked_add(60000).ok_or("clock overflow")?)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let task = TaskSpec::new(draft).map_err(|e| e.to_string())?;
    let mut pd = support::proposal(&task, &[2, 3]).fields().clone();
    pd.context = task.fields().context.clone();
    for action in &mut pd.actions {
        let mut ad = action.fields().clone();
        ad.context = task.fields().context.clone();
        *action = ActionSpec::new(ad).map_err(|e| e.to_string())?;
    }
    let proposal = ActionProposal::new(pd, &task).map_err(|e| e.to_string())?;
    let (mut runtime, host) = checked(Runtime::new(services, Limits::default()))?;
    let session = checked(runtime.enroll(&host, support::grant()))?;
    checked(runtime.create_task(&session, task.clone()))?;
    let execution = checked(runtime.admit(&session, task.clone(), proposal.clone()))?;
    assert_eq!(
        checked(runtime.execution(&session, &execution))?
            .fields()
            .state,
        ExecutionState::Queued
    );
    assert_eq!(
        checked(runtime.admit(&session, task.clone(), proposal.clone()))?,
        execution
    );
    checked(runtime.step())?;
    assert_eq!(
        checked(runtime.task(&session, &task.fields().task_id))?.value,
        2
    );
    checked(runtime.step())?;
    let result = checked(runtime.execution(&session, &execution))?;
    assert_eq!(result.fields().state, ExecutionState::Succeeded);
    let state = checked(runtime.task(&session, &task.fields().task_id))?;
    assert_eq!(state.value, 5);
    assert_eq!(state.performed_steps, 2);
    assert_eq!(
        checked(runtime.admit(&session, task.clone(), proposal.clone()))?,
        execution
    );
    let events = checked(runtime.events(&session, &task.fields().task_id, None, 256))?;
    for event in &events {
        for evidence in &event.fields().evidence {
            checked(runtime.observation(&session, evidence))?;
        }
    }
    let mut expected = vec![
        codec::WireRecord::Task(task.clone()),
        codec::WireRecord::Proposal {
            task: task.clone(),
            proposal: proposal.clone(),
        },
        codec::WireRecord::Execution {
            task: task.clone(),
            proposal: proposal.clone(),
            execution: Box::new(result),
        },
    ];
    expected.extend(events.iter().cloned().map(codec::WireRecord::Event));
    let directory = TestDirectory::new().map_err(|e| e.to_string())?;
    let path = directory.0.join("journal.eylj");
    let storage =
        eylema_journal::NativeStorage::create_new(&path, full_sync).map_err(|e| e.to_string())?;
    let mut journal =
        eylema_journal::Journal::create_new(storage, NativeHash, eylema_journal::Limits::default())
            .map_err(|e| e.to_string())?;
    assert!(
        matches!(eylema_journal::NativeStorage::open_existing(&path, full_sync), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock)
    );
    let mut receipts = vec![];
    for record in &expected {
        let bytes = codec::encode_record(record).map_err(|e| e.to_string())?;
        let value = codec::decode(&bytes).map_err(|e| e.to_string())?;
        receipts.push(journal.append(&value).map_err(|e| e.to_string())?);
    }
    journal.close();
    let storage = eylema_journal::NativeStorage::open_existing(&path, full_sync)
        .map_err(|e| e.to_string())?;
    let mut reopened =
        eylema_journal::Journal::open(storage, NativeHash, eylema_journal::Limits::default())
            .map_err(|e| e.to_string())?;
    let stored = reopened.read_records().map_err(|e| e.to_string())?;
    assert_eq!(stored.len(), expected.len());
    for ((record, expected), receipt) in stored.iter().zip(&expected).zip(&receipts) {
        assert_eq!(record.sequence, receipt.sequence);
        assert_eq!(record.digest, receipt.digest);
        assert_eq!(
            &codec::decode_record(&codec::encode(&record.payload).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?,
            expected
        );
    }
    reopened.close();
    println!(
        "Native journal proof passed: value={}, performed_steps={}, events={}, typed_records={}, full_sync=file+parent+F_FULLFSYNC, lock_conflict=WouldBlock, reopened_exact=true",
        state.value,
        state.performed_steps,
        events.len(),
        stored.len()
    );
    println!(
        "Actual macOS files/CommonCrypto/Security/owned executor. Semantic runtime/authority restart recovery, physical power-loss/OS-crash, browser/GPU/model/training/performance not_run."
    );
    Ok(())
}
struct NativeHash;
impl eylema_journal::Sha256 for NativeHash {
    fn digest(&mut self, bytes: &[u8]) -> eylema_journal::Result<[u8; 32]> {
        sha256(bytes).map_err(|_| eylema_journal::JournalError::HashUnavailable)
    }
}
fn full_sync(file: &std::fs::File) -> std::io::Result<()> {
    eylema_platform::full_sync_file(file).map_err(std::io::Error::other)
}
struct TestDirectory(std::path::PathBuf);
impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let id = random_id(IdKind::Request).map_err(std::io::Error::other)?;
        let path = std::env::temp_dir().join(format!("eylema-journal-{id}"));
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("journal-proof: {error}");
        std::process::exit(1);
    }
}
