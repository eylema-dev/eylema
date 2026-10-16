use super::*;
use eylema_contracts::model_contract::ModelManifest;

fn manifest() -> ModelManifest {
    ModelManifest::from_bytes(include_bytes!(
        "../../tests/fixtures/g68/embedding-profile.v1.eyl"
    ))
    .expect("pinned G67 embedding manifest")
}

#[test]
fn g68_profile_is_closed_and_uses_its_own_vocabulary_layout() {
    let manifest = manifest();
    let layout = embedding_layout(&manifest).expect("G68 exact layout");
    assert_eq!(
        manifest.to_bytes().unwrap(),
        include_bytes!("../../tests/fixtures/g68/embedding-profile.v1.eyl")
    );
    assert_eq!(layout.tensors().len(), 310);
    assert_eq!(
        layout
            .tensors()
            .iter()
            .find(|t| t.name() == "embed_tokens.weight")
            .unwrap()
            .shape(),
        [151_669, 1024]
    );
    assert!(
        layout
            .tensors()
            .iter()
            .all(|t| !t.name().starts_with("model.") && t.name() != "lm_head.weight")
    );
    assert_eq!(
        layout
            .tensors()
            .iter()
            .find(|t| t.name() == "layers.27.self_attn.q_proj.weight")
            .unwrap()
            .shape(),
        [2048, 1024]
    );
    assert_eq!(
        embedding_layout(
            &ModelManifest::from_bytes(include_bytes!(
                "../../tests/fixtures/g30/cpu-profile.v1.eyl"
            ))
            .unwrap()
        )
        .unwrap_err(),
        crate::LoadError::InvalidProfile
    );
    assert_eq!(
        embedding_layout(
            &ModelManifest::from_bytes(include_bytes!(
                "../../tests/fixtures/g57/cpu-profile.v1.eyl"
            ))
            .unwrap()
        )
        .unwrap_err(),
        crate::LoadError::InvalidProfile
    );
}

#[test]
fn context_budget_rejects_unknown_and_overlimit_sizes() {
    let valid = EmbeddingBudget {
        context: 4096,
        max_resident_bytes: MAX_RESIDENT_BYTES,
        max_work: MAX_WORK,
    };
    assert!(valid.validate().is_ok());
    for context in [0, 4097, usize::MAX] {
        assert_eq!(
            EmbeddingBudget { context, ..valid }.validate(),
            Err(CpuError::ContextLimit)
        );
    }
    assert_eq!(
        EmbeddingBudget {
            max_work: MAX_WORK + 1,
            ..valid
        }
        .validate(),
        Err(CpuError::WorkLimit)
    );
    assert_eq!(
        EmbeddingBudget {
            max_resident_bytes: MAX_RESIDENT_BYTES + 1,
            ..valid
        }
        .validate(),
        Err(CpuError::ResidentLimit)
    );
}

#[test]
fn last_valid_pooling_supports_left_and_right_padding_and_rejects_holes() {
    let states = [1., 2., 3., 4., 5., 6., 7., 8., 9.];
    assert_eq!(
        pool_last_valid(&states, 3, 3, &[0, 1, 1]).unwrap(),
        [7., 8., 9.]
    );
    assert_eq!(
        pool_last_valid(&states, 3, 3, &[1, 1, 0]).unwrap(),
        [4., 5., 6.]
    );
    assert_eq!(
        pool_last_valid(&states, 3, 3, &[1, 0, 1]),
        Err(CpuError::InvalidInput)
    );
    assert_eq!(
        pool_last_valid(&states, 3, 3, &[0, 0, 0]),
        Err(CpuError::InvalidInput)
    );
    assert_eq!(
        pool_last_valid(&states, 3, 3, &[1, 2, 0]),
        Err(CpuError::InvalidLayout)
    );
    assert_eq!(
        pool_last_valid(&states, 3, 4, &[1, 1, 0]),
        Err(CpuError::InvalidLayout)
    );
}

#[test]
fn normalized_dot_product_is_cosine_only_for_finite_1024d_vectors() {
    let mut left = vec![0.0; 1024];
    let mut right = vec![0.0; 1024];
    left[0] = 1.0;
    right[1] = 1.0;
    assert_eq!(cosine_similarity(&left, &right).unwrap(), 0.0);
    right[0] = -1.0;
    assert_eq!(cosine_similarity(&left, &right).unwrap(), -1.0);
    assert_eq!(
        cosine_similarity(&left[..8], &right[..8]),
        Err(CpuError::InvalidLayout)
    );
    right[5] = f32::NAN;
    assert_eq!(cosine_similarity(&left, &right), Err(CpuError::NonFinite));
}

