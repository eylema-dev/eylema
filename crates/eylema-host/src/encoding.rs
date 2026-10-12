use crate::*;
use services::Call;
pub(crate) fn list(v: Vec<Value>) -> Value {
    Value::List(v)
}
pub(crate) fn text(s: &str) -> Value {
    Value::Text(s.into())
}
pub(crate) fn num(n: usize) -> Value {
    Value::Unsigned(n as u64)
}
pub(crate) fn fields<const N: usize>(v: &Value) -> Result<&[Value; N]> {
    match v {
        Value::List(v) => v
            .as_slice()
            .try_into()
            .map_err(|_| HostError::InvalidRecord),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn values(v: &Value, max: usize) -> Result<&[Value]> {
    match v {
        Value::List(v) if v.len() <= max => Ok(v),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn string(v: &Value) -> Result<&str> {
    match v {
        Value::Text(s) => Ok(s),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn unsigned(v: &Value) -> Result<u64> {
    match v {
        Value::Unsigned(n) => Ok(*n),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn index(v: &Value) -> Result<usize> {
    usize::try_from(unsigned(v)?).map_err(|_| HostError::InvalidRecord)
}
pub(crate) fn boolean(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn hash(v: &Value) -> Result<[u8; 32]> {
    match v {
        Value::Bytes(b) => b
            .as_slice()
            .try_into()
            .map_err(|_| HostError::InvalidRecord),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn typed<T>(value: std::result::Result<T, ValidationError>) -> Result<T> {
    value.map_err(|_| HostError::InvalidRecord)
}
pub(crate) fn err(code: ErrorCode) -> Value {
    list(vec![text("error"), text(code.as_str())])
}
pub(crate) fn outcome(v: eylema_runtime::Result<Value>) -> Value {
    match v {
        Ok(v) => list(vec![text("ok"), v]),
        Err(e) => err(e),
    }
}
pub(crate) fn decode_outcome(v: &Value) -> Result<Value> {
    let [kind, v] = fields(v)?;
    match string(kind)? {
        "ok" => Ok(v.clone()),
        "error" => Err(HostError::Runtime(
            ErrorCode::ALL
                .into_iter()
                .find(|e| Some(e.as_str()) == string(v).ok())
                .ok_or(HostError::InvalidRecord)?,
        )),
        _ => Err(HostError::InvalidRecord),
    }
}
pub(crate) fn wire(record: &codec::WireRecord) -> Result<Value> {
    codec::encode_record(record)
        .and_then(|b| codec::decode(&b))
        .map_err(|_| HostError::InvalidRecord)
}
pub(crate) fn record(v: &Value) -> Result<codec::WireRecord> {
    codec::encode(v)
        .and_then(|b| codec::decode_record(&b))
        .map_err(|_| HostError::InvalidRecord)
}
pub(crate) fn limits(l: eylema_runtime::Limits) -> Value {
    list(vec![
        num(l.tasks),
        num(l.executions),
        num(l.sessions),
        num(l.queued),
        num(l.events),
        num(l.idempotency),
        num(l.resident_wire_bytes),
    ])
}
pub(crate) fn decode_limits(v: &Value) -> Result<eylema_runtime::Limits> {
    let [a, b, c, d, e, f, g] = fields(v)?;
    Ok(eylema_runtime::Limits {
        tasks: index(a)?,
        executions: index(b)?,
        sessions: index(c)?,
        queued: index(d)?,
        events: index(e)?,
        idempotency: index(f)?,
        resident_wire_bytes: index(g)?,
    })
}
pub(crate) fn scope(s: &ResourceScope) -> Value {
    list(s.resources().map(|i| text(i.as_str())).collect())
}
pub(crate) fn decode_scope(v: &Value) -> Result<ResourceScope> {
    typed(ResourceScope::new(
        values(v, MAX_REFERENCES)?
            .iter()
            .map(|v| typed(ResourceId::new(string(v)?)))
            .collect::<Result<_>>()?,
    ))
}
pub(crate) fn grant(g: &Grant) -> Value {
    list(vec![
        text(g.principal.as_str()),
        scope(&g.resource_scope),
        list(g.permissions.iter().map(|p| num(*p as usize)).collect()),
        Value::Unsigned(g.policy_revision.get()),
    ])
}
pub(crate) fn decode_grant(v: &Value) -> Result<Grant> {
    let [p, s, ps, r] = fields(v)?;
    let permissions = [
        Permission::Read,
        Permission::CreateTask,
        Permission::ProposeAction,
        Permission::ExecuteAction,
        Permission::ObserveEvents,
        Permission::RequestCancellation,
        Permission::ApproveAction,
        Permission::GovernPolicy,
    ];
    Ok(Grant {
        principal: typed(PrincipalRef::new(string(p)?))?,
        resource_scope: decode_scope(s)?,
        permissions: values(ps, 8)?
            .iter()
            .map(|v| {
                permissions
                    .get(index(v)?)
                    .copied()
                    .ok_or(HostError::InvalidRecord)
            })
            .collect::<Result<_>>()?,
        policy_revision: typed(Revision::new(unsigned(r)?))?,
    })
}
pub(crate) fn oversight(o: &OversightRequirements) -> Value {
    list(vec![
        list(o.checkpoints().iter().map(|c| text(c.as_str())).collect()),
        Value::Bool(o.requires_observation()),
        Value::Bool(o.requires_intervention()),
        o.governance().map_or(Value::Null, |g| text(g.as_str())),
    ])
}
pub(crate) fn decode_oversight(v: &Value) -> Result<OversightRequirements> {
    let [c, o, i, g] = fields(v)?;
    typed(OversightRequirements::new(
        values(c, MAX_REFERENCES)?
            .iter()
            .map(|v| typed(CheckpointRef::new(string(v)?)))
            .collect::<Result<_>>()?,
        boolean(o)?,
        boolean(i)?,
        if *g == Value::Null {
            None
        } else {
            Some(typed(GovernanceRef::new(string(g)?))?)
        },
    ))
}
pub(crate) fn binding(b: &ApprovalBinding) -> Value {
    list(vec![
        text(b.task_id.as_str()),
        text(b.proposal_id.as_str()),
        Value::Unsigned(b.plan_revision.get()),
        Value::Bytes(b.payload_sha256.to_vec()),
        Value::Unsigned(b.state_revision.get()),
        Value::Unsigned(b.policy_revision.get()),
        scope(&b.resource_scope),
        list(b.checkpoints.iter().map(|c| text(c.as_str())).collect()),
        Value::Unsigned(b.expires_at.unix_ms()),
    ])
}
pub(crate) fn decode_binding(v: &Value) -> Result<ApprovalBinding> {
    let [t, p, r, h, s, pr, scope, c, e] = fields(v)?;
    Ok(ApprovalBinding {
        task_id: typed(TaskId::new(string(t)?))?,
        proposal_id: typed(ProposalId::new(string(p)?))?,
        plan_revision: typed(Revision::new(unsigned(r)?))?,
        payload_sha256: hash(h)?,
        state_revision: typed(Revision::new(unsigned(s)?))?,
        policy_revision: typed(Revision::new(unsigned(pr)?))?,
        resource_scope: decode_scope(scope)?,
        checkpoints: values(c, MAX_REFERENCES)?
            .iter()
            .map(|v| typed(CheckpointRef::new(string(v)?)))
            .collect::<Result<_>>()?,
        expires_at: typed(Timestamp::new(unsigned(e)?))?,
    })
}
pub(crate) fn revocation(r: &Revocation) -> Value {
    list(vec![
        text(r.verified_principal.as_str()),
        Value::Unsigned(r.timestamp.unix_ms()),
    ])
}
pub(crate) fn approval(a: &ApprovalRecord) -> Value {
    list(vec![
        text(a.approval_id.as_str()),
        binding(&a.binding),
        text(a.verified_principal.as_str()),
        Value::Unsigned(a.decided_at.unix_ms()),
        a.revocation.as_ref().map_or(Value::Null, revocation),
    ])
}
pub(crate) fn policy(p: &PolicyChange) -> Value {
    list(vec![
        text(p.decision_id.as_str()),
        text(p.task_id.as_str()),
        text(p.verified_principal.as_str()),
        Value::Unsigned(p.previous_revision.get()),
        Value::Unsigned(p.accepted_revision.get()),
        Value::Unsigned(p.timestamp.unix_ms()),
    ])
}
pub(crate) fn receipt(r: &CancellationReceipt) -> Value {
    list(vec![
        text(r.execution_id.as_str()),
        text(r.receipt_event.as_str()),
        text(r.verified_principal.as_str()),
        Value::Unsigned(r.requested_at.unix_ms()),
    ])
}
pub(crate) fn calls(c: &[Call]) -> Value {
    list(
        c.iter()
            .map(|c| match c {
                Call::Now(v) => list(vec![num(0), outcome(v.map(Value::Unsigned))]),
                Call::Execution(v) => list(vec![num(1), outcome(v.clone().map(Value::Text))]),
                Call::Event(v) => list(vec![num(2), outcome(v.clone().map(Value::Text))]),
                Call::Hash(v) => list(vec![num(3), outcome(v.map(|b| Value::Bytes(b.to_vec())))]),
            })
            .collect(),
    )
}
pub(crate) fn decode_calls(v: &Value) -> Result<Vec<Call>> {
    values(v, services::MAX_CALLS)?
        .iter()
        .map(|v| {
            let [tag, result] = fields(v)?;
            let result = match decode_outcome(result) {
                Ok(v) => Ok(v),
                Err(HostError::Runtime(e)) => Err(e),
                Err(e) => return Err(e),
            };
            Ok(match index(tag)? {
                0 => Call::Now(match result {
                    Ok(v) => Ok(unsigned(&v)?),
                    Err(e) => Err(e),
                }),
                1 => Call::Execution(match result {
                    Ok(v) => Ok(typed(ExecutionId::new(string(&v)?))?.as_str().into()),
                    Err(e) => Err(e),
                }),
                2 => Call::Event(match result {
                    Ok(v) => Ok(typed(EventId::new(string(&v)?))?.as_str().into()),
                    Err(e) => Err(e),
                }),
                3 => Call::Hash(match result {
                    Ok(v) => Ok(hash(&v)?),
                    Err(e) => Err(e),
                }),
                _ => return Err(HostError::InvalidRecord),
            })
        })
        .collect()
}
