use eylema_contracts::model_contract::ModelManifest;
use eylema_model::tokenizer::{AdmissionBudget, EmbeddingTokenizer, TokenizeError};
use std::io::Read;

struct Never;
impl Read for Never {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("profile mismatch must reject before artifact access")
    }
}

fn budget() -> AdmissionBudget {
    AdmissionBudget {
        max_resident_bytes: 134_217_728,
        max_read_calls: 1024,
        max_interruptions: 16,
        max_work_per_artifact: 134_217_728,
    }
}

#[test]
fn embedding_tokenizer_rejects_decision_and_qwen_manifests_before_reads() {
    for bytes in [
        include_bytes!("fixtures/g30/cpu-profile.v1.eyl").as_slice(),
        include_bytes!("fixtures/g57/cpu-profile.v1.eyl").as_slice(),
    ] {
        let manifest = ModelManifest::from_bytes(bytes).unwrap();
        assert_eq!(
            EmbeddingTokenizer::load(&mut Never, &mut Never, &manifest, budget()).unwrap_err(),
            TokenizeError::UnsupportedProfile
        );
    }
}

#[test]
fn embedding_profile_manifest_is_byte_identical_to_g67_catalog_fixture() {
    let bytes = include_bytes!("fixtures/g68/embedding-profile.v1.eyl");
    let manifest = ModelManifest::from_bytes(bytes).unwrap();
    assert_eq!(manifest.to_bytes().unwrap(), bytes);
    assert_eq!(manifest.fields().id.as_str(), "qwen3-embedding-0.6b");
}
