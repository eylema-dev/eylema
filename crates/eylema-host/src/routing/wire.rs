use super::dispatch::CallPayload;
use super::*;
use eylema_contracts::{codec::WireRecord, tool::MAX_TOOL_INPUT_BYTES};
use eylema_retrieval::{QueryInput, SearchQuery};
fn pin(p: &Pin) -> Value {
    Value::List(vec![text(p.id.as_str()), number(p.revision.get())])
}
fn source(s: &SourcePin) -> Value {
    Value::List(vec![
        text(s.reference.project.as_str()),
        text(s.reference.id.as_str()),
        Value::Bytes(s.digest.to_vec()),
    ])
}
fn model(m: &ModelBinding) -> Value {
    Value::List(vec![
        text(m.role.as_str()),
        text(m.profile.as_str()),
        number(m.revision.get()),
        Value::Bytes(m.manifest.to_vec()),
        Value::Bytes(m.artifact.to_vec()),
        Value::Bytes(m.tokenizer.to_vec()),
        Value::Bytes(m.preprocessing.to_vec()),
        number(m.context_limit),
        number(m.resident_parameters),
        number(m.active_parameters),
        text(&m.resident),
        text(&m.invocation),
        text(&m.backend),
    ])
}
fn entry(e: &Entry) -> Value {
    Value::List(vec![
        entry_value(e),
        Value::List(e.sources.iter().map(source).collect()),
        Value::List(e.models.iter().map(model).collect()),
        pin(&e.quality.pin),
        number(e.quality.valid_from),
        number(e.quality.valid_until),
        number(e.context_limit),
        pin(&e.resource),
        number(match e.grounding {
            SourceStatus::DeclaredConsistent => 0,
            SourceStatus::Unknown => 1,
            SourceStatus::Contradictory => 2,
        }),
    ])
}
fn request(r: &Request) -> Value {
    Value::List(vec![
        text(r.project.as_str()),
        pin(&r.registry),
        pin(&r.policy),
        pin(&r.resource),
        pin(&r.quality),
        number(match r.intent {
            Intent::Retrieval => 0,
            Intent::TypedSkill => 1,
            Intent::Code => 2,
            Intent::Document => 3,
        }),
        Value::List(r.candidates.iter().map(|id| text(id.as_str())).collect()),
        Value::Bool(r.local_only),
        number(r.context),
        number(r.response),
        Value::Bool(r.verified_tokens_required),
    ])
}
fn vector(v: &[f32]) -> Result<Value, RouteError> {
    if v.len() > 4096 || v.iter().any(|x| !x.is_finite()) {
        return Err(RouteError::Limit);
    }
    Ok(Value::Bytes(
        v.iter().flat_map(|v| v.to_bits().to_be_bytes()).collect(),
    ))
}
fn query(q: &SearchQuery) -> Result<Value, RouteError> {
    q.validate()?;
    let body = match &q.input {
        QueryInput::Vector(v) => Value::List(vec![number(0), vector(v)?]),
        QueryInput::Lexical(t) => {
            if t.len() > 2048 {
                return Err(RouteError::Limit);
            }
            Value::List(vec![number(1), text(t)])
        }
        QueryInput::Hybrid {
            text: t,
            vector: v,
            vector_weight,
        } => {
            if t.len() > 2048 {
                return Err(RouteError::Limit);
            }
            Value::List(vec![
                number(2),
                text(t),
                vector(v)?,
                number(u64::from(*vector_weight)),
            ])
        }
    };
    let b = &q.binding;
    Ok(Value::List(vec![
        text(q.project.as_str()),
        Value::List(vec![
            text(b.space.id.as_str()),
            text(b.space.model.as_str()),
            text(b.space.preprocessing.as_str()),
            number(b.space.revision.get()),
            number(u64::from(b.space.dimensions)),
            Value::Bytes(b.manifest_sha256.to_vec()),
            Value::Bytes(b.artifact_sha256.to_vec()),
            Value::Bytes(b.tokenizer_sha256.to_vec()),
            Value::Bytes(b.preprocessing_sha256.to_vec()),
            text(&b.chunk_profile),
            text(&b.lexical_profile),
            text(&b.score_profile),
        ]),
        number(q.generation),
        Value::List(
            q.sources
                .iter()
                .map(|r| Value::List(vec![text(r.project.as_str()), text(r.id.as_str())]))
                .collect(),
        ),
        body,
        number(q.top_k as u64),
        number(q.max_hits as u64),
        number(q.max_bytes as u64),
        q.max_tokens.map_or(Value::Null, |v| number(u64::from(v))),
    ]))
}
pub(super) fn fingerprint(
    e: &Entry,
    r: &Request,
    a: &[Assessment],
    payload: CallPayload<'_>,
    c: &mut RouteControl,
) -> Result<[u8; 32], RouteError> {
    if r.candidates.len() > 8 || a.len() > 8 {
        return Err(RouteError::Limit);
    }
    e.validate()?;
    // Bound opaque typed-record copies BEFORE cloning or generic encoding.
    let preflight = match payload {
        CallPayload::Retrieval(q) => {
            if q.sources.len() > 16 {
                return Err(RouteError::Limit);
            }
            match &q.input {
                QueryInput::Vector(v) if v.len() > 4096 => return Err(RouteError::Limit),
                QueryInput::Hybrid { text, vector, .. }
                    if text.len() > 2048 || vector.len() > 4096 =>
                {
                    return Err(RouteError::Limit);
                }
                QueryInput::Lexical(t) if t.len() > 2048 => return Err(RouteError::Limit),
                _ => {}
            }
            32768
        }
        CallPayload::Skill {
            plan,
            definition,
            approvals,
        } => {
            if plan.bindings().len() > 8 || definition.calls().len() > 8 || approvals.len() > 8 {
                return Err(RouteError::Limit);
            }
            let t = plan.task().fields();
            let mut n = 16384usize
                .checked_add(t.objective.len() * 2)
                .and_then(|n| n.checked_add(t.resource_scope.resources().len() * 1536))
                .ok_or(RouteError::Limit)?;
            for action in &plan.proposal().fields().actions {
                let f = action.fields();
                if f.payload.len() > MAX_TOOL_INPUT_BYTES
                    || f.required_capabilities.len() > 64
                    || f.authority.oversight().checkpoints().len() > 64
                {
                    return Err(RouteError::Limit);
                }
                n = n
                    .checked_add(
                        4096 + f.resource_scope.resources().len() * 512
                            + f.authority.resource_scope().resources().len() * 512
                            + f.required_capabilities.len() * 256
                            + f.authority.oversight().checkpoints().len() * 256
                            + f.payload.len() * 2,
                    )
                    .ok_or(RouteError::Limit)?;
            }
            if n > 65536 {
                return Err(RouteError::Limit);
            }
            n
        }
    };
    c.charge(preflight * 3)?;
    c.reserve(preflight * 4 + 131072)?;
    let body = match payload {
        CallPayload::Retrieval(q) => Value::List(vec![text("retrieval"), query(q)?]),
        CallPayload::Skill {
            plan,
            definition,
            approvals,
        } => {
            let task = codec::encode_record(&WireRecord::Task(plan.task().clone()))
                .map_err(|_| RouteError::InvalidInput)?;
            let proposal = codec::encode_record(&WireRecord::Proposal {
                task: plan.task().clone(),
                proposal: plan.proposal().clone(),
            })
            .map_err(|_| RouteError::InvalidInput)?;
            Value::List(vec![
                text("skill"),
                Value::Bytes(task),
                Value::Bytes(proposal),
                Value::List(plan.bindings().iter().map(|b| b.to_value()).collect()),
                text(definition.id().as_str()),
                number(definition.revision().get()),
                number(u64::from(definition.iterations())),
                Value::List(
                    definition
                        .calls()
                        .iter()
                        .map(|d| {
                            Value::List(vec![
                                text(d.reference.project.as_str()),
                                text(d.reference.id.as_str()),
                                text(d.skill.as_str()),
                                number(d.revision.get()),
                            ])
                        })
                        .collect(),
                ),
                Value::List(approvals.iter().map(|id| text(id.as_str())).collect()),
            ])
        }
    };
    let value = Value::List(vec![
        text("route-call-v1"),
        entry(e),
        request(r),
        Value::List(
            a.iter()
                .map(|a| {
                    Value::List(vec![
                        text(a.route.as_str()),
                        number(u64::from(a.utility)),
                        pin(&a.score_profile),
                        a.provenance.as_ref().map_or(Value::Null, source),
                    ])
                })
                .collect(),
        ),
        body,
    ]);
    let bytes = encode(&value, 65536, c)?;
    c.charge(bytes.len())?;
    let digest = crate::native::NativeHash
        .digest(&bytes)
        .map_err(|_| RouteError::Unavailable)?;
    c.check()?;
    Ok(digest)
}

