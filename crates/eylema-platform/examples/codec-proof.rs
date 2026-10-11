//! Development-only native consumer and bounded cross-target fixtures.
#[path = "../../eylema-contracts/tests/support/mod.rs"]
mod support;
use eylema_contracts::{codec::*, *};
use eylema_platform::{IdKind, hex, id_from_entropy, sha256};
use std::{fs, io::Read, path::Path};
use support::*;

fn field<'a>(v: &'a mut Value, key: &str) -> &'a mut Value {
    let Value::Map(fields) = v else {
        panic!("fixture map")
    };
    &mut fields.iter_mut().find(|(name, _)| name == key).unwrap().1
}
fn process(bytes: &[u8], mode: u32) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err("aggregate input bound".into());
    }
    Ok(match mode {
        0 => encode(&decode(bytes)?)?,
        1 => encode_record(&decode_record(bytes)?)?,
        2 => match decode_record(bytes)? {
            WireRecord::Proposal { task, proposal }
            | WireRecord::Execution { task, proposal, .. } => {
                canonical_approval_payload(&task, &proposal)?
            }
            _ => return Err("not a proposal binding".into()),
        },
        3 if bytes.len() == 17 => {
            id_from_entropy(IdKind::from_byte(bytes[0])?, &bytes[1..].try_into()?).into_bytes()
        }
        _ => return Err("invalid mode/entropy input".into()),
    })
}
fn fixtures(root: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    let mut manifest = String::new();
    let mut add = |name: &str,
                   mode: u32,
                   bytes: Vec<u8>,
                   valid: bool|
     -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Assert expected native classification before sharing fixtures with Wasm.
        if process(&bytes, mode).is_ok() != valid {
            return Err(format!("fixture expectation: {name}").into());
        }
        fs::write(root.join(format!("{name}.bin")), bytes)?;
        manifest.push_str(&format!("{name}|{mode}|{valid}\n"));
        Ok(())
    };
    let t = task();
    let p = ActionProposal::new(proposal(), &t)?;
    let e = ExecutionRecord::new(execution(), &p)?;
    let task_bytes = encode_record(&WireRecord::Task(t.clone()))?;
    let proposal_bytes = encode_record(&WireRecord::Proposal {
        task: t.clone(),
        proposal: p.clone(),
    })?;
    let execution_bytes = encode_record(&WireRecord::Execution {
        task: t.clone(),
        proposal: p.clone(),
        execution: Box::new(e),
    })?;
    add("task", 1, task_bytes.clone(), true)?;
    add("proposal", 1, proposal_bytes.clone(), true)?;
    add("execution", 1, execution_bytes.clone(), true)?;
    add("approval", 2, proposal_bytes.clone(), true)?;
    add("approval-execution", 2, execution_bytes.clone(), true)?;
    add("approval-wrong-kind", 2, task_bytes.clone(), false)?;
    let event = EventRecord::new(EventDraft {
        contract_version: ContractVersion::CURRENT,
        event_id: EventId::new("event:1")?,
        task_id: TaskId::new("task:1")?,
        execution_id: Some(ExecutionId::new("execution:1")?),
        state_revision: revision(2),
        timestamp: Timestamp::new(1000)?,
        correlation_id: context().correlation_id,
        kind: EventKind::ExecutionProgress,
        evidence: vec![evidence()],
    })?;
    add("event", 1, encode_record(&WireRecord::Event(event))?, true)?;
    for (i, code) in ErrorCode::ALL.into_iter().enumerate() {
        add(
            &format!("error-{i}"),
            1,
            encode_record(&WireRecord::Error(error(code)))?,
            true,
        )?;
    }
    let generic = Value::Map(vec![
        ("b".into(), Value::Null),
        ("a".into(), Value::Unsigned(42)),
    ]);
    add("reference", 0, encode(&generic)?, true)?;
    add(
        "unicode",
        0,
        encode(&Value::Text("Türkçe e\u{301} é".into()))?,
        true,
    )?;
    add(
        "scalar-list",
        0,
        encode(&Value::List(vec![
            Value::Signed(i64::MIN),
            Value::Unsigned(u64::MAX),
            Value::Bool(true),
            Value::Bytes(vec![0, 255]),
        ]))?,
        true,
    )?;
    add(
        "max-payload",
        0,
        encode(&Value::Bytes(vec![42; MAX_PAYLOAD_BYTES]))?,
        true,
    )?;
    let mut nested = Value::Null;
    for _ in 0..MAX_DEPTH {
        nested = Value::List(vec![nested])
    }
    add("max-depth", 0, encode(&nested)?, true)?;
    let mut deep = HEADER.to_vec();
    for _ in 0..MAX_DEPTH + 1 {
        deep.extend([7, 0, 0, 0, 1])
    }
    deep.push(0);
    add("too-deep", 0, deep, false)?;
    let mut nodes = HEADER.to_vec();
    nodes.extend([7, 0, 1, 0, 0]);
    nodes.extend(vec![0; MAX_ITEMS]);
    add("too-many-nodes", 0, nodes, false)?;
    add("oversized", 0, vec![0; MAX_WIRE_BYTES + 1], false)?;
    let mut oversized = HEADER.to_vec();
    oversized.push(5);
    oversized.extend(((MAX_PAYLOAD_BYTES + 1) as u32).to_be_bytes());
    oversized.extend(vec![0; MAX_PAYLOAD_BYTES + 1]);
    add("oversized-blob", 0, oversized, false)?;
    let mut invalid = HEADER.to_vec();
    invalid.extend([6, 0, 0, 0, 1, 255]);
    add("bad-utf8", 0, invalid, false)?;
    let mut invalid = HEADER.to_vec();
    invalid.push(255);
    add("bad-tag", 0, invalid, false)?;
    let mut invalid = HEADER.to_vec();
    invalid.extend([8, 255, 255, 255, 255]);
    add("bad-count", 0, invalid, false)?;
    for (name, keys) in [("duplicate-map", *b"aa"), ("unordered-map", *b"ba")] {
        let mut invalid = HEADER.to_vec();
        invalid.extend([
            8, 0, 0, 0, 2, 0, 0, 0, 1, keys[0], 0, 0, 0, 0, 1, keys[1], 0,
        ]);
        add(name, 0, invalid, false)?;
    }
    for index in 0..HEADER.len() {
        let mut invalid = task_bytes.clone();
        invalid[index] ^= 128;
        add(&format!("bad-header-{index}"), 1, invalid, false)?;
    }
    for end in [0, 1, 7, 8, task_bytes.len() / 2, task_bytes.len() - 1] {
        add(
            &format!("truncated-{end}"),
            1,
            task_bytes[..end].to_vec(),
            false,
        )?;
    }
    let mut trailing = task_bytes.clone();
    trailing.push(0);
    add("trailing", 1, trailing, false)?;
    let original = decode(&execution_bytes)?;
    let mut invalid = original.clone();
    let Value::Map(fields) = &mut invalid else {
        unreachable!()
    };
    fields.push(("unknown".into(), Value::Null));
    add("unknown-field", 1, encode(&invalid)?, false)?;
    let mut invalid = original.clone();
    let Value::Map(fields) = &mut invalid else {
        unreachable!()
    };
    fields.retain(|(key, _)| key != "proposal");
    add("missing-field", 1, encode(&invalid)?, false)?;
    let mut invalid = original.clone();
    *field(field(&mut invalid, "execution"), "task_id") = Value::Text("foreign:1".into());
    add("binding-drift", 1, encode(&invalid)?, false)?;
    let mut invalid = original.clone();
    *field(
        field(field(&mut invalid, "task"), "budget"),
        "max_concurrency",
    ) = Value::Unsigned(u64::MAX);
    add("narrow-overflow", 1, encode(&invalid)?, false)?;
    let mut transported_task = t.fields().clone();
    transported_task.context.request_id = RequestId::new("request:other")?;
    transported_task.context.correlation_id = CorrelationId::new("trace:other")?;
    let mut transported_proposal = p.fields().clone();
    transported_proposal.context = transported_task.context.clone();
    let mut action = transported_proposal.actions[0].fields().clone();
    action.context = transported_task.context.clone();
    transported_proposal.actions[0] = ActionSpec::new(action)?;
    let tt = TaskSpec::new(transported_task)?;
    let tp = ActionProposal::new(transported_proposal, &tt)?;
    add(
        "approval-transport",
        2,
        encode_record(&WireRecord::Proposal {
            task: tt,
            proposal: tp,
        })?,
        true,
    )?;
    let mut changed = p.fields().clone();
    let mut action = changed.actions[0].fields().clone();
    action.payload = b"changed effect".to_vec();
    changed.actions[0] = ActionSpec::new(action)?;
    let cp = ActionProposal::new(changed, &t)?;
    add(
        "approval-effect-change",
        2,
        encode_record(&WireRecord::Proposal {
            task: t,
            proposal: cp,
        })?,
        true,
    )?;
    for kind in 0..7 {
        let mut input = vec![kind];
        input.extend([0xab; 16]);
        add(&format!("id-{kind}"), 3, input, true)?;
    }
    add("id-bad-kind", 3, vec![255; 17], false)?;
    add("id-short", 3, vec![0; 16], false)?;
    fs::write(root.join("manifest.txt"), manifest)?;
    Ok(())
}
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, path] if command == "fixtures" => fixtures(Path::new(path)),
        [command, mode, path] if command == "probe" => {
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take((MAX_WIRE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            match process(&bytes, mode.parse()?) {
                Ok(output) => println!(
                    "{{\"accepted\":true,\"bytes\":\"{}\",\"sha256\":\"{}\"}}",
                    hex(&output),
                    hex(&sha256(&output)?)
                ),
                Err(_) => println!("{{\"accepted\":false}}"),
            }
            Ok(())
        }
        _ => Err("usage: codec-proof fixtures DIR | probe MODE FILE".into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("codec-proof: {error}");
        std::process::exit(1)
    }
}
