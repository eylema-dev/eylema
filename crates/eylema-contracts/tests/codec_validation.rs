mod support;
use eylema_contracts::{codec::*, *};
use support::*;

fn map_field<'a>(value: &'a mut Value, key: &str) -> &'a mut Value {
    let Value::Map(fields) = value else {
        panic!("expected map")
    };
    &mut fields.iter_mut().find(|(name, _)| name == key).unwrap().1
}
fn record() -> WireRecord {
    let task = task();
    let proposal = ActionProposal::new(proposal(), &task).unwrap();
    let execution = ExecutionRecord::new(execution(), &proposal).unwrap();
    WireRecord::Execution {
        task,
        proposal,
        execution: Box::new(execution),
    }
}
fn assert_roundtrip(record: &WireRecord) {
    let bytes = encode_record(record).unwrap();
    assert_eq!(
        encode_record(&decode_record(&bytes).unwrap()).unwrap(),
        bytes
    );
}
#[test]
fn fixed_reference_vector_sorts_maps_and_preserves_scalar_types() {
    let v = Value::Map(vec![
        ("b".into(), Value::Null),
        ("a".into(), Value::Unsigned(42)),
    ]);
    let expected = [
        0x45, 0x59, 0x4c, 1, 0, 1, 0, 8, 0, 0, 0, 2, 0, 0, 0, 1, b'a', 4, 0, 0, 0, 0, 0, 0, 0, 42,
        0, 0, 0, 1, b'b', 0,
    ];
    assert_eq!(encode(&v).unwrap(), expected);
    assert_eq!(encode(&decode(&expected).unwrap()).unwrap(), expected);
    assert_ne!(
        encode(&Value::Signed(42)).unwrap(),
        encode(&Value::Unsigned(42)).unwrap()
    );
    assert_ne!(
        encode(&Value::Text("é".into())).unwrap(),
        encode(&Value::Text("e\u{301}".into())).unwrap()
    );
    let values = Value::List(vec![
        Value::Bool(true),
        Value::Bool(false),
        Value::Signed(i64::MIN),
        Value::Unsigned(u64::MAX),
        Value::Bytes(vec![0, 255]),
        Value::Text("Türkçe\nEnglish".into()),
    ]);
    assert_eq!(decode(&encode(&values).unwrap()).unwrap(), values);
}
#[test]
fn all_record_families_roundtrip_with_internal_bindings() {
    assert_roundtrip(&WireRecord::Task(task()));
    let t = task();
    let p = ActionProposal::new(proposal(), &t).unwrap();
    assert_roundtrip(&WireRecord::Proposal {
        task: t,
        proposal: p,
    });
    assert_roundtrip(&record());
    let event = EventRecord::new(EventDraft {
        contract_version: ContractVersion::CURRENT,
        event_id: EventId::new("event:1").unwrap(),
        task_id: TaskId::new("task:1").unwrap(),
        execution_id: Some(ExecutionId::new("execution:1").unwrap()),
        state_revision: revision(2),
        timestamp: Timestamp::new(1000).unwrap(),
        correlation_id: context().correlation_id,
        kind: EventKind::ExecutionProgress,
        evidence: vec![evidence()],
    })
    .unwrap();
    assert_roundtrip(&WireRecord::Event(event));
    for code in ErrorCode::ALL {
        assert_roundtrip(&WireRecord::Error(error(code)));
    }
    let t = task();
    let p = ActionProposal::new(proposal(), &t).unwrap();
    let mut e = execution();
    e.state = ExecutionState::OutcomeUnknown;
    e.effects = vec![effect(EffectOutcome::Unknown)];
    e.error = Some(error(ErrorCode::OutcomeUnknown));
    e.result_evidence = vec![evidence()];
    let e = ExecutionRecord::new(e, &p).unwrap();
    assert_roundtrip(&WireRecord::Execution {
        task: t,
        proposal: p,
        execution: Box::new(e),
    });
}
#[test]
fn every_truncation_unknown_version_tag_and_trailing_byte_fail() {
    let bytes = encode_record(&record()).unwrap();
    for end in 0..bytes.len() {
        assert!(decode_record(&bytes[..end]).is_err(), "prefix {end}");
    }
    for index in 0..HEADER.len() {
        let mut v = bytes.clone();
        v[index] ^= 0x80;
        assert!(decode(&v).is_err());
    }
    let mut v = HEADER.to_vec();
    v.push(255);
    assert!(decode(&v).is_err());
    let mut v = bytes.clone();
    v.push(0);
    assert!(decode_record(&v).is_err());
    let mut v = HEADER.to_vec();
    v.extend([6, 0, 0, 0, 1, 255]);
    assert!(decode(&v).is_err());
    let mut v = HEADER.to_vec();
    v.extend([7, 255, 255, 255, 255]);
    assert!(decode(&v).is_err());
}
#[test]
fn duplicate_unordered_and_invalid_map_keys_fail() {
    assert!(
        encode(&Value::Map(vec![
            ("a".into(), Value::Null),
            ("a".into(), Value::Bool(true))
        ]))
        .is_err()
    );
    for key in ["".into(), "a\0b".into(), "a".repeat(MAX_ID_BYTES + 1)] {
        assert!(encode(&Value::Map(vec![(key, Value::Null)])).is_err());
    }
    for keys in [*b"aa", *b"ba"] {
        let mut v = HEADER.to_vec();
        v.extend([
            8, 0, 0, 0, 2, 0, 0, 0, 1, keys[0], 0, 0, 0, 0, 1, keys[1], 0,
        ]);
        assert!(decode(&v).is_err());
    }
}
#[test]
fn byte_depth_node_and_aggregate_limits_apply_on_both_sides() {
    for v in [
        Value::Text("x".repeat(MAX_TEXT_BYTES + 1)),
        Value::Bytes(vec![0; MAX_PAYLOAD_BYTES + 1]),
        Value::List(vec![Value::Null; MAX_ITEMS]),
    ] {
        assert!(encode(&v).is_err());
    }
    let text = Value::Text("x".repeat(MAX_TEXT_BYTES));
    assert_eq!(decode(&encode(&text).unwrap()).unwrap(), text);
    let bytes = Value::Bytes(vec![0; MAX_PAYLOAD_BYTES]);
    assert_eq!(decode(&encode(&bytes).unwrap()).unwrap(), bytes);
    let mut depth = Value::Null;
    for _ in 0..MAX_DEPTH {
        depth = Value::List(vec![depth]);
    }
    assert!(decode(&encode(&depth).unwrap()).is_ok());
    depth = Value::List(vec![depth]);
    assert!(encode(&depth).is_err());
    let mut too_deep = HEADER.to_vec();
    for _ in 0..MAX_DEPTH + 1 {
        too_deep.extend([7, 0, 0, 0, 1]);
    }
    too_deep.push(0);
    assert!(decode(&too_deep).is_err());
    let mut too_many = HEADER.to_vec();
    too_many.extend([7, 0, 1, 0, 0]);
    too_many.extend(vec![0; MAX_ITEMS]);
    assert!(decode(&too_many).is_err());
    let mut oversized_text = HEADER.to_vec();
    oversized_text.push(6);
    oversized_text.extend(((MAX_TEXT_BYTES + 1) as u32).to_be_bytes());
    oversized_text.extend(vec![b'x'; MAX_TEXT_BYTES + 1]);
    assert!(decode(&oversized_text).is_err());
    let aggregate = Value::List(vec![Value::Bytes(vec![0; MAX_PAYLOAD_BYTES]); 4]);
    assert!(encode(&aggregate).is_err());
    assert!(decode(&vec![0; MAX_WIRE_BYTES + 1]).is_err());
}
#[test]
fn typed_unknown_missing_narrow_overflow_and_binding_drift_fail() {
    let original = decode(&encode_record(&record()).unwrap()).unwrap();
    let mut extra = original.clone();
    let Value::Map(fields) = &mut extra else {
        unreachable!()
    };
    fields.push(("unknown".into(), Value::Null));
    assert!(decode_record(&encode(&extra).unwrap()).is_err());
    let mut missing = original.clone();
    let Value::Map(fields) = &mut missing else {
        unreachable!()
    };
    fields.retain(|(key, _)| key != "proposal");
    assert!(decode_record(&encode(&missing).unwrap()).is_err());
    for field in ["max_payload_bytes", "max_concurrency"] {
        let mut v = original.clone();
        *map_field(map_field(map_field(&mut v, "task"), "budget"), field) =
            Value::Unsigned(u64::MAX);
        assert!(decode_record(&encode(&v).unwrap()).is_err());
    }
    for field in ["task_id", "proposal_id", "plan_revision"] {
        let mut v = original.clone();
        *map_field(map_field(&mut v, "execution"), field) = if field == "plan_revision" {
            Value::Unsigned(9)
        } else {
            Value::Text("foreign:9".into())
        };
        assert!(decode_record(&encode(&v).unwrap()).is_err());
    }
    let mut v = original.clone();
    *map_field(map_field(&mut v, "proposal"), "state_revision") = Value::Unsigned(9);
    assert!(decode_record(&encode(&v).unwrap()).is_err());
    let mut v = original.clone();
    *map_field(
        map_field(map_field(&mut v, "task"), "context"),
        "contract_version",
    ) = Value::Text("0.2.0".into());
    assert!(decode_record(&encode(&v).unwrap()).is_err());
}
#[test]
fn approval_bytes_ignore_transport_but_bind_effect_fields_and_action_order() {
    let t = task();
    let p = ActionProposal::new(proposal(), &t).unwrap();
    let original = canonical_approval_payload(&t, &p).unwrap();
    let mut td = t.fields().clone();
    td.context.request_id = RequestId::new("request:other").unwrap();
    td.context.correlation_id = CorrelationId::new("trace:other").unwrap();
    let transported_task = TaskSpec::new(td.clone()).unwrap();
    let mut pd = p.fields().clone();
    pd.context = td.context.clone();
    let mut ad = pd.actions[0].fields().clone();
    ad.context = td.context.clone();
    pd.actions[0] = ActionSpec::new(ad.clone()).unwrap();
    let transported = ActionProposal::new(pd.clone(), &transported_task).unwrap();
    assert_eq!(
        canonical_approval_payload(&transported_task, &transported).unwrap(),
        original
    );
    for (field, value) in [
        ("payload", Value::Bytes(b"context request:other".to_vec())),
        ("idempotency_key", Value::Text("idem:other".into())),
        ("operation_id", Value::Text("otherOp".into())),
    ] {
        let mut v = decode(
            &encode_record(&WireRecord::Proposal {
                task: t.clone(),
                proposal: p.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        let Value::List(actions) = map_field(map_field(&mut v, "proposal"), "actions") else {
            unreachable!()
        };
        *map_field(&mut actions[0], field) = value;
        let WireRecord::Proposal { task, proposal } = decode_record(&encode(&v).unwrap()).unwrap()
        else {
            unreachable!()
        };
        assert_ne!(
            canonical_approval_payload(&task, &proposal).unwrap(),
            original
        );
    }
    td.objective = "A different effect goal".into();
    let changed_task = TaskSpec::new(td).unwrap();
    assert_ne!(
        canonical_approval_payload(&changed_task, &transported).unwrap(),
        original
    );
    let mut p2 = p.fields().clone();
    let mut a2 = ad;
    a2.action_id = ActionId::new("action:2").unwrap();
    a2.idempotency_key = IdempotencyKey::new("idem:2").unwrap();
    p2.actions.push(ActionSpec::new(a2).unwrap());
    let forward = ActionProposal::new(p2.clone(), &t).unwrap();
    p2.actions.reverse();
    let reversed = ActionProposal::new(p2, &t).unwrap();
    assert_ne!(
        canonical_approval_payload(&t, &forward).unwrap(),
        canonical_approval_payload(&t, &reversed).unwrap()
    );
}
#[test]
fn unordered_declared_sets_are_canonical_and_wire_rejects_unsorted_sets() {
    let t = task();
    let mut pd = proposal();
    let mut ad = pd.actions[0].fields().clone();
    ad.required_capabilities = vec![
        CapabilityId::new("cap:z").unwrap(),
        CapabilityId::new("cap:a").unwrap(),
    ];
    pd.actions[0] = ActionSpec::new(ad.clone()).unwrap();
    let first = ActionProposal::new(pd.clone(), &t).unwrap();
    ad.required_capabilities.reverse();
    pd.actions[0] = ActionSpec::new(ad).unwrap();
    let second = ActionProposal::new(pd, &t).unwrap();
    assert_eq!(
        canonical_approval_payload(&t, &first).unwrap(),
        canonical_approval_payload(&t, &second).unwrap()
    );
    let mut v = decode(
        &encode_record(&WireRecord::Proposal {
            task: t,
            proposal: first,
        })
        .unwrap(),
    )
    .unwrap();
    let Value::List(actions) = map_field(map_field(&mut v, "proposal"), "actions") else {
        unreachable!()
    };
    let Value::List(capabilities) = map_field(&mut actions[0], "required_capabilities") else {
        unreachable!()
    };
    capabilities.reverse();
    assert!(decode_record(&encode(&v).unwrap()).is_err());
}
