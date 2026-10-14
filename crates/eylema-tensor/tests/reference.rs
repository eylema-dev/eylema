use eylema_tensor::{
    Tensor, embedding, linear, reshape, residual_add, rms_norm, silu, swiglu, tied_logits,
    transpose,
};

fn tensor(shape: &[usize], values: &[f32]) -> Tensor {
    Tensor::new(shape, values).unwrap()
}
fn close(actual: f32, expected: f64) -> bool {
    let error = (f64::from(actual) - expected).abs();
    actual.is_finite()
        && expected.is_finite()
        && (error <= 3e-5 || (expected != 0. && error / expected.abs() <= 3e-5))
}
fn values(actual: &Tensor, shape: &[usize], expected: &[f64]) {
    assert_eq!(actual.shape(), shape);
    assert_eq!(actual.values().len(), expected.len());
    for (&a, &e) in actual.values().iter().zip(expected) {
        assert!(close(a, e), "{a} != {e}");
    }
}

#[test]
fn basic_embedding_projection_and_tied_orientation() {
    let table = tensor(&[3, 2], &[1., 2., 3., 5., 7., 11.]);
    values(
        &embedding(&table, &[2, 0]).unwrap(),
        &[2, 2],
        &[7., 11., 1., 2.],
    );
    let input = tensor(&[2, 3], &[1., 2., 3., 4., 5., 6.]);
    let weight = tensor(&[2, 3], &[1., 0., 2., 0., 3., 1.]);
    values(
        &linear(&input, &weight).unwrap(),
        &[2, 2],
        &[7., 9., 16., 21.],
    );
    values(
        &tied_logits(&tensor(&[1, 2], &[1., 2.]), &table).unwrap(),
        &[1, 3],
        &[5., 13., 29.],
    );
}

#[test]
fn basic_normalization_activation_and_gated_down_projection() {
    values(
        &rms_norm(&tensor(&[1, 2], &[3., 4.]), &tensor(&[2], &[1., 1.]), 0.5).unwrap(),
        &[1, 2],
        &[0.8320502943378437, 1.1094003924504583],
    );
    values(
        &silu(&tensor(&[3], &[0., 1., -1.])).unwrap(),
        &[3],
        &[0., 0.7310585786300049, -0.2689414213699951],
    );
    let extreme = silu(&tensor(&[2], &[80., -80.])).unwrap();
    assert!(extreme.values().iter().all(|v| v.is_finite()));
    assert!(close(extreme.values()[0], 80.));
    let input = tensor(&[1, 2], &[1., 2.]);
    let gate = tensor(&[2, 2], &[1., 0., 0., 1.]);
    let up = tensor(&[2, 2], &[2., 0., 0., 3.]);
    let down = tensor(&[1, 2], &[1., 1.]);
    values(
        &swiglu(&input, &gate, &up, &down).unwrap(),
        &[1, 1],
        &[12.031682092994599],
    );
}

#[test]
fn basic_residual_materialized_reshape_and_distinct_axis_transpose() {
    values(
        &residual_add(&tensor(&[2], &[1., 2.]), &tensor(&[2], &[3., 5.])).unwrap(),
        &[2],
        &[4., 7.],
    );
    let source = tensor(&[2, 2, 3], &(0..12).map(|v| v as f32).collect::<Vec<_>>());
    values(
        &transpose(&source, &[2, 0, 1]).unwrap(),
        &[3, 2, 2],
        &[0., 3., 6., 9., 1., 4., 7., 10., 2., 5., 8., 11.],
    );
    values(
        &reshape(&source, &[3, 4]).unwrap(),
        &[3, 4],
        &[0., 1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11.],
    );
    assert_eq!(source.shape(), &[2, 2, 3]);
}
