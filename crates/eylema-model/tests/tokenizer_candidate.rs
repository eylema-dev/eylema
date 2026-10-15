//! Real public-artifact tokenizer acceptance; ordinary CI honestly skips absent assets.
use eylema_contracts::model_contract::{AssignmentStatus, ModelManifest, check_assignment};
use eylema_contracts::platform::{DeploymentKind, LoopMode};
use eylema_model::tokenizer::{AdmissionBudget, PromptMessage, PromptRole, TextBudget, Tokenizer};
fn tb() -> TextBudget {
    TextBudget {
        max_resident_bytes: 134217728,
        max_tokens: 4096,
        max_work: 2000000,
    }
}
#[test]
#[ignore = "requires explicitly authorized private public tokenizer/config artifacts"]
fn candidate_vectors_on_native() {
    let root = std::env::var("G29_ASSETS").expect("explicit public artifact directory required");
    let mut file = std::fs::File::open(std::path::Path::new(&root).join("tokenizer.json")).unwrap();
    let mut cfg =
        std::fs::File::open(std::path::Path::new(&root).join("tokenizer_config.json")).unwrap();
    let m = ModelManifest::from_bytes(include_bytes!(
        "../../eylema-contracts/tests/fixtures/qwen3-profile.v1.eyl"
    ))
    .unwrap();
    let t = Tokenizer::load(
        &mut file,
        &mut cfg,
        &m,
        AdmissionBudget {
            max_resident_bytes: 134217728,
            max_read_calls: 1024,
            max_interruptions: 16,
            max_work_per_artifact: 134217728,
        },
    )
    .unwrap();
    prompt_boundary_vectors(&t);
    let got = t.encode_text("", tb()).unwrap();
    assert_eq!(got.ids(), &[], "text_00");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "");
    let got = t.encode_text("Hello world", tb()).unwrap();
    assert_eq!(got.ids(), &[9707, 1879], "text_01");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "Hello world");
    let got = t.encode_text(" Hello world", tb()).unwrap();
    assert_eq!(got.ids(), &[21927, 1879], "text_02");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), " Hello world");
    let got = t.encode_text("I'M we're can't 'ſ", tb()).unwrap();
    assert_eq!(
        got.ids(),
        &[40, 27603, 582, 2299, 646, 944, 364, 144815],
        "text_03"
    );
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "I'M we're can't 'ſ");
    let got = t.encode_text("İstanbul ı I ÇĞŞ", tb()).unwrap();
    assert_eq!(
        got.ids(),
        &[47780, 45116, 220, 3777, 358, 63005, 125823, 125074],
        "text_04"
    );
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "İstanbul ı I ÇĞŞ");
    let got = t.encode_text("ÇĞŞ", tb()).unwrap();
    assert_eq!(got.ids(), &[45913, 125823, 125074], "text_05");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "ÇĞŞ");
    let got = t.encode_text("q̣̇", tb()).unwrap();
    assert_eq!(got.ids(), &[80, 136, 96, 136, 229], "text_06");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "q̣̇");
    let got = t.encode_text("가", tb()).unwrap();
    assert_eq!(got.ids(), &[19969], "text_07");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "가");
    let got = t.encode_text("你好世界", tb()).unwrap();
    assert_eq!(got.ids(), &[108386, 99489], "text_08");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "你好世界");
    let got = t.encode_text("🙂🚀𐐀", tb()).unwrap();
    assert_eq!(got.ids(), &[145080, 145836, 123934, 238, 222], "text_09");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "🙂🚀𐐀");
    let got = t.encode_text("123456", tb()).unwrap();
    assert_eq!(got.ids(), &[16, 17, 18, 19, 20, 21], "text_10");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "123456");
    let got = t.encode_text(" \t Hello\r\nworld  \n\t", tb()).unwrap();
    assert_eq!(got.ids(), &[7018, 21927, 319, 14615, 2303, 197], "text_11");
    assert_eq!(
        t.decode(got.ids(), tb()).unwrap(),
        " \t Hello\r\nworld  \n\t"
    );
    let got = t.encode_text("abababa", tb()).unwrap();
    assert_eq!(got.ids(), &[370, 370, 12004], "text_12");
    assert_eq!(t.decode(got.ids(), tb()).unwrap(), "abababa");
    let messages = [PromptMessage {
        role: PromptRole::User,
        content: "Hi",
    }];
    let got = t.encode_prompt(&messages, tb()).unwrap();
    assert_eq!(
        got.ids(),
        &[
            151644, 872, 198, 13048, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271
        ],
        "prompt_00"
    );
    assert_eq!(
        t.decode(got.ids(), tb()).unwrap(),
        "<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    );
    let messages = [
        PromptMessage {
            role: PromptRole::System,
            content: "Be precise.",
        },
        PromptMessage {
            role: PromptRole::User,
            content: "İstanbul?",
        },
    ];
    let got = t.encode_prompt(&messages, tb()).unwrap();
    assert_eq!(
        got.ids(),
        &[
            151644, 8948, 198, 3430, 23560, 13, 151645, 198, 151644, 872, 198, 47780, 45116, 30,
            151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271
        ],
        "prompt_01"
    );
    assert_eq!(
        t.decode(got.ids(), tb()).unwrap(),
        "<|im_start|>system\nBe precise.<|im_end|>\n<|im_start|>user\nİstanbul?<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    );
    let messages = [
        PromptMessage {
            role: PromptRole::User,
            content: "One",
        },
        PromptMessage {
            role: PromptRole::Assistant,
            content: "Two",
        },
        PromptMessage {
            role: PromptRole::User,
            content: "Three",
        },
    ];
    let got = t.encode_prompt(&messages, tb()).unwrap();
    assert_eq!(
        got.ids(),
        &[
            151644, 872, 198, 3966, 151645, 198, 151644, 77091, 198, 11613, 151645, 198, 151644,
            872, 198, 19641, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271
        ],
        "prompt_02"
    );
    assert_eq!(
        t.decode(got.ids(), tb()).unwrap(),
        "<|im_start|>user\nOne<|im_end|>\n<|im_start|>assistant\nTwo<|im_end|>\n<|im_start|>user\nThree<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    );
    let messages = [
        PromptMessage {
            role: PromptRole::System,
            content: "Be precise.",
        },
        PromptMessage {
            role: PromptRole::User,
            content: "One",
        },
        PromptMessage {
            role: PromptRole::Assistant,
            content: "Two",
        },
        PromptMessage {
            role: PromptRole::User,
            content: "Three",
        },
    ];
    let got = t.encode_prompt(&messages, tb()).unwrap();
    assert_eq!(
        got.ids(),
        &[
            151644, 8948, 198, 3430, 23560, 13, 151645, 198, 151644, 872, 198, 3966, 151645, 198,
            151644, 77091, 198, 11613, 151645, 198, 151644, 872, 198, 19641, 151645, 198, 151644,
            77091, 198, 151667, 271, 151668, 271
        ],
        "prompt_03"
    );
    assert_eq!(
        t.decode(got.ids(), tb()).unwrap(),
        "<|im_start|>system\nBe precise.<|im_end|>\n<|im_start|>user\nOne<|im_end|>\n<|im_start|>assistant\nTwo<|im_end|>\n<|im_start|>user\nThree<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    );
    assert!(matches!(
        check_assignment(
            &m,
            &m.fields().profiles[0],
            DeploymentKind::OwnedRustCpu,
            LoopMode::LocalOnly
        )
        .unwrap(),
        AssignmentStatus::UnsupportedLocal(_)
    ));
}

fn prompt_boundary_vectors(t: &Tokenizer) {
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: "",
        }];
        let expected: &[u32] = &[
            151644, 872, 198, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_04");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: "\n",
        }];
        let expected: &[u32] = &[
            151644, 872, 271, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_05");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: "\r\n",
        }];
        let expected: &[u32] = &[
            151644, 872, 198, 319, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_06");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n\r\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: " \n",
        }];
        let expected: &[u32] = &[
            151644, 872, 26809, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_07");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n \n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: "\t",
        }];
        let expected: &[u32] = &[
            151644, 872, 198, 197, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_08");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n\t<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: "\nİstanbul",
        }];
        let expected: &[u32] = &[
            151644, 872, 271, 47780, 45116, 151645, 198, 151644, 77091, 198, 151667, 271, 151668,
            271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_09");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n\nİstanbul<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: "̧",
        }];
        let expected: &[u32] = &[
            151644, 872, 198, 136, 100, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_10");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\ņ<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [
            PromptMessage {
                role: PromptRole::System,
                content: "\n",
            },
            PromptMessage {
                role: PromptRole::User,
                content: "\r\n",
            },
        ];
        let expected: &[u32] = &[
            151644, 8948, 271, 151645, 198, 151644, 872, 198, 319, 151645, 198, 151644, 77091, 198,
            151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_11");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>system\n\n<|im_end|>\n<|im_start|>user\n\r\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [
            PromptMessage {
                role: PromptRole::User,
                content: "One",
            },
            PromptMessage {
                role: PromptRole::Assistant,
                content: "\nTwo",
            },
            PromptMessage {
                role: PromptRole::User,
                content: "\nThree",
            },
        ];
        let expected: &[u32] = &[
            151644, 872, 198, 3966, 151645, 198, 151644, 77091, 271, 11613, 151645, 198, 151644,
            872, 271, 19641, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_12");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\nOne<|im_end|>\n<|im_start|>assistant\n\nTwo<|im_end|>\n<|im_start|>user\n\nThree<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: " \r\n",
        }];
        let expected: &[u32] = &[
            151644, 872, 198, 2529, 151645, 198, 151644, 77091, 198, 151667, 271, 151668, 271,
        ];
        let mut budget = tb();
        budget.max_tokens = 4096;
        let got = t.encode_prompt(&messages, budget).unwrap();
        assert_eq!(got.ids(), expected, "prompt_13");
        assert_eq!(
            t.decode(got.ids(), tb()).unwrap(),
            "<|im_start|>user\n \r\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        budget.max_tokens = expected.len();
        assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), expected);
        budget.max_tokens -= 1;
        assert_eq!(
            t.encode_prompt(&messages, budget).unwrap_err(),
            eylema_model::tokenizer::TokenizeError::TokenLimit
        );
    }
}
