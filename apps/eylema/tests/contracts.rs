//! The native client consumes the same portable version/validation semantics.
use eylema_contracts::*;

#[test]
fn native_client_constructs_shared_task_and_rejects_same_invalid_contracts() {
    let draft = TaskDraft {
        context: RequestContext {
            request_id: RequestId::new("native:request").unwrap(),
            correlation_id: CorrelationId::new("native:trace").unwrap(),
            contract_version: ContractVersion::new(0, 1, 0).unwrap(),
        },
        task_id: TaskId::new("native:task").unwrap(),
        revision: Revision::new(1).unwrap(),
        objective: "İncele / inspect".into(),
        resource_scope: ResourceScope::new(vec![ResourceId::new("state:task").unwrap()]).unwrap(),
        budget: Budget::new(1, 2, 128, 1, Timestamp::new(1000).unwrap()).unwrap(),
        target: ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeCpu,
            GpuApi::None,
        )
        .unwrap(),
    };
    let task = TaskSpec::new(draft.clone()).unwrap();
    assert_eq!(
        task.fields().context.contract_version.components(),
        (0, 1, 0)
    );
    let mut invalid = draft;
    invalid.objective.clear();
    assert_eq!(
        TaskSpec::new(invalid).unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert!(ContractVersion::new(0, 2, 0).is_err());
    assert!(
        ExecutionTarget::new(ExecutionContext::Browser, Backend::NativeGpu, GpuApi::Metal).is_err()
    );
}
