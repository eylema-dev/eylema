use eylema_contracts::{
    codec::{self, CodecError, Value},
    model_contract::{
        AssignmentStatus, DigestSource, ModelManifest, RightsStatus, UnsupportedReason,
        check_assignment,
    },
    platform::{DeploymentKind, LoopMode, RoleKind, SemanticProfileId},
};

const FIXTURE: &[u8] = include_bytes!("fixtures/qwen3-reranker-profile.v1.eyl");
fn declared() -> ModelManifest {
    ModelManifest::from_bytes(FIXTURE).unwrap()
}

#[test]
fn actual_reranker_fixture_decodes_and_roundtrips_exact_source_and_role() {
    let model = declared();
    assert_eq!(model.to_bytes().unwrap(), FIXTURE);
    assert_eq!(model.fields().id.as_str(), "qwen3-reranker-0.6b");
    let commit = model
        .fields()
        .source_commit
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<String>();
    assert_eq!(commit, "e61197ed45024b0ed8a2d74b80b4d909f1255473");
    assert_eq!(model.fields().profiles.len(), 1);
    let profile = &model.fields().profiles[0];
    assert_eq!(profile.role, RoleKind::Decision);
    assert_eq!(profile.input_schema.as_str(), "decision.query-options.v1");
    assert_eq!(
        profile.output_units.as_str(),
        "uncalibrated-score-micros.v1"
    );
    assert_eq!(profile.state_reset.as_str(), "reset-per-pair.v1");
    assert_eq!(model.fields().configuration.context_limit, 4096);
    assert_eq!(
        model
            .fields()
            .configuration
            .dimensions
            .iter()
            .find(|(k, _)| k == "vocab_size")
            .unwrap()
            .1,
        151669
    );
    assert_eq!(model.attention_projection_width(), Some(2048));
}

#[test]
fn publisher_readme_declaration_and_absent_large_bytes_remain_declared() {
    let model = declared();
    assert_eq!(model.fields().rights.license_component, "README.md");
    assert_eq!(model.fields().rights.license_id, "Apache-2.0");
    assert_eq!(
        model.fields().rights.status,
        RightsStatus::PublisherTermsPinned
    );
    for name in ["model.safetensors", "tokenizer.json"] {
        assert_eq!(
            model
                .fields()
                .components
                .iter()
                .find(|c| c.name == name)
                .unwrap()
                .digest_source,
            DigestSource::PublisherAdvertised
        );
    }
    assert_eq!(
        model
            .fields()
            .components
            .iter()
            .find(|c| c.name == "vocab.json")
            .unwrap()
            .digest_source,
        DigestSource::AuditMeasuredSmallBytes
    );
    assert!(
        !model
            .fields()
            .components
            .iter()
            .any(|c| c.name == "LICENSE")
    );
}

#[test]
fn new_scoring_operators_stay_unsupported_without_external_fallback() {
    let model = declared();
    let profile = &model.fields().profiles[0];
    for deployment in [DeploymentKind::OwnedRustCpu, DeploymentKind::OwnedRustMetal] {
        assert_eq!(
            check_assignment(&model, profile, deployment, LoopMode::LocalOnly).unwrap(),
            AssignmentStatus::UnsupportedLocal(UnsupportedReason::Operators)
        );
    }
    assert!(
        check_assignment(
            &model,
            profile,
            DeploymentKind::ExternalConfigured,
            LoopMode::LocalOnly
        )
        .is_err()
    );
    assert_eq!(
        check_assignment(
            &model,
            profile,
            DeploymentKind::ExternalConfigured,
            LoopMode::ExplicitExternal
        )
        .unwrap(),
        AssignmentStatus::ExternalConfigurationRequired
    );
}

#[test]
fn score_profile_cannot_be_substituted_for_confidence_policy_or_carry_state() {
    let model = declared();
    for field in ["role", "units", "reset"] {
        let mut other = model.fields().profiles[0].clone();
        match field {
            "role" => other.role = RoleKind::Policy,
            "units" => {
                other.output_units = SemanticProfileId::new("calibrated-confidence.v1").unwrap()
            }
            _ => other.state_reset = SemanticProfileId::new("carry-state.v1").unwrap(),
        }
        assert!(
            check_assignment(
                &model,
                &other,
                DeploymentKind::OwnedRustCpu,
                LoopMode::LocalOnly
            )
            .is_err()
        );
    }
}

#[test]
fn reranker_source_rights_and_workload_rejections_use_unchanged_constructor() {
    let model = declared();
    for field in ["source", "rights", "workload", "missing_component"] {
        let mut draft = model.fields().clone();
        match field {
            "source" => draft.source_commit = [1; 20],
            "rights" => draft.rights.status = RightsStatus::Unresolved,
            "workload" => draft.configuration.batch_size = u64::MAX,
            _ => draft.configuration.weights_component = "absent.safetensors".into(),
        }
        assert!(ModelManifest::new(draft).is_err());
    }
}

#[test]
fn declared_score_catalog_does_not_accept_authority_fields_or_trailing_wire() {
    let Value::Map(map) = codec::decode(FIXTURE).unwrap() else {
        panic!("fixture map")
    };
    for key in [
        "permissions",
        "accepted",
        "can_execute",
        "calibrated_confidence",
    ] {
        let mut changed = map.clone();
        changed.push((key.into(), Value::Bool(true)));
        assert_eq!(
            ModelManifest::from_bytes(&codec::encode(&Value::Map(changed)).unwrap()),
            Err(CodecError::InvalidData)
        );
    }
    let mut trailing = FIXTURE.to_vec();
    trailing.push(0);
    assert_eq!(
        ModelManifest::from_bytes(&trailing),
        Err(CodecError::NonCanonical)
    );
}
