mod support;
use eylema_contracts::{
    codec::{self, Value},
    transport::*,
    *,
};
#[test]
fn transport_requests_roundtrip_with_typed_original_context() {
    let task = support::task();
    let proposal = ActionProposal::new(support::proposal(), &task).unwrap();
    let cases = vec![
        Request::Create(task.clone()),
        Request::Admit {
            task: task.clone(),
            proposal,
            approvals: vec![],
        },
        Request::Task(task.fields().task_id.clone()),
        Request::Execution(ExecutionId::new("execution:1").unwrap()),
        Request::Events {
            task: task.fields().task_id.clone(),
            after: None,
            limit: 256,
        },
        Request::Observation(EvidenceRef::new("evidence:1").unwrap()),
        Request::Cancel(ExecutionId::new("execution:1").unwrap()),
    ];
    for request in cases {
        let bytes = encode_request(&request).unwrap();
        assert_eq!(
            encode_request(&decode_request(&bytes).unwrap()).unwrap(),
            bytes
        );
    }
}
#[test]
fn version_fields_kind_reference_limit_and_response_envelope_are_closed() {
    for value in [
        Value::List(vec![]),
        Value::List(vec![
            Value::Unsigned(2),
            Value::Unsigned(2),
            Value::Text("task:1".into()),
        ]),
        Value::List(vec![
            Value::Unsigned(1),
            Value::Unsigned(99),
            Value::Bool(true),
        ]),
        Value::List(vec![
            Value::Unsigned(1),
            Value::Unsigned(2),
            Value::Text("task:1".into()),
            Value::Bool(true),
        ]),
    ] {
        assert!(decode_request(&codec::encode(&value).unwrap()).is_err());
    }
    assert!(
        encode_request(&Request::Events {
            task: TaskId::new("task:1").unwrap(),
            after: None,
            limit: 0
        })
        .is_err()
    );
    assert!(
        encode_request(&Request::Events {
            task: TaskId::new("task:1").unwrap(),
            after: None,
            limit: 257
        })
        .is_err()
    );
    assert_eq!(
        decode_response(&encode_response(Value::Unsigned(5)).unwrap()).unwrap(),
        Value::Unsigned(5)
    );
    assert_eq!(
        decode_response(&encode_error("FORBIDDEN").unwrap()),
        Err(TransportError::Remote("FORBIDDEN".into()))
    );
    assert!(encode_error("private\nvalue").is_err());
}
