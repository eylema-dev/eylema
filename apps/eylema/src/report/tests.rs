use super::{source::*, types::*};
#[test]
fn source_parser_is_closed_and_bounded() {
    let source = parse_document(b"language=en;site=North;owner=Mira;checklist=C7\n").unwrap();
    assert_eq!(source.owner.as_deref(), Some("Mira"));
    assert_eq!(source.checklist.as_deref(), Some("C7"));
    for invalid in [
        "language=en;site=North;owner=Mira;owner=Leon",
        "language=en;site=North;run=stop",
        "language=en;site=North",
        "language=en;site=North;owner=\u{1b}stop",
        "language=zz;site=North;owner=Mira",
    ] {
        assert_eq!(
            parse_document(invalid.as_bytes()).err(),
            Some(ReportError::InvalidInput)
        );
    }
    assert_eq!(
        parse_document(&vec![b'x'; 1025]).err(),
        Some(ReportError::Limit)
    );
    assert!(parse_document(b"language=tr;site=G\xc3\xbcney;owner=Ece;checklist=C9\n").is_ok());
    assert!(parse_document(b"language=en;site=West;owner=Leon\n").is_ok());
}
#[test]
fn need_is_closed_and_repair_is_finite() {
    let need = ReportNeed::new(
        "project:a",
        "task:report-a",
        1,
        vec![("en".into(), "North".into())],
    )
    .unwrap();
    assert_ne!(need.digest().unwrap(), [0; 32]);
    assert!(ReportNeed::new("project:a", "task:report-a", 0, vec![]).is_err());
    let mut budget = JobLedger::new(100).unwrap();
    assert!(budget.reserve(60).is_ok());
    assert!(budget.reserve(41).is_err());
    assert!(budget.repair().is_ok());
    assert!(budget.repair().is_err());
}
