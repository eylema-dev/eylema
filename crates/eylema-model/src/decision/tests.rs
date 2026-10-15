use super::*;
#[test]
fn question_bounds_and_identity() {
    let options = [DecisionOption {
        id: "a",
        candidate: "A",
    }];
    let questions = [DecisionQuestion {
        id: "q",
        instruction: "pick",
        query: "Q",
        options: &options,
    }];
    assert!(DecisionBatch::new(&questions).is_ok());
    assert!(DecisionBatch::new(&[]).is_err());
    let bad_id = [DecisionQuestion {
        id: "q/../x",
        instruction: "pick",
        query: "Q",
        options: &options,
    }];
    assert!(DecisionBatch::new(&bad_id).is_err());
    let duplicate_options = [
        DecisionOption {
            id: "same",
            candidate: "A",
        },
        DecisionOption {
            id: "same",
            candidate: "B",
        },
    ];
    let duplicate_option_question = [DecisionQuestion {
        id: "other",
        instruction: "pick",
        query: "Q",
        options: &duplicate_options,
    }];
    assert!(DecisionBatch::new(&duplicate_option_question).is_err());
    let dup = [
        DecisionQuestion {
            id: "q",
            instruction: "pick",
            query: "Q",
            options: &options,
        },
        DecisionQuestion {
            id: "q",
            instruction: "pick",
            query: "Q",
            options: &options,
        },
    ];
    assert!(DecisionBatch::new(&dup).is_err());
}
#[test]
fn score_reference_rounding_and_abstention() {
    assert_eq!(score::head([0., 0.]).unwrap().2, 500000);
    assert_eq!(score::head([2., 0.]).unwrap().2, 880797);
    assert_eq!(score::head([-2., 0.]).unwrap().2, 119203);
    assert!(score::head([f32::NAN, 0.]).is_err());
    assert!(score::head([40., -40.]).is_ok());
    assert_eq!(score::micros(0.0000005).unwrap(), 0);
    assert_eq!(score::micros(0.0000015).unwrap(), 2);
    assert_eq!(
        score::choose(&[Some(750000), Some(650000)]),
        DecisionReason::Chosen
    );
    assert_eq!(
        score::choose(&[Some(750000), Some(650001)]),
        DecisionReason::InsufficientMargin
    );
    assert_eq!(
        score::choose(&[Some(749999), Some(0)]),
        DecisionReason::BelowFloor
    );
    assert_eq!(score::choose(&[Some(750000)]), DecisionReason::Chosen);
    assert!(score::head([1000.0, -1000.0]).is_ok());
    assert_eq!(score::head([0.0, 0.0]).unwrap().2, 500000);
    assert_eq!(
        score::choose(&[Some(880797), Some(880797)]),
        DecisionReason::Tie
    );
    assert_eq!(
        score::choose(&[None, Some(880797)]),
        DecisionReason::MissingScore
    );
}

#[test]
fn output_reservation_accounts_for_owned_results_and_choice_ids() {
    let options = [
        DecisionOption {
            id: "x",
            candidate: "first",
        },
        DecisionOption {
            id: "longer-id",
            candidate: "second",
        },
    ];
    let questions = [DecisionQuestion {
        id: "question",
        instruction: "instruction",
        query: "query",
        options: &options,
    }];
    let batch = DecisionBatch::new(&questions).unwrap();
    let actual = request::output_reservation(&batch).unwrap();
    let expected = std::mem::size_of::<DecisionResult>()
        + std::mem::size_of::<DecisionReceipt>()
        + std::mem::size_of::<QuestionScores>()
        + 2 * std::mem::size_of::<OptionScore>()
        + "question".len()
        + "x".len()
        + "longer-id".len()
        + "longer-id".len();
    assert_eq!(actual, expected as u64);
    let mut id = String::new();
    id.try_reserve_exact("longer-id".len()).unwrap();
    id.push_str("longer-id");
    let mut question_id = String::new();
    question_id.try_reserve_exact("question".len()).unwrap();
    question_id.push_str("question");
    let mut choice = String::new();
    choice.try_reserve_exact("longer-id".len()).unwrap();
    let result_question = QuestionScores {
        id: question_id,
        scores: vec![
            OptionScore {
                id: id.clone(),
                logits: [0.0; 2],
                delta: 0.0,
                probability: 0.0,
                micros: 0,
                cumulative: 0,
            },
            OptionScore {
                id,
                logits: [0.0; 2],
                delta: 0.0,
                probability: 0.0,
                micros: 0,
                cumulative: 0,
            },
        ],
        choice: Some(choice),
        reason: DecisionReason::MissingScore,
    };
    assert!(request::actual_output_bytes(&[result_question], 1).unwrap() >= actual);
}

