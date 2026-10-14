use eylema_platform::{PlatformError, sha256, sha256_stream::Sha256Stream};

fn hex32(s: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
}

#[test]
fn known_empty_and_abc_vectors() {
    assert_eq!(
        Sha256Stream::new(1).unwrap().finish().unwrap(),
        hex32("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    );
    let mut h = Sha256Stream::new(3).unwrap();
    h.update(b"a").unwrap();
    h.update(b"").unwrap();
    h.update(b"bc").unwrap();
    assert_eq!(
        h.finish().unwrap(),
        hex32("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
}

#[test]
fn standard_long_vector_across_block_boundaries() {
    let data = vec![b'a'; 1_000_000];
    for chunk in [63, 64, 65, 65536] {
        let mut h = Sha256Stream::new(1_000_000).unwrap();
        for bytes in data.chunks(chunk) {
            h.update(bytes).unwrap();
        }
        assert_eq!(
            h.finish().unwrap(),
            hex32("cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0")
        );
    }
}

#[test]
fn five_mib_stream_keeps_old_wire_limit() {
    let data: Vec<u8> = (0..5_242_880usize).map(|i| (i % 256) as u8).collect();
    assert_eq!(sha256(&data), Err(PlatformError::TooLarge));
    for chunk in [65536, 1_048_576] {
        let mut h = Sha256Stream::new(5_242_880).unwrap();
        for bytes in data.chunks(chunk) {
            h.update(bytes).unwrap();
        }
        assert_eq!(
            h.finish().unwrap(),
            hex32("2e7cab6314e9614b6f2da12630661c3038e5592025f6534ba5823c3b340a1cb6")
        );
    }
}

#[test]
fn bounds_fail_closed_and_poison_even_empty_updates() {
    assert!(matches!(
        Sha256Stream::new(0),
        Err(PlatformError::InvalidInput)
    ));
    assert!(matches!(
        Sha256Stream::new(2_147_483_649),
        Err(PlatformError::TooLarge)
    ));
    let mut h = Sha256Stream::new(2).unwrap();
    assert_eq!(h.update(b"abc"), Err(PlatformError::TooLarge));
    assert_eq!(h.update(b""), Err(PlatformError::ServiceFailure));
    assert_eq!(h.finish(), Err(PlatformError::ServiceFailure));
    let mut h = Sha256Stream::new(2_147_483_648).unwrap();
    assert_eq!(h.update(&vec![0; 1_048_577]), Err(PlatformError::TooLarge));
    assert_eq!(h.finish(), Err(PlatformError::ServiceFailure));
}

#[test]
fn limit_is_cumulative_not_per_call() {
    let mut h = Sha256Stream::new(3).unwrap();
    h.update(b"ab").unwrap();
    assert_eq!(h.update(b"cd"), Err(PlatformError::TooLarge));
    assert_eq!(h.finish(), Err(PlatformError::ServiceFailure));
}
