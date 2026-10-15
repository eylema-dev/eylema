use eylema_contracts::model_contract::ModelManifest;

#[test]
fn native_cpu_profile_roundtrips_exactly() {
    let bytes = include_bytes!("fixtures/g30/cpu-profile.v1.eyl");
    let manifest = ModelManifest::from_bytes(bytes).expect("closed CPU manifest must decode");
    let fields = manifest.fields();
    assert_eq!(fields.id.as_str(), "qwen3-0.6b.cpu");
    assert_eq!(fields.revision.get(), 2);
    assert_eq!(fields.profiles.len(), 4);
    assert_eq!(manifest.to_bytes().expect("canonical encode"), bytes);
    assert!(fields.profiles.iter().all(|profile| profile.action_semantics.as_str() == "proposal-only.v1"));
}
