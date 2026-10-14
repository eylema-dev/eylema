//! Explicit person acceptance entry point, never invoked by repository/native CI.
use super::*;
use eylema_platform::human::{HumanError, HumanPurpose};
use eylema_runtime::{CancellationOutcome, HostServices};
fn checked<T>(r: eylema_host::Result<T>) -> Result<T> {
    r.map_err(|e| {
        CliError(match e {
            eylema_host::HostError::Runtime(code) => code.as_str(),
            eylema_host::HostError::Journal(_) => "HUMAN_PROOF_STORAGE_FAILED",
            _ => "HUMAN_PROOF_RECORD_FAILED",
        })
    })
}
fn make_id(kind: eylema_platform::IdKind) -> Result<String> {
    eylema_platform::random_id(kind).map_err(|_| CliError("ENTROPY_UNAVAILABLE"))
}
fn grant() -> Result<Grant> {
    let mut g = cli_grant()?;
    g.resource_scope = ResourceScope::new(vec![
        ResourceId::new(HumanPurpose::AcceptanceProof.resource())
            .map_err(|_| CliError("INTERNAL_ERROR"))?,
    ])
    .map_err(|_| CliError("INTERNAL_ERROR"))?;
    Ok(g)
}
fn plan(deadline: Timestamp, name: &str, delta: &[u64]) -> Result<(TaskSpec, ActionProposal)> {
    let revision = Revision::new(1).map_err(|_| CliError("INTERNAL_ERROR"))?;
    let context = RequestContext {
        request_id: RequestId::new(make_id(eylema_platform::IdKind::Request)?)
            .map_err(|_| CliError("INTERNAL_ERROR"))?,
        correlation_id: CorrelationId::new(make_id(eylema_platform::IdKind::Correlation)?)
            .map_err(|_| CliError("INTERNAL_ERROR"))?,
        contract_version: ContractVersion::CURRENT,
    };
    let task_id = TaskId::new(name).map_err(|_| CliError("INTERNAL_ERROR"))?;
    let scope = grant()?.resource_scope;
    let budget = Budget::new(8, 32, 1024, 1, deadline).map_err(|_| CliError("INTERNAL_ERROR"))?;
    let target = ExecutionTarget::new(
        ExecutionContext::NativeMacos,
        Backend::NativeCpu,
        GpuApi::None,
    )
    .map_err(|_| CliError("INTERNAL_ERROR"))?;
    let task = TaskSpec::new(TaskDraft {
        context: context.clone(),
        task_id: task_id.clone(),
        revision,
        objective: "Bounded real-person approval and partial-stop demonstration".into(),
        resource_scope: scope.clone(),
        budget,
        target,
    })
    .map_err(|_| CliError("INTERNAL_ERROR"))?;
    let mut actions = vec![];
    for (i, n) in delta.iter().enumerate() {
        let authority = AuthorityRequirement::new(
            grant()?.principal,
            revision,
            scope.clone(),
            vec![Permission::ExecuteAction],
            OversightRequirements::new(vec![], true, true, None)
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
        )
        .map_err(|_| CliError("INTERNAL_ERROR"))?;
        let a = ActionSpec::new(ActionDraft {
            context: context.clone(),
            action_id: ActionId::new(format!("action:{name}:{i}"))
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
            task_id: task_id.clone(),
            operation_id: OperationId::new("addTaskValue")
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
            operation_version: ContractVersion::CURRENT,
            side_effect_class: SideEffectClass::RecordMutation,
            resource_scope: scope.clone(),
            expected_revision: Some(revision),
            authority,
            required_capabilities: vec![
                CapabilityId::new("task:state").map_err(|_| CliError("INTERNAL_ERROR"))?,
            ],
            budget,
            target,
            idempotency_key: IdempotencyKey::new(format!("{name}:{i}"))
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
            payload: codec::encode(&Value::Unsigned(*n)).map_err(|_| CliError("INTERNAL_ERROR"))?,
        })
        .map_err(|_| CliError("INTERNAL_ERROR"))?;
        actions.push(a);
    }
    let proposal = ActionProposal::new(
        ProposalDraft {
            context,
            proposal_id: ProposalId::new(format!("proposal:{name}"))
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
            task_id,
            plan_revision: revision,
            state_revision: revision,
            policy_revision: revision,
            actions,
        },
        &task,
    )
    .map_err(|_| CliError("INTERNAL_ERROR"))?;
    Ok((task, proposal))
}
pub(super) fn run(path: &Path) -> Result<()> {
    // Refuse existing/symlink paths before any prompt or writes. Never reuse user journals.
    if path
        .try_exists()
        .map_err(|_| CliError("STORAGE_UNAVAILABLE"))?
    {
        return Err(CliError("PROOF_DIRECTORY_EXISTS"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| CliError("STORAGE_UNAVAILABLE"))?;
    }
    #[cfg(not(unix))]
    {
        return Err(CliError("HUMAN_AUTH_UNAVAILABLE"));
    }
    let (mut host, owner) = open_host(path)?;
    let mut human_grant = grant()?;
    human_grant
        .permissions
        .extend([Permission::RequestCancellation, Permission::ApproveAction]);
    let request = checked(host.request_device_owner(
        &owner,
        human_grant,
        HumanPurpose::AcceptanceProof,
        Duration::from_secs(300),
    ))?;
    let proof = request.authenticate(Duration::from_secs(60)).map_err(|e| {
        CliError(match e {
            HumanError::Unavailable => "HUMAN_AUTH_UNAVAILABLE",
            HumanError::Denied => "HUMAN_AUTH_DENIED",
            HumanError::Timeout => "HUMAN_AUTH_TIMEOUT",
            _ => "HUMAN_AUTH_FAILED",
        })
    })?;
    let human = checked(host.enroll_device_owner(&owner, request, proof))?;
    let session = checked(host.enroll(&owner, grant()?))?;
    let now = NativeServices
        .now()
        .map_err(|_| CliError("CLOCK_UNAVAILABLE"))?;
    let deadline = Timestamp::new(
        now.unix_ms()
            .checked_add(60_000)
            .ok_or(CliError("CLOCK_UNAVAILABLE"))?,
    )
    .map_err(|_| CliError("CLOCK_UNAVAILABLE"))?;
    let (task, proposal) = plan(deadline, "task:g73-person", &[2, 3])?;
    let oversight = OversightRequirements::new(
        vec![CheckpointRef::new("checkpoint:g73-person").map_err(|_| CliError("INTERNAL_ERROR"))?],
        true,
        true,
        None,
    )
    .map_err(|_| CliError("INTERNAL_ERROR"))?;
    checked(host.create_governed_task(&owner, &session, task.clone(), oversight))?;
    if host.admit(&session, task.clone(), proposal.clone())
        != Err(eylema_host::HostError::Runtime(ErrorCode::ApprovalRequired))
    {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    let approval_deadline =
        Timestamp::new(now.unix_ms() + 30_000).map_err(|_| CliError("CLOCK_UNAVAILABLE"))?;
    let binding = checked(host.preview_approval(&session, &task, &proposal, approval_deadline))?;
    let approval = checked(host.device_owner_approval(&human, &task, &proposal, binding))?;
    let execution =
        checked(host.admit_with_approvals(&session, task.clone(), proposal, &[approval]))?;
    checked(host.step())?;
    if checked(host.task(&session, &task.fields().task_id))?.value != 2 {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    if !matches!(
        checked(host.device_owner_cancel(&human, &execution))?,
        CancellationOutcome::Requested(_)
    ) {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    checked(host.step())?;
    let result = checked(host.execution(&session, &execution))?;
    if result.fields().state != ExecutionState::Cancelled
        || result.fields().effective_stop_evidence.is_none()
        || checked(host.task(&session, &task.fields().task_id))?.value != 2
    {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    checked(host.revoke_device_owner(&human))?;
    if host.device_owner_cancel(&human, &execution).is_ok() {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    host.close();
    let (mut reopened, new_owner) = open_host(path)?;
    let reader = checked(reopened.enroll(&new_owner, grant()?))?;
    if reopened.device_owner_cancel(&human, &execution).is_ok()
        || checked(reopened.task(&reader, &task.fields().task_id))?.value != 2
        || checked(reopened.execution(&reader, &execution))?
            .fields()
            .state
            != ExecutionState::Cancelled
    {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    let (next, p) = plan(deadline, "task:g73-fresh-required", &[1])?;
    checked(
        reopened.create_governed_task(
            &new_owner,
            &reader,
            next.clone(),
            OversightRequirements::new(vec![], true, true, None)
                .map_err(|_| CliError("INTERNAL_ERROR"))?,
        ),
    )?;
    if reopened.admit(&reader, next, p)
        != Err(eylema_host::HostError::Runtime(
            ErrorCode::CapabilityUnavailable,
        ))
    {
        return Err(CliError("HUMAN_PROOF_FAILED"));
    }
    reopened.close();
    println!(
        "{{\"status\":\"passed\",\"person_authentication\":true,\"unapproved_denied\":true,\"effective_stop\":true,\"completed_value\":2,\"remaining_action_applied\":false,\"revoked_denied\":true,\"reopen_preserved\":true,\"fresh_enrollment_required\":true}}"
    );
    Ok(())
}
