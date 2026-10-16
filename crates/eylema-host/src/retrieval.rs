//! Current-authority retrieval adapter. Journal integrity is neither consent nor erasure.
#![allow(clippy::too_many_arguments)]
use crate::{
    Owner,
    project_data::{DataClock, DataError, DataSession, ProjectStore},
};
use eylema_contracts::{
    platform::ProjectId,
    project_data::{DataRef, DataUse},
};
use eylema_journal::{Sha256, Storage};
use eylema_retrieval::{
    Control, DocumentInput, IndexStore, RepresentationBinding, RetrievalError, SearchQuery,
    Snapshot,
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostRetrievalError {
    Forbidden,
    Data(DataError),
    Core(RetrievalError),
}
impl From<DataError> for HostRetrievalError {
    fn from(e: DataError) -> Self {
        Self::Data(e)
    }
}
impl From<RetrievalError> for HostRetrievalError {
    fn from(e: RetrievalError) -> Self {
        Self::Core(e)
    }
}
type R<T> = Result<T, HostRetrievalError>;
pub struct SourceVectors {
    pub reference: DataRef,
    pub vectors: Vec<Vec<f32>>,
}
pub struct RetrievalOwner {
    identity: Arc<()>,
}
pub struct RetrievalHost<T: Storage, H: Sha256> {
    index: IndexStore<T, H>,
    data_identity: Arc<()>,
    identity: Arc<()>,
    project: ProjectId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexReceipt {
    pub generation: u64,
    pub digest: [u8; 32],
}
pub struct RetrievalResponse {
    bytes: Vec<u8>,
    hits: usize,
    sources: usize,
}
impl RetrievalResponse {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn hit_count(&self) -> usize {
        self.hits
    }
    pub fn source_count(&self) -> usize {
        self.sources
    }
}
impl std::fmt::Debug for RetrievalResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalResponse")
            .field("bytes", &self.bytes.len())
            .field("hits", &self.hits)
            .field("sources", &self.sources)
            .finish()
    }
}
fn read_source<DT: Storage, DH: Sha256, C: DataClock>(
    data: &mut ProjectStore<DT, DH, C>,
    session: &DataSession,
    reference: &DataRef,
    purpose: DataUse,
    c: &mut Control,
) -> R<(Vec<u8>, [u8; 32])> {
    c.check()?;
    c.reserve(65536)?;
    c.charge(65536)?;
    let m = data.authorized_metadata(session, reference, purpose)?;
    let bytes = data.read(session, reference, purpose)?;
    if bytes.is_empty() || bytes.len() > 8192 {
        return Err(RetrievalError::LimitExceeded.into());
    }
    c.release(65536 - bytes.len());
    c.check()?;
    Ok((bytes, m.digest))
}
fn documents<DT: Storage, DH: Sha256, C: DataClock>(
    data: &mut ProjectStore<DT, DH, C>,
    s: &DataSession,
    sources: Vec<SourceVectors>,
    dimensions: usize,
    c: &mut Control,
) -> R<Vec<DocumentInput>> {
    if sources.len() > 16 {
        return Err(RetrievalError::LimitExceeded.into());
    }
    c.check()?;
    c.charge(16 * 128)?;
    let mut count = 0usize;
    for source in &sources {
        if source.vectors.len() > 128 {
            return Err(RetrievalError::LimitExceeded.into());
        }
        count = count
            .checked_add(source.vectors.len())
            .ok_or(RetrievalError::LimitExceeded)?;
        if count > 128 {
            return Err(RetrievalError::LimitExceeded.into());
        }
        for vector in &source.vectors {
            if vector.len() != dimensions {
                return Err(RetrievalError::InvalidVector.into());
            }
        }
    }
    let vector_bytes = count
        .checked_mul(dimensions)
        .and_then(|n| n.checked_mul(4))
        .ok_or(RetrievalError::LimitExceeded)?;
    if vector_bytes > 1024 * 1024 {
        return Err(RetrievalError::LimitExceeded.into());
    }
    c.reserve(vector_bytes + count * std::mem::size_of::<Vec<f32>>())?;
    c.charge(vector_bytes)?;
    c.reserve(16 * 512)?;
    c.charge(16 * 16 * 128)?;
    let mut out = Vec::with_capacity(sources.len());
    for source in sources {
        let (body, digest) = read_source(data, s, &source.reference, DataUse::Collect, c)?;
        out.push(DocumentInput {
            reference: source.reference,
            body,
            put_digest: digest,
            vectors: source.vectors,
        });
    }
    Ok(out)
}
fn recheck<DT: Storage, DH: Sha256, C: DataClock>(
    data: &mut ProjectStore<DT, DH, C>,
    session: &DataSession,
    sources: &[(DataRef, [u8; 32])],
    purpose: DataUse,
    c: &mut Control,
) -> R<()> {
    c.charge(sources.len() * 1024)?;
    data.trusted_time()?;
    for (r, digest) in sources {
        let m = data.authorized_metadata_at_observed_time(session, r, purpose)?;
        if m.digest != *digest {
            return Err(RetrievalError::IncompatibleBinding.into());
        }
    }
    c.check()?;
    Ok(())
}
fn lineage(snapshot: &Snapshot, c: &mut Control) -> R<Vec<(DataRef, [u8; 32])>> {
    c.reserve(16 * 512)?;
    c.charge(16 * 512)?;
    Ok(snapshot
        .documents()
        .iter()
        .map(|d| (d.reference().clone(), d.put_digest()))
        .collect())
}
impl<T: Storage, H: Sha256> RetrievalHost<T, H> {
    pub fn create<DT: Storage, DH: Sha256, C: DataClock>(
        storage: T,
        journal_hash: H,
        mut content_hash: H,
        data: &mut ProjectStore<DT, DH, C>,
        data_owner: &Owner,
        collect_session: &DataSession,
        binding: &RepresentationBinding,
        sources: Vec<SourceVectors>,
        c: &mut Control,
    ) -> R<(Self, RetrievalOwner)> {
        let (project, id) = data.retrieval_binding(data_owner)?;
        let project = project.clone();
        let data_identity = id.clone();
        binding.validate()?;
        let docs = documents(
            data,
            collect_session,
            sources,
            usize::from(binding.space.dimensions),
            c,
        )?;
        let snapshot = Snapshot::build(
            project.clone(),
            binding.clone(),
            1,
            docs,
            &mut content_hash,
            c,
        )?;
        let sources = lineage(&snapshot, c)?;
        recheck(data, collect_session, &sources, DataUse::Collect, c)?;
        let index = IndexStore::create(storage, journal_hash, content_hash, snapshot, c)?;
        recheck(data, collect_session, &sources, DataUse::Collect, c)?;
        let identity = Arc::new(());
        let owner = RetrievalOwner {
            identity: identity.clone(),
        };
        Ok((
            Self {
                index,
                data_identity,
                identity,
                project,
            },
            owner,
        ))
    }
    pub fn open<DT: Storage, DH: Sha256, C: DataClock>(
        storage: T,
        journal_hash: H,
        content_hash: H,
        data: &mut ProjectStore<DT, DH, C>,
        data_owner: &Owner,
        execute_session: &DataSession,
        binding: &RepresentationBinding,
        c: &mut Control,
    ) -> R<(Self, RetrievalOwner)> {
        let (project, id) = data.retrieval_binding(data_owner)?;
        let project = project.clone();
        let data_identity = id.clone();
        let index = IndexStore::open(storage, journal_hash, content_hash, &project, binding, c)?;
        for doc in index.current().documents() {
            let (body, digest) =
                read_source(data, execute_session, doc.reference(), DataUse::Execute, c)?;
            if digest != doc.put_digest() {
                return Err(RetrievalError::IncompatibleBinding.into());
            }
            let mut end = 0;
            for x in doc.chunks() {
                let r = x.range();
                c.charge(x.text().len())?;
                if body.get(r.clone()) != Some(x.text().as_bytes()) {
                    return Err(RetrievalError::IncompatibleBinding.into());
                }
                end = r.end;
            }
            if end != body.len() {
                return Err(RetrievalError::IncompatibleBinding.into());
            }
        }
        let sources = lineage(index.current(), c)?;
        recheck(data, execute_session, &sources, DataUse::Execute, c)?;
        let identity = Arc::new(());
        let owner = RetrievalOwner {
            identity: identity.clone(),
        };
        Ok((
            Self {
                index,
                data_identity,
                identity,
                project,
            },
            owner,
        ))
    }
    fn live<DT: Storage, DH: Sha256, C: DataClock>(&self, data: &ProjectStore<DT, DH, C>) -> R<()> {
        let (p, id) = data.memory_binding();
        if p != &self.project || !Arc::ptr_eq(id, &self.data_identity) {
            return Err(HostRetrievalError::Forbidden);
        }
        Ok(())
    }
    pub fn replace<DT: Storage, DH: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<DT, DH, C>,
        data_owner: &Owner,
        owner: &RetrievalOwner,
        collect_session: &DataSession,
        expected_generation: u64,
        binding: &RepresentationBinding,
        sources: Vec<SourceVectors>,
        c: &mut Control,
    ) -> R<IndexReceipt> {
        data.retrieval_binding(data_owner)?;
        self.live(data)?;
        if !Arc::ptr_eq(&owner.identity, &self.identity) {
            return Err(HostRetrievalError::Forbidden);
        }
        if self.index.is_reopen_required() {
            return Err(RetrievalError::NeedsReopen.into());
        }
        if self.index.current().binding() != binding {
            return Err(RetrievalError::IncompatibleBinding.into());
        }
        let docs = documents(
            data,
            collect_session,
            sources,
            usize::from(binding.space.dimensions),
            c,
        )?;
        let generation = expected_generation
            .checked_add(1)
            .ok_or(RetrievalError::StaleGeneration)?;
        let next =
            self.index
                .build_next(self.project.clone(), binding.clone(), generation, docs, c)?;
        let sources = lineage(&next, c)?;
        recheck(data, collect_session, &sources, DataUse::Collect, c)?;
        let receipt = self.index.append(next, expected_generation, c)?;
        if let Err(e) = recheck(data, collect_session, &sources, DataUse::Collect, c) {
            self.index.require_reopen();
            return Err(e);
        }
        Ok(IndexReceipt {
            generation,
            digest: receipt.digest,
        })
    }
    pub fn search<DT: Storage, DH: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<DT, DH, C>,
        execute_session: &DataSession,
        q: &SearchQuery,
        c: &mut Control,
    ) -> R<RetrievalResponse> {
        self.live(data)?;
        if q.sources.is_empty() || q.sources.len() > 16 {
            return Err(RetrievalError::InvalidInput.into());
        }
        c.reserve(16 * 512)?;
        c.charge(16 * 512)?;
        let mut sources = Vec::with_capacity(q.sources.len());
        for r in &q.sources {
            let m = data.authorized_metadata(execute_session, r, DataUse::Execute)?;
            sources.push((r.clone(), m.digest));
        }
        if self.index.is_reopen_required() {
            return Err(RetrievalError::NeedsReopen.into());
        }
        for (r, digest) in &sources {
            let doc = self
                .index
                .current()
                .documents()
                .iter()
                .find(|d| d.reference() == r)
                .ok_or(RetrievalError::NotIndexed)?;
            if *digest != doc.put_digest() {
                return Err(RetrievalError::IncompatibleBinding.into());
            }
        }
        let ranked = self.index.current().search(q, c)?;
        let bytes = ranked.encode(q.max_bytes, c)?;
        recheck(data, execute_session, &sources, DataUse::Execute, c)?;
        Ok(RetrievalResponse {
            bytes,
            hits: ranked.hit_count(),
            sources: ranked.source_count(),
        })
    }
}
