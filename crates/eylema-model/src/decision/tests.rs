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
