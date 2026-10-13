mod support;
use eylema_contracts::{platform::SemanticProfileId, tool::*, *};
use eylema_runtime::tool::*;
fn id(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn profile() -> ToolProfile {
    ToolProfile::new(id("tool:add"), support::rev(1), OwnedTool::AddTaskValue)
}
fn plan() -> ToolPlan {
    let t = support::task("task:registry", 20, 50);
    let p = support::proposal(&t, &[2]);
    let a = p.fields().actions[0].fields();
    let b = ToolBinding {
        action_id: a.action_id.clone(),
        profile: id("tool:add"),
        revision: support::rev(1),
        input_schema: id("eyl:u64:v1"),
        output_schema: id("eyl:task-value:v1"),
        source: None,
    };
    ToolPlan::new(t, p, vec![b]).unwrap()
}
#[test]
fn registry_is_bounded_immutable_and_rejects_stale_or_missing_profiles() {
    assert!(ToolRegistry::new(vec![]).is_err());
    assert!(ToolRegistry::new(vec![profile(), profile()]).is_err());
    let profiles = (0..65)
        .map(|n| {
            ToolProfile::new(
                id(&format!("tool:{n}")),
                support::rev(1),
                OwnedTool::AddTaskValue,
            )
        })
        .collect();
    assert!(ToolRegistry::new(profiles).is_err());
    let profiles = (0..64)
        .map(|n| {
            ToolProfile::new(
                id(&format!("tool:{n}")),
                support::rev(1),
                OwnedTool::AddTaskValue,
            )
        })
        .collect();
    assert!(ToolRegistry::new(profiles).is_ok());
    let r = ToolRegistry::new(vec![profile()]).unwrap();
    let p = plan();
    assert_eq!(r.validate(&p), Ok(()));
    let mut b = p.bindings().to_vec();
    b[0].revision = support::rev(2);
    let q = ToolPlan::new(p.task().clone(), p.proposal().clone(), b).unwrap();
    assert_eq!(r.validate(&q), Err(ErrorCode::RevisionConflict));
    let r = ToolRegistry::new(vec![ToolProfile::new(
        id("tool:other"),
        support::rev(1),
        OwnedTool::AddTaskValue,
    )])
    .unwrap();
    assert_eq!(r.validate(&p), Err(ErrorCode::CapabilityUnavailable));
}
#[test]
fn schema_effect_operation_payload_and_target_mismatches_fail_closed() {
    let r = ToolRegistry::new(vec![profile()]).unwrap();
    let p = plan();
    let mut b = p.bindings().to_vec();
    b[0].input_schema = id("eyl:text:v1");
    assert_eq!(
        r.validate(&ToolPlan::new(p.task().clone(), p.proposal().clone(), b).unwrap()),
        Err(ErrorCode::InvalidRequest)
    );
    for n in 0..5 {
        let mut a = p.proposal().fields().actions[0].fields().clone();
        match n {
            0 => a.operation_id = OperationId::new("arbitraryProcess").unwrap(),
            1 => a.side_effect_class = SideEffectClass::Compute,
            2 => a.payload = vec![0; MAX_TOOL_INPUT_BYTES + 1],
            3 => a.required_capabilities = vec![],
            _ => {
                a.payload =
                    eylema_contracts::codec::encode(&eylema_contracts::codec::Value::Signed(2))
                        .unwrap()
            }
        }
        let mut d = p.proposal().fields().clone();
        d.actions = vec![ActionSpec::new(a).unwrap()];
        let q = ToolPlan::new(
            p.task().clone(),
            ActionProposal::new(d, p.task()).unwrap(),
            p.bindings().to_vec(),
        )
        .unwrap();
        assert!(r.validate(&q).is_err());
    }
}

#[test]
fn gpu_browser_targets_and_read_source_payloads_have_no_silent_handler() {
    let p = plan();
    let r = ToolRegistry::new(vec![profile()]).unwrap();
    for target in [
        ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeGpu,
            GpuApi::Metal,
        )
        .unwrap(),
        ExecutionTarget::new(ExecutionContext::Browser, Backend::WasmCpu, GpuApi::None).unwrap(),
    ] {
        let mut t = p.task().fields().clone();
        t.target = target;
        let t = TaskSpec::new(t).unwrap();
        let mut d = p.proposal().fields().clone();
        let mut a = d.actions[0].fields().clone();
        a.target = target;
        d.actions[0] = ActionSpec::new(a).unwrap();
        let q = ToolPlan::new(
            t.clone(),
            ActionProposal::new(d, &t).unwrap(),
            p.bindings().to_vec(),
        )
        .unwrap();
        assert_eq!(r.validate(&q), Err(ErrorCode::CapabilityUnavailable));
    }
    let r = ToolRegistry::new(vec![ToolProfile::new(
        id("tool:add"),
        support::rev(1),
        OwnedTool::ReadTaskValue,
    )])
    .unwrap();
    assert!(r.validate(&p).is_err());
    let mut d = p.proposal().fields().clone();
    let mut a = d.actions[0].fields().clone();
    a.operation_id = OperationId::new("readTaskValue").unwrap();
    a.side_effect_class = SideEffectClass::Read;
    a.payload.clear();
    a.expected_revision = None;
    d.actions = vec![ActionSpec::new(a).unwrap()];
    let mut b = p.bindings().to_vec();
    b[0].input_schema = id("eyl:empty:v1");
    b[0].source = Some(eylema_contracts::project_data::DataRef {
        project: eylema_contracts::platform::ProjectId::new("project:a").unwrap(),
        id: eylema_contracts::project_data::DataId::new("input").unwrap(),
    });
    let read = ToolPlan::new(
        p.task().clone(),
        ActionProposal::new(d, p.task()).unwrap(),
        b,
    )
    .unwrap();
    assert_eq!(r.validate(&read), Err(ErrorCode::InvalidRequest));
}
