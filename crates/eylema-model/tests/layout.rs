mod support;
use eylema_contracts::model_contract::ModelManifest;
use eylema_model::*;
use support::*;
#[test]
fn independent_manifest_and_layout_identities() {
    for (dt, n, mh, lh) in [
        (
            StoredDtype::F32,
            134,
            "a07a425b526f80c9ddc442b2371f879e4d48776ce59a423e6d275e1172d578e4",
            "3cd159cb41fd1ce2b69f116d0d57f7b76beaf2765c9b8e641eb0a4b1fbef78ce",
        ),
        (
            StoredDtype::BF16,
            127,
            "26715b9716edea1aee238978fe454302e1bd045d56f26c766563b8052e42e162",
            "1e1ce7a52f068ede6f7b9adee6a049c2d33fa80fb1b8dfe27d9d8f4be1d05883",
        ),
    ] {
        let m = manifest(dt, fixture(dt).len() as u64, digest(dt));
        assert_eq!(fixture(dt).len() as u64, n);
        let l = layout(&m);
        assert_eq!(l.manifest_sha256(), hex32(mh));
        assert_eq!(l.layout_sha256(), hex32(lh));
        assert_eq!(l.tensors()[0].shape(), &[2]);
        assert_eq!(l.tensors()[1].name(), "b");
    }
}
#[test]
fn shape_name_and_extent_rejection() {
    for name in ["", "_hidden", "with space", "é"] {
        assert!(TensorSpec::new(name, StoredDtype::F32, &[1]).is_err());
    }
    for shape in [&[][..], &[0][..], &[1, 1, 1, 1, 1][..]] {
        assert_eq!(
            TensorSpec::new("a", StoredDtype::F32, shape).unwrap_err(),
            LoadError::InvalidLayout
        );
    }
    assert_eq!(
        TensorSpec::new("a", StoredDtype::F32, &[u64::MAX, 2]).unwrap_err(),
        LoadError::SizeLimit
    );
    assert!(TensorSpec::new(&"a".repeat(129), StoredDtype::F32, &[1]).is_err());
}
#[test]
fn duplicate_mixed_and_unsupported_layouts_reject() {
    let m = manifest(StoredDtype::F32, 134, digest(StoredDtype::F32));
    let a = TensorSpec::new("a", StoredDtype::F32, &[2]).unwrap();
    assert!(
        WeightLayout::new(
            &m,
            &[
                TensorSpec::new("a", StoredDtype::F32, &[2]).unwrap(),
                TensorSpec::new("a", StoredDtype::F32, &[2]).unwrap()
            ]
        )
        .is_err()
    );
    assert!(
        WeightLayout::new(
            &m,
            &(0..1025)
                .map(|_| TensorSpec::new("a", StoredDtype::F32, &[2]).unwrap())
                .collect::<Vec<_>>()
        )
        .is_err()
    );
    assert!(WeightLayout::new(&m, &[]).is_err());
    assert!(
        WeightLayout::new(
            &m,
            &[a, TensorSpec::new("b", StoredDtype::BF16, &[1]).unwrap()]
        )
        .is_err()
    );
    for (fmt, q) in [("pickle", "none"), ("safetensors.v1", "int4")] {
        let mut d = m.fields().clone();
        d.artifact_format = fmt.into();
        d.quantization = q.into();
        assert!(WeightLayout::new(&ModelManifest::new(d).unwrap(), layout(&m).tensors()).is_err());
    }
}
