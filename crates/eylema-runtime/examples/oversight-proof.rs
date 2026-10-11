//! Actual OS-backed execution with explicitly simulated human-host attestation.
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
    let (mut runtime, host) = checked(Runtime::new(services, Limits::default()))?;
    let session = checked(runtime.enroll(&host, support::grant()))?;
    let mut grant = support::grant();
    grant.principal =
        PrincipalRef::new("principal:simulated-verified-human").map_err(|e| e.to_string())?;
    grant.permissions.extend([
        Permission::RequestCancellation,
        Permission::ApproveAction,
        Permission::GovernPolicy,
    ]);
    // Test-only host attestation. This consumer has no person-login adapter.
    let human = checked(runtime.enroll_verified_human(&host, grant))?;
    for scenario in ["cancel", "revoke", "late"] {
        let id = random_id(IdKind::Task).map_err(|e| e.to_string())?;
        let mut draft = support::task(&id, 10, 30).fields().clone();
        draft.context.request_id =
            RequestId::new(random_id(IdKind::Request).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        draft.context.correlation_id =
            CorrelationId::new(random_id(IdKind::Correlation).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        draft.budget = Budget::new(
            10,
            30,
            1024,
            1,
            Timestamp::new(now.unix_ms().checked_add(60000).ok_or("clock overflow")?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let task = TaskSpec::new(draft).map_err(|e| e.to_string())?;
        let requirements = OversightRequirements::new(
            vec![CheckpointRef::new("checkpoint:native-proof").map_err(|e| e.to_string())?],
            true,
            true,
            None,
        )
        .map_err(|e| e.to_string())?;
        checked(runtime.create_governed_task(&host, &session, task.clone(), requirements))?;
        let proposal = support::proposal(&task, &[2, 3]);
        assert_eq!(
            runtime.admit(&session, task.clone(), proposal.clone()),
            Err(ErrorCode::ApprovalRequired)
        );
        let binding = checked(runtime.preview_approval(
            human.session(),
            &task,
            &proposal,
            Timestamp::new(now.unix_ms() + 30000).map_err(|e| e.to_string())?,
        ))?;
        let approval = checked(runtime.create_approval(&human, &task, &proposal, binding))?;
        let execution = checked(runtime.admit_with_approvals(
            &session,
            task.clone(),
            proposal,
            std::slice::from_ref(&approval),
        ))?;
        checked(runtime.step())?;
        assert_eq!(
            checked(runtime.task(&session, &task.fields().task_id))?.value,
            2
        );
        match scenario {
            "cancel" => {
                let receipt = checked(runtime.request_cancellation(&human, &execution))?;
                assert!(matches!(receipt, CancellationOutcome::Requested(_)));
                let pending = checked(runtime.execution(&session, &execution))?;
                assert_eq!(pending.fields().state, ExecutionState::CancelRequested);
                assert!(pending.fields().effective_stop_evidence.is_none());
                checked(runtime.step())?;
                let result = checked(runtime.execution(&session, &execution))?;
                assert_eq!(result.fields().state, ExecutionState::Cancelled);
                let evidence = result
                    .fields()
                    .effective_stop_evidence
                    .as_ref()
                    .ok_or("missing actual stop observation")?;
                assert_eq!(checked(runtime.observation(&session, evidence))?.after, 2);
            }
            "revoke" => {
                checked(runtime.revoke_approval(&human, &approval))?;
                checked(runtime.step())?;
                let result = checked(runtime.execution(&session, &execution))?;
                assert_eq!(result.fields().state, ExecutionState::Failed);
                assert_eq!(
                    result
                        .fields()
                        .error
                        .as_ref()
                        .ok_or("missing revocation error")?
                        .code(),
                    ErrorCode::Forbidden
                );
            }
            _ => {
                checked(runtime.step())?;
                let result = checked(runtime.execution(&session, &execution))?;
                assert_eq!(result.fields().state, ExecutionState::Succeeded);
                assert_eq!(
                    checked(runtime.request_cancellation(&human, &execution))?,
                    CancellationOutcome::AlreadyTerminal(Box::new(result.clone()))
                );
                assert_eq!(checked(runtime.execution(&session, &execution))?, result);
            }
        }
        let snapshot = checked(runtime.task(&session, &task.fields().task_id))?;
        assert_eq!(snapshot.value, if scenario == "late" { 5 } else { 2 });
        let events = checked(runtime.events(&session, &task.fields().task_id, None, 256))?;
        for event in &events {
            for evidence in &event.fields().evidence {
                checked(runtime.observation(&session, evidence))?;
            }
        }
        println!(
            "Native oversight proof passed: scenario={}, task={}, execution={}, value={}, performed_steps={}, events={}",
            scenario,
            task.fields().task_id.as_str(),
            execution.as_str(),
            snapshot.value,
            snapshot.performed_steps,
            events.len()
        );
    }
    println!(
        "Actual SystemTime/CommonCrypto/Security and owned executor. Human verification is explicitly a simulated trusted-host attestation; real login/person/browser/HTTP/journal/restart/GPU/model/training/performance not_run."
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("oversight-proof: {error}");
        std::process::exit(1)
    }
}
