//! Current-authority joins precede unchanged G70 ranking. Cached queries retain no grants.
use super::*;
use eylema_contracts::{
    codec::{self, Value},
    platform::SemanticProfileId,
    project_data::DataMetadata,
};
use eylema_retrieval::{DocumentInput, QueryInput, RepresentationBinding, SearchQuery, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryStatus {
    Evidence,
    Conflict,
    NoEvidence,
}
#[derive(Clone)]
pub struct MemoryCitation {
    pub pin: SourcePin,
    pub authorization_revision: Revision,
}
pub struct MemoryContext {
    bytes: Vec<u8>,
    status: MemoryStatus,
    records: Vec<Record>,
    citations: Vec<MemoryCitation>,
    sources: usize,
}
impl MemoryContext {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn status(&self) -> MemoryStatus {
        self.status
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    pub fn citations(&self) -> &[MemoryCitation] {
        &self.citations
    }
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
    pub fn source_count(&self) -> usize {
        self.sources
    }
}
pub struct MemoryCachedQuery {
    identity: Arc<()>,
    query: Query,
}
type Auth = BTreeMap<DataRef, Option<DataMetadata>>;
fn binding(p: &RepresentationProfile) -> RepresentationBinding {
    RepresentationBinding {
        space: p.space.clone(),
        manifest_sha256: p.manifest_sha256,
        artifact_sha256: p.artifact_sha256,
        tokenizer_sha256: p.tokenizer_sha256,
        preprocessing_sha256: p.preprocessing_sha256,
        chunk_profile: "utf8-window1024-v1".into(),
        lexical_profile: "unicode-alnum-lower-rust199-v1".into(),
        score_profile: "scalar-micro-half-even-v1".into(),
    }
}
fn predicate(filter: &[SemanticProfileId], p: &Option<SemanticProfileId>) -> bool {
    filter.is_empty() || p.as_ref().is_some_and(|p| filter.contains(p))
}
impl<T: Storage, H: Sha256> MemoryStore<T, H> {
    pub fn cache(&self, q: &Query) -> MemoryResult<MemoryCachedQuery> {
        if q.fields().project != self.profile.project || q.fields().generation != self.generation {
            return Err(MemoryStoreError::Conflict);
        }
        Ok(MemoryCachedQuery {
            identity: self.identity.clone(),
            query: q.clone(),
        })
    }
    pub fn query_cached<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        session: &MemorySession,
        cached: &MemoryCachedQuery,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryContext> {
        if !Arc::ptr_eq(&self.identity, &cached.identity) {
            return Err(MemoryStoreError::Forbidden);
        }
        self.query(data, session, &cached.query, c)
    }
    fn source<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
        session: &MemorySession,
        p: &SourcePin,
        purpose: DataUse,
        auth: &mut Auth,
        c: &mut MemoryControl,
    ) -> MemoryResult<bool> {
        c.inner.charge(256)?;
        if !auth.contains_key(&p.reference) {
            let result = c.call(|| {
                data.authorized_metadata_at_observed_time(&session.source, &p.reference, purpose)
            });
            let m = match result {
                Ok(m) => Some(m),
                Err(MemoryStoreError::Forbidden) => None,
                Err(e) => return Err(e),
            };
            auth.insert(p.reference.clone(), m);
        }
        Ok(auth
            .get(&p.reference)
            .and_then(|m| m.as_ref())
            .is_some_and(|m| m.digest == p.put_digest))
    }
    #[allow(clippy::too_many_arguments)]
    fn eligible<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
        session: &MemorySession,
        s: &Stored,
        q: &QueryDraft,
        now: u64,
        auth: &mut Auth,
        c: &mut MemoryControl,
    ) -> MemoryResult<bool> {
        let d = &s.record.draft;
        if s.deleted
            || d.observed_at > now
            || now >= d.expires_at
            || q.effective_at < d.valid_from
            || q.effective_at >= d.expires_at
            || !q.origins.contains(&d.origin)
            || q.knowledge == KnowledgeMode::Reusable && !s.record.eligible_knowledge()
        {
            return Ok(false);
        }
        for p in d
            .sources
            .iter()
            .chain(std::iter::once(&s.record.projection))
        {
            if !self.source(data, session, p, q.purpose, auth, c)? {
                return Ok(false);
            }
        }
        let derived = auth
            .get(&s.record.projection.reference)
            .and_then(|m| m.as_ref())
            .is_some_and(|m| m.kind == eylema_contracts::project_data::DataKind::Derived);
        Ok(derived)
    }
    #[allow(clippy::too_many_arguments)]
    fn entity_eligible<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
        session: &MemorySession,
        id: &RecordId,
        q: &QueryDraft,
        now: u64,
        auth: &mut Auth,
        c: &mut MemoryControl,
    ) -> MemoryResult<bool> {
        match self.records.get(id) {
            Some(s) if s.record.draft.kind == RecordKind::Entity => {
                self.eligible(data, session, s, q, now, auth, c)
            }
            _ => Ok(false),
        }
    }
    pub fn query<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        session: &MemorySession,
        query: &Query,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryContext> {
        self.session(data, session)?;
        let q = query.fields();
        c.inner.check()?;
        if q.max_tokens.is_some() {
            return Err(MemoryStoreError::TokenCounterUnavailable);
        }
        if q.project != self.profile.project
            || q.profile != self.profile.id
            || q.revision != self.profile.revision
        {
            return Err(MemoryStoreError::IncompatibleProfile);
        }
        if q.generation != self.generation {
            return Err(MemoryStoreError::Conflict);
        }
        if q.representation
            .as_ref()
            .is_some_and(|r| r != &self.profile.representation)
        {
            return Err(MemoryStoreError::IncompatibleProfile);
        }
        let n = self.records.len();
        c.inner.charge(
            n.checked_mul(n)
                .and_then(|v| v.checked_mul(256))
                .and_then(|v| v.checked_add(n * 4096))
                .ok_or(MemoryStoreError::Limit)?,
        )?;
        let reserved = n
            .checked_mul(8192)
            .and_then(|v| v.checked_add(131072))
            .ok_or(MemoryStoreError::Limit)?;
        c.inner.reserve(reserved)?;
        let result = self.query_inner(data, session, q, c);
        c.inner.release(reserved);
        c.credits = 0;
        result
    }
    fn query_inner<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        session: &MemorySession,
        q: &QueryDraft,
        c: &mut MemoryControl,
    ) -> MemoryResult<MemoryContext> {
        let now = c.call(|| data.trusted_time())?;
        let mut auth = Auth::new();
        let mut reached = BTreeMap::<RecordId, u8>::new();
        let mut links = BTreeSet::<RecordId>::new();
        let mut structural = BTreeSet::<RecordId>::new();
        for id in q.seeds.iter().chain(q.entities.iter()) {
            if !self.entity_eligible(data, session, id, q, now, &mut auth, c)? {
                return Err(MemoryStoreError::Forbidden);
            }
            structural.insert(id.clone());
            if q.seeds.contains(id) {
                reached.insert(id.clone(), 0);
            }
        }
        for depth in 0..q.hops {
            c.inner.check()?;
            let frontier: Vec<_> = reached
                .iter()
                .filter(|(_, d)| **d == depth)
                .map(|(id, _)| id.clone())
                .collect();
            if frontier.is_empty() {
                break;
            }
            for (id, s) in &self.records {
                let d = &s.record.draft;
                if d.kind != RecordKind::Relation
                    || !predicate(&q.edge_predicates, &d.predicate)
                    || d.subject.as_ref().is_none_or(|p| !frontier.contains(p))
                {
                    continue;
                }
                if !self.eligible(data, session, s, q, now, &mut auth, c)? {
                    continue;
                }
                let Some(TypedObject::Entity(target)) = &d.object else {
                    continue;
                };
                if !self.entity_eligible(data, session, target, q, now, &mut auth, c)? {
                    continue;
                }
                links.insert(id.clone());
                structural.insert(target.clone());
                reached.entry(target.clone()).or_insert(depth + 1);
                if reached.len() + links.len() > 128 {
                    return Err(MemoryStoreError::Limit);
                }
            }
        }
        let mut hits = Vec::<(RecordId, u8)>::new();
        for (id, s) in &self.records {
            let d = &s.record.draft;
            if !q.kinds.contains(&d.kind)
                || !predicate(&q.result_predicates, &d.predicate)
                || (!q.entities.is_empty()
                    && d.subject.as_ref().is_none_or(|p| !q.entities.contains(p)))
            {
                continue;
            }
            let depth = if q.seeds.is_empty() {
                0
            } else if let Some(subject) = &d.subject {
                match reached.get(subject) {
                    Some(h) if d.kind == RecordKind::Entity => *h,
                    Some(h) if *h < q.hops => *h + 1,
                    _ => continue,
                }
            } else {
                continue;
            };
            if !self.eligible(data, session, s, q, now, &mut auth, c)? {
                continue;
            }
            if let Some(subject) = &d.subject
                && d.kind != RecordKind::Entity
            {
                if !self.entity_eligible(data, session, subject, q, now, &mut auth, c)? {
                    continue;
                }
                structural.insert(subject.clone());
            }
            if let Some(TypedObject::Entity(target)) = &d.object {
                if !self.entity_eligible(data, session, target, q, now, &mut auth, c)? {
                    continue;
                }
                structural.insert(target.clone());
            }
            hits.push((id.clone(), depth));
            if hits.len() > 128 {
                return Err(MemoryStoreError::Limit);
            }
        }
        hits.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        let mut conflict_groups = Vec::<BTreeSet<RecordId>>::new();
        for (i, (id, _)) in hits.iter().enumerate() {
            let a = &self.records[id].record.draft;
            if a.predicate.as_ref().is_none_or(|p| {
                !self
                    .profile
                    .predicates
                    .iter()
                    .any(|s| &s.id == p && s.cardinality == Cardinality::SingleValue)
            }) {
                continue;
            }
            let mut group = BTreeSet::new();
            for (other, _) in hits.iter().skip(i + 1) {
                let b = &self.records[other].record.draft;
                if a.subject == b.subject
                    && a.predicate == b.predicate
                    && a.object != b.object
                    && a.valid_from < b.expires_at
                    && b.valid_from < a.expires_at
                {
                    group.insert(id.clone());
                    group.insert(other.clone());
                }
            }
            if !group.is_empty() {
                conflict_groups.push(group)
            }
        }
        let mut indexed_bytes = Value::Null;
        if !matches!(q.rank, RankMode::Graph) && !hits.is_empty() {
            if hits.len() > 16 {
                return Err(MemoryStoreError::Limit);
            }
            let mut documents = Vec::new();
            for (id, _) in &hits {
                let stored = &self.records[id];
                let pin = &stored.record.projection;
                let len = c.call(|| {
                    data.authorized_payload_len(&session.source, &pin.reference, q.purpose)
                })?;
                if len > 1024 {
                    return Err(MemoryStoreError::Limit);
                }
                c.inner
                    .charge(len.checked_mul(2).ok_or(MemoryStoreError::Limit)? + 256)?;
                let body = c.call(|| data.read(&session.source, &pin.reference, q.purpose))?;
                if body != wire::projection(&stored.record.draft)? {
                    return Err(MemoryStoreError::Forbidden);
                }
                let vector = stored
                    .vector
                    .as_ref()
                    .ok_or(MemoryStoreError::IncompatibleProfile)?;
                documents.push(DocumentInput {
                    reference: pin.reference.clone(),
                    put_digest: pin.put_digest,
                    body,
                    vectors: vec![vector.clone()],
                });
            }
            let binding = binding(&self.profile.representation);
            let input = match &q.rank {
                RankMode::Lexical(s) => QueryInput::Lexical(s.clone()),
                RankMode::Vector(v) => QueryInput::Vector(v.clone()),
                RankMode::Hybrid {
                    text,
                    vector,
                    vector_weight,
                } => QueryInput::Hybrid {
                    text: text.clone(),
                    vector: vector.clone(),
                    vector_weight: *vector_weight,
                },
                RankMode::Graph => unreachable!(),
            };
            let sources = documents.iter().map(|d| d.reference.clone()).collect();
            let mut hash = self.hash.clone();
            let snapshot = Snapshot::build(
                self.profile.project.clone(),
                binding.clone(),
                self.generation,
                documents,
                &mut hash,
                c.inner_mut(),
            )?;
            let search = SearchQuery {
                project: self.profile.project.clone(),
                binding,
                generation: self.generation,
                sources,
                input,
                top_k: q.max_hits.min(16),
                max_hits: 16,
                max_bytes: q.max_bytes,
                max_tokens: None,
            };
            let ranked = snapshot.search(&search, c.inner_mut())?;
            let bytes = ranked.encode(q.max_bytes, c.inner_mut())?;
            indexed_bytes = codec::decode(&bytes).map_err(|_| MemoryStoreError::Backend)?;
            let results = wire::array(&indexed_bytes, 8)?;
            let Value::List(rows) = &results[7] else {
                return Err(MemoryStoreError::Backend);
            };
            let mut order = vec![];
            for row in rows {
                let a = wire::array(row, 10)?;
                let reference = wire::string(&a[1])?;
                let id = hits
                    .iter()
                    .find(|(id, _)| {
                        self.records[id].record.projection.reference.id.as_str() == reference
                    })
                    .ok_or(MemoryStoreError::Backend)?
                    .0
                    .clone();
                order.push((id, 0));
            }
            hits = order;
        }
        let selected_ids: BTreeSet<_> = hits
            .iter()
            .take(q.max_hits)
            .map(|(id, _)| id.clone())
            .collect();
        let mut conflicts = BTreeSet::new();
        for group in conflict_groups {
            if group.iter().any(|id| selected_ids.contains(id)) {
                conflicts.extend(group)
            }
        }
        if !conflicts.is_subset(&selected_ids) {
            return Err(MemoryStoreError::IncompleteConflict);
        }
        if hits.len() > q.max_hits {
            return Err(if conflicts.is_empty() {
                MemoryStoreError::Limit
            } else {
                MemoryStoreError::IncompleteConflict
            });
        }
        let has_evidence = !hits.is_empty();
        if has_evidence {
            for id in links {
                if !hits.iter().any(|(old, _)| old == &id) {
                    let d = &self.records[&id].record.draft;
                    let depth = d
                        .subject
                        .as_ref()
                        .and_then(|s| reached.get(s))
                        .copied()
                        .unwrap_or(0)
                        + 1;
                    hits.push((id, depth));
                }
            }
        } else {
            structural.clear()
        }
        if matches!(q.rank, RankMode::Graph) {
            hits.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)))
        }
        if hits.len() > q.max_hits {
            return Err(MemoryStoreError::Limit);
        }
        let mut all_pins = BTreeMap::<DataRef, SourcePin>::new();
        let mut citations = BTreeMap::<DataRef, MemoryCitation>::new();
        for id in hits.iter().map(|(id, _)| id).chain(structural.iter()) {
            let s = &self.records[id];
            for pin in s
                .record
                .draft
                .sources
                .iter()
                .chain(std::iter::once(&s.record.projection))
            {
                all_pins.insert(pin.reference.clone(), pin.clone());
            }
            for pin in &s.record.draft.sources {
                let metadata = auth
                    .get(&pin.reference)
                    .and_then(|m| m.as_ref())
                    .ok_or(MemoryStoreError::Forbidden)?;
                citations.insert(
                    pin.reference.clone(),
                    MemoryCitation {
                        pin: pin.clone(),
                        authorization_revision: metadata.revision,
                    },
                );
            }
        }
        if all_pins.len() > q.max_sources {
            return Err(MemoryStoreError::Limit);
        }
        let status = if !has_evidence {
            MemoryStatus::NoEvidence
        } else if conflicts.is_empty() {
            MemoryStatus::Evidence
        } else {
            MemoryStatus::Conflict
        };
        let records: Vec<_> = hits
            .iter()
            .map(|(id, _)| self.records[id].record.clone())
            .collect();
        let citation_list: Vec<_> = citations.into_values().collect();
        let result = wire::list(vec![
            wire::text("memory-context"),
            Value::Unsigned(1),
            wire::text(self.profile.project.as_str()),
            wire::text(self.profile.id.as_str()),
            Value::Unsigned(self.generation),
            wire::text(match status {
                MemoryStatus::NoEvidence => "no_evidence",
                MemoryStatus::Evidence => "evidence",
                MemoryStatus::Conflict => "conflict",
            }),
            wire::list(
                records
                    .iter()
                    .map(|r| {
                        wire::record(&Stored {
                            record: r.clone(),
                            vector: None,
                            deleted: false,
                        })
                    })
                    .collect(),
            ),
            wire::list(
                citation_list
                    .iter()
                    .map(|v| {
                        wire::list(vec![
                            wire::pin(&v.pin),
                            Value::Unsigned(v.authorization_revision.get()),
                        ])
                    })
                    .collect(),
            ),
            indexed_bytes,
        ]);
        c.inner
            .charge(q.max_bytes.checked_mul(2).ok_or(MemoryStoreError::Limit)?)?;
        let bytes = codec::encode(&result).map_err(|_| MemoryStoreError::Limit)?;
        if bytes.len() > q.max_bytes {
            return Err(if status == MemoryStatus::Conflict {
                MemoryStoreError::IncompleteConflict
            } else {
                MemoryStoreError::Limit
            });
        }
        let final_now = c.call(|| data.trusted_time())?;
        for pin in all_pins.values() {
            self.pin(data, &session.source, pin, q.purpose, c)?
        }
        for id in hits.iter().map(|(id, _)| id).chain(structural.iter()) {
            let s = &self.records[id];
            if s.deleted
                || s.record.draft.observed_at > final_now
                || s.record.draft.expires_at <= final_now
            {
                return Err(MemoryStoreError::Forbidden);
            }
        }
        c.inner.check()?;
        Ok(MemoryContext {
            bytes,
            status,
            records,
            citations: citation_list,
            sources: all_pins.len(),
        })
    }
}
