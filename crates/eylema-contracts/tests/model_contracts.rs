use eylema_contracts::{
    Revision,
    codec::{self, CodecError, Value},
    model_contract::*,
    platform::*,
};
fn sem(s: &str) -> SemanticProfileId {
    SemanticProfileId::new(s).unwrap()
}
fn role(kind: RoleKind) -> RoleInterface {
    RoleInterface {
        role: kind,
        version: Revision::new(1).unwrap(),
        input_schema: sem("text.messages.v1"),
        output_schema: sem("text.proposal.v1"),
        input_units: sem("utf8.v1"),
        output_units: sem("utf8.v1"),
        preprocessing: sem("qwen3.template.v1"),
        action_semantics: sem("proposal-only.v1"),
        state_reset: sem("reset-between-steps.v1"),
    }
}
fn draft() -> ModelManifestDraft {
    let components = [
        ("config.json", 726, DigestSource::AuditMeasuredSmallBytes),
        ("LICENSE", 11343, DigestSource::AuditMeasuredSmallBytes),
        (
            "model.safetensors",
            1503300328,
            DigestSource::PublisherAdvertised,
        ),
        (
            "tokenizer.json",
            11422654,
            DigestSource::PublisherAdvertised,
        ),
        (
            "tokenizer_config.json",
            9732,
            DigestSource::AuditMeasuredSmallBytes,
        ),
        ("operators.json", 512, DigestSource::AuditMeasuredSmallBytes),
        ("roles.json", 768, DigestSource::AuditMeasuredSmallBytes),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, size, digest_source))| ArtifactComponent {
        name: name.into(),
        size,
        sha256: [(i + 1) as u8; 32],
        digest_source,
    })
    .collect();
    ModelManifestDraft {
        id: ModelId::new("qwen3-0.6b").unwrap(),
        revision: Revision::new(1).unwrap(),
        source_repository: "https://huggingface.co/Qwen/Qwen3-0.6B".into(),
        source_commit: [1; 20],
        architecture: "qwen3-dense.v1".into(),
        artifact_format: "safetensors.v1".into(),
        numeric_format: "bfloat16".into(),
        quantization: "none".into(),
        components,
        operators: [
            "qwen3.embedding_gather.v1",
            "qwen3.linear_no_bias.v1",
            "qwen3.rmsnorm_fp32.v1",
            "qwen3.silu.v1",
            "qwen3.swiglu.v1",
            "qwen3.residual_add.v1",
            "qwen3.rope_default.v1",
            "qwen3.reshape_transpose.v1",
            "qwen3.grouped_kv_repeat.v1",
            "qwen3.causal_mask.v1",
            "qwen3.scaled_attention.v1",
            "qwen3.softmax_fp32.v1",
            "qwen3.kv_cache_append.v1",
            "qwen3.tied_logits.v1",
            "qwen3.greedy_bounded.v1",
            "qwen3.dtype_and_layout.v1",
        ]
        .into_iter()
        .map(|id| OperatorRequirement {
            id: sem(id),
            spec_sha256: [6; 32],
        })
        .collect(),
        rights: RightsRecord {
            publisher: "Qwen".into(),
            source_repository: "https://huggingface.co/Qwen/Qwen3-0.6B".into(),
            source_commit: [1; 20],
            license_id: "Apache-2.0".into(),
            license_component: "LICENSE".into(),
            status: RightsStatus::PublisherTermsPinned,
        },
        configuration: ConfigurationRecord {
            config_component: "config.json".into(),
            weights_component: "model.safetensors".into(),
            tokenizer_component: Some("tokenizer.json".into()),
            tokenizer_config_component: Some("tokenizer_config.json".into()),
            template_sha256: Some([8; 32]),
            preprocessing_component: "roles.json".into(),
            operators_component: "operators.json".into(),
            dimensions: vec![
                ("hidden_size".into(), 1024),
                ("intermediate_size".into(), 3072),
                ("layers".into(), 28),
                ("attention_heads".into(), 16),
                ("kv_heads".into(), 8),
                ("head_dim".into(), 128),
                ("vocab_size".into(), 151936),
                ("context_capacity".into(), 40960),
            ],
            batch_size: 1,
            context_limit: 4096,
            max_new_tokens: 128,
        },
        profiles: vec![role(RoleKind::Planning), role(RoleKind::Reasoning)],
    }
}
fn error_field(d: ModelManifestDraft) -> &'static str {
    ModelManifest::new(d).unwrap_err().field
}
fn wire_mutation(f: impl FnOnce(&mut Vec<(String, Value)>)) -> Vec<u8> {
    let b = ModelManifest::new(draft()).unwrap().to_bytes().unwrap();
    let Value::Map(mut m) = codec::decode(&b).unwrap() else {
        panic!()
    };
    f(&mut m);
    codec::encode(&Value::Map(m)).unwrap()
}
#[test]
fn declared_qwen_profile_roundtrip_is_not_execution() {
    let m = ModelManifest::new(draft()).unwrap();
    let b = m.to_bytes().unwrap();
    let n = ModelManifest::from_bytes(&b).unwrap();
    assert_eq!(n.fields().configuration.context_limit, 4096);
    assert_eq!(n.to_bytes().unwrap(), b);
    assert_eq!(
        check_assignment(
            &n,
            &role(RoleKind::Planning),
            DeploymentKind::OwnedRustCpu,
            LoopMode::LocalOnly
        )
        .unwrap(),
        AssignmentStatus::UnsupportedLocal(UnsupportedReason::ModelExecutionNotImplemented)
    );
}
#[test]
fn exact_role_and_semantics_required() {
    let m = ModelManifest::new(draft()).unwrap();
    for r in [
        role(RoleKind::Decision),
        RoleInterface {
            input_units: sem("seconds.v1"),
            ..role(RoleKind::Planning)
        },
        RoleInterface {
            state_reset: sem("carry-state.v1"),
            ..role(RoleKind::Planning)
        },
        RoleInterface {
            preprocessing: sem("foreign.template.v1"),
            ..role(RoleKind::Planning)
        },
    ] {
        assert_eq!(
            check_assignment(&m, &r, DeploymentKind::OwnedRustCpu, LoopMode::LocalOnly)
                .unwrap_err()
                .field,
            "profile"
        );
    }
}
#[test]
fn publisher_digest_is_not_measured_weights() {
    let m = ModelManifest::new(draft()).unwrap();
    assert_eq!(
        m.fields()
            .components
            .iter()
            .find(|c| c.name == "model.safetensors")
            .unwrap()
            .digest_source,
        DigestSource::PublisherAdvertised
    );
    let n = ModelManifest::from_bytes(&m.to_bytes().unwrap()).unwrap();
    assert_eq!(
        n.fields().components[2].digest_source,
        DigestSource::PublisherAdvertised
    );
    assert!(matches!(
        check_assignment(
            &n,
            &role(RoleKind::Planning),
            DeploymentKind::OwnedRustMetal,
            LoopMode::LocalOnly
        )
        .unwrap(),
        AssignmentStatus::UnsupportedLocal(_)
    ));
}
#[test]
fn unknown_authority_and_support_fields_fail() {
    for key in ["accepted", "can_execute", "verified_weights", "permissions"] {
        let b = wire_mutation(|m| m.push((key.into(), Value::Bool(true))));
        assert_eq!(ModelManifest::from_bytes(&b), Err(CodecError::InvalidData));
    }
}
#[test]
fn dangling_or_duplicate_components_fail() {
    let mut d = draft();
    d.components.push(d.components[0].clone());
    assert_eq!(error_field(d), "components");
    let mut d = draft();
    d.configuration.weights_component = "missing.safetensors".into();
    assert_eq!(error_field(d), "configuration");
    let mut d = draft();
    d.operators.push(d.operators[0].clone());
    assert_eq!(error_field(d), "operators");
    let mut d = draft();
    d.profiles.push(d.profiles[0].clone());
    assert_eq!(error_field(d), "profiles");
}
#[test]
fn zero_identity_or_unresolved_rights_fail() {
    let mut d = draft();
    d.source_commit = [0; 20];
    assert_eq!(error_field(d), "source_commit");
    let mut d = draft();
    d.components[0].sha256 = [0; 32];
    assert_eq!(error_field(d), "components");
    let mut d = draft();
    d.operators[0].spec_sha256 = [0; 32];
    assert_eq!(error_field(d), "operators");
    let mut d = draft();
    d.rights.status = RightsStatus::Unresolved;
    assert_eq!(error_field(d), "rights");
    let mut d = draft();
    d.rights.source_commit = [2; 20];
    assert_eq!(error_field(d), "rights");
}
#[test]
fn flat_name_revision_bounds_and_size_overflow_fail() {
    for name in ["../weights", "/weights", "weights\\file", ".", "a:b", " a"] {
        let mut d = draft();
        d.components[0].name = name.into();
        assert_eq!(error_field(d), "components");
    }
    let mut d = draft();
    d.components[0].size = u64::MAX;
    assert_eq!(error_field(d), "components");
    let mut d = draft();
    d.source_repository = "https://huggingface.co/Qwen/Qwen3-0.6B?revision=main".into();
    assert_eq!(error_field(d), "source_repository");
    let mut d = draft();
    d.components = vec![d.components[0].clone(); 65];
    assert_eq!(error_field(d), "components");
}
#[test]
fn oversized_noncanonical_trailing_wire_fail() {
    assert_eq!(
        ModelManifest::from_bytes(&vec![0; 32769]),
        Err(CodecError::TooLarge)
    );
    let mut b = ModelManifest::new(draft()).unwrap().to_bytes().unwrap();
    b.push(0);
    assert_eq!(ModelManifest::from_bytes(&b), Err(CodecError::NonCanonical));
    let b = wire_mutation(|m| {
        m.iter_mut().find(|(k, _)| k == "revision").unwrap().1 = Value::Unsigned(0)
    });
    assert!(matches!(
        ModelManifest::from_bytes(&b),
        Err(CodecError::InvalidContract(_))
    ));
}
#[test]
fn qwen_attention_projection_need_not_equal_hidden() {
    let m = ModelManifest::new(draft()).unwrap();
    assert_eq!(m.attention_projection_width().unwrap(), 2048);
    assert_eq!(m.fields().configuration.dimensions[0].1, 1024);
    let mut d = draft();
    d.configuration
        .dimensions
        .iter_mut()
        .find(|(k, _)| k == "attention_heads")
        .unwrap()
        .1 = 15;
    assert_eq!(error_field(d), "configuration");
    let mut d = draft();
    d.configuration
        .dimensions
        .iter_mut()
        .find(|(k, _)| k == "head_dim")
        .unwrap()
        .1 = u64::MAX;
    assert_eq!(error_field(d), "configuration");
    let mut d = draft();
    d.configuration.context_limit = 40961;
    assert_eq!(error_field(d), "configuration");
}
#[test]
fn local_unsupported_never_external_fallback() {
    let m = ModelManifest::new(draft()).unwrap();
    assert_eq!(
        check_assignment(
            &m,
            &role(RoleKind::Planning),
            DeploymentKind::ExternalConfigured,
            LoopMode::LocalOnly
        )
        .unwrap_err()
        .field,
        "deployment"
    );
    assert_eq!(
        check_assignment(
            &m,
            &role(RoleKind::Planning),
            DeploymentKind::ExternalConfigured,
            LoopMode::ExplicitExternal
        )
        .unwrap(),
        AssignmentStatus::ExternalConfigurationRequired
    );
    for (key, want) in [
        ("architecture", UnsupportedReason::Architecture),
        ("format", UnsupportedReason::Format),
        ("precision", UnsupportedReason::Precision),
    ] {
        let mut d = draft();
        match key {
            "architecture" => d.architecture = "foreign.v1".into(),
            "format" => d.artifact_format = "gguf.v3".into(),
            _ => d.numeric_format = "float8".into(),
        };
        let n = ModelManifest::new(d).unwrap();
        assert_eq!(
            check_assignment(
                &n,
                &role(RoleKind::Planning),
                DeploymentKind::OwnedRustCpu,
                LoopMode::ExplicitExternal
            )
            .unwrap(),
            AssignmentStatus::UnsupportedLocal(want)
        );
    }
}
#[test]
fn actual_catalog_fixture_decodes_to_declared_identity_without_execution() {
    let m = ModelManifest::from_bytes(include_bytes!("fixtures/qwen3-profile.v1.eyl")).unwrap();
    assert_eq!(m.fields().id.as_str(), "qwen3-0.6b");
    assert_eq!(m.fields().configuration.context_limit, 4096);
    assert_eq!(m.attention_projection_width(), Some(2048));
    assert_eq!(
        m.fields()
            .profiles
            .iter()
            .map(|p| p.role)
            .collect::<Vec<_>>(),
        vec![
            RoleKind::Reasoning,
            RoleKind::Planning,
            RoleKind::Specialist
        ]
    );
    let weight = m
        .fields()
        .components
        .iter()
        .find(|c| c.name == "model.safetensors")
        .unwrap();
    assert_eq!(weight.size, 1503300328);
    assert_eq!(weight.digest_source, DigestSource::PublisherAdvertised);
    let expected = "f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b";
    let actual = weight
        .sha256
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
    for p in &m.fields().profiles {
        for deployment in [DeploymentKind::OwnedRustCpu, DeploymentKind::OwnedRustMetal] {
            assert_eq!(
                check_assignment(&m, p, deployment, LoopMode::LocalOnly).unwrap(),
                AssignmentStatus::UnsupportedLocal(UnsupportedReason::ModelExecutionNotImplemented)
            );
        }
    }
    assert_eq!(
        m.to_bytes().unwrap(),
        include_bytes!("fixtures/qwen3-profile.v1.eyl")
    );
}
#[test]
fn qwen_layer_vocabulary_and_cache_extents_cannot_overflow() {
    for name in [
        "layers",
        "vocab_size",
        "hidden_size",
        "intermediate_size",
        "attention_heads",
        "head_dim",
    ] {
        let mut d = draft();
        d.configuration
            .dimensions
            .iter_mut()
            .find(|(k, _)| k == name)
            .unwrap()
            .1 = u64::MAX;
        assert_eq!(error_field(d), "configuration");
    }
}
#[test]
fn generic_workload_multiplication_is_checked_before_profile_classification() {
    let mut d = draft();
    d.architecture = "foreign.v1".into();
    d.configuration.batch_size = u64::MAX;
    d.configuration.context_limit = u64::MAX;
    assert_eq!(error_field(d), "configuration");
}
#[test]
fn unknown_qwen_prefixed_or_missing_operators_report_operator_gap() {
    for id in ["qwen3.not_a_real_operator.v1", "foreign.op.v1"] {
        let mut d = draft();
        d.operators[0].id = sem(id);
        let m = ModelManifest::new(d).unwrap();
        assert_eq!(
            check_assignment(
                &m,
                &role(RoleKind::Planning),
                DeploymentKind::OwnedRustCpu,
                LoopMode::LocalOnly
            )
            .unwrap(),
            AssignmentStatus::UnsupportedLocal(UnsupportedReason::Operators)
        );
    }
    let mut d = draft();
    d.operators.pop();
    let m = ModelManifest::new(d).unwrap();
    assert_eq!(
        check_assignment(
            &m,
            &role(RoleKind::Planning),
            DeploymentKind::OwnedRustCpu,
            LoopMode::LocalOnly
        )
        .unwrap(),
        AssignmentStatus::UnsupportedLocal(UnsupportedReason::Operators)
    );
}
