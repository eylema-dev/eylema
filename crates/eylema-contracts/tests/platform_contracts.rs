use eylema_contracts::Revision;
use eylema_contracts::platform::*;

fn sem(value: &str) -> SemanticProfileId {
    SemanticProfileId::new(value).unwrap()
}
fn interface(role: RoleKind) -> RoleInterface {
    RoleInterface {
        role,
        version: Revision::new(1).unwrap(),
        input_schema: sem("bounded-payload.v1"),
        output_schema: sem("bounded-payload.v1"),
        input_units: sem("unitless"),
        output_units: sem("unitless"),
        preprocessing: sem("identity.v1"),
        action_semantics: sem("proposal-only.v1"),
        state_reset: sem("step-boundary.v1"),
    }
}
fn profile(id: &str, role: RoleKind, deployment: DeploymentKind) -> ModelProfile {
    ModelProfile::new(ModelProfileDraft {
        id: ModelProfileId::new(id).unwrap(),
        model: ModelId::new("candidate-model").unwrap(),
        artifact: [7; 32],
        artifact_revision: Revision::new(1).unwrap(),
        interface: interface(role),
        deployment,
    })
    .unwrap()
}
fn binding(id: &str, profile: &str, role: RoleKind) -> RoleBinding {
    RoleBinding {
        id: RoleId::new(id).unwrap(),
        expected: interface(role),
        profile: ModelProfileId::new(profile).unwrap(),
    }
}
fn draft(id: &str) -> LoopDraft {
    LoopDraft {
        id: LoopId::new(id).unwrap(),
        revision: Revision::new(1).unwrap(),
        project: ProjectId::new("private-project").unwrap(),
        mode: LoopMode::LocalOnly,
        roles: vec![binding("planner", "p", RoleKind::Planning)],
        edges: vec![],
        max_steps: 64,
        max_iterations: 4,
    }
}
fn registry() -> ModelRegistry {
    ModelRegistry::new(vec![profile(
        "p",
        RoleKind::Planning,
        DeploymentKind::OwnedRustCpu,
    )])
    .unwrap()
}

#[test]
fn placement_and_document_consumers_share_contracts() {
    assert_eq!(
        RoleKind::ALL.map(RoleKind::as_str),
        [
            "perception",
            "reasoning",
            "planning",
            "decision",
            "policy",
            "world_model",
            "specialist",
            "critic",
            "representation"
        ]
    );
    let registry = registry();
    let mut placement = draft("placement");
    placement
        .roles
        .push(binding("replan", "p", RoleKind::Planning));
    placement.edges = vec![
        LoopEdge {
            from: RoleId::new("planner").unwrap(),
            to: RoleId::new("replan").unwrap(),
        },
        LoopEdge {
            from: RoleId::new("replan").unwrap(),
            to: RoleId::new("planner").unwrap(),
        },
    ];
    let placement = LoopDefinition::new(placement, &registry).unwrap();
    let document = LoopDefinition::new(draft("document-review"), &registry).unwrap();
    assert_eq!(placement.fields().id.as_str(), "placement");
    assert_eq!(document.fields().id.as_str(), "document-review");
    assert_eq!(placement.fields().edges.len(), 2);
    assert_eq!(placement.profiles()[0].fields().artifact, [7; 32]);
    assert_eq!(
        document.profiles()[0].fields().model.as_str(),
        "candidate-model"
    );
    for role in RoleKind::ALL {
        let registry =
            ModelRegistry::new(vec![profile("p", role, DeploymentKind::OwnedRustMetal)]).unwrap();
        let mut single = draft("one-role");
        single.roles = vec![binding("single", "p", role)];
        assert!(LoopDefinition::new(single, &registry).is_ok());
    }
}

#[test]
fn incompatible_role_semantics_fail() {
    let registry = registry();
    for field in 0..9 {
        let mut candidate = draft("mismatch");
        let expected = &mut candidate.roles[0].expected;
        match field {
            0 => expected.role = RoleKind::Decision,
            1 => expected.version = Revision::new(2).unwrap(),
            2 => expected.input_schema = sem("other-schema"),
            3 => expected.output_schema = sem("other-schema"),
            4 => expected.input_units = sem("meters"),
            5 => expected.output_units = sem("meters"),
            6 => expected.preprocessing = sem("other-preprocessor"),
            7 => expected.action_semantics = sem("execute-without-approval"),
            8 => expected.state_reset = sem("carry-state"),
            _ => unreachable!(),
        }
        assert!(
            LoopDefinition::new(candidate, &registry).is_err(),
            "semantic field {field}"
        );
    }
}

