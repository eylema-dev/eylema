use crate::RetrievalError as E;
use crate::{
    wire::{self, array, digest, list, number, string, text},
    *,
};
use eylema_contracts::{
    Revision,
    codec::Value,
    memory::EmbeddingSpace,
    platform::{ModelProfileId, ProjectId, SemanticProfileId},
    project_data::{DataId, DataRef},
};
use eylema_journal::Sha256;
use std::ops::Range;
pub struct Chunk {
    range: Range<usize>,
    text: String,
    hash: [u8; 32],
    terms: Vec<String>,
    vector: Vec<f32>,
}
impl Chunk {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn text_hash(&self) -> [u8; 32] {
        self.hash
    }
}
pub struct Document {
    reference: DataRef,
    digest: [u8; 32],
    chunks: Vec<Chunk>,
}
impl Document {
    pub fn reference(&self) -> &DataRef {
        &self.reference
    }
    pub fn put_digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }
}
pub struct Snapshot {
    project: ProjectId,
    binding: RepresentationBinding,
    generation: u64,
    documents: Vec<Document>,
    retained: usize,
}
impl Snapshot {
    pub fn project(&self) -> &ProjectId {
        &self.project
    }
    pub fn binding(&self) -> &RepresentationBinding {
        &self.binding
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained
    }
    pub fn build(
        project: ProjectId,
        binding: RepresentationBinding,
        generation: u64,
        mut inputs: Vec<DocumentInput>,
        hash: &mut impl Sha256,
        c: &mut Control,
    ) -> RetrievalResult<Self> {
        binding.validate()?;
        if generation == 0 || inputs.len() > 16 {
            return Err(E::InvalidInput);
        }
        let d = usize::from(binding.space.dimensions);
        let mut total = 0usize;
        let mut retained = 4096usize;
        for x in &inputs {
            if x.reference.project != project {
                return Err(E::WrongProject);
            }
            if x.put_digest == [0; 32]
                || x.body.is_empty()
                || x.body.len() > 8192
                || x.vectors.len() > 128
            {
                return Err(E::InvalidInput);
            }
            total = total.checked_add(x.vectors.len()).ok_or(E::LimitExceeded)?;
            let n = x
                .body
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(x.vectors.len() * (d * 4 + 128 * 88 + 512)))
                .ok_or(E::LimitExceeded)?;
            retained = retained.checked_add(n).ok_or(E::LimitExceeded)?;
            if x.vectors.iter().any(|v| v.len() != d) {
                return Err(E::InvalidVector);
            }
        }
        if total > 128
            || total
                .checked_mul(d * 4)
                .is_none_or(|n| n > wire::MAX_SNAPSHOT)
        {
            return Err(E::LimitExceeded);
        }
        c.reserve(retained)?;
        c.charge(retained + 16 * 16 * 128)?;
        inputs.sort_by(|a, b| a.reference.cmp(&b.reference));
        if inputs.windows(2).any(|w| w[0].reference == w[1].reference) {
            return Err(E::InvalidInput);
        }
        let mut docs = Vec::with_capacity(inputs.len());
        let mut count = 0;
        for input in inputs {
            let rs = chunk::ranges(&input.body, c)?;
            if rs.len() != input.vectors.len() {
                return Err(E::InvalidInput);
            }
            count += rs.len();
            if count > 128 {
                return Err(E::LimitExceeded);
            }
            let s = std::str::from_utf8(&input.body).map_err(|_| E::InvalidInput)?;
            let mut chunks = Vec::with_capacity(rs.len());
            for (r, v) in rs.into_iter().zip(input.vectors) {
                score::unit_norm(&v, d, c)?;
                let t = &s[r.clone()];
                c.charge(t.len() * 2)?;
                let terms = match chunk::terms(t, 128, c) {
                    Ok(v) => v,
                    Err(E::InvalidInput) => vec![],
                    Err(e) => return Err(e),
                };
                chunks.push(Chunk {
                    range: r,
                    text: t.into(),
                    hash: hash.digest(t.as_bytes())?,
                    terms,
                    vector: v,
                });
            }
            docs.push(Document {
                reference: input.reference,
                digest: input.put_digest,
                chunks,
            });
        }
        let s = Self {
            project,
            binding,
            generation,
            documents: docs,
            retained,
        };
        s.encode(c)?;
        Ok(s)
    }
    fn encoded_size(&self) -> RetrievalResult<usize> {
        let b = &self.binding;
        let binding = 5
            + 3 * 5
            + b.space.id.as_str().len()
            + b.space.model.as_str().len()
            + b.space.preprocessing.as_str().len()
            + 18
            + 4 * 37
            + 3 * 5
            + b.chunk_profile.len()
            + b.lexical_profile.len()
            + b.score_profile.len();
        let mut n = 7
            + 5
            + 5
            + "retrieval-snapshot".len()
            + 9
            + 5
            + self.project.as_str().len()
            + binding
            + 9
            + 5;
        for d in &self.documents {
            n += 5
                + 5
                + d.reference.project.as_str().len()
                + 5
                + d.reference.id.as_str().len()
                + 37
                + 5;
            for x in &d.chunks {
                n += 5
                    + 18
                    + 37
                    + 5
                    + x.text.len()
                    + 5
                    + x.terms.iter().map(|s| 5 + s.len()).sum::<usize>()
                    + 5
                    + x.vector.len() * 4;
            }
        }
        if n > wire::MAX_SNAPSHOT {
            return Err(E::LimitExceeded);
        }
        Ok(n)
    }
    pub fn encode(&self, c: &mut Control) -> RetrievalResult<Value> {
        let n = self.encoded_size()?;
        c.charge(n)?;
        c.reserve(self.retained)?;
        let v = Value::List(vec![
            text("retrieval-snapshot"),
            Value::Unsigned(1),
            text(self.project.as_str()),
            encode_binding(&self.binding),
            Value::Unsigned(self.generation),
            Value::List(
                self.documents
                    .iter()
                    .map(|d| {
                        Value::List(vec![
                            text(d.reference.project.as_str()),
                            text(d.reference.id.as_str()),
                            Value::Bytes(d.digest.to_vec()),
                            Value::List(
                                d.chunks
                                    .iter()
                                    .map(|x| {
                                        Value::List(vec![
                                            Value::Unsigned(x.range.start as u64),
                                            Value::Unsigned(x.range.end as u64),
                                            Value::Bytes(x.hash.to_vec()),
                                            text(&x.text),
                                            Value::List(x.terms.iter().map(|s| text(s)).collect()),
                                            Value::Bytes(
                                                x.vector
                                                    .iter()
                                                    .flat_map(|v| v.to_le_bytes())
                                                    .collect(),
                                            ),
                                        ])
                                    })
                                    .collect(),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ]);
        wire::extent(&v)?;
        Ok(v)
    }
    pub fn decode(v: &Value, hash: &mut impl Sha256, c: &mut Control) -> RetrievalResult<Self> {
        let n = wire::extent(v)?;
        c.reserve(n * 4 + 32768 * std::mem::size_of::<Value>())?;
        c.charge(n * 4)?;
        let e = list(v, 6)?;
        if string(&e[0])? != "retrieval-snapshot" || number(&e[1])? != 1 {
            return Err(E::CorruptSnapshot);
        }
        let project = ProjectId::new(string(&e[2])?).map_err(|_| E::CorruptSnapshot)?;
        let binding = decode_binding(&e[3])?;
        let generation = number(&e[4])?;
        let mut inputs = vec![];
        let mut previous = None;
        for v in array(&e[5], 16)? {
            let d = list(v, 4)?;
            let reference = DataRef {
                project: ProjectId::new(string(&d[0])?).map_err(|_| E::CorruptSnapshot)?,
                id: DataId::new(string(&d[1])?).map_err(|_| E::CorruptSnapshot)?,
            };
            if previous.as_ref().is_some_and(|p| p >= &reference) {
                return Err(E::CorruptSnapshot);
            }
            previous = Some(reference.clone());
            let mut body = vec![];
            let mut vectors = vec![];
            for v in array(&d[3], 128)? {
                let x = list(v, 6)?;
                let start = number(&x[0])?;
                let end = number(&x[1])?;
                let t = string(&x[3])?;
                if start != body.len() as u64
                    || end != start + t.len() as u64
                    || end > 8192
                    || t.is_empty()
                    || t.len() > 1024
                    || hash.digest(t.as_bytes())? != digest(&x[2])?
                {
                    return Err(E::CorruptSnapshot);
                }
                body.extend_from_slice(t.as_bytes());
                let Value::Bytes(b) = &x[5] else {
                    return Err(E::CorruptSnapshot);
                };
                if b.len() != usize::from(binding.space.dimensions) * 4 {
                    return Err(E::CorruptSnapshot);
                }
                vectors.push(
                    b.chunks_exact(4)
                        .map(|b| f32::from_le_bytes(b.try_into().expect("four bytes")))
                        .collect(),
                );
            }
            inputs.push(DocumentInput {
                reference,
                put_digest: digest(&d[2])?,
                body,
                vectors,
            });
        }
        let s =
            Self::build(project, binding, generation, inputs, hash, c).map_err(|e| match e {
                E::Cancelled | E::Deadline | E::LimitExceeded => e,
                _ => E::CorruptSnapshot,
            })?;
        if s.encode(c)? != *v {
            return Err(E::CorruptSnapshot);
        }
        Ok(s)
    }
    pub fn search(&self, q: &SearchQuery, c: &mut Control) -> RetrievalResult<Ranked> {
        q.validate()?;
        if q.project != self.project {
            return Err(E::WrongProject);
        }
        if q.binding != self.binding {
            return Err(E::IncompatibleBinding);
        }
        if q.generation != self.generation {
            return Err(E::StaleGeneration);
        }
        c.reserve(self.retained)?;
        c.charge(16 * 16 * 128)?;
        let (mode, weight, vector, terms) = match &q.input {
            QueryInput::Vector(v) => ("vector", None, Some(v), None),
            QueryInput::Lexical(t) => ("lexical", None, None, Some(query_terms(t, c)?)),
            QueryInput::Hybrid {
                text,
                vector,
                vector_weight,
            } => {
                if !(1..1_000_000).contains(vector_weight) {
                    return Err(E::InvalidInput);
                }
                (
                    "hybrid",
                    Some(*vector_weight),
                    Some(vector),
                    Some(query_terms(text, c)?),
                )
            }
        };
        if let Some(v) = vector {
            score::unit_norm(v, usize::from(self.binding.space.dimensions), c)?;
        }
        let mut hits = vec![];
        for reference in &q.sources {
            let d = self
                .documents
                .iter()
                .find(|d| &d.reference == reference)
                .ok_or(E::NotIndexed)?;
            for x in &d.chunks {
                let v = vector
                    .map(|v| score::vector_micro(v, &x.vector, v.len(), c))
                    .transpose()?;
                let l = terms
                    .as_ref()
                    .map(|t| {
                        if x.terms.is_empty() {
                            Ok(0)
                        } else {
                            score::jaccard_micro(t, &x.terms, c)
                        }
                    })
                    .transpose()?;
                let score = match (v, l, weight) {
                    (Some(v), Some(l), Some(w)) => score::hybrid_micro(v, l, w)?,
                    (Some(v), _, _) => v,
                    (_, Some(l), _) => l,
                    _ => return Err(E::InvalidInput),
                };
                c.reserve(x.text.len() + 1024)?;
                c.charge(x.text.len() + 1024)?;
                hits.push(Value::List(vec![
                    text(d.reference.project.as_str()),
                    text(d.reference.id.as_str()),
                    Value::Bytes(d.digest.to_vec()),
                    Value::Unsigned(x.range.start as u64),
                    Value::Unsigned(x.range.end as u64),
                    Value::Bytes(x.hash.to_vec()),
                    text(&x.text),
                    v.map_or(Value::Null, Value::Signed),
                    l.map_or(Value::Null, Value::Signed),
                    Value::Signed(score),
                ]));
            }
        }
        c.charge(128 * 128 * 256)?;
        hits.sort_by(|a, b| {
            let a = list(a, 10).expect("closed hit");
            let b = list(b, 10).expect("closed hit");
            let Value::Signed(sa) = a[9] else {
                unreachable!()
            };
            let Value::Signed(sb) = b[9] else {
                unreachable!()
            };
            sb.cmp(&sa)
                .then_with(|| string(&a[1]).unwrap().cmp(string(&b[1]).unwrap()))
                .then_with(|| number(&a[3]).unwrap().cmp(&number(&b[3]).unwrap()))
        });
        let mut take = q.top_k.min(hits.len());
        while take > 0
            && take < hits.len()
            && list(&hits[take - 1], 10)?[9] == list(&hits[take], 10)?[9]
        {
            take += 1;
        }
        if take > q.max_hits {
            return Err(E::TieLimit);
        }
        hits.truncate(take);
        let mut source_ids = Vec::new();
        for h in &hits {
            let s = string(&list(h, 10)?[1])?;
            if !source_ids.contains(&s) {
                source_ids.push(s)
            }
        }
        let sources = source_ids.len();
        let value = Value::List(vec![
            text("retrieval-result"),
            Value::Unsigned(1),
            text(self.project.as_str()),
            encode_binding(&self.binding),
            Value::Unsigned(self.generation),
            text(mode),
            weight.map_or(Value::Null, |w| Value::Unsigned(w.into())),
            Value::List(hits),
        ]);
        let ranked = Ranked {
            value,
            hits: take,
            sources,
        };
        ranked.encode(q.max_bytes, c)?;
        Ok(ranked)
    }
}
fn query_terms(t: &str, c: &mut Control) -> RetrievalResult<Vec<String>> {
    if t.len() > 2048 {
        return Err(E::LimitExceeded);
    }
    chunk::terms(t, 32, c)
}
pub struct Ranked {
    value: Value,
    hits: usize,
    sources: usize,
}
impl Ranked {
    pub fn hit_count(&self) -> usize {
        self.hits
    }
    pub fn source_count(&self) -> usize {
        self.sources
    }
    pub fn encode(&self, max_bytes: usize, c: &mut Control) -> RetrievalResult<Vec<u8>> {
        if max_bytes == 0 || max_bytes > 65536 {
            return Err(E::LimitExceeded);
        }
        wire::encoded(&self.value, max_bytes, c)
    }
}
fn encode_binding(b: &RepresentationBinding) -> Value {
    Value::List(vec![
        text(b.space.id.as_str()),
        text(b.space.model.as_str()),
        text(b.space.preprocessing.as_str()),
        Value::Unsigned(b.space.revision.get()),
        Value::Unsigned(b.space.dimensions.into()),
        Value::Bytes(b.manifest_sha256.to_vec()),
        Value::Bytes(b.artifact_sha256.to_vec()),
        Value::Bytes(b.tokenizer_sha256.to_vec()),
        Value::Bytes(b.preprocessing_sha256.to_vec()),
        text(&b.chunk_profile),
        text(&b.lexical_profile),
        text(&b.score_profile),
    ])
}
fn decode_binding(v: &Value) -> RetrievalResult<RepresentationBinding> {
    let b = list(v, 12)?;
    let out = RepresentationBinding {
        space: EmbeddingSpace {
            id: SemanticProfileId::new(string(&b[0])?).map_err(|_| E::CorruptSnapshot)?,
            model: ModelProfileId::new(string(&b[1])?).map_err(|_| E::CorruptSnapshot)?,
            preprocessing: SemanticProfileId::new(string(&b[2])?)
                .map_err(|_| E::CorruptSnapshot)?,
            revision: Revision::new(number(&b[3])?).map_err(|_| E::CorruptSnapshot)?,
            dimensions: u16::try_from(number(&b[4])?).map_err(|_| E::CorruptSnapshot)?,
        },
        manifest_sha256: digest(&b[5])?,
        artifact_sha256: digest(&b[6])?,
        tokenizer_sha256: digest(&b[7])?,
        preprocessing_sha256: digest(&b[8])?,
        chunk_profile: string(&b[9])?.into(),
        lexical_profile: string(&b[10])?.into(),
        score_profile: string(&b[11])?.into(),
    };
    out.validate()?;
    Ok(out)
}
