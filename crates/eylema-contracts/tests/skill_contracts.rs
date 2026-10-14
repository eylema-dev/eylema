use eylema_contracts::{codec::Value, platform::*, project_data::*, skill::*, tool::*, *};
fn sid(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn package() -> SkillDefinition {
    SkillDefinition::new(
        SkillId::new("skill:add").unwrap(),
        Revision::new(1).unwrap(),
        ToolProfile::new(
            sid("tool:add"),
            Revision::new(1).unwrap(),
            OwnedTool::AddTaskValue,
        ),
        sid(OWNED_SKILL_ADAPTER),
        None,
        sid("acceptance:declared"),
    )
    .unwrap()
}
fn call() -> SkillCall {
    SkillCall {
        reference: DataRef {
            project: ProjectId::new("project:a").unwrap(),
            id: DataId::new("package:add").unwrap(),
        },
        skill: SkillId::new("skill:add").unwrap(),
        revision: Revision::new(1).unwrap(),
    }
}
#[test]
fn closed_leaf_artifact_cannot_install_nested_code_or_change_schema() {
    let p = package();
    assert_eq!(SkillDefinition::from_value(&p.to_value()).unwrap(), p);
    let Value::List(mut extra) = p.to_value() else {
        unreachable!()
    };
    extra.push(Value::Text("authority:execute".into()));
    assert!(SkillDefinition::from_value(&Value::List(extra)).is_err());
    let Value::List(mut nested) = p.to_value() else {
        unreachable!()
    };
    nested[6] = Value::Text("nested".into());
    assert!(SkillDefinition::from_value(&Value::List(nested)).is_err());
    let Value::List(mut schema) = p.to_value() else {
        unreachable!()
    };
    schema[7] = Value::Text("eyl:text:v1".into());
    assert!(SkillDefinition::from_value(&Value::List(schema)).is_err());
    assert!(
        ModelDependency::new(
            ModelProfileId::new("model:p").unwrap(),
            Revision::new(1).unwrap(),
            [0; 32],
            sid("adapter:model")
        )
        .is_err()
    );
}
#[test]
fn finite_loop_iteration_product_and_project_scope_are_enforced() {
    let make = |calls, it| {
        SkillLoop::new(
            LoopId::new("loop:one").unwrap(),
            Revision::new(1).unwrap(),
            calls,
            it,
        )
    };
    assert!(make(vec![call()], 8).is_ok());
    assert!(make(vec![call()], 0).is_err());
    assert!(make(vec![call()], 9).is_err());
    assert!(make(vec![call(); 2], 5).is_err());
    assert!(make(vec![], 1).is_err());
    let mut foreign = call();
    foreign.reference.project = ProjectId::new("project:b").unwrap();
    assert!(make(vec![call(), foreign], 1).is_err());
}
