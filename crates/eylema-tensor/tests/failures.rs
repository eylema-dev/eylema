use eylema_tensor::{Layout, Tensor, TensorError, validate_layout};

use eylema_tensor::{
    embedding, linear, reshape, residual_add, rms_norm, swiglu, tied_logits, transpose,
};

#[test]
fn basic_operators_reject_shapes_indices_parameters_and_intermediate_overflow() {
    let one = Tensor::new(&[1, 1], &[1.]).unwrap();
    let huge = Tensor::new(&[1, 1], &[f32::MAX]).unwrap();
    assert_eq!(
        embedding(&one, &[1]).unwrap_err(),
        TensorError::IndexOutOfBounds
    );
    assert_eq!(embedding(&one, &[]).unwrap_err(), TensorError::InvalidShape);
    assert_eq!(
        embedding(&one, &[0; 17]).unwrap_err(),
        TensorError::ExtentLimit
    );
    assert_eq!(
        linear(&one, &Tensor::new(&[1, 2], &[1., 2.]).unwrap()).unwrap_err(),
        TensorError::InvalidShape
    );
    assert_eq!(
        linear(&huge, &Tensor::new(&[1, 1], &[2.]).unwrap()).unwrap_err(),
        TensorError::NonFinite
    );
    assert_eq!(
        tied_logits(&one, &Tensor::new(&[65, 1], &[1.; 65]).unwrap()).unwrap_err(),
        TensorError::ExtentLimit
    );
    for epsilon in [0., -1., f32::NAN, f32::INFINITY] {
        assert_eq!(
            rms_norm(&one, &Tensor::new(&[1], &[1.]).unwrap(), epsilon).unwrap_err(),
            TensorError::InvalidParameter
        );
    }
    assert_eq!(
        rms_norm(&huge, &Tensor::new(&[1], &[1.]).unwrap(), 1e-6).unwrap_err(),
        TensorError::NonFinite
    );
    assert_eq!(
        residual_add(&huge, &huge).unwrap_err(),
        TensorError::NonFinite
    );
    assert_eq!(reshape(&one, &[2]).unwrap_err(), TensorError::InvalidShape);
    assert_eq!(
        transpose(&one, &[0, 0]).unwrap_err(),
        TensorError::InvalidParameter
    );
    assert_eq!(
        transpose(&one, &[0, 2]).unwrap_err(),
        TensorError::InvalidParameter
    );
    assert_eq!(one.values(), &[1.]);
}

#[test]
fn basic_swiglu_checks_entire_work_before_projection() {
    let matrix = Tensor::new(&[64, 64], &vec![0.; 4096]).unwrap();
    assert_eq!(
        swiglu(&matrix, &matrix, &matrix, &matrix).unwrap_err(),
        TensorError::WorkLimit
    );
}

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
