use eylema_contracts::{
    model_contract::{DigestSource, ModelManifest},
    platform::ModelId,
};
use eylema_model::{StoredDtype, TensorSpec, WeightLayout};
pub fn hex32(s: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
}
pub const F32_H: &str = r#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,8]},"b":{"dtype":"F32","shape":[1,2],"data_offsets":[8,16]}}"#;
pub const BF16_H: &str = r#"{"a":{"dtype":"BF16","shape":[2],"data_offsets":[0,4]},"b":{"dtype":"BF16","shape":[1,2],"data_offsets":[4,8]}}"#;
pub fn digest(dt: StoredDtype) -> [u8; 32] {
    hex32(if dt == StoredDtype::F32 {
        "11d2df05593bc9528b04010cc008fe018fc8f97142b3694be97e3981fb6bfcdb"
    } else {
        "56b3dc53f2a738710ebf7a70d2af4a80eea24820004915fe823015be0f06e35d"
    })
}
pub fn fixture(dt: StoredDtype) -> Vec<u8> {
    let (h, p): (&str, &[u8]) = if dt == StoredDtype::F32 {
        (
            F32_H,
            &[0, 0, 128, 63, 0, 0, 0, 192, 0, 0, 64, 64, 0, 0, 128, 64],
        )
    } else {
        (BF16_H, &[128, 63, 0, 192, 64, 64, 128, 64])
    };
    let mut f = (h.len() as u64).to_le_bytes().to_vec();
    f.extend_from_slice(h.as_bytes());
    f.extend_from_slice(p);
    f
}
pub fn manifest(dt: StoredDtype, size: u64, hash: [u8; 32]) -> ModelManifest {
    let m = ModelManifest::from_bytes(include_bytes!(
        "../../../eylema-contracts/tests/fixtures/qwen3-profile.v1.eyl"
    ))
    .unwrap();
    let mut d = m.fields().clone();
    d.id = ModelId::new("synthetic-g28").unwrap();
    d.architecture = "synthetic-weight-layout.v1".into();
    d.source_repository = "https://github.com/eylema-dev/eylema-models".into();
    d.source_commit = [0x77; 20];
    d.numeric_format = if dt == StoredDtype::F32 {
        "float32"
    } else {
        "bfloat16"
    }
    .into();
    d.rights.publisher = "eylema-dev".into();
    d.rights.source_repository = d.source_repository.clone();
    d.rights.source_commit = d.source_commit;
    let c = d
        .components
        .iter_mut()
        .find(|x| x.name == d.configuration.weights_component)
        .unwrap();
    c.size = size;
    c.sha256 = hash;
    c.digest_source = DigestSource::AuditMeasuredSmallBytes;
    ModelManifest::new(d).unwrap()
}
pub fn layout(m: &ModelManifest) -> WeightLayout {
    let dt = if m.fields().numeric_format == "float32" {
        StoredDtype::F32
    } else {
        StoredDtype::BF16
    };
    WeightLayout::new(
        m,
        &[
            TensorSpec::new("a", dt, &[2]).unwrap(),
            TensorSpec::new("b", dt, &[1, 2]).unwrap(),
        ],
    )
    .unwrap()
}
