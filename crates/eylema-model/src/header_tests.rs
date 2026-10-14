#[path = "../tests/support/mod.rs"]
mod support;
use crate::{LoadError, StoredDtype, header::validate_header};
use support::*;
fn check(h: &[u8], n: u64) -> Result<usize, LoadError> {
    let m = manifest(StoredDtype::F32, 134, digest(StoredDtype::F32));
    validate_header(h, n, &layout(&m)).map(|v| v.len())
}
#[test]
fn positive_and_escape_equivalence() {
    let f = fixture(StoredDtype::F32);
    assert_eq!(check(&f[8..118], 16), Ok(2));
    let m = manifest(StoredDtype::BF16, 127, digest(StoredDtype::BF16));
    let v = validate_header(BF16_H.as_bytes(), 8, &layout(&m)).unwrap();
    assert_eq!((v[0].start, v[0].end, v[1].start, v[1].end), (0, 4, 4, 8));
    assert_eq!(
        check(F32_H.replace("\"a\"", "\"\\u0061\"").as_bytes(), 16),
        Ok(2)
    );
    assert_eq!(check(format!("{}   ", F32_H).as_bytes(), 16), Ok(2));
    assert_eq!(
        check(
            F32_H
                .replacen(
                    "{",
                    "{\"__metadata__\":{\"secret\":\"private\\nvalue😀\"},",
                    1
                )
                .as_bytes(),
            16
        ),
        Ok(2)
    );
}
#[test]
fn map_geometry_and_interval_defects_reject() {
    for h in [
        F32_H.replace("\"shape\":[2]", "\"shape\":[1,2]"),
        F32_H.replace("\"dtype\":\"F32\"", "\"dtype\":\"BF16\""),
        F32_H.replace("[8,16]", "[4,12]"),
        F32_H.replace("[8,16]", "[9,17]"),
        F32_H.replace("\"b\"", "\"a\""),
        F32_H.replace("\"b\"", "\"\\u0061\""),
        F32_H.replace("\"dtype\":", "\"unknown\":0,\"dtype\":"),
        F32_H.replace("\"dtype\":\"F32\",", "\"dtype\":\"F32\",\"dtype\":\"F32\","),
        F32_H.replace("\"a\"", "\"extra\""),
    ] {
        assert!(check(h.as_bytes(), 16).is_err(), "accepted defect");
    }
    assert!(check(F32_H.as_bytes(), 17).is_err());
    assert!(check(F32_H.replace("[8,16]", "[4,12]").as_bytes(), 12).is_err());
    assert!(
        check(
            br#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,8]}}"#,
            16
        )
        .is_err()
    );
}
#[test]
fn malformed_unicode_numbers_and_padding_reject() {
    for h in [
        F32_H.replace("[2]", "[-2]"),
        F32_H.replace("[2]", "[02]"),
        F32_H.replace("[2]", "[2.0]"),
        F32_H.replace("[2]", "[2e0]"),
        F32_H.replace("[2]", "[18446744073709551616]"),
        F32_H.replace("\"a\"", "\"\\uD800\""),
        F32_H.replace("\"a\"", "\"\\q\""),
        format!("{}\n", F32_H),
    ] {
        assert!(check(h.as_bytes(), 16).is_err());
    }
    assert!(check(&[b'{', 255, b'}'], 16).is_err());
    assert!(check(br#"{"__metadata__":{"x":1}}"#, 0).is_err());
}
#[test]
fn bounded_parser_depth_nodes_metadata_and_header() {
    let deep = format!("{{\"x\":{}0{}}}", "[".repeat(7), "]".repeat(7));
    assert_eq!(check(deep.as_bytes(), 16), Err(LoadError::WorkLimit));
    let nodes = format!("{{\"x\":[{}]}}", vec!["0"; 16385].join(","));
    assert_eq!(check(nodes.as_bytes(), 16), Err(LoadError::WorkLimit));
    assert_eq!(check(&vec![b' '; 1048577], 16), Err(LoadError::SizeLimit));
    for metadata in [
        format!("{{\"{}\":\"x\"}}", "k".repeat(129)),
        format!("{{\"k\":\"{}\"}}", "v".repeat(4097)),
        format!(
            "{{{}}}",
            (0..129)
                .map(|i| format!("\"k{i}\":\"v\""))
                .collect::<Vec<_>>()
                .join(",")
        ),
    ] {
        assert!(check(format!("{{\"__metadata__\":{metadata}}}").as_bytes(), 16).is_err());
    }
}
