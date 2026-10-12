//! Versioned local transport adapter over the owned typed EYL codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportError {
    Malformed,
    Remote(String),
}
use crate::{
    codec::{self, Value, WireRecord},
    *,
};

#[derive(Clone)]
pub enum Request {
    Create(TaskSpec),
    Admit {
        task: TaskSpec,
        proposal: ActionProposal,
        approvals: Vec<EventId>,
    },
    Task(TaskId),
    Execution(ExecutionId),
    Events {
        task: TaskId,
        after: Option<EventId>,
        limit: u16,
    },
    Observation(EvidenceRef),
    Cancel(ExecutionId),
}
fn list(v: Vec<Value>) -> Value {
    Value::List(v)
}
fn text(v: &str) -> Value {
    Value::Text(v.into())
}
fn wire(v: &WireRecord) -> Result<Value, TransportError> {
    codec::encode_record(v)
        .map(Value::Bytes)
        .map_err(|_| TransportError::Malformed)
}
pub fn encode_request(request: &Request) -> Result<Vec<u8>, TransportError> {
    let (op, payload) = match request {
        Request::Create(t) => (0, wire(&WireRecord::Task(t.clone()))?),
        Request::Admit {
            task,
            proposal,
            approvals,
        } => {
            if approvals.len() > MAX_REFERENCES {
                return Err(TransportError::Malformed);
            }
            (
                1,
                list(vec![
                    wire(&WireRecord::Proposal {
                        task: task.clone(),
                        proposal: proposal.clone(),
                    })?,
                    list(approvals.iter().map(|x| text(x.as_str())).collect()),
                ]),
            )
        }
        Request::Task(id) => (2, text(id.as_str())),
        Request::Execution(id) => (3, text(id.as_str())),
        Request::Events { task, after, limit } => {
            if !(1..=256).contains(limit) {
                return Err(TransportError::Malformed);
            }
            (
                4,
                list(vec![
                    text(task.as_str()),
                    after.as_ref().map_or(Value::Null, |x| text(x.as_str())),
                    Value::Unsigned((*limit).into()),
                ]),
            )
        }
        Request::Observation(id) => (5, text(id.as_str())),
        Request::Cancel(id) => (6, text(id.as_str())),
    };
    codec::encode(&list(vec![
        Value::Unsigned(1),
        Value::Unsigned(op),
        payload,
    ]))
    .map_err(|_| TransportError::Malformed)
}
fn fields<const N: usize>(v: Value) -> Result<[Value; N], TransportError> {
    if let Value::List(v) = v {
        v.try_into().map_err(|_| TransportError::Malformed)
    } else {
        Err(TransportError::Malformed)
    }
}
fn string(v: Value) -> Result<String, TransportError> {
    if let Value::Text(v) = v {
        Ok(v)
    } else {
        Err(TransportError::Malformed)
    }
}
fn record(v: Value) -> Result<WireRecord, TransportError> {
    if let Value::Bytes(v) = v {
        codec::decode_record(&v).map_err(|_| TransportError::Malformed)
    } else {
        Err(TransportError::Malformed)
    }
}
pub fn decode_request(bytes: &[u8]) -> Result<Request, TransportError> {
    let [version, op, payload] =
        fields(codec::decode(bytes).map_err(|_| TransportError::Malformed)?)?;
    if version != Value::Unsigned(1) {
        return Err(TransportError::Malformed);
    }
    Ok(match op {
        Value::Unsigned(0) => {
            let WireRecord::Task(t) = record(payload)? else {
                return Err(TransportError::Malformed);
            };
            Request::Create(t)
        }
        Value::Unsigned(1) => {
            let [p, approvals] = fields(payload)?;
            let WireRecord::Proposal { task, proposal } = record(p)? else {
                return Err(TransportError::Malformed);
            };
            let Value::List(ids) = approvals else {
                return Err(TransportError::Malformed);
            };
            if ids.len() > MAX_REFERENCES {
                return Err(TransportError::Malformed);
            }
            let approvals = ids
                .into_iter()
                .map(|v| EventId::new(string(v)?).map_err(|_| TransportError::Malformed))
                .collect::<Result<_, _>>()?;
            Request::Admit {
                task,
                proposal,
                approvals,
            }
        }
        Value::Unsigned(2) => {
            Request::Task(TaskId::new(string(payload)?).map_err(|_| TransportError::Malformed)?)
        }
        Value::Unsigned(3) => Request::Execution(
            ExecutionId::new(string(payload)?).map_err(|_| TransportError::Malformed)?,
        ),
        Value::Unsigned(4) => {
            let [task, after, limit] = fields(payload)?;
            let task = TaskId::new(string(task)?).map_err(|_| TransportError::Malformed)?;
            let after = if after == Value::Null {
                None
            } else {
                Some(EventId::new(string(after)?).map_err(|_| TransportError::Malformed)?)
            };
            let Value::Unsigned(limit @ 1..=256) = limit else {
                return Err(TransportError::Malformed);
            };
            Request::Events {
                task,
                after,
                limit: limit as u16,
            }
        }
        Value::Unsigned(5) => Request::Observation(
            EvidenceRef::new(string(payload)?).map_err(|_| TransportError::Malformed)?,
        ),
        Value::Unsigned(6) => Request::Cancel(
            ExecutionId::new(string(payload)?).map_err(|_| TransportError::Malformed)?,
        ),
        _ => return Err(TransportError::Malformed),
    })
}
pub fn decode_response(bytes: &[u8]) -> Result<Value, TransportError> {
    let [kind, value] = fields(codec::decode(bytes).map_err(|_| TransportError::Malformed)?)?;
    match kind {
        Value::Text(v) if v == "ok" => Ok(value),
        Value::Text(v) if v == "error" => Err(TransportError::Remote(string(value)?)),
        _ => Err(TransportError::Malformed),
    }
}
pub fn encode_response(value: Value) -> Result<Vec<u8>, TransportError> {
    codec::encode(&list(vec![text("ok"), value])).map_err(|_| TransportError::Malformed)
}
pub fn encode_error(code: &str) -> Result<Vec<u8>, TransportError> {
    if code.is_empty()
        || code.len() > 96
        || !code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(TransportError::Malformed);
    }
    codec::encode(&list(vec![text("error"), text(code)])).map_err(|_| TransportError::Malformed)
}