#[test]
fn duplicate_dangling_and_resource_bounds_fail() {
    let registry = registry();
    assert!(ModelRegistry::new(vec![]).is_err());
    let duplicate = profile("p", RoleKind::Planning, DeploymentKind::OwnedRustCpu);
    assert!(ModelRegistry::new(vec![duplicate.clone(), duplicate]).is_err());
    let profiles: Vec<_> = (0..128)
        .map(|i| {
            profile(
                &format!("p{i}"),
                RoleKind::Planning,
                DeploymentKind::OwnedRustCpu,
            )
        })
        .collect();
    assert!(ModelRegistry::new(profiles.clone()).is_ok());
    let mut too_many = profiles;
    too_many.push(profile(
        "extra",
        RoleKind::Planning,
        DeploymentKind::OwnedRustCpu,
    ));
    assert!(ModelRegistry::new(too_many).is_err());
    let mut zero = profile("p", RoleKind::Planning, DeploymentKind::OwnedRustCpu)
        .fields()
        .clone();
    zero.artifact = [0; 32];
    assert!(ModelProfile::new(zero).is_err());
    assert!(LoopId::new("../escape").is_err());
    assert!(ProjectId::new(" ").is_err());
    assert!(ModelProfileId::new("x".repeat(97)).is_err());
    assert!(SemanticProfileId::new("ölçek").is_err());
    assert!(ModelId::new("").is_err());
    assert!(RoleId::new("bad/id").is_err());

    let mut duplicate_role = draft("duplicate");
    duplicate_role.roles.push(duplicate_role.roles[0].clone());
    assert!(LoopDefinition::new(duplicate_role, &registry).is_err());
    let mut empty = draft("empty");
    empty.roles.clear();
    assert!(LoopDefinition::new(empty, &registry).is_err());
    let mut missing = draft("missing");
    missing.roles[0].profile = ModelProfileId::new("absent").unwrap();
    assert!(LoopDefinition::new(missing, &registry).is_err());
    for (steps, iterations) in [(0, 1), (4097, 1), (1, 0), (1, 1025)] {
        let mut candidate = draft("bad-budget");
        candidate.max_steps = steps;
        candidate.max_iterations = iterations;
        assert!(LoopDefinition::new(candidate, &registry).is_err());
    }
    let mut boundary = draft("boundary");
    boundary.max_steps = 4096;
    boundary.max_iterations = 1024;
    boundary.roles = (0..32)
        .map(|i| binding(&format!("r{i}"), "p", RoleKind::Planning))
        .collect();
    boundary.edges = (0..128)
        .map(|i| LoopEdge {
            from: RoleId::new(format!("r{}", i / 32)).unwrap(),
            to: RoleId::new(format!("r{}", i % 32)).unwrap(),
        })
        .collect();
    assert!(LoopDefinition::new(boundary.clone(), &registry).is_ok());
    let mut excess_roles = boundary.clone();
    excess_roles
        .roles
        .push(binding("r32", "p", RoleKind::Planning));
    assert!(LoopDefinition::new(excess_roles, &registry).is_err());
    let mut excess_edges = boundary.clone();
    excess_edges.edges.push(LoopEdge {
        from: RoleId::new("r4").unwrap(),
        to: RoleId::new("r0").unwrap(),
    });
    assert!(LoopDefinition::new(excess_edges, &registry).is_err());
    let mut repeated = boundary.clone();
    repeated.edges.push(boundary.edges[0].clone());
    repeated.edges.remove(1);
    assert!(LoopDefinition::new(repeated, &registry).is_err());
    for absent_source in [true, false] {
        let mut missing_edge = draft("missing-edge");
        missing_edge.edges.push(LoopEdge {
            from: RoleId::new(if absent_source { "absent" } else { "planner" }).unwrap(),
            to: RoleId::new(if absent_source { "planner" } else { "absent" }).unwrap(),
        });
        assert!(LoopDefinition::new(missing_edge, &registry).is_err());
    }
}

#[test]
fn edge_semantics_and_local_only_fail() {
    for units in [false, true] {
        let source = profile("p", RoleKind::Planning, DeploymentKind::OwnedRustCpu);
        let mut destination = profile("q", RoleKind::Decision, DeploymentKind::OwnedRustCpu)
            .fields()
            .clone();
        if units {
            destination.interface.input_units = sem("meters");
        } else {
            destination.interface.input_schema = sem("different-schema");
        }
        let destination = ModelProfile::new(destination).unwrap();
        let registry = ModelRegistry::new(vec![source, destination.clone()]).unwrap();
        let mut candidate = draft("edge-mismatch");
        candidate.roles.push(RoleBinding {
            id: RoleId::new("decision").unwrap(),
            expected: destination.fields().interface.clone(),
            profile: ModelProfileId::new("q").unwrap(),
        });
        candidate.edges.push(LoopEdge {
            from: RoleId::new("planner").unwrap(),
            to: RoleId::new("decision").unwrap(),
        });
        assert!(LoopDefinition::new(candidate, &registry).is_err());
    }
    let external = ModelRegistry::new(vec![profile(
        "p",
        RoleKind::Planning,
        DeploymentKind::ExternalConfigured,
    )])
    .unwrap();
    assert!(LoopDefinition::new(draft("private-local"), &external).is_err());
    let mut explicit = draft("explicit-external");
    explicit.mode = LoopMode::ExplicitExternal;
    assert!(LoopDefinition::new(explicit, &external).is_ok()); // Metadata, not disclosure permission.
}

#[test]
fn replacement_preserves_existing_identity() {
    let registry = registry();
    let original = draft("immutable");
    let old = LoopDefinition::new(original.clone(), &registry).unwrap();
    let mut replacement = registry
        .resolve(&ModelProfileId::new("p").unwrap())
        .unwrap()
        .fields()
        .clone();
    replacement.artifact = [9; 32];
    replacement.artifact_revision = Revision::new(2).unwrap();
    let replacement = ModelRegistry::new(vec![ModelProfile::new(replacement).unwrap()]).unwrap();
    let mut new_draft = original;
    new_draft.revision = Revision::new(2).unwrap();
    let new = LoopDefinition::new(new_draft.clone(), &replacement).unwrap();
    new_draft.roles.clear();
    assert_eq!(old.profiles()[0].fields().artifact, [7; 32]);
    assert_eq!(old.profiles()[0].fields().artifact_revision.get(), 1);
    assert_eq!(new.profiles()[0].fields().artifact, [9; 32]);
    assert_eq!(new.profiles()[0].fields().artifact_revision.get(), 2);
    assert_eq!(new.fields().roles.len(), 1);
    assert!(
        registry
            .resolve(&ModelProfileId::new("absent").unwrap())
            .is_none()
    );
}
