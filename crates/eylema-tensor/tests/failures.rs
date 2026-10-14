use eylema_tensor::{Layout, Tensor, TensorError, validate_layout};

#[test]
fn tensor_constructor_enforces_shape_and_finite_values() {
    let tensor = Tensor::new(&[2, 2], &[1., 2., 3., 4.]).unwrap();
    assert_eq!(tensor.shape(), &[2, 2]);
    assert_eq!(tensor.values(), &[1., 2., 3., 4.]);
    for shape in [&[][..], &[1, 1, 1, 1, 1], &[0, 2]] {
        assert_eq!(
            Tensor::new(shape, &[]).unwrap_err(),
            TensorError::InvalidShape
        );
    }
    assert_eq!(
        Tensor::new(&[usize::MAX, 2], &[]).unwrap_err(),
        TensorError::ExtentOverflow
    );
    assert_eq!(
        Tensor::new(&[4097], &[]).unwrap_err(),
        TensorError::ExtentLimit
    );
    assert_eq!(
        Tensor::new(&[2], &[1.]).unwrap_err(),
        TensorError::InvalidShape
    );
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            Tensor::new(&[1], &[value]).unwrap_err(),
            TensorError::NonFinite
        );
    }
    let boundary = Tensor::new(&[4096], &vec![0.; 4096]).unwrap();
    assert_eq!(boundary.values().len(), 4096);
}

#[test]
fn tensor_layout_rejects_unsupported_precision() {
    assert_eq!(validate_layout(Layout::F32RowMajor), Ok(()));
    for layout in [Layout::F16RowMajor, Layout::BF16RowMajor, Layout::Quantized] {
        assert_eq!(validate_layout(layout), Err(TensorError::UnsupportedLayout));
    }
}