#[test]
fn request_identity_binds_question_and_option_boundaries() {
    let words = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t",
    ];
    let first_options = [
        DecisionOption {
            id: words[3],
            candidate: words[4],
        },
        DecisionOption {
            id: words[5],
            candidate: words[6],
        },
        DecisionOption {
            id: words[7],
            candidate: words[8],
        },
    ];
    let second_options = [
        DecisionOption {
            id: words[12],
            candidate: words[13],
        },
        DecisionOption {
            id: words[14],
            candidate: words[15],
        },
        DecisionOption {
            id: words[16],
            candidate: words[17],
        },
        DecisionOption {
            id: words[18],
            candidate: words[19],
        },
    ];
    let two_questions = [
        DecisionQuestion {
            id: words[0],
            instruction: words[1],
            query: words[2],
            options: &first_options,
        },
        DecisionQuestion {
            id: words[9],
            instruction: words[10],
            query: words[11],
            options: &second_options,
        },
    ];
    let singleton_options = [
        [DecisionOption {
            id: words[3],
            candidate: words[4],
        }],
        [DecisionOption {
            id: words[8],
            candidate: words[9],
        }],
        [DecisionOption {
            id: words[13],
            candidate: words[14],
        }],
        [DecisionOption {
            id: words[18],
            candidate: words[19],
        }],
    ];
    let four_questions = [
        DecisionQuestion {
            id: words[0],
            instruction: words[1],
            query: words[2],
            options: &singleton_options[0],
        },
        DecisionQuestion {
            id: words[5],
            instruction: words[6],
            query: words[7],
            options: &singleton_options[1],
        },
        DecisionQuestion {
            id: words[10],
            instruction: words[11],
            query: words[12],
            options: &singleton_options[2],
        },
        DecisionQuestion {
            id: words[15],
            instruction: words[16],
            query: words[17],
            options: &singleton_options[3],
        },
    ];
    let first = DecisionBatch::new(&two_questions).unwrap();
    let second = DecisionBatch::new(&four_questions).unwrap();
    let control =
        crate::cpu::CpuControl::new(std::time::Instant::now() + std::time::Duration::from_secs(30))
            .unwrap();
    assert_ne!(
        request::identity(&first, &control).unwrap(),
        request::identity(&second, &control).unwrap()
    );
}

#[test]
fn full_decision_workspace_is_known_before_frontend_admission() {
    let options = [DecisionOption {
        id: "a",
        candidate: "A",
    }];
    let questions = [DecisionQuestion {
        id: "q",
        instruction: "pick",
        query: "Q",
        options: &options,
    }];
    let batch = DecisionBatch::new(&questions).unwrap();
    let dimensions = crate::decision::profile::dimensions();
    let (bytes, work) = request::workspace_reservation(dimensions, 4).unwrap();
    let base = 128u64;
    let output = request::output_reservation(&batch).unwrap();
    let exact = base + output + bytes;
    let work_limit = work + output;
    assert!(request::preflight(&batch, base, dimensions, 4, exact, work_limit, 0).is_ok());
    assert_eq!(
        request::preflight(&batch, base, dimensions, 4, exact - 1, work_limit, 0),
        Err(crate::cpu::CpuError::ResidentLimit)
    );
    assert_eq!(
        request::preflight(&batch, base, dimensions, 4, exact, work_limit - 1, 0),
        Err(crate::cpu::CpuError::WorkLimit)
    );
}

