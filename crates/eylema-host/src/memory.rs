//! Ephemeral declared-source registry and exact-reference context, not graph/vector/SQL execution.
use crate::{
    Owner,
    project_data::{DataClock, DataError, DataSession, ProjectStore},
};
use eylema_contracts::{
    codec::{self, Value},
    memory::*,
    platform::{ProjectId, SemanticProfileId},
    project_data::{DataId, DataKind},
};
use eylema_journal::{Sha256, Storage};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    InvalidRequest,
    Forbidden,
    WrongProject,
    NotFound,
    IncompatibleProfile,
    BudgetExceeded,
    Expired,
    NotYetValid,
    Unverified,
    BackendUnavailable,
    TokenCounterUnavailable,
    InvalidPayload,
    Data(DataError),
}
impl From<DataError> for MemoryError {
    fn from(e: DataError) -> Self {
        Self::Data(e)
    }
}
pub type MemoryResult<T> = Result<T, MemoryError>;
#[derive(Clone, Eq, PartialEq)]
pub struct MemoryContext {
    pub bytes: Vec<u8>,
    pub sources: usize,
    pub tokens: Option<u32>,
}
impl std::fmt::Debug for MemoryContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryContext")
            .field("serialized_bytes", &self.bytes.len())
            .field("sources", &self.sources)
            .field("tokens", &self.tokens)
            .finish()
    }
}
/// A cached request retains references only. It contains no content or permission receipt.
pub struct CachedQuery {
    identity: Arc<()>,
    query: MemoryQuery,
}
struct Source {
    draft: MemorySourceDraft,
    digest: [u8; 32],
}
pub struct MemoryRegistry {
    identity: Arc<()>,
    data_identity: Option<Arc<()>>,
    project: ProjectId,
    profiles: BTreeMap<SemanticProfileId, MemoryProfile>,
    sources: BTreeMap<DataId, Source>,
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
impl MemoryRegistry {
    pub fn new(project: ProjectId, profiles: Vec<MemoryProfile>) -> MemoryResult<(Self, Owner)> {
        if profiles.is_empty() || profiles.len() > MAX_MEMORY_ENTRIES {
            return Err(MemoryError::BudgetExceeded);
        }
        let mut map = BTreeMap::new();
        for profile in profiles {
            if map.insert(profile.fields().id.clone(), profile).is_some() {
                return Err(MemoryError::InvalidRequest);
            }
        }
        let identity = Arc::new(());
        let o = Owner {
            identity: identity.clone(),
        };
        Ok((
            Self {
                identity,
                data_identity: None,
                project,
                profiles: map,
                sources: BTreeMap::new(),
            },
            o,
        ))
    }
    pub fn register<T: Storage, H: Sha256, C: DataClock>(
        &mut self,
        owner: &Owner,
        data: &mut ProjectStore<T, H, C>,
        data_owner: &Owner,
        draft: MemorySourceDraft,
    ) -> MemoryResult<()> {
        if !Arc::ptr_eq(&self.identity, &owner.identity) {
            return Err(MemoryError::Forbidden);
        }
        draft.validate().map_err(|_| MemoryError::InvalidRequest)?;
        if draft.reference.project != self.project {
            return Err(MemoryError::WrongProject);
        }
        if self.sources.contains_key(&draft.reference.id) {
            return Err(MemoryError::InvalidRequest);
        }
        if self.sources.len() >= MAX_MEMORY_ENTRIES {
            return Err(MemoryError::BudgetExceeded);
        }
        let profile = self
            .profiles
            .get(&draft.profile)
            .ok_or(MemoryError::NotFound)?;
        let p = profile.fields();
        if !p.kinds.contains(&draft.kind)
            || p.schema != draft.schema
            || (draft.kind == MemoryKind::Semantic && p.embedding != draft.embedding)
        {
            return Err(MemoryError::IncompatibleProfile);
        }
        let metadata = data.inspect(data_owner, &draft.reference)?;
        if metadata.digest == [0; 32]
            || metadata.kind == DataKind::TrainingCandidate
            || draft.expires_at > metadata.expires_at
        {
            return Err(MemoryError::InvalidRequest);
        }
        let now = data.trusted_time()?;
        if metadata.withdrawn {
            return Err(MemoryError::Data(DataError::Forbidden));
        }
        if now >= metadata.expires_at || now >= draft.expires_at {
            return Err(MemoryError::Expired);
        }
        if draft.observed_at > now {
            return Err(MemoryError::NotYetValid);
        }
        let (project, identity) = data.memory_binding();
        if project != &self.project {
            return Err(MemoryError::WrongProject);
        }
        if self
            .data_identity
            .as_ref()
            .is_some_and(|bound| !Arc::ptr_eq(bound, identity))
        {
            return Err(MemoryError::Forbidden);
        }
        self.data_identity = Some(identity.clone());
        self.sources.insert(
            draft.reference.id.clone(),
            Source {
                draft,
                digest: metadata.digest,
            },
        );
        Ok(())
    }
    pub fn cache(&self, q: &MemoryQuery) -> MemoryResult<CachedQuery> {
        if q.fields().project != self.project {
            return Err(MemoryError::WrongProject);
        }
        Ok(CachedQuery {
            identity: self.identity.clone(),
            query: q.clone(),
        })
    }
    pub fn assemble_cached<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        session: &DataSession,
        cache: &CachedQuery,
    ) -> MemoryResult<MemoryContext> {
        if !Arc::ptr_eq(&self.identity, &cache.identity) {
            return Err(MemoryError::Forbidden);
        }
        self.assemble(data, session, &cache.query)
    }
    pub fn assemble<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        session: &DataSession,
        query: &MemoryQuery,
    ) -> MemoryResult<MemoryContext> {
        let q = query.fields();
        if q.project != self.project {
            return Err(MemoryError::WrongProject);
        }
        let profile = self
            .profiles
            .get(&q.profile)
            .ok_or(MemoryError::NotFound)?
            .fields();
        if profile.revision != q.revision
            || q.kinds.iter().any(|k| !profile.kinds.contains(k))
            || (q.kinds.contains(&MemoryKind::Semantic) && q.embedding != profile.embedding)
        {
            return Err(MemoryError::IncompatibleProfile);
        }
        if !profile.roles.contains(&q.role) {
            return Err(MemoryError::Forbidden);
        }
        if profile.backend != MemoryBackend::OwnedReferences {
            return Err(MemoryError::BackendUnavailable);
        }
        if q.max_tokens.is_some() {
            return Err(MemoryError::TokenCounterUnavailable);
        }
        let (project, identity) = data.memory_binding();
        if project != &self.project {
            return Err(MemoryError::WrongProject);
        }
        if self
            .data_identity
            .as_ref()
            .is_some_and(|bound| !Arc::ptr_eq(bound, identity))
        {
            return Err(MemoryError::Forbidden);
        }
        let now = data.trusted_time()?;
        let mut entries = vec![];
        for id in &q.sources {
            // Filter data access before exposing private memory-source status.
            let reference = eylema_contracts::project_data::DataRef {
                project: self.project.clone(),
                id: id.clone(),
            };
            let metadata = data.authorized_metadata(session, &reference, q.purpose)?;
            let source = self.sources.get(id).ok_or(MemoryError::NotFound)?;
            let s = &source.draft;
            if s.profile != q.profile
                || s.schema != profile.schema
                || !q.kinds.contains(&s.kind)
                || (s.kind == MemoryKind::Semantic && s.embedding != q.embedding)
                || metadata.digest != source.digest
            {
                return Err(MemoryError::IncompatibleProfile);
            }
            let observed = data.trusted_time()?;
            validity(s, observed)?;
            if q.verified_only && !s.verification.is_verified() {
                return Err(MemoryError::Unverified);
            }
            let body = data.read(session, &s.reference, q.purpose)?;
            std::str::from_utf8(&body).map_err(|_| MemoryError::InvalidPayload)?;
            entries.push(Value::List(vec![
                text(s.reference.project.as_str()),
                text(id.as_str()),
                Value::Bytes(source.digest.to_vec()),
                text(s.kind.as_str()),
                text(s.profile.as_str()),
                Value::Unsigned(profile.revision.get()),
                text(s.schema.as_str()),
                Value::Unsigned(s.observed_at),
                Value::Unsigned(s.valid_from),
                Value::Unsigned(s.expires_at),
                text(s.verification.as_str()),
                s.confidence_bps
                    .map_or(Value::Null, |c| Value::Unsigned(c as u64)),
                encode_space(&s.embedding),
                Value::List(
                    metadata
                        .parents
                        .iter()
                        .map(|p| Value::List(vec![text(p.project.as_str()), text(p.id.as_str())]))
                        .collect(),
                ),
                Value::Unsigned(metadata.revision.get()),
                Value::Bytes(body),
            ]));
        }
        let count = entries.len();
        let value = Value::List(vec![
            text("memory-context"),
            Value::Unsigned(1),
            text(self.project.as_str()),
            text(q.profile.as_str()),
            Value::Unsigned(q.revision.get()),
            text(q.role.as_str()),
            text(match q.purpose {
                eylema_contracts::project_data::DataUse::Execute => "execute",
                eylema_contracts::project_data::DataUse::Collect => "collect",
                eylema_contracts::project_data::DataUse::Train => "train",
                _ => unreachable!("validated local purpose"),
            }),
            Value::Unsigned(now),
            Value::List(entries),
        ]);
        let bytes = codec::encode(&value).map_err(|_| MemoryError::BudgetExceeded)?;
        if bytes.len() > q.max_bytes {
            return Err(MemoryError::BudgetExceeded);
        }
        // Stage the complete result, then recheck each source at the handoff.
        for id in &q.sources {
            let source = self.sources.get(id).expect("validated source");
            let metadata = data.authorized_metadata(session, &source.draft.reference, q.purpose)?;
            if metadata.digest != source.digest {
                return Err(MemoryError::IncompatibleProfile);
            }
        }
        // One common persisted handoff observation covers every source and the
        // session. Independent per-source clocks could expire earlier entries
        // or the session while later entries are checked.
        let handoff = data.trusted_time()?;
        for id in &q.sources {
            let source = self.sources.get(id).expect("validated source");
            let metadata = data.authorized_metadata_at_observed_time(
                session,
                &source.draft.reference,
                q.purpose,
            )?;
            if metadata.digest != source.digest {
                return Err(MemoryError::IncompatibleProfile);
            }
            validity(&source.draft, handoff)?;
        }
        Ok(MemoryContext {
            bytes,
            sources: count,
            tokens: None,
        })
    }
}
fn validity(s: &MemorySourceDraft, now: u64) -> MemoryResult<()> {
    if now < s.valid_from || now < s.observed_at {
        return Err(MemoryError::NotYetValid);
    }
    if now >= s.expires_at {
        return Err(MemoryError::Expired);
    }
    Ok(())
}
