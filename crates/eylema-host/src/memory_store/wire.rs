//! Closed versioned memory events; public values do not authenticate a journal.
use super::{MemoryResult as R, MemoryStoreError as E, Stored};
use eylema_contracts::{
    Revision,
    codec::{self, Value},
    memory::durable::*,
    platform::{ProjectId, SemanticProfileId},
    project_data::{DataId, DataRef},
};
pub(super) fn text(s: &str) -> Value {
    Value::Text(s.into())
}
pub(super) fn list(v: Vec<Value>) -> Value {
    Value::List(v)
}
fn optional<T>(v: &Option<T>, f: impl FnOnce(&T) -> Value) -> Value {
    v.as_ref().map_or(Value::Null, f)
}
fn number(n: u64) -> Value {
    Value::Unsigned(n)
}
fn bytes(v: [u8; 32]) -> Value {
    Value::Bytes(v.to_vec())
}
pub(super) fn representation(p: &RepresentationProfile) -> Value {
    list(vec![
        text(p.space.id.as_str()),
        text(p.space.model.as_str()),
        text(p.space.preprocessing.as_str()),
        number(p.space.revision.get()),
        number(p.space.dimensions as u64),
        bytes(p.manifest_sha256),
        bytes(p.artifact_sha256),
        bytes(p.tokenizer_sha256),
        bytes(p.preprocessing_sha256),
    ])
}
pub(super) fn header(p: &StoreProfile) -> R<Value> {
    let predicates = p
        .predicates
        .iter()
        .map(|v| {
            list(vec![
                text(v.id.as_str()),
                text(match v.cardinality {
                    Cardinality::SingleValue => "single",
                    Cardinality::SetValue => "set",
                }),
            ])
        })
        .collect();
    let identity = codec::encode(&list(vec![
        text(p.schema.as_str()),
        number(p.revision.get()),
        list(predicates),
        representation(&p.representation),
    ]))
    .map_err(|_| E::InvalidRequest)?;
    Ok(list(vec![
        text("eyleme.memory"),
        number(1),
        text(p.project.as_str()),
        text(p.id.as_str()),
        text(p.schema.as_str()),
        number(p.revision.get()),
        Value::Bytes(identity),
    ]))
}
pub(super) fn pin(p: &SourcePin) -> Value {
    list(vec![
        text(p.reference.project.as_str()),
        text(p.reference.id.as_str()),
        bytes(p.put_digest),
    ])
}
pub(super) fn object(o: &TypedObject) -> Value {
    match o {
        TypedObject::Entity(e) => list(vec![text("entity"), text(e.as_str())]),
        TypedObject::Text(s) => list(vec![text("text"), text(s)]),
        TypedObject::Integer(n) => list(vec![text("integer"), Value::Signed(*n)]),
        TypedObject::Boolean(b) => list(vec![text("boolean"), Value::Bool(*b)]),
    }
}
pub(super) fn record(s: &Stored) -> Value {
    let d = &s.record.draft;
    list(vec![
        text(d.project.as_str()),
        text(d.id.as_str()),
        number(d.revision.get()),
        text(d.profile.as_str()),
        text(d.schema.as_str()),
        text(d.kind.as_str()),
        text(d.origin.as_str()),
        text(s.record.admission.as_str()),
        number(d.observed_at),
        number(d.valid_from),
        number(d.expires_at),
        optional(&d.confidence_bps, |v| number(*v as u64)),
        optional(&d.subject, |v| text(v.as_str())),
        optional(&d.predicate, |v| text(v.as_str())),
        optional(&d.object, object),
        optional(&d.episode_status, |v| text(v.as_str())),
        optional(&d.episode_text, |v| text(v)),
        list(d.sources.iter().map(pin).collect()),
        pin(&s.record.projection),
        optional(&s.vector, |v| {
            Value::Bytes(v.iter().flat_map(|n| n.to_le_bytes()).collect())
        }),
    ])
}
pub(super) fn event(
    generation: u64,
    op: &str,
    id: &RecordId,
    expected: Option<Revision>,
    record_revision: Revision,
    s: Option<&Stored>,
) -> Value {
    list(vec![
        text("memory.event"),
        number(1),
        number(generation),
        text(op),
        text(id.as_str()),
        optional(&expected, |v| number(v.get())),
        number(record_revision.get()),
        s.map_or(Value::Null, record),
    ])
}
pub(super) fn array(v: &Value, len: usize) -> R<&[Value]> {
    match v {
        Value::List(a) if a.len() == len => Ok(a),
        _ => Err(E::InvalidRequest),
    }
}
pub(super) fn string(v: &Value) -> R<&str> {
    match v {
        Value::Text(s) => Ok(s),
        _ => Err(E::InvalidRequest),
    }
}
pub(super) fn uint(v: &Value) -> R<u64> {
    match v {
        Value::Unsigned(n) => Ok(*n),
        _ => Err(E::InvalidRequest),
    }
}
pub(super) fn revision(v: &Value) -> R<Revision> {
    Revision::new(uint(v)?).map_err(|_| E::InvalidRequest)
}
fn parse_optional<T>(v: &Value, f: impl FnOnce(&Value) -> R<T>) -> R<Option<T>> {
    if *v == Value::Null {
        Ok(None)
    } else {
        f(v).map(Some)
    }
}
fn parse_id(v: &Value) -> R<DataId> {
    DataId::new(string(v)?).map_err(|_| E::InvalidRequest)
}
fn parse_profile(v: &Value) -> R<SemanticProfileId> {
    SemanticProfileId::new(string(v)?).map_err(|_| E::InvalidRequest)
}
fn parse_pin(v: &Value) -> R<SourcePin> {
    let a = array(v, 3)?;
    let digest = match &a[2] {
        Value::Bytes(b) => b.as_slice().try_into().map_err(|_| E::InvalidRequest)?,
        _ => return Err(E::InvalidRequest),
    };
    Ok(SourcePin {
        reference: DataRef {
            project: ProjectId::new(string(&a[0])?).map_err(|_| E::InvalidRequest)?,
            id: parse_id(&a[1])?,
        },
        put_digest: digest,
    })
}
fn parse_object(v: &Value) -> R<TypedObject> {
    let a = array(v, 2)?;
    match string(&a[0])? {
        "entity" => Ok(TypedObject::Entity(parse_id(&a[1])?)),
        "text" => Ok(TypedObject::Text(string(&a[1])?.into())),
        "integer" => match &a[1] {
            Value::Signed(n) => Ok(TypedObject::Integer(*n)),
            _ => Err(E::InvalidRequest),
        },
        "boolean" => match &a[1] {
            Value::Bool(b) => Ok(TypedObject::Boolean(*b)),
            _ => Err(E::InvalidRequest),
        },
        _ => Err(E::InvalidRequest),
    }
}
pub(super) fn decode(v: &Value, p: &StoreProfile) -> R<Stored> {
    let a = array(v, 20)?;
    let source_list = match &a[17] {
        Value::List(a) if a.len() <= 8 => a,
        _ => return Err(E::Limit),
    };
    let vector = parse_optional(&a[19], |v| match v {
        Value::Bytes(b) if b.len() == 4096 => Ok(b
            .chunks_exact(4)
            .map(|x| f32::from_le_bytes(x.try_into().expect("four bytes")))
            .collect::<Vec<_>>()),
        _ => Err(E::InvalidRequest),
    })?;
    let d = RecordDraft {
        project: ProjectId::new(string(&a[0])?).map_err(|_| E::InvalidRequest)?,
        id: parse_id(&a[1])?,
        revision: revision(&a[2])?,
        profile: parse_profile(&a[3])?,
        schema: parse_profile(&a[4])?,
        kind: RecordKind::parse(string(&a[5])?)?,
        origin: Origin::parse(string(&a[6])?)?,
        observed_at: uint(&a[8])?,
        valid_from: uint(&a[9])?,
        expires_at: uint(&a[10])?,
        confidence_bps: parse_optional(&a[11], |v| {
            u16::try_from(uint(v)?).map_err(|_| E::InvalidRequest)
        })?,
        subject: parse_optional(&a[12], parse_id)?,
        predicate: parse_optional(&a[13], parse_profile)?,
        object: parse_optional(&a[14], parse_object)?,
        episode_status: parse_optional(&a[15], |v| Ok(EpisodeStatus::parse(string(v)?)?))?,
        episode_text: parse_optional(&a[16], |v| Ok(string(v)?.into()))?,
        sources: source_list.iter().map(parse_pin).collect::<R<Vec<_>>>()?,
    };
    d.validate(p)?;
    let projection = parse_pin(&a[18])?;
    if projection.reference.project != p.project || projection.put_digest == [0; 32] {
        return Err(E::InvalidRequest);
    }
    Ok(Stored {
        record: Record {
            draft: d,
            admission: Admission::parse(string(&a[7])?)?,
            projection,
        },
        vector,
        deleted: false,
    })
}
pub(super) fn expected(v: &Value) -> R<Option<Revision>> {
    parse_optional(v, revision)
}
pub(super) fn escaped(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub(super) fn projection(d: &RecordDraft) -> R<Vec<u8>> {
    let object = match &d.object {
        Some(TypedObject::Entity(v)) => v.as_str().to_string(),
        Some(TypedObject::Text(v)) => v.clone(),
        Some(TypedObject::Integer(v)) => v.to_string(),
        Some(TypedObject::Boolean(v)) => v.to_string(),
        None if d.kind == RecordKind::Entity => d.id.as_str().to_string(),
        None => format!(
            "{}:{}",
            d.episode_status.map_or("unknown", |s| s.as_str()),
            d.episode_text.as_deref().unwrap_or("")
        ),
    };
    let body = format!(
        "kind={}; subject={}; predicate={}; object={}\n",
        d.kind.as_str(),
        d.subject.as_ref().map_or("", |s| s.as_str()),
        d.predicate.as_ref().map_or("", |s| s.as_str()),
        escaped(&object)
    )
    .into_bytes();
    if body.len() > 1024 {
        return Err(E::Limit);
    }
    Ok(body)
}
