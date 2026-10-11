//! Actual native embedded consumer. No network, persistence or external effects.
#[path = "../tests/support/mod.rs"]
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
        checked(runtime.admit(&session, task.clone(), proposal))?,
        execution
    );
    let events = checked(runtime.events(&session, &task.fields().task_id, None, 256))?;
    for event in &events {
        for evidence in &event.fields().evidence {
            checked(runtime.observation(&session, evidence))?;
        }
    }
    println!(
        "Native executor proof passed: task={}, execution={}, state=succeeded, value={}, steps={}, events={}, identical replay=same execution",
        task.fields().task_id.as_str(),
        execution.as_str(),
        state.value,
        state.performed_steps,
        events.len()
    );
    println!(
        "Services: actual SystemTime / CommonCrypto SHA256 / Security entropy. Embedded owned memory only; browser/journal/restart/cancellation/human approval/GPU/model/training not_run."
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("executor-proof: {error}");
        std::process::exit(1)
    }
}