/// Exact lexical overlap admission, not semantic relevance or a model score.
/// Input is the opaque response of the unchanged owned G70 host, never raw caller bytes.
pub(super) fn lexical_no_match(
    response: &crate::retrieval::RetrievalResponse,
    c: &mut RouteControl,
) -> Result<bool, RouteError> {
    let bytes = response.bytes();
    if bytes.len() > 65536 {
        return Err(RouteError::Limit);
    }
    c.charge(bytes.len() * 4 + 4096)?;
    c.reserve(bytes.len() * 3 + 512 * std::mem::size_of::<Value>())?;
    let v = codec::decode(bytes).map_err(|_| RouteError::InvalidInput)?;
    let Value::List(root) = v else {
        return Err(RouteError::InvalidInput);
    };
    if root.len() != 8
        || root[0] != text("retrieval-result")
        || root[1] != number(1)
        || root[5] != text("lexical")
    {
        return Err(RouteError::InvalidInput);
    }
    let Value::List(hits) = &root[7] else {
        return Err(RouteError::InvalidInput);
    };
    if hits.len() > 32 {
        return Err(RouteError::Limit);
    }
    let mut positive = false;
    for hit in hits {
        let Value::List(fields) = hit else {
            return Err(RouteError::InvalidInput);
        };
        if fields.len() != 10 {
            return Err(RouteError::InvalidInput);
        };
        let Value::Signed(score) = fields[8] else {
            return Err(RouteError::InvalidInput);
        };
        if score > 0 {
            positive = true;
        }
    }
    c.check()?;
    Ok(!positive)
}
