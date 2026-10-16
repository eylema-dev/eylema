use super::*;
use std::io::{Cursor, Read};
const TINY: &[u8] = include_bytes!("../../tests/fixtures/g29/synthetic-tokenizer.json");
const CONFIG: &[u8] = include_bytes!("../../tests/fixtures/g29/pinned-config.json");
pub(super) fn manifest() -> eylema_contracts::model_contract::ModelManifest {
    eylema_contracts::model_contract::ModelManifest::from_bytes(include_bytes!(
        "../../../eylema-contracts/tests/fixtures/qwen3-profile.v1.eyl"
    ))
    .unwrap()
}
fn ab() -> AdmissionBudget {
    AdmissionBudget {
        max_resident_bytes: 134217728,
        max_read_calls: 1024,
        max_interruptions: 16,
        max_work_per_artifact: 134217728,
    }
}
struct Never;
impl Read for Never {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("reader used before known rejection")
    }
}
#[test]
fn known_limits_before_reader() {
    let mut budget = ab();
    budget.max_resident_bytes = 1;
    assert_eq!(
        Tokenizer::load(&mut Never, &mut Never, &manifest(), budget).unwrap_err(),
        TokenizeError::ResidentLimit
    );
    let mut d = manifest().fields().clone();
    d.configuration.context_limit = 1024;
    let changed = eylema_contracts::model_contract::ModelManifest::new(d).unwrap();
    assert_eq!(
        Tokenizer::load(&mut Never, &mut Never, &changed, ab()).unwrap_err(),
        TokenizeError::UnsupportedProfile
    );
}
struct CountingEof(u32);
impl Read for CountingEof {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        self.0 += 1;
        Ok(0)
    }
}
#[test]
fn known_work_and_reads_reject_before_either_reader() {
    // Independent constants: ceil(11_422_654 / 65_536) + ceil(9732 / 65_536)
    // + one EOF probe per artifact = 178 shared reader calls at minimum.
    for (work, reads) in [
        (1, 1024),
        (11_422_653, 1024),
        (134217728, 1),
        (134217728, 177),
    ] {
        let mut budget = ab();
        budget.max_work_per_artifact = work;
        budget.max_read_calls = reads;
        let mut tokenizer = CountingEof(0);
        let mut config = CountingEof(0);
        let error = Tokenizer::load(&mut tokenizer, &mut config, &manifest(), budget).unwrap_err();
        assert_eq!(
            (error, tokenizer.0, config.0),
            (TokenizeError::WorkLimit, 0, 0)
        );
        assert_eq!(
            Tokenizer::load(&mut Never, &mut Never, &manifest(), budget).unwrap_err(),
            TokenizeError::WorkLimit
        );
    }
}
#[test]
fn necessary_input_floors_do_not_replace_incremental_checks() {
    let mut budget = ab();
    budget.max_work_per_artifact = 11_422_654;
    budget.max_read_calls = 178;
    let mut tokenizer = CountingEof(0);
    let mut config = CountingEof(0);
    assert_eq!(
        Tokenizer::load(&mut tokenizer, &mut config, &manifest(), budget).unwrap_err(),
        TokenizeError::Io
    );
    assert_eq!((tokenizer.0, config.0), (1, 0));
    // The synthetic tokenizer is smaller than its config: check both known sizes.
    budget = ab();
    budget.max_work_per_artifact = TINY.len() as u64;
    assert!(CONFIG.len() > TINY.len());
    assert_eq!(
        admission::admit(
            &mut Never,
            &mut Never,
            &manifest(),
            budget,
            admission::test_expected()
        )
        .unwrap_err(),
        TokenizeError::WorkLimit
    );
    budget = ab();
    // Two data reads and two EOF probes; accepting this floor must still reject EOF.
    budget.max_read_calls = 4;
    let mut tokenizer = CountingEof(0);
    assert_eq!(
        admission::admit(
            &mut tokenizer,
            &mut Never,
            &manifest(),
            budget,
            admission::test_expected()
        )
        .unwrap_err(),
        TokenizeError::Io
    );
    assert_eq!(tokenizer.0, 1);
}
#[test]
fn full_identity_and_closed_semantics() {
    let got = admission::test_load(TINY, CONFIG, ab()).unwrap();
    assert_eq!(got.vocab.len(), 259);
    assert_eq!(got.merges.len(), 3);
    assert_eq!(
        Tokenizer::load(
            &mut Cursor::new(TINY),
            &mut Cursor::new(CONFIG),
            &manifest(),
            ab()
        )
        .unwrap_err(),
        TokenizeError::Io
    );
    let mut e = admission::test_expected();
    e.tokenizer_sha256 = [1; 32];
    assert_eq!(
        admission::admit(
            &mut Cursor::new(TINY),
            &mut Cursor::new(CONFIG),
            &manifest(),
            ab(),
            e
        )
        .unwrap_err(),
        TokenizeError::HashMismatch
    );
    let mut extra = TINY.to_vec();
    extra.push(b' ');
    assert_eq!(
        admission::admit(
            &mut Cursor::new(extra),
            &mut Cursor::new(CONFIG),
            &manifest(),
            ab(),
            admission::test_expected()
        )
        .unwrap_err(),
        TokenizeError::SizeLimit
    );
}
#[test]
fn duplicate_and_merge_validation() {
    for (old, new, want) in [
        (
            "\"a\":97",
            "\"a\":97,\"\\u0061\":99",
            TokenizeError::InvalidJson,
        ),
        ("\"a\":97", "\"a\":98", TokenizeError::InvalidVocabulary),
        (
            "[\"ab\",\"a\"]",
            "[\"unknown\",\"a\"]",
            TokenizeError::InvalidMerge,
        ),
        ("\"NFC\"", "\"NFKC\"", TokenizeError::InvalidConfig),
        (
            "\"add_prefix_space\":false",
            "\"add_prefix_space\":true",
            TokenizeError::InvalidConfig,
        ),
    ] {
        let text = std::str::from_utf8(TINY).unwrap();
        assert!(text.contains(old));
        let bad = text.replacen(old, new, 1);
        assert_eq!(
            admission::test_load_reidentified(bad.as_bytes(), CONFIG, ab()).unwrap_err(),
            want
        );
    }
}
#[test]
fn reader_retry_and_work_bounds() {
    struct Interrupt(u32);
    impl Read for Interrupt {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            self.0 += 1;
            Err(std::io::ErrorKind::Interrupted.into())
        }
    }
    let mut bad = Interrupt(0);
    assert_eq!(
        admission::admit(
            &mut bad,
            &mut Cursor::new(CONFIG),
            &manifest(),
            ab(),
            admission::test_expected()
        )
        .unwrap_err(),
        TokenizeError::WorkLimit
    );
    assert_eq!(bad.0, 17);
    let mut b = ab();
    b.max_read_calls = 1;
    assert_eq!(
        admission::admit(
            &mut Cursor::new(TINY),
            &mut Cursor::new(CONFIG),
            &manifest(),
            b,
            admission::test_expected()
        )
        .unwrap_err(),
        TokenizeError::WorkLimit
    );
    b = ab();
    b.max_work_per_artifact = 1;
    assert_eq!(
        admission::test_load(TINY, CONFIG, b).unwrap_err(),
        TokenizeError::WorkLimit
    );
}
#[test]
fn redacted_storage() {
    let got = admission::test_load(TINY, CONFIG, ab()).unwrap();
    let debug = format!("{got:?}");
    assert!(!debug.contains("im_start") && !debug.contains("chat_template"));
}
#[test]
fn hard_limits_and_byte_alphabet_failure() {
    let mut e = admission::test_expected();
    e.tokenizer_size = 16777217;
    assert_eq!(
        admission::admit(&mut Never, &mut Never, &manifest(), ab(), e).unwrap_err(),
        TokenizeError::SizeLimit
    );
    let mut e = admission::test_expected();
    e.config_size = 65537;
    assert_eq!(
        admission::admit(&mut Never, &mut Never, &manifest(), ab(), e).unwrap_err(),
        TokenizeError::SizeLimit
    );
    let text = std::str::from_utf8(TINY)
        .unwrap()
        .replacen("\"a\":97", "\"nonalphabet\":97", 1);
    assert_eq!(
        admission::test_load_reidentified(text.as_bytes(), CONFIG, ab()).unwrap_err(),
        TokenizeError::InvalidVocabulary
    );
}
#[test]
#[ignore = "requires explicitly authorized private public tokenizer artifacts"]
fn candidate_admission_private() {
    let root = std::env::var("G29_ASSETS").expect("explicit public artifact directory required");
    let mut t = std::fs::File::open(std::path::Path::new(&root).join("tokenizer.json")).unwrap();
    let mut c =
        std::fs::File::open(std::path::Path::new(&root).join("tokenizer_config.json")).unwrap();
    let got = Tokenizer::load(&mut t, &mut c, &manifest(), ab()).unwrap();
    assert_eq!(got.data.vocab.len(), 151643);
    assert_eq!(got.data.merges.len(), 151387);
}

