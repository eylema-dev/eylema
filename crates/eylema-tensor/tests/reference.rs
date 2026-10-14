use eylema_tensor::{
    Tensor, embedding, linear, reshape, residual_add, rms_norm, silu, swiglu, tied_logits,
    transpose,
};

use eylema_tensor::{
    AttentionMask, KvCache, RopeSpec, attention, causal_mask, greedy, repeat_kv, rope, softmax,
};

fn tensor(shape: &[usize], values: &[f32]) -> Tensor {
    Tensor::new(shape, values).unwrap()
}

#[test]
fn attention_rope_split_half_frequencies_and_grouped_head_order() {
    let input = tensor(&[1, 1, 1, 4], &[1., 2., 3., 4.]);
    values(
        &rope(
            &input,
            &RopeSpec {
                positions: vec![0],
                theta: 10000.,
                rotary_dim: 4,
            },
        )
        .unwrap(),
        &[1, 1, 1, 4],
        &[1., 2., 3., 4.],
    );
    values(
        &rope(
            &input,
            &RopeSpec {
                positions: vec![1],
                theta: 10000.,
                rotary_dim: 4,
            },
        )
        .unwrap(),
        &[1, 1, 1, 4],
        &[
            -1.9841106485555495,
            1.959900667496664,
            2.4623779024123156,
            4.019799668334994,
        ],
    );
    values(
        &rope(
            &input,
            &RopeSpec {
                positions: vec![1],
                theta: 1000000.,
                rotary_dim: 4,
            },
        )
        .unwrap(),
        &[1, 1, 1, 4],
        &[
            -1.9841106485555495,
            1.99599900066675,
            2.4623779024123156,
            4.001997999666833,
        ],
    );
    values(
        &repeat_kv(&tensor(&[1, 2, 2, 1], &[1., 2., 3., 4.]), 4).unwrap(),
        &[1, 4, 2, 1],
        &[1., 2., 1., 2., 3., 4., 3., 4.],
    );
}

#[test]
fn attention_typed_masks_stable_softmax_and_value_axes() {
    let mask = causal_mask(1, 1, &[1, 2], &[0, 1, 2], &[true, false, true]).unwrap();
    assert_eq!(mask.shape(), &[1, 1, 2, 3]);
    assert_eq!(mask.allowed(), &[true, false, false, true, false, true]);
    values(
        &softmax(&tensor(&[2], &[1001., 1002.]), &[true, true]).unwrap(),
        &[2],
        &[0.2689414213699951, 0.7310585786300049],
    );
    values(
        &softmax(&tensor(&[2], &[1001., 1002.]), &[true, false]).unwrap(),
        &[2],
        &[1., 0.],
    );
    let q = tensor(&[1, 1, 2, 2], &[1., 1., 1., 1.]);
    let k = tensor(&[1, 1, 2, 2], &[1., 0., 0., 1.]);
    let v = tensor(&[1, 1, 2, 2], &[2., 4., 6., 8.]);
    let all = AttentionMask::new([1, 1, 2, 2], &[true; 4]).unwrap();
    values(
        &attention(&q, &k, &v, &all, 1.).unwrap(),
        &[1, 1, 2, 2],
        &[4., 6., 4., 6.],
    );
    let causal = causal_mask(1, 1, &[0, 1], &[0, 1], &[true, true]).unwrap();
    values(
        &attention(&q, &k, &v, &causal, 1.).unwrap(),
        &[1, 1, 2, 2],
        &[2., 4., 4., 6.],
    );
    let q2 = tensor(&[1, 2, 1, 1], &[0., 0.]);
    let k2 = tensor(&[1, 2, 2, 1], &[1., 3., 5., 7.]);
    let v2 = tensor(&[1, 2, 2, 1], &[2., 4., 10., 14.]);
    values(
        &attention(
            &q2,
            &k2,
            &v2,
            &AttentionMask::new([1, 2, 1, 2], &[true; 4]).unwrap(),
            1.,
        )
        .unwrap(),
        &[1, 2, 1, 1],
        &[3., 12.],
    );
}

#[test]
fn attention_nonuniform_scores_and_scale_across_distinct_batches() {
    // Independent Decimal80 expectations; catches zeroed QK scores or ignored scale.
    let q = tensor(&[2, 1, 2, 1], &[1., 2., -1., 1.]);
    let k = tensor(&[2, 1, 2, 1], &[0., 2., 1., 3.]);
    let v = tensor(&[2, 1, 2, 1], &[2., 6., 10., 20.]);
    let mask = AttentionMask::new([2, 1, 2, 2], &[true; 8]).unwrap();
    values(
        &attention(&q, &k, &v, &mask, 0.5).unwrap(),
        &[2, 1, 2, 1],
        &[
            4.9242343145200195,
            5.52318831191153,
            12.689414213699951,
            17.31058578630005,
        ],
    );
    values(
        &attention(&q, &k, &v, &mask, 1.).unwrap(),
        &[2, 1, 2, 1],
        &[
            5.52318831191153,
            5.928055160151634,
            11.192029220221176,
            18.807970779778824,
        ],
    );
}

#[test]
fn attention_cache_appends_tokens_per_head_and_greedy_uses_original_ids() {
    let empty = KvCache::empty(2).unwrap();
    let first = empty
        .append(
            &tensor(&[1, 2, 1, 1], &[1., 10.]),
            &tensor(&[1, 2, 1, 2], &[2., 3., 20., 30.]),
        )
        .unwrap();
    let second = first
        .append(
            &tensor(&[1, 2, 1, 1], &[4., 40.]),
            &tensor(&[1, 2, 1, 2], &[5., 6., 50., 60.]),
        )
        .unwrap();
    values(second.key().unwrap(), &[1, 2, 2, 1], &[1., 4., 10., 40.]);
    values(
        second.value().unwrap(),
        &[1, 2, 2, 2],
        &[2., 3., 5., 6., 20., 30., 50., 60.],
    );
    assert_eq!(first.len(), 1);
    assert_eq!(empty.len(), 0);
    assert_eq!(greedy(&[5., 5., 1.], &[9, 3, 1]), Ok(3));
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