#[test]
fn exact_g67_prefix_and_document_mode_are_distinct() {
    assert_eq!(
        profile::QUERY_PREFIX,
        "Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery:"
    );
    assert_eq!(profile::QUERY_PREFIX.len(), 91);
    assert_eq!(profile::TOKENIZER_SIZE, 11_423_705);
    assert_eq!(profile::TOKENIZER_CONFIG_SIZE, 9_706);
    assert_ne!(
        profile::TOKENIZER_SHA,
        crate::tokenizer::Tokenizer::test_synthetic().tokenizer_sha256()
    );
}

#[test]
fn headless_embedding_header_rejects_foreign_names_shapes_and_extra_head() {
    // Independently declared artifact schema: two base tensors plus eleven per layer.
    let mut entries = vec![
        ("embed_tokens.weight".to_owned(), vec![151669u64, 1024]),
        ("norm.weight".to_owned(), vec![1024]),
    ];
    for layer in 0..28 {
        for (suffix, shape) in [
            ("input_layernorm.weight", vec![1024]),
            ("post_attention_layernorm.weight", vec![1024]),
            ("self_attn.q_proj.weight", vec![2048, 1024]),
            ("self_attn.k_proj.weight", vec![1024, 1024]),
            ("self_attn.v_proj.weight", vec![1024, 1024]),
            ("self_attn.o_proj.weight", vec![1024, 2048]),
            ("self_attn.q_norm.weight", vec![128]),
            ("self_attn.k_norm.weight", vec![128]),
            ("mlp.gate_proj.weight", vec![3072, 1024]),
            ("mlp.up_proj.weight", vec![3072, 1024]),
            ("mlp.down_proj.weight", vec![1024, 3072]),
        ] {
            entries.push((format!("layers.{layer}.{suffix}"), shape));
        }
    }
    let mut end = 0u64;
    let rows: Vec<_> = entries.iter().map(|(name,shape)| {
        let start=end; end+=shape.iter().product::<u64>()*2;
        format!("\"{name}\":{{\"dtype\":\"BF16\",\"shape\":{shape:?},\"data_offsets\":[{start},{end}]}}")
    }).collect();
    assert_eq!(end, 1_191_553_024);
    let text = format!("{{{}}}", rows.join(","));
    let layout = embedding_layout(&manifest()).unwrap();
    assert_eq!(
        crate::header::validate_header(text.as_bytes(), end, &layout)
            .unwrap()
            .len(),
        310
    );
    for bad in [
        text.replacen("embed_tokens.weight", "model.embed_tokens.weight", 1),
        text.replacen("layers.0.", "model.layers.0.", 1),
        text.replacen("BF16", "F32", 1),
        text.replacen("151669", "151936", 1),
        format!("{{{}}}", rows[1..].join(",")),
        text.replacen("\"norm.weight\"", "\"lm_head.weight\"", 1),
    ] {
        assert!(crate::header::validate_header(bad.as_bytes(), end, &layout).is_err());
    }
}

#[test]
fn request_work_cap_is_bound_before_tokenization() {
    let tokenizer = crate::tokenizer::embedding_test_fixture();
    let d = crate::cpu::profile::Dimensions {
        h: 2,
        inter: 2,
        qh: 1,
        kh: 1,
        w: 2,
        layers: 1,
        vocab: 256,
    };
    let mut map = std::collections::BTreeMap::new();
    map.insert("model.embed_tokens.weight".to_owned(), vec![1.; 512]);
    map.insert("model.norm.weight".to_owned(), vec![1.; 2]);
    for (suffix, n) in [
        ("input_layernorm.weight", 2),
        ("post_attention_layernorm.weight", 2),
        ("self_attn.q_proj.weight", 4),
        ("self_attn.k_proj.weight", 4),
        ("self_attn.v_proj.weight", 4),
        ("self_attn.o_proj.weight", 4),
        ("self_attn.q_norm.weight", 2),
        ("self_attn.k_norm.weight", 2),
        ("mlp.gate_proj.weight", 4),
        ("mlp.up_proj.weight", 4),
        ("mlp.down_proj.weight", 4),
    ] {
        map.insert(format!("model.layers.0.{suffix}"), vec![1.; n]);
    }
    let model = EmbeddingCpuModel {
        weights: crate::cpu::weights::BoundWeights::synthetic(d, &map).unwrap(),
        manifest_sha256: tokenizer.manifest_sha256(),
        tokenizer: &tokenizer,
        artifact_sha256: [0; 32],
        base_bytes: 0,
        admission_work: 0,
    };
    let control =
        CpuControl::new(std::time::Instant::now() + std::time::Duration::from_secs(60)).unwrap();
    let budget = EmbeddingBudget {
        context: 4096,
        max_resident_bytes: MAX_RESIDENT_BYTES,
        max_work: 1,
    };
    assert_eq!(
        model
            .embed_document("a", budget, control.clone())
            .unwrap_err(),
        CpuError::WorkLimit
    );
    assert!(control.observed_work() <= 1);
}
