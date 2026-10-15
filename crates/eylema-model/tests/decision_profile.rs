use eylema_contracts::model_contract::ModelManifest;
use eylema_model::{
    cpu::CpuControl,
    tokenizer::{AdmissionBudget, DecisionTokenizer, TokenizeError},
};
use std::{
    io::Read,
    time::{Duration, Instant},
};
fn manifest() -> ModelManifest {
    ModelManifest::from_bytes(include_bytes!("fixtures/g57/cpu-profile.v1.eyl")).unwrap()
}
struct Never;
impl Read for Never {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("premature reader")
    }
}
fn budget() -> AdmissionBudget {
    AdmissionBudget {
        max_resident_bytes: 134217728,
        max_read_calls: 1024,
        max_interruptions: 16,
        max_work_per_artifact: 134217728,
    }
}
#[test]
fn decision_profile_roundtrip_and_closed_identity() {
    let m = manifest();
    assert_eq!(
        m.to_bytes().unwrap(),
        include_bytes!("fixtures/g57/cpu-profile.v1.eyl")
    );
    assert_eq!(
        m.fields()
            .configuration
            .dimensions
            .iter()
            .find(|(n, _)| n == "vocab_size")
            .unwrap()
            .1,
        151669
    );
    assert_eq!(m.fields().configuration.context_limit, 4096);
}
#[test]
fn decision_tokenizer_profile_isolation() {
    let c = CpuControl::new(Instant::now() + Duration::from_secs(30)).unwrap();
    let mut b = budget();
    b.max_work_per_artifact = 1;
    assert_eq!(
        DecisionTokenizer::load(&mut Never, &mut Never, &manifest(), b, &c).unwrap_err(),
        TokenizeError::WorkLimit
    );
    let old = ModelManifest::from_bytes(include_bytes!(
        "../../eylema-contracts/tests/fixtures/qwen3-profile.v1.eyl"
    ))
    .unwrap();
    assert_eq!(
        DecisionTokenizer::load(&mut Never, &mut Never, &old, budget(), &c).unwrap_err(),
        TokenizeError::UnsupportedProfile
    );
}
