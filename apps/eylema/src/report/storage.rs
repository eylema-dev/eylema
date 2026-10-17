//! G63/G88 storage consumer; retained records never constitute a live permission.
use super::{source::parse_document, types::*};
use eylema_contracts::{
    codec::{self, Value},
    memory::{EmbeddingSpace, durable::*},
    platform::{ModelProfileId, ProjectId, RoleKind, SemanticProfileId},
    project_data::*,
    *,
};
use eylema_host::{
    Owner,
    memory_store::*,
    native::NativeHash,
    project_data::{DataSession, NativeDataClock, ProjectStore, project_journal_limits},
    tool::{ToolReport, ToolStatus},
};
use eylema_journal::{Journal, NativeStorage};
use std::{
    path::Path,
    time::{Duration, Instant},
};
pub type Data = ProjectStore<NativeStorage, NativeHash, NativeDataClock>;
pub type Memory = MemoryStore<NativeStorage, NativeHash>;
pub fn id(s: &str) -> Result<DataId> {
    DataId::new(s).map_err(|_| ReportError::InvalidInput)
}
pub fn semantic(s: &str) -> Result<SemanticProfileId> {
    SemanticProfileId::new(s).map_err(|_| ReportError::InvalidInput)
}
pub fn rev(n: u64) -> Revision {
    Revision::new(n).expect("bounded nonzero revision")
}
pub fn permissions() -> DataPermissions {
    DataPermissions::new(vec![DataUse::Execute, DataUse::Collect]).expect("closed purposes")
}
pub fn sync(f: &std::fs::File) -> std::io::Result<()> {
    eylema_platform::full_sync_file(f).map_err(std::io::Error::other)
}
pub fn memory_control() -> Result<MemoryControl> {
    MemoryControl::new(
        MemoryBudget {
            max_work: 100_000_000,
            max_live_bytes: 67108864,
            deadline: Instant::now() + Duration::from_secs(5),
        },
        MemoryCancel::new(),
    )
    .map_err(|_| ReportError::Limit)
}
pub fn pin(m: &DataMetadata) -> SourcePin {
    SourcePin {
        reference: m.reference.clone(),
        put_digest: m.digest,
    }
}
pub fn data_error(e: eylema_host::project_data::DataError) -> ReportError {
    use eylema_host::project_data::DataError::*;
    match e {
        RevisionConflict => ReportError::RevisionConflict,
        Journal(_) | Poisoned => ReportError::OutcomeUnknown,
        BudgetExceeded => ReportError::Limit,
        _ => ReportError::Forbidden,
    }
}
pub fn check_pin(data: &mut Data, session: &DataSession, p: &SourcePin) -> Result<()> {
    let m = data
        .authorized_metadata(session, &p.reference, DataUse::Execute)
        .map_err(data_error)?;
    if m.digest != p.put_digest {
        return Err(ReportError::RevisionConflict);
    }
    Ok(())
}
pub fn validate_checkpoint(
    r: &ToolReport,
    e: &ExecutionId,
    a: &ActionId,
    count: u64,
) -> Result<()> {
    if &r.execution_id != e
        || r.status != ToolStatus::Completed
        || r.outputs.len() != 1
        || &r.outputs[0].action_id != a
        || codec::decode(&r.outputs[0].bytes).ok() != Some(Value::Unsigned(count))
    {
        return Err(ReportError::InvalidWitness);
    }
    Ok(())
}
pub struct SourceRow {
    pub id: String,
    pub entity: String,
    pub body: Vec<u8>,
}
pub struct SourceValue {
    pub entity: String,
    pub document: SourceDocument,
    pub pin: SourcePin,
    pub authorization_revision: Revision,
}
pub struct JobData {
    pub data: Data,
    pub owner: Owner,
    pub session: DataSession,
    pub memory: Memory,
    pub memory_owner: MemoryOwner,
    pub memory_session: MemorySession,
    pub profile: StoreProfile,
    pub sources: Vec<SourceValue>,
    source_manifest: Vec<SourcePin>,
    pub expires: u64,
    pub operations: u32,
    pub work: u64,
}
pub fn profile(project: ProjectId) -> Result<StoreProfile> {
    let m = eylema_contracts::model_contract::ModelManifest::from_bytes(include_bytes!(
        "../../../../crates/eylema-model/tests/fixtures/g68/embedding-profile.v1.eyl"
    ))
    .map_err(|_| ReportError::Unsupported)?;
    let hash = |name: &str| {
        m.fields()
            .components
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.sha256)
            .ok_or(ReportError::Unsupported)
    };
    Ok(StoreProfile {
        project,
        id: semantic("report-memory:v1")?,
        schema: semantic("report-records:v1")?,
        revision: rev(1),
        predicates: vec![
            PredicateProfile {
                id: semantic("owner")?,
                cardinality: Cardinality::SingleValue,
            },
            PredicateProfile {
                id: semantic("checklist")?,
                cardinality: Cardinality::SingleValue,
            },
        ],
        representation: RepresentationProfile {
            space: EmbeddingSpace {
                id: semantic("qwen3-embedding-1024:v1")?,
                model: ModelProfileId::new("qwen3-embedding-0.6b")
                    .map_err(|_| ReportError::Unsupported)?,
                preprocessing: semantic("eylema.qwen3-embedding-text.v1")?,
                revision: rev(1),
                dimensions: 1024,
            },
            manifest_sha256: eylema_platform::sha256(
                &m.to_bytes().map_err(|_| ReportError::Unsupported)?,
            )
            .map_err(|_| ReportError::Unsupported)?,
            artifact_sha256: hash("model.safetensors")?,
            tokenizer_sha256: hash("tokenizer.json")?,
            preprocessing_sha256: hash("embedding-profile.v1.json")?,
        },
    })
}
pub fn projection_name(record: &str, revision: u64) -> String {
    format!("m:{record}:{revision}")
}
pub fn scope(rows: &[SourceRow]) -> Result<Vec<DataId>> {
    let mut scope = vec![];
    for row in rows {
        id(&row.entity)?;
        scope.push(id(&row.id)?);
        parse_document(&row.body)?;
        for record in [
            row.entity.clone(),
            format!("f:{}:owner:{}", row.entity, row.id),
            format!("f:{}:checklist:{}", row.entity, row.id),
        ] {
            for version in [1, 2] {
                scope.push(id(&projection_name(&record, version))?);
            }
        }
    }
    for name in [
        "review",
        "need",
        "model:documents",
        "model:queries",
        "model:plans",
        "model:decisions",
        "prepared",
        "absence",
        "count",
        "episode",
        "episode:projection",
        "approval",
        "final",
        "outcome",
        "expire:source",
        "expire:output",
    ] {
        scope.push(id(name)?)
    }
    scope.sort();
    scope.dedup();
    if scope.len() > 64 {
        return Err(ReportError::Limit);
    }
    Ok(scope)
}
impl JobData {
    pub fn create(dir: &Path, project: ProjectId, rows: &[SourceRow]) -> Result<Self> {
        if rows.is_empty()
            || rows.len() > 16
            || rows.iter().map(|r| r.body.len()).sum::<usize>() > 8192
        {
            return Err(ReportError::Limit);
        }
        let scope = scope(rows)?;
        let journal = Journal::create_new(
            NativeStorage::create_new(&dir.join("data.eylj"), sync)
                .map_err(|_| ReportError::OutcomeUnknown)?,
            NativeHash,
            project_journal_limits(),
        )
        .map_err(|_| ReportError::OutcomeUnknown)?;
        let (mut data, owner) = ProjectStore::create_new(journal, project.clone(), NativeDataClock)
            .map_err(data_error)?;
        let now = data.trusted_time().map_err(data_error)?;
        let expires = now.checked_add(86400000).ok_or(ReportError::Limit)?;
        let session = data
            .enroll(
                &owner,
                DataGrant {
                    permissions: permissions(),
                    scope,
                    expires_at: expires,
                },
            )
            .map_err(data_error)?;
        let profile = profile(project)?;
        let mut c = memory_control()?;
        let (memory, memory_owner) = MemoryStore::create_new(
            NativeStorage::create_new(&dir.join("memory.eylj"), sync)
                .map_err(|_| ReportError::OutcomeUnknown)?,
            NativeHash,
            &mut data,
            &owner,
            profile.clone(),
            &mut c,
        )
        .map_err(|_| ReportError::OutcomeUnknown)?;
        let memory_session = memory
            .enroll(&memory_owner, &mut data, &session, &mut c)
            .map_err(|_| ReportError::Forbidden)?;
        let mut this = Self {
            data,
            owner,
            session,
            memory,
            memory_owner,
            memory_session,
            profile,
            sources: vec![],
            source_manifest: vec![],
            expires,
            operations: 2,
            work: 200_000_000,
        };
        for row in rows {
            let meta = this.put(&row.id, row.body.clone(), vec![], DataKind::Artifact)?;
            this.source_manifest.push(pin(&meta));
            this.sources.push(SourceValue {
                entity: row.entity.clone(),
                document: parse_document(&row.body)?,
                pin: pin(&meta),
                authorization_revision: meta.revision,
            });
        }
        Ok(this)
    }
    pub fn reference(&self, name: &str) -> Result<DataRef> {
        Ok(DataRef {
            project: self.profile.project.clone(),
            id: id(name)?,
        })
    }
    pub fn put(
        &mut self,
        name: &str,
        payload: Vec<u8>,
        parents: Vec<DataRef>,
        kind: DataKind,
    ) -> Result<DataMetadata> {
        let cost = (payload.len() as u64)
            .checked_mul(16)
            .and_then(|n| n.checked_add(parents.len() as u64 * 256 + 1024))
            .ok_or(ReportError::Limit)?;
        self.charge(cost)?;
        let reference = self.reference(name)?;
        let meta = self
            .data
            .collect(
                &self.session,
                DataDraft {
                    reference: reference.clone(),
                    kind,
                    parents,
                    payload,
                    expires_at: self.expires,
                },
            )
            .map_err(data_error)?;
        self.data
            .set_policy(&self.owner, &reference, meta.revision, permissions(), false)
            .map_err(data_error)
    }
    pub fn authorize_sources(&mut self) -> Result<()> {
        self.charge(self.sources.len() as u64 * 1024)?;
        for s in &self.sources {
            let m = self
                .data
                .authorized_metadata(&self.session, &s.pin.reference, DataUse::Execute)
                .map_err(data_error)?;
            if m.digest != s.pin.put_digest || m.revision != s.authorization_revision {
                return Err(ReportError::RevisionConflict);
            }
        }
        Ok(())
    }
    pub(super) fn reserve_memory(&mut self) -> Result<()> {
        if self.operations >= 32 {
            return Err(ReportError::Limit);
        }
        self.charge(100_000_000)?;
        self.operations += 1;
        Ok(())
    }
    fn charge_memory(&mut self, c: &MemoryControl) -> Result<()> {
        if c.work_used() > 100_000_000 {
            return Err(ReportError::Limit);
        }
        Ok(())
    }
    fn charge(&mut self, n: u64) -> Result<()> {
        let next = self.work.checked_add(n).ok_or(ReportError::Limit)?;
        if next > 4_000_000_000 {
            return Err(ReportError::Limit);
        }
        self.work = next;
        Ok(())
    }
    pub fn draft(&mut self, source: usize, predicate: Option<&str>) -> Result<RecordDraft> {
        let s = self.sources.get(source).ok_or(ReportError::InvalidInput)?;
        let name = predicate.map_or(s.entity.clone(), |p| {
            format!("f:{}:{p}:{}", s.entity, s.pin.reference.id.as_str())
        });
        let value = match predicate {
            Some("owner") => s.document.owner.clone(),
            Some("checklist") => s.document.checklist.clone(),
            None => None,
            _ => return Err(ReportError::InvalidInput),
        };
        if predicate.is_some() && value.is_none() {
            return Err(ReportError::NoEvidence);
        }
        let now = self.data.trusted_time().map_err(data_error)?;
        Ok(RecordDraft {
            project: self.profile.project.clone(),
            id: id(&name)?,
            revision: rev(1),
            profile: self.profile.id.clone(),
            schema: self.profile.schema.clone(),
            kind: if predicate.is_none() {
                RecordKind::Entity
            } else {
                RecordKind::Fact
            },
            origin: Origin::DocumentClaim,
            observed_at: now,
            valid_from: now,
            expires_at: self.expires,
            confidence_bps: None,
            subject: Some(id(&s.entity)?),
            predicate: predicate.map(semantic).transpose()?,
            object: value.map(TypedObject::Text),
            episode_status: None,
            episode_text: None,
            sources: vec![s.pin.clone()],
        })
    }
    pub fn store_reviewed(
        &mut self,
        draft: RecordDraft,
        vector: Option<&[f32]>,
        review: SourcePin,
    ) -> Result<SourcePin> {
        let expected = Memory::preview_projection(&self.profile, &draft)
            .map_err(|_| ReportError::InvalidInput)?;
        let a = self.reference(&projection_name(draft.id.as_str(), 1))?;
        let b = self.reference(&projection_name(draft.id.as_str(), 2))?;
        self.reserve_memory()?;
        let mut c = memory_control()?;
        self.memory
            .propose(
                &mut self.data,
                &self.memory_session,
                self.memory.generation(),
                None,
                draft.clone(),
                vector,
                a,
                &mut c,
            )
            .map_err(|_| ReportError::OutcomeUnknown)?;
        self.charge_memory(&c)?;
        self.reserve_memory()?;
        let mut c = memory_control()?;
        self.memory
            .review(
                &self.memory_owner,
                &mut self.data,
                &self.owner,
                &self.memory_session,
                self.memory.generation(),
                &draft.id,
                rev(1),
                review,
                b.clone(),
                &mut c,
            )
            .map_err(|_| ReportError::OutcomeUnknown)?;
        self.charge_memory(&c)?;
        let meta = self
            .data
            .set_policy(&self.owner, &b, rev(1), permissions(), false)
            .map_err(data_error)?;
        if self
            .data
            .read(&self.session, &b, DataUse::Execute)
            .map_err(data_error)?
            != expected
        {
            return Err(ReportError::InvalidWitness);
        }
        Ok(pin(&meta))
    }
    pub fn query(&mut self, entity: &str, text: &str, vector: Vec<f32>) -> Result<MemoryContext> {
        self.query_scope(vec![id(entity)?], text, vector)
    }
    fn query_scope(
        &mut self,
        entities: Vec<DataId>,
        text: &str,
        vector: Vec<f32>,
    ) -> Result<MemoryContext> {
        self.authorize_sources()?;
        let now = self.data.trusted_time().map_err(data_error)?;
        let query = Query::new(QueryDraft {
            project: self.profile.project.clone(),
            profile: self.profile.id.clone(),
            revision: rev(1),
            generation: self.memory.generation(),
            role: RoleKind::Reasoning,
            purpose: DataUse::Execute,
            kinds: vec![RecordKind::Fact],
            origins: vec![Origin::DocumentClaim],
            knowledge: KnowledgeMode::Reusable,
            effective_at: now,
            entities,
            edge_predicates: vec![],
            result_predicates: vec![semantic("owner")?, semantic("checklist")?],
            seeds: vec![],
            hops: 0,
            rank: RankMode::Hybrid {
                text: text.into(),
                vector,
                vector_weight: 500000,
            },
            representation: Some(self.profile.representation.clone()),
            max_hits: 4,
            max_sources: 16,
            max_bytes: 16384,
            max_tokens: None,
        })
        .map_err(|_| ReportError::InvalidInput)?;
        self.reserve_memory()?;
        let mut c = memory_control()?;
        let result = self
            .memory
            .query(&mut self.data, &self.memory_session, &query, &mut c)
            .map_err(|_| ReportError::Forbidden)?;
        self.charge_memory(&c)?;
        Ok(result)
    }
    /// Absence is scoped to the complete, reviewed input manifest, never top-k alone.
    pub fn prove_absence(
        &mut self,
        need: &ReportNeed,
        query_receipt: &SourcePin,
        vector: Vec<f32>,
    ) -> Result<AbsenceWitness> {
        let context = self.query_scope(vec![], "Unknown owner checklist", vector)?;
        if context.status() == MemoryStatus::Conflict {
            return Err(ReportError::Conflict);
        }
        self.charge(65536)?;
        self.authorize_sources()?;
        if self.sources.len() != self.source_manifest.len()
            || self
                .sources
                .iter()
                .zip(&self.source_manifest)
                .any(|(s, p)| s.pin != *p)
        {
            return Err(ReportError::InvalidWitness);
        }
        for source in &self.sources {
            let body = self
                .data
                .read(&self.session, &source.pin.reference, DataUse::Execute)
                .map_err(data_error)?;
            if parse_document(&body)? != source.document {
                return Err(ReportError::InvalidWitness);
            }
        }
        let documents = self
            .sources
            .iter()
            .map(|s| s.document.clone())
            .collect::<Vec<_>>();
        absent(&documents)?;
        let coverage = verify_pins(&mut self.data, &self.session, &self.source_manifest)?;
        if query_receipt.reference != self.reference("model:queries")? {
            return Err(ReportError::InvalidWitness);
        }
        let query = verify_pins(
            &mut self.data,
            &self.session,
            std::slice::from_ref(query_receipt),
        )?;
        let need_hash = need.digest()?;
        if need.project != self.profile.project {
            return Err(ReportError::InvalidWitness);
        }
        let generation = self.memory.generation();
        let documents = documents_digest(&documents)?;
        let retrieval =
            eylema_platform::sha256(context.bytes()).map_err(|_| ReportError::Unsupported)?;
        let value = absence_value(
            need_hash, generation, documents, &coverage, &query, retrieval,
        );
        let mut parents = coverage.references();
        parents.extend(query.references());
        parents.sort();
        let record = self.put(
            "absence",
            super::encoded(&value)?,
            parents,
            DataKind::Derived,
        )?;
        Ok(AbsenceWitness {
            need: need_hash,
            generation,
            documents,
            coverage,
            query,
            retrieval,
            record: pin(&record),
        })
    }
}
fn absent(documents: &[SourceDocument]) -> Result<()> {
    if documents.is_empty() || documents.len() > 16 {
        return Err(ReportError::InvalidWitness);
    }
    if documents
        .iter()
        .any(|s| s.language == "en" && s.site == "Unknown")
    {
        return Err(ReportError::InvalidWitness);
    }
    Ok(())
}
fn documents_digest(documents: &[SourceDocument]) -> Result<[u8; 32]> {
    let value = Value::List(
        documents
            .iter()
            .map(|s| {
                Value::List(vec![
                    Value::Text(s.language.clone()),
                    Value::Text(s.site.clone()),
                    s.owner
                        .as_ref()
                        .map_or(Value::Null, |v| Value::Text(v.clone())),
                    s.checklist
                        .as_ref()
                        .map_or(Value::Null, |v| Value::Text(v.clone())),
                ])
            })
            .collect(),
    );
    eylema_platform::sha256(&super::encoded(&value)?).map_err(|_| ReportError::Unsupported)
}
fn absence_value(
    need: [u8; 32],
    generation: u64,
    documents: [u8; 32],
    coverage: &VerifiedPins,
    query: &VerifiedPins,
    retrieval: [u8; 32],
) -> Value {
    Value::List(vec![
        Value::Text("report:absence:v1".into()),
        Value::Bytes(need.to_vec()),
        Value::Unsigned(generation),
        Value::List(vec![
            Value::Text("en".into()),
            Value::Text("Unknown".into()),
            Value::Text("owner".into()),
            Value::Text("checklist".into()),
        ]),
        Value::Bytes(documents.to_vec()),
        coverage.value(),
        query.value(),
        Value::Bytes(retrieval.to_vec()),
    ])
}
/// Only a successful owned retrieval and complete current input coverage can mint this type.
pub struct AbsenceWitness {
    need: [u8; 32],
    generation: u64,
    documents: [u8; 32],
    coverage: VerifiedPins,
    query: VerifiedPins,
    retrieval: [u8; 32],
    record: SourcePin,
}
impl AbsenceWitness {
    pub fn validate(&self, need: &ReportNeed, documents: &[SourceDocument]) -> Result<()> {
        absent(documents)?;
        if self.need != need.digest()?
            || self.documents != documents_digest(documents)?
            || self.retrieval == [0; 32]
        {
            return Err(ReportError::InvalidWitness);
        }
        Ok(())
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn reference(&self) -> DataRef {
        self.record.reference.clone()
    }
    pub fn recheck(&self, data: &mut Data, session: &DataSession) -> Result<()> {
        self.coverage.recheck(data, session)?;
        self.query.recheck(data, session)?;
        check_pin(data, session, &self.record)?;
        let value = absence_value(
            self.need,
            self.generation,
            self.documents,
            &self.coverage,
            &self.query,
            self.retrieval,
        );
        if data
            .read(session, &self.record.reference, DataUse::Execute)
            .map_err(data_error)?
            != super::encoded(&value)?
        {
            return Err(ReportError::InvalidWitness);
        }
        let mut parents = self.coverage.references();
        parents.extend(self.query.references());
        parents.sort();
        let metadata = data
            .authorized_metadata(session, &self.record.reference, DataUse::Execute)
            .map_err(data_error)?;
        if metadata.parents != parents {
            return Err(ReportError::InvalidWitness);
        }
        Ok(())
    }
    pub fn reopen(
        data: &mut Data,
        session: &DataSession,
        need: &ReportNeed,
        rows: &[SourceRow],
        generation: u64,
    ) -> Result<Self> {
        let reference = DataRef {
            project: need.project.clone(),
            id: id("absence")?,
        };
        let bytes = data
            .read(session, &reference, DataUse::Execute)
            .map_err(data_error)?;
        let decoded = super::wire::decode_closed(&bytes)?;
        let fields = super::wire::list(&decoded)?;
        if fields.len() != 8 {
            return Err(ReportError::InvalidWitness);
        }
        let hash = |v: &Value| -> Result<[u8; 32]> {
            if let Value::Bytes(b) = v {
                b.as_slice()
                    .try_into()
                    .map_err(|_| ReportError::InvalidWitness)
            } else {
                Err(ReportError::InvalidWitness)
            }
        };
        let coverage = VerifiedPins::from_value(data, session, &fields[5])?;
        let query = VerifiedPins::from_value(data, session, &fields[6])?;
        let query_ref = DataRef {
            project: need.project.clone(),
            id: id("model:queries")?,
        };
        if query.references() != vec![query_ref] || coverage.references().len() != rows.len() {
            return Err(ReportError::InvalidWitness);
        }
        let mut expected_pins = vec![];
        let mut documents = vec![];
        for row in rows {
            let r = DataRef {
                project: need.project.clone(),
                id: id(&row.id)?,
            };
            let m = data
                .authorized_metadata(session, &r, DataUse::Execute)
                .map_err(data_error)?;
            if data
                .read(session, &r, DataUse::Execute)
                .map_err(data_error)?
                != row.body
            {
                return Err(ReportError::RevisionConflict);
            }
            expected_pins.push(pin(&m));
            documents.push(parse_document(&row.body)?);
        }
        if verify_pins(data, session, &expected_pins)?.value() != coverage.value() {
            return Err(ReportError::InvalidWitness);
        }
        let metadata = data
            .authorized_metadata(session, &reference, DataUse::Execute)
            .map_err(data_error)?;
        let result = Self {
            need: hash(&fields[1])?,
            generation,
            documents: hash(&fields[4])?,
            coverage,
            query,
            retrieval: hash(&fields[7])?,
            record: pin(&metadata),
        };
        result.validate(need, &documents)?;
        if decoded
            != absence_value(
                result.need,
                generation,
                result.documents,
                &result.coverage,
                &result.query,
                result.retrieval,
            )
        {
            return Err(ReportError::InvalidWitness);
        }
        result.recheck(data, session)?;
        let prepared = DataRef {
            project: need.project.clone(),
            id: id("prepared")?,
        };
        if !data
            .authorized_metadata(session, &prepared, DataUse::Execute)
            .map_err(data_error)?
            .parents
            .contains(&result.reference())
        {
            return Err(ReportError::InvalidWitness);
        }
        Ok(result)
    }
}
pub struct VerifiedPins {
    rows: Vec<(SourcePin, Revision)>,
}
impl VerifiedPins {
    pub fn from_value(data: &mut Data, session: &DataSession, value: &Value) -> Result<Self> {
        let rows = super::wire::list(value)?;
        if rows.is_empty() || rows.len() > 16 {
            return Err(ReportError::Limit);
        }
        let mut parsed = vec![];
        for row in rows {
            let v = super::wire::list(row)?;
            if v.len() != 4 {
                return Err(ReportError::InvalidWitness);
            }
            let reference = DataRef {
                project: ProjectId::new(super::wire::text(&v[0])?)
                    .map_err(|_| ReportError::InvalidInput)?,
                id: id(super::wire::text(&v[1])?)?,
            };
            let Value::Bytes(b) = &v[2] else {
                return Err(ReportError::InvalidWitness);
            };
            let put_digest = b
                .as_slice()
                .try_into()
                .map_err(|_| ReportError::InvalidWitness)?;
            let revision = Revision::new(super::wire::number(&v[3])?)
                .map_err(|_| ReportError::InvalidWitness)?;
            if parsed
                .last()
                .is_some_and(|(p, _): &(SourcePin, Revision)| p.reference >= reference)
            {
                return Err(ReportError::InvalidWitness);
            }
            parsed.push((
                SourcePin {
                    reference,
                    put_digest,
                },
                revision,
            ));
        }
        let pins = Self { rows: parsed };
        pins.recheck(data, session)?;
        Ok(pins)
    }
    pub fn value(&self) -> Value {
        Value::List(
            self.rows
                .iter()
                .map(|(p, r)| {
                    Value::List(vec![
                        Value::Text(p.reference.project.as_str().into()),
                        Value::Text(p.reference.id.as_str().into()),
                        Value::Bytes(p.put_digest.to_vec()),
                        Value::Unsigned(r.get()),
                    ])
                })
                .collect(),
        )
    }
    pub fn references(&self) -> Vec<DataRef> {
        self.rows.iter().map(|(p, _)| p.reference.clone()).collect()
    }
    pub fn recheck(&self, data: &mut Data, session: &DataSession) -> Result<()> {
        for (p, r) in &self.rows {
            let m = data
                .authorized_metadata(session, &p.reference, DataUse::Execute)
                .map_err(data_error)?;
            if m.digest != p.put_digest || m.revision != *r {
                return Err(ReportError::RevisionConflict);
            }
        }
        Ok(())
    }
}
pub fn verify_pins(
    data: &mut Data,
    session: &DataSession,
    pins: &[SourcePin],
) -> Result<VerifiedPins> {
    if pins.is_empty() || pins.len() > 16 {
        return Err(ReportError::Limit);
    }
    let mut rows = vec![];
    for p in pins {
        let m = data
            .authorized_metadata(session, &p.reference, DataUse::Execute)
            .map_err(data_error)?;
        if m.digest != p.put_digest {
            return Err(ReportError::RevisionConflict);
        }
        if !rows.iter().any(|(old, _)| old == p) {
            rows.push((p.clone(), m.revision));
        }
    }
    rows.sort_by(|a, b| a.0.reference.cmp(&b.0.reference));
    Ok(VerifiedPins { rows })
}
pub fn reopen_data(
    dir: &Path,
    project: ProjectId,
    rows: &[SourceRow],
) -> Result<(Data, Owner, DataSession)> {
    let journal = Journal::open(
        NativeStorage::open_existing(&dir.join("data.eylj"), sync)
            .map_err(|_| ReportError::OutcomeUnknown)?,
        NativeHash,
        project_journal_limits(),
    )
    .map_err(|_| ReportError::OutcomeUnknown)?;
    let (mut data, owner) = Data::open(journal, project, NativeDataClock).map_err(data_error)?;
    let now = data.trusted_time().map_err(data_error)?;
    let session = data
        .enroll(
            &owner,
            DataGrant {
                permissions: permissions(),
                scope: scope(rows)?,
                expires_at: now.checked_add(86400000).ok_or(ReportError::Limit)?,
            },
        )
        .map_err(data_error)?;
    Ok((data, owner, session))
}