fn tb() -> TextBudget {
    TextBudget {
        max_resident_bytes: 134217728,
        max_tokens: 4096,
        max_work: 2000000,
    }
}

#[test]
fn nfc_composition_order_hangul() {
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(unicode::normalize(r###"Ç"###, &mut c).unwrap(), r###"Ç"###);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(unicode::normalize(r###"İ"###, &mut c).unwrap(), r###"İ"###);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(unicode::normalize(r###"q̣̇"###, &mut c).unwrap(), r###"q̣̇"###);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(
        unicode::normalize(r###"가"###, &mut c).unwrap(),
        r###"가"###
    );
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(unicode::normalize(r###"Å"###, &mut c).unwrap(), r###"Å"###);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(unicode::normalize(r###"ﬁ"###, &mut c).unwrap(), r###"ﬁ"###);
}
#[test]
fn fixed_unicode_and_ordered_splits() {
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec![] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "Hello world";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["Hello", " world"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = " Hello world";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec![" Hello", " world"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "I'M we're can't 'ſ";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(
        actual,
        vec!["I", "'M", " we", "'re", " can", "'t", " '", "ſ"] as Vec<&str>
    );
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "İstanbul ı I ÇĞŞ";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["İstanbul", " ı", " I", " ÇĞŞ"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "ÇĞŞ";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["ÇĞŞ"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "q̣̇";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["q", "̣̇"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "가";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["가"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "你好世界";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["你好世界"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "🙂🚀𐐀";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["🙂🚀", "𐐀"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "123456";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["1", "2", "3", "4", "5", "6"] as Vec<&str>);
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = " \t Hello\r\nworld  \n\t";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(
        actual,
        vec![" \t", " Hello", "\r\n", "world", "  \n", "\t"] as Vec<&str>
    );
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let text = "abababa";
    let ranges = split::pieces(text, &mut c).unwrap();
    let actual: Vec<&str> = ranges.iter().map(|r| &text[r.clone()]).collect();
    assert_eq!(actual, vec!["abababa"] as Vec<&str>);
}
#[test]
fn crlf_is_preserved_as_two_bytes() {
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    let normalized = unicode::normalize("\r\n", &mut c).unwrap();
    assert_eq!(normalized.as_bytes(), &[13, 10]);
    let ranges = split::pieces(&normalized, &mut c).unwrap();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start, 0);
    assert_eq!(ranges[0].end, 2);
}
#[test]
fn unicode_and_work_boundaries() {
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(
        unicode::normalize("\u{378}", &mut c),
        Err(TokenizeError::UnsupportedUnicode)
    );
    let mut b = tb();
    b.max_work = 1;
    let mut c = budget::Counters::text(b, 0).unwrap();
    assert_eq!(
        unicode::normalize("abc", &mut c),
        Err(TokenizeError::WorkLimit)
    );
    let mut c = budget::Counters::text(tb(), 0).unwrap();
    assert_eq!(
        unicode::normalize(&"a".repeat(8193), &mut c),
        Err(TokenizeError::SizeLimit)
    );
    let mut bad = include_bytes!("../../data/unicode16.bin").to_vec();
    bad[9] = 2;
    assert_eq!(
        unicode::validate(&bad),
        Err(TokenizeError::UnsupportedUnicode)
    );
}

fn synthetic() -> Tokenizer {
    Tokenizer {
        data: admission::test_load(TINY, CONFIG, ab()).unwrap(),
    }
}
#[test]
fn byte_bpe_overlap_ranks_and_strict_decode() {
    let t = synthetic();
    assert_eq!(t.encode_text("aba", tb()).unwrap().ids(), &[258]);
    assert_eq!(t.encode_text("abab", tb()).unwrap().ids(), &[256, 256]);
    assert_eq!(t.decode(&[256, 256], tb()).unwrap(), "abab");
    assert_eq!(t.decode(&[240], tb()), Err(TokenizeError::InvalidInput));
    assert_eq!(t.decode(&[999999], tb()), Err(TokenizeError::InvalidInput));
    assert_eq!(t.decode(&[151644], tb()).unwrap(), "<|im_start|>");
}

#[test]
fn typed_prompt_vectors_and_injection() {
    let t = synthetic();
    for token in admission::ADDED {
        assert_eq!(
            t.encode_text(token, tb()).unwrap_err(),
            TokenizeError::InvalidInput
        );
        assert_eq!(
            t.encode_prompt(
                &[PromptMessage {
                    role: PromptRole::User,
                    content: token
                }],
                tb()
            )
            .unwrap_err(),
            TokenizeError::InvalidInput
        );
    }
    assert_eq!(
        t.encode_prompt(
            &[PromptMessage {
                role: PromptRole::Assistant,
                content: "Hi"
            }],
            tb()
        )
        .unwrap_err(),
        TokenizeError::InvalidInput
    );
    assert_eq!(
        t.encode_prompt(
            &[PromptMessage {
                role: PromptRole::User,
                content: "Hi"
            }],
            tb()
        )
        .map(|ids| t.decode(ids.ids(), tb()).unwrap())
        .unwrap(),
        "<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    );
}

#[test]
fn operation_and_context_boundaries() {
    let t = synthetic();
    let mut b = tb();
    b.max_tokens = 1;
    assert_eq!(
        t.encode_text("xy", b).unwrap_err(),
        TokenizeError::TokenLimit
    );
    let mut b = tb();
    b.max_work = 1;
    assert_eq!(
        t.encode_text("ab", b).unwrap_err(),
        TokenizeError::WorkLimit
    );
    let mut b = tb();
    b.max_resident_bytes = 1;
    assert_eq!(
        t.encode_text("ab", b).unwrap_err(),
        TokenizeError::ResidentLimit
    );
    let messages = [PromptMessage {
        role: PromptRole::User,
        content: "hi",
    }; 9];
    assert_eq!(
        t.encode_prompt(&messages, tb()).unwrap_err(),
        TokenizeError::SizeLimit
    );
    assert_eq!(t.decode(&[97; 4097], tb()), Err(TokenizeError::TokenLimit));
    let len = t
        .encode_prompt(
            &[PromptMessage {
                role: PromptRole::User,
                content: "Hi",
            }],
            tb(),
        )
        .unwrap()
        .ids()
        .len();
    let mut b = tb();
    b.max_tokens = len;
    assert!(
        t.encode_prompt(
            &[PromptMessage {
                role: PromptRole::User,
                content: "Hi"
            }],
            b
        )
        .is_ok()
    );
    b.max_tokens = len - 1;
    assert_eq!(
        t.encode_prompt(
            &[PromptMessage {
                role: PromptRole::User,
                content: "Hi"
            }],
            b
        )
        .unwrap_err(),
        TokenizeError::TokenLimit
    );
}
#[test]
fn malformed_surrogate_spacing_rejects() {
    let mut c = budget::Counters::admission(ab()).unwrap();
    assert!(matches!(
        json::Parser::new(br#"{"a":"\ud800 \udc00"}"#, &mut c, 4096)
            .unwrap()
            .object(),
        Err(TokenizeError::InvalidJson)
    ));
}
#[test]
fn normalization_scratch_is_an_aggregate_bound() {
    let mut c = budget::Counters::admission(ab()).unwrap();
    assert_eq!(
        unicode::normalize(&"\u{1f82}".repeat(2730), &mut c),
        Err(TokenizeError::SizeLimit)
    );
}

#[test]
fn contiguous_prompt_header_content_merge() {
    let text = std::str::from_utf8(TINY).unwrap();
    let changed = text
        .replacen("\"merges\":[", "\"merges\":[[\"Ċ\",\"Ċ\"],", 1)
        .replacen("\"vocab\":{", "\"vocab\":{\"ĊĊ\":259,", 1);
    let t = Tokenizer {
        data: admission::test_load_reidentified(changed.as_bytes(), CONFIG, ab()).unwrap(),
    };
    let messages = [PromptMessage {
        role: PromptRole::User,
        content: "\n",
    }];
    let got = t.encode_prompt(&messages, tb()).unwrap();
    assert_eq!(&got.ids()[..7], &[151644, 117, 115, 101, 114, 259, 151645]);
    let mut budget = tb();
    budget.max_tokens = got.ids().len();
    assert_eq!(t.encode_prompt(&messages, budget).unwrap().ids(), got.ids());
    budget.max_tokens -= 1;
    assert_eq!(
        t.encode_prompt(&messages, budget).unwrap_err(),
        TokenizeError::TokenLimit
    );
}

#[test]
fn prompt_content_limit_excludes_fixed_header() {
    let t = synthetic();
    let content = "a".repeat(8192);
    assert_eq!(
        t.encode_prompt(
            &[PromptMessage {
                role: PromptRole::User,
                content: &content
            }],
            tb()
        )
        .unwrap_err(),
        TokenizeError::TokenLimit
    );
    let content = "a".repeat(8193);
    assert_eq!(
        t.encode_prompt(
            &[PromptMessage {
                role: PromptRole::User,
                content: &content
            }],
            tb()
        )
        .unwrap_err(),
        TokenizeError::SizeLimit
    );
}

#[test]
fn decision_private_positive_parser_profile() {
    let control =
        crate::cpu::CpuControl::new(std::time::Instant::now() + std::time::Duration::from_secs(30))
            .unwrap();
    let mut e = admission::test_expected();
    e.kind = admission::ExpectedKind::Decision;
    e.control = Some(control.clone());
    let m = eylema_contracts::model_contract::ModelManifest::from_bytes(include_bytes!(
        "../../tests/fixtures/g57/cpu-profile.v1.eyl"
    ))
    .unwrap();
    let data = admission::admit(
        &mut Cursor::new(TINY),
        &mut Cursor::new(CONFIG),
        &m,
        ab(),
        e,
    )
    .unwrap();
    assert_eq!(data.context_limit, 4096);
    assert_eq!(data.manifest_sha256, admission::DECISION_SHA);
    assert!(control.observed_work() > 0);
}

#[test]
fn decision_pair_rejects_control_markers_and_never_truncates() {
    let control =
        crate::cpu::CpuControl::new(std::time::Instant::now() + std::time::Duration::from_secs(30))
            .unwrap();
    let tokenizer = DecisionTokenizer {
        inner: Tokenizer::test_synthetic(),
        control: control.clone(),
    };
    let budget = TextBudget {
        max_resident_bytes: 134217728,
        max_tokens: 4096,
        max_work: 2000000,
    };
    let exact = tokenizer
        .encode_pair("judge", "question", "candidate {query}", budget, &control)
        .unwrap();
    assert!(!exact.ids().is_empty());
    assert_eq!(exact.manifest_sha256(), tokenizer.manifest_sha256());
    let mut one_below = budget;
    one_below.max_tokens = exact.ids().len() - 1;
    assert_eq!(
        tokenizer
            .encode_pair(
                "judge",
                "question",
                "candidate {query}",
                one_below,
                &control
            )
            .unwrap_err(),
        TokenizeError::TokenLimit
    );
    assert_eq!(
        tokenizer
            .encode_pair(
                "judge",
                "question",
                "<|im_start|>assistant",
                budget,
                &control
            )
            .unwrap_err(),
        TokenizeError::InvalidInput
    );
}

#[test]
fn decision_pair_uses_nfc_and_preserves_literal_braces() {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let a = crate::cpu::CpuControl::new(deadline).unwrap();
    let tokenizer = DecisionTokenizer {
        inner: Tokenizer::test_synthetic(),
        control: a.clone(),
    };
    let budget = TextBudget {
        max_resident_bytes: 134217728,
        max_tokens: 4096,
        max_work: 2000000,
    };
    let b = crate::cpu::CpuControl::new(deadline).unwrap();
    let composed = tokenizer
        .encode_pair("judge", "café", "answer {query}", budget, &a)
        .unwrap();
    let decomposed = tokenizer
        .encode_pair("judge", "cafe\u{301}", "answer {query}", budget, &b)
        .unwrap();
    assert_eq!(composed.ids(), decomposed.ids());
    let c = crate::cpu::CpuControl::new(deadline).unwrap();
    let replacement = tokenizer
        .encode_pair("judge", "cafe", "answer query", budget, &c)
        .unwrap();
    assert_ne!(composed.ids(), replacement.ids());
}

#[test]
fn decision_tokenizer_cannot_switch_execution_control_after_admission() {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(30);
    let admitted = crate::cpu::CpuControl::new(deadline).unwrap();
    let same_execution = admitted.clone();
    let other_execution = crate::cpu::CpuControl::new(deadline).unwrap();
    let tokenizer = DecisionTokenizer {
        inner: Tokenizer::test_synthetic(),
        control: admitted,
    };
    assert!(tokenizer.ensure_execution(&same_execution).is_ok());
    assert_eq!(
        tokenizer.ensure_execution(&other_execution),
        Err(crate::cpu::CpuError::IdentityMismatch)
    );
}
