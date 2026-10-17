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
#[test]
fn wire_extents_precede_allocation() {
    use super::wire::*;
    use eylema_contracts::codec::{self, Value};
    let valid = codec::encode(&Value::List(vec![Value::Text("ok".into())])).unwrap();
    assert!(decode_closed(&valid).is_ok());
    assert!(decode_closed(&codec::encode(&Value::Map(vec![])).unwrap()).is_err());
    assert!(decode_closed(&codec::encode(&Value::List(vec![Value::Null; 33])).unwrap()).is_err());
    let mut nested = Value::Null;
    for _ in 0..10 {
        nested = Value::List(vec![nested]);
    }
    assert!(decode_closed(&codec::encode(&nested).unwrap()).is_err());
    let mut io = FrameBudget::default();
    assert!(
        io.read(&mut std::io::Cursor::new(8193u32.to_be_bytes()))
            .is_err()
    );
    let mut io = FrameBudget::default();
    assert!(io.read(&mut std::io::Cursor::new([0, 0, 0, 8, 1])).is_err());
    for _ in 0..255 {
        assert!(io.write(&mut std::io::sink(), &valid).is_ok());
    }
    assert!(io.write(&mut std::io::sink(), &valid).is_err());
}
#[test]
fn planner_input_dependency() {
    use super::models::decision_query;
    assert!(decision_query("North owner checklist", "").is_err());
    let query = decision_query("North owner checklist", "Check sources.").unwrap();
    assert_eq!(
        query,
        "Request: North owner checklist\nPlanning proposal: Check sources."
    );
    assert!(decision_query("North owner checklist", &"x".repeat(8193)).is_err());
}
#[test]
fn terminal_and_worker_failure_preserve_budget() {
    let mut b = JobLedger::new(100).unwrap();
    b.reserve(80).unwrap();
    assert!(b.reserve(21).is_err());
    assert!(b.reserve(20).is_ok());
    assert!(b.reserve(1).is_err());
}
#[test]
fn one_profile_resident_and_root_budget() {
    use super::models::*;
    assert!(
        ModelRequest::Plan(vec!["x".repeat(8193)])
            .validate()
            .is_err()
    );
    assert!(
        ModelRequest::Documents(vec!["x".into(); 10])
            .validate()
            .is_err()
    );
    assert!(ModelRequest::Queries(vec![]).validate().is_err());
    assert!(ModelRequest::Plan(vec!["North".into()]).validate().is_ok());
}
#[test]
fn worker_identity_and_replay_are_closed() {
    use super::worker::validate_envelope;
    use eylema_contracts::codec::Value;
    let packet = Value::List(vec![
        Value::Bytes(vec![7; 32]),
        Value::Unsigned(1),
        Value::Null,
    ]);
    assert!(validate_envelope(&packet, [7; 32], 1).is_ok());
    assert!(validate_envelope(&packet, [8; 32], 1).is_err());
    assert!(validate_envelope(&packet, [7; 32], 2).is_err());
}
#[test]
fn stop_cancels_registered_controls_and_prevents_dispatch() {
    use super::control::Controls;
    use eylema_model::cpu::{CpuControl, CpuWorkGroup};
    use std::time::{Duration, Instant};
    let c = Controls::default();
    let cpu = CpuControl::new_job(
        Instant::now() + Duration::from_secs(1),
        100,
        CpuWorkGroup::new(100).unwrap(),
    )
    .unwrap();
    c.register(cpu).unwrap();
    c.stop();
    assert!(c.check().is_err());
    assert!(
        c.register(CpuControl::new(Instant::now() + Duration::from_secs(1)).unwrap())
            .is_err()
    );
}
