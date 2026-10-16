use eylema_retrieval::{chunk::*, *};
use std::time::{Duration, Instant};
fn c() -> Control {
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
// Catches byte truncation and normalizing before finding original word boundaries.
#[test]
fn utf8_windows_cover_exact_bytes() {
    let s = "a".repeat(1023) + "é";
    assert_eq!(
        ranges(s.as_bytes(), &mut c()).unwrap(),
        vec![0..1023, 1023..1025]
    );
    let s = "a".repeat(1022) + "é";
    assert_eq!(ranges(s.as_bytes(), &mut c()).unwrap(), vec![0..1024]);
    for b in [vec![b'a'; 8193], vec![255], vec![0], vec![]] {
        assert!(ranges(&b, &mut c()).is_err());
    }
}
#[test]
fn lexical_expansion_and_caps() {
    assert_eq!(
        terms("İSTANBUL", 32, &mut c()).unwrap(),
        vec!["i\u{307}stanbul"]
    );
    assert_eq!(
        terms("CAFÉ cafe\u{301}", 32, &mut c()).unwrap(),
        vec!["cafe", "café"]
    );
    assert_eq!(terms("red red", 32, &mut c()).unwrap(), vec!["red"]);
    assert!(terms(&"a".repeat(65), 32, &mut c()).is_err());
    let q = (0..33).map(|i| format!("w{i} ")).collect::<String>();
    assert!(terms(&q, 32, &mut c()).is_err());
    assert!(terms("", 32, &mut c()).is_err());
}
