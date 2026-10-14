use eylema_contracts::{
    codec::{self, CodecError, Value},
    model_contract::{
        AssignmentStatus, DigestSource, ModelManifest, RightsStatus, UnsupportedReason,
        check_assignment,
    },
    platform::{DeploymentKind, LoopMode, RoleKind, SemanticProfileId},
};

const FIXTURE: &[u8] = include_bytes!("fixtures/qwen3-embedding-profile.v1.eyl");
fn declared() -> ModelManifest {
    ModelManifest::from_bytes(FIXTURE).unwrap()
}

#[test]
fn actual_embedding_fixture_roundtrips_exact_source_and_representation_interface() {
    let model = declared();
    assert_eq!(model.to_bytes().unwrap(), FIXTURE);
    assert_eq!(model.fields().id.as_str(), "qwen3-embedding-0.6b");
    let commit = model
        .fields()
        .source_commit
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<String>();
    assert_eq!(commit, "97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3");
    assert_eq!(model.fields().profiles.len(), 1);
    let profile = &model.fields().profiles[0];
    assert_eq!(profile.role, RoleKind::Representation);
    assert_eq!(
        profile.input_schema.as_str(),
        "representation.query-document.v1"
    );
    assert_eq!(profile.output_units.as_str(), "l2-vector-1024.v1");
    assert_eq!(profile.state_reset.as_str(), "reset-per-text.v1");
    assert_eq!(model.fields().configuration.context_limit, 4096);
    assert_eq!(
        model
            .fields()
            .configuration
            .dimensions
            .iter()
            .find(|(k, _)| k == "context_capacity")
            .unwrap()
            .1,
        32768
    );
    assert_eq!(model.attention_projection_width(), Some(2048));
}

#[test]
fn pooling_alias_and_readme_terms_are_measured_declarations_not_artifact_execution() {
    let model = declared();
    assert_eq!(model.fields().rights.license_component, "README.md");
    assert_eq!(model.fields().rights.license_id, "Apache-2.0");
    assert_eq!(
        model.fields().rights.status,
        RightsStatus::PublisherTermsPinned
    );
    for name in [
        "pooling-config.json",
        "modules.json",
        "config_sentence_transformers.json",
        "vocab.json",
    ] {
        let component = model
            .fields()
            .components
            .iter()
            .find(|c| c.name == name)
            .unwrap();
        assert_eq!(
            component.digest_source,
            DigestSource::AuditMeasuredSmallBytes
        );
    }
    for name in ["model.safetensors", "tokenizer.json"] {
        let component = model
            .fields()
            .components
            .iter()
            .find(|c| c.name == name)
            .unwrap();
        assert_eq!(component.digest_source, DigestSource::PublisherAdvertised);
    }
    assert!(
        !model
            .fields()
            .components
            .iter()
            .any(|c| c.name == "LICENSE")
    );
}

#[test]
fn pooling_normalization_and_retrieval_operators_remain_unsupported_and_local_only() {
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
fn different_vector_units_preprocessing_role_and_reset_do_not_match_assignment() {
    let model = declared();
    for field in ["units", "prefix", "pool", "norm", "role", "reset"] {
        let mut other = model.fields().profiles[0].clone();
        match field {
            "units" => other.output_units = SemanticProfileId::new("l2-vector-512.v1").unwrap(),
            "prefix" => {
                other.preprocessing = SemanticProfileId::new("document-as-query.v1").unwrap()
            }
            "pool" => other.preprocessing = SemanticProfileId::new("mean-pooling.v1").unwrap(),
            "norm" => {
                other.output_units = SemanticProfileId::new("unnormalized-vector-1024.v1").unwrap()
            }
            "role" => other.role = RoleKind::Decision,
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
fn actual_embedding_draft_rejects_foreign_rights_overflow_and_dangling_pool_reference() {
    let model = declared();
    for field in ["source", "rights", "extent", "component"] {
        let mut draft = model.fields().clone();
        match field {
            "source" => draft.source_commit = [1; 20],
            "rights" => draft.rights.status = RightsStatus::Unresolved,
            "extent" => draft.configuration.batch_size = u64::MAX,
            _ => draft.configuration.preprocessing_component = "missing-pool.json".into(),
        }
        assert!(ModelManifest::new(draft).is_err());
    }
}

#[test]
fn embedding_declaration_rejects_authority_fields_and_noncanonical_trailing_wire() {
    let Value::Map(map) = codec::decode(FIXTURE).unwrap() else {
        panic!("fixture map")
    };
    for key in [
        "permissions",
        "accepted",
        "can_train",
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
