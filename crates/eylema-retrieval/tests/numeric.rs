use eylema_retrieval::{score::*, *};
use std::time::{Duration, Instant};
fn control() -> Control {
    Control::new(
        RetrievalBudget {
            max_work: 100_000_000,
            max_live_bytes: 64 * 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(30),
        },
        CancelToken::new(),
    )
    .unwrap()
}
// Catches half-up, sign and implicit normalization substitutions.
#[test]
fn frozen_scalar_and_half_even_vectors() {
    let cases = [
        (0x3f800000, 0, 1., 1_000_000),
        (0, 0x3f800000, 1., 0),
        (0xbf800000, 0, 1., -1_000_000),
        (0x3f19999a, 0x3f4ccccd, 1.000000023841858, 600_000),
        (0x3c000000, 0x3f7ffe00, 1.0000000004656613, 7812),
        (0x3cc00000, 0x3f7fedff, 0.999999978130292, 23438),
        (0xbc000000, 0x3f7ffe00, 1.0000000004656613, -7812),
        (0xbcc00000, 0x3f7fedff, 0.999999978130292, -23438),
    ];
    for d in [2, 1024, 4096] {
        for (a, b, n, m) in cases {
            let mut v = vec![0.; d];
            v[0] = f32::from_bits(a);
            v[1] = f32::from_bits(b);
            let mut q = vec![0.; d];
            q[0] = 1.;
            assert!((unit_norm(&v, d, &mut control()).unwrap() - n).abs() <= 2e-12);
            assert_eq!(vector_micro(&q, &v, d, &mut control()).unwrap(), m);
        }
    }
    assert_eq!(
        vector_micro(&[1.], &[1.], 1, &mut control()).unwrap(),
        1_000_000
    );
    for v in [vec![0.], vec![f32::NAN], vec![f32::INFINITY], vec![2.]] {
        assert_eq!(
            unit_norm(&v, 1, &mut control()),
            Err(RetrievalError::InvalidVector)
        );
    }
    assert_eq!(
        unit_norm(&[1.], 0, &mut control()),
        Err(RetrievalError::InvalidVector)
    );
    assert_eq!(
        unit_norm(&vec![0.; 4097], 4097, &mut control()),
        Err(RetrievalError::InvalidVector)
    );
}
#[test]
fn rational_lexical_and_signed_hybrid() {
    let all: Vec<_> = (0..128).map(|i| format!("{i:03}")).collect();
    assert_eq!(
        jaccard_micro(&all[..1], &all, &mut control()).unwrap(),
        7812
    );
    assert_eq!(
        jaccard_micro(&all[..3], &all, &mut control()).unwrap(),
        23438
    );
    for (v, l, w, want) in [
        (0, 333333, 500000, 166666),
        (3, 0, 500000, 2),
        (-3, 0, 500000, -2),
        (1, 0, 500000, 0),
        (-1, 0, 500000, 0),
    ] {
        assert_eq!(hybrid_micro(v, l, w).unwrap(), want);
    }
    for w in [0, 1_000_000] {
        assert!(hybrid_micro(0, 0, w).is_err());
    }
    assert!(hybrid_micro(i64::MAX, 0, 1).is_err());
}
#[test]
fn controls_reject_before_work() {
    let token = CancelToken::new();
    token.cancel();
    assert!(matches!(
        Control::new(
            RetrievalBudget {
                max_work: 1,
                max_live_bytes: 1,
                deadline: Instant::now() + Duration::from_secs(1)
            },
            token
        ),
        Err(RetrievalError::Cancelled)
    ));
    let mut c = Control::new(
        RetrievalBudget {
            max_work: 1,
            max_live_bytes: 1,
            deadline: Instant::now() + Duration::from_secs(1),
        },
        CancelToken::new(),
    )
    .unwrap();
    assert_eq!(
        unit_norm(&[1.; 4096], 4096, &mut c),
        Err(RetrievalError::LimitExceeded)
    );
    assert_eq!(c.work_used(), 0);
    assert!(matches!(
        Control::new(
            RetrievalBudget {
                max_work: 1,
                max_live_bytes: 1,
                deadline: Instant::now()
            },
            CancelToken::new()
        ),
        Err(RetrievalError::Deadline)
    ));
}