#[test]
fn synthetic_score_api_batches_resets_and_cancels_atomically() {
    use crate::cpu::{CpuControl, CpuWorkGroup, profile::Dimensions, weights::BoundWeights};
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };

    let dimensions = Dimensions {
        h: 2,
        inter: 2,
        qh: 1,
        kh: 1,
        w: 2,
        layers: 1,
        vocab: 151669,
    };
    let mut tensors = BTreeMap::<String, Vec<f32>>::new();
    tensors.insert(
        "model.embed_tokens.weight".into(),
        vec![0.0; dimensions.vocab * dimensions.h],
    );
    tensors.insert("model.norm.weight".into(), vec![1.0; dimensions.h]);
    for (name, len) in [
        ("input_layernorm.weight", dimensions.h),
        ("post_attention_layernorm.weight", dimensions.h),
        (
            "self_attn.q_proj.weight",
            dimensions.qh * dimensions.w * dimensions.h,
        ),
        (
            "self_attn.k_proj.weight",
            dimensions.kh * dimensions.w * dimensions.h,
        ),
        (
            "self_attn.v_proj.weight",
            dimensions.kh * dimensions.w * dimensions.h,
        ),
        (
            "self_attn.o_proj.weight",
            dimensions.h * dimensions.qh * dimensions.w,
        ),
        ("self_attn.q_norm.weight", dimensions.w),
        ("self_attn.k_norm.weight", dimensions.w),
        ("mlp.gate_proj.weight", dimensions.inter * dimensions.h),
        ("mlp.up_proj.weight", dimensions.inter * dimensions.h),
        ("mlp.down_proj.weight", dimensions.h * dimensions.inter),
    ] {
        tensors.insert(crate::cpu::profile::name(0, name).unwrap(), vec![0.0; len]);
    }
    let weights = BoundWeights::synthetic(dimensions, &tensors).unwrap();
    let manifest = ModelManifest::from_bytes(include_bytes!(
        "../../tests/fixtures/g57/cpu-profile.v1.eyl"
    ))
    .unwrap();
    let group = CpuWorkGroup::new(2_500_000_000_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let bound = CpuControl::new_job(deadline, 800_000_000_000, group.clone()).unwrap();
    let tokenizer = crate::tokenizer::DecisionTokenizer {
        inner: crate::tokenizer::Tokenizer::test_synthetic(),
        control: bound.clone(),
    };
    let model = DecisionModel {
        weights,
        manifest: &manifest,
        tokenizer: &tokenizer,
        base: 1_000_000,
        admission_work: 0,
        control: bound.clone(),
        dimensions,
    };
    let options_a = [
        DecisionOption {
            id: "a",
            candidate: "Paris",
        },
        DecisionOption {
            id: "b",
            candidate: "Pacific",
        },
    ];
    let options_b = [
        DecisionOption {
            id: "b",
            candidate: "Pacific",
        },
        DecisionOption {
            id: "a",
            candidate: "Paris",
        },
    ];
    let questions = [
        DecisionQuestion {
            id: "first",
            instruction: "judge",
            query: "capital",
            options: &options_a,
        },
        DecisionQuestion {
            id: "second",
            instruction: "judge",
            query: "city",
            options: &options_b,
        },
    ];
    let batch = DecisionBatch::new(&questions).unwrap();
    let budget = DecisionBudget {
        context: 4096,
        max_resident_bytes: 32_000_000,
        max_work: 2_000_000_000,
    };
    let result = model.score(&batch, budget, bound.clone()).unwrap();
    assert_eq!(result.questions().len(), 2);
    assert_eq!(result.questions()[0].question_id(), "first");
    assert_eq!(result.questions()[1].question_id(), "second");
    for q in result.questions() {
        assert_eq!(q.choice(), None);
        assert_eq!(q.reason(), DecisionReason::Tie);
        assert_eq!(q.scores().len(), 2);
        assert!(q.scores().iter().all(|s| s.score_micros() == 500_000));
    }
    for (qid, oid) in [
        ("first", "a"),
        ("first", "b"),
        ("second", "a"),
        ("second", "b"),
    ] {
        assert_eq!(pair_score_for_test(&result, qid, oid).logits(), [0.0, 0.0]);
    }
    let cancelled = CpuControl::new_job(deadline, 800_000_000_000, group).unwrap();
    cancelled.stop_after(1_000);
    assert!(matches!(
        model.score(&batch, budget, cancelled),
        Err(crate::cpu::CpuError::Cancelled)
    ));
}

fn pair_score_for_test<'a>(
    result: &'a DecisionResult,
    question: &str,
    option: &str,
) -> &'a OptionScore {
    result
        .questions()
        .iter()
        .find(|q| q.question_id() == question)
        .unwrap()
        .scores()
        .iter()
        .find(|s| s.option_id() == option)
        .unwrap()
}
