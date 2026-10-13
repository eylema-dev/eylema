mod support {
    use eylema_contracts::{codec, *};
    pub fn context() -> RequestContext {
        RequestContext {
            request_id: RequestId::new("request:1").unwrap(),
            correlation_id: CorrelationId::new("trace:1").unwrap(),
            contract_version: ContractVersion::CURRENT,
        }
    }
    pub fn rev(n: u64) -> Revision {
        Revision::new(n).unwrap()
    }
    pub fn scope() -> ResourceScope {
        ResourceScope::new(vec![ResourceId::new("state:1").unwrap()]).unwrap()
    }
    pub fn task(name: &str, steps: u64, events: u64) -> TaskSpec {
        TaskSpec::new(TaskDraft {
            context: context(),
            task_id: TaskId::new(name).unwrap(),
            revision: rev(2),
            objective: "Advance bounded owned task state".into(),
            resource_scope: scope(),
            budget: Budget::new(steps, events, 1024, 1, Timestamp::new(2000).unwrap()).unwrap(),
            target: ExecutionTarget::new(
                ExecutionContext::NativeMacos,
                Backend::NativeCpu,
                GpuApi::None,
            )
            .unwrap(),
        })
        .unwrap()
    }
    pub fn proposal(task: &TaskSpec, deltas: &[u64]) -> ActionProposal {
        let actions = deltas
            .iter()
            .enumerate()
            .map(|(i, n)| {
                ActionSpec::new(ActionDraft {
                    context: context(),
                    action_id: ActionId::new(format!("action:{i}")).unwrap(),
                    task_id: task.fields().task_id.clone(),
                    operation_id: OperationId::new("addTaskValue").unwrap(),
                    operation_version: ContractVersion::CURRENT,
                    side_effect_class: SideEffectClass::RecordMutation,
                    resource_scope: scope(),
                    expected_revision: Some(task.fields().revision),
                    authority: AuthorityRequirement::new(
                        PrincipalRef::new("principal:local").unwrap(),
                        rev(3),
                        scope(),
                        vec![Permission::ExecuteAction],
                        OversightRequirements::new(vec![], true, false, None).unwrap(),
                    )
                    .unwrap(),
                    required_capabilities: vec![CapabilityId::new("task:state").unwrap()],
                    budget: task.fields().budget,
                    target: task.fields().target,
                    idempotency_key: IdempotencyKey::new(format!(
                        "{}:step:{i}",
                        task.fields().task_id.as_str()
                    ))
                    .unwrap(),
                    payload: codec::encode(&codec::Value::Unsigned(*n)).unwrap(),
                })
                .unwrap()
            })
            .collect();
        ActionProposal::new(
            ProposalDraft {
                context: context(),
                proposal_id: ProposalId::new(format!(
                    "{}:proposal:1",
                    task.fields().task_id.as_str()
                ))
                .unwrap(),
                task_id: task.fields().task_id.clone(),
                plan_revision: rev(1),
                state_revision: task.fields().revision,
                policy_revision: rev(3),
                actions,
            },
            task,
        )
        .unwrap()
    }
}
use eylema_contracts::{codec::Value, platform::SemanticProfileId, tool::*, *};
fn id(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn binding(action: &ActionSpec) -> ToolBinding {
    ToolBinding {
        action_id: action.fields().action_id.clone(),
        profile: id("tool:add"),
        revision: Revision::new(1).unwrap(),
        input_schema: id("eyl:u64:v1"),
        output_schema: id("eyl:task-value:v1"),
        source: None,
    }
}
#[test]
fn typed_bindings_are_closed_and_never_mint_authority() {
    let t = support::task("task:tool", 20, 50);
    let p = support::proposal(&t, &[2]);
    let b = binding(&p.fields().actions[0]);
    let v = b.to_value();
    assert_eq!(ToolBinding::from_value(&v).unwrap(), b);
    let Value::List(mut fields) = v else { panic!() };
    fields.push(Value::Text("grant:execute".into()));
    assert!(ToolBinding::from_value(&Value::List(fields)).is_err());
    let mut v = b.to_value();
    if let Value::List(ref mut f) = v {
        f[1] = Value::Unsigned(99);
    }
    assert!(ToolBinding::from_value(&v).is_err());
    assert!(ToolPlan::new(t, p, vec![b]).is_ok());
}
#[test]
fn plan_bounds_order_missing_duplicate_and_foreign_action_fail() {
    let t = support::task("task:bounds", 30, 100);
    let p = support::proposal(&t, &[1, 2]);
    let b: Vec<_> = p.fields().actions.iter().map(binding).collect();
    assert!(ToolPlan::new(t.clone(), p.clone(), vec![]).is_err());
    assert!(ToolPlan::new(t.clone(), p.clone(), vec![b[0].clone(), b[0].clone()]).is_err());
    assert!(ToolPlan::new(t.clone(), p.clone(), vec![b[1].clone(), b[0].clone()]).is_err());
    assert!(ToolPlan::new(t.clone(), p, b).is_ok());
    let p = support::proposal(&t, &[1; 9]);
    let b = p.fields().actions.iter().map(binding).collect();
    assert!(ToolPlan::new(t, p, b).is_err());
}
#[test]
fn handler_semantics_and_source_shape_are_explicit() {
    let p = ToolProfile::new(
        id("tool:add"),
        Revision::new(1).unwrap(),
        OwnedTool::AddTaskValue,
    );
    assert_eq!(p.input_schema().as_str(), "eyl:u64:v1");
    assert_eq!(p.output_schema().as_str(), "eyl:task-value:v1");
    assert_eq!(p.operation().effect(), SideEffectClass::RecordMutation);
    assert_eq!(OwnedTool::ReadTaskValue.input_schema(), "eyl:empty:v1");
    let t = support::task("task:source", 20, 50);
    let p = support::proposal(&t, &[3]);
    let mut b = binding(&p.fields().actions[0]);
    b.source = Some(eylema_contracts::project_data::DataRef {
        project: eylema_contracts::platform::ProjectId::new("project:a").unwrap(),
        id: eylema_contracts::project_data::DataId::new("input").unwrap(),
    });
    assert_eq!(ToolBinding::from_value(&b.to_value()).unwrap(), b);
    assert!(format!("{:?}", ToolPlan::new(t, p, vec![b]).unwrap()).contains("actions: 1"));
}
