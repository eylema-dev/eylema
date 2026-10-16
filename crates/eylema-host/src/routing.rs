//! Source-scoped transient routing facade; selection never grants authority.
#![allow(clippy::too_many_arguments)]
use crate::{
    Owner,
    project_data::{DataClock, DataSession, ProjectStore},
};
use eylema_contracts::{
    Revision,
    codec::{self, Value},
    platform::ProjectId,
    project_data::{DataKind, DataUse},
};
use eylema_journal::{Sha256, Storage};
use eylema_runtime::routing::*;
use std::sync::Arc;
mod discovery;
mod dispatch;
mod guard;
mod wire;
pub use dispatch::{CallPayload, DispatchReceipt, Selection, SelectionTicket};
pub use guard::RouteControl;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteError {
    InvalidInput,
    Stale,
    Forbidden,
    Unavailable,
    Limit,
    Cancelled,
    Deadline,
    AdmissionOutcomeUnknown,
    Retrieval(eylema_retrieval::RetrievalError),
    Tool(crate::tool::ToolError),
}
impl From<eylema_retrieval::RetrievalError> for RouteError {
    fn from(e: eylema_retrieval::RetrievalError) -> Self {
        use eylema_retrieval::RetrievalError as E;
        match e {
            E::LimitExceeded => Self::Limit,
            E::Cancelled => Self::Cancelled,
            E::Deadline => Self::Deadline,
            _ => Self::Retrieval(e),
        }
    }
}
impl From<RoutingError> for RouteError {
    fn from(e: RoutingError) -> Self {
        match e {
            RoutingError::InvalidInput => Self::InvalidInput,
            RoutingError::Stale => Self::Stale,
            RoutingError::ResourceLimit | RoutingError::ContextLimit => Self::Limit,
            _ => Self::Unavailable,
        }
    }
}
pub struct RegistryConfig<'a> {
    pub registry: Stamp,
    pub policy: Stamp,
    pub entries: Vec<Entry>,
    pub resources: Vec<ResourceView<'a>>,
}
pub struct RouterOwner {
    _identity: Arc<()>,
}
pub struct Router<'a> {
    config: RegistryConfig<'a>,
    identity: Arc<()>,
    data_identity: Arc<()>,
    project: ProjectId,
}
#[derive(Clone)]
struct Observation {
    pin: SourcePin,
    revision: Revision,
    kind: DataKind,
    expires_at: u64,
}
pub struct Catalog {
    bytes: Vec<u8>,
    entries: usize,
}
pub struct Description {
    bytes: Vec<u8>,
    entries: Vec<RouteId>,
    observed: Vec<Observation>,
    identity: Arc<()>,
}
impl Catalog {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn entry_count(&self) -> usize {
        self.entries
    }
}
impl Description {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn source_count(&self) -> usize {
        self.observed.len()
    }
}
impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalog")
            .field("entries", &self.entries)
            .finish()
    }
}
impl std::fmt::Debug for Description {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Description")
            .field("sources", &self.observed.len())
            .finish()
    }
}
impl<'a> Router<'a> {
    pub fn register<T: Storage, H: Sha256, C: DataClock>(
        data: &mut ProjectStore<T, H, C>,
        owner: &Owner,
        session: &DataSession,
        config: RegistryConfig<'a>,
        c: &mut RouteControl,
    ) -> Result<(Self, RouterOwner), RouteError> {
        c.check()?;
        if config.entries.is_empty()
            || config.entries.len() > 16
            || config.resources.is_empty()
            || config.resources.len() > 16
        {
            return Err(RouteError::Limit);
        }
        let (project, identity) = data
            .retrieval_binding(owner)
            .map_err(|_| RouteError::Forbidden)?;
        let project = project.clone();
        let data_identity = identity.clone();
        c.reserve(4 * 1024 * 1024)?;
        c.charge(16 * 8 * 128)?;
        for (n, e) in config.entries.iter().enumerate() {
            e.validate()?;
            if e.sources.is_empty() || config.entries[..n].iter().any(|p| p.id == e.id) {
                return Err(RouteError::InvalidInput);
            }
            if e.sources.iter().any(|s| s.reference.project != project) {
                return Err(RouteError::Forbidden);
            }
        }
        for (n, v) in config.resources.iter().enumerate() {
            if config.resources[..n].iter().any(|p| {
                p.snapshot.id == v.snapshot.id && p.snapshot.revision == v.snapshot.revision
            }) {
                return Err(RouteError::InvalidInput);
            }
        }
        let identity = Arc::new(());
        let router = Self {
            config,
            identity: identity.clone(),
            data_identity,
            project,
        };
        let pins: Vec<_> = router
            .config
            .entries
            .iter()
            .flat_map(|e| e.sources.clone())
            .collect();
        let observed = router.observe(data, session, &pins, c)?;
        router.recheck(data, session, &observed, c)?;
        c.release(4 * 1024 * 1024);
        Ok((
            router,
            RouterOwner {
                _identity: identity,
            },
        ))
    }
    fn live<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<T, H, C>,
    ) -> Result<(), RouteError> {
        let (p, id) = data.memory_binding();
        if *p != self.project || !Arc::ptr_eq(id, &self.data_identity) {
            Err(RouteError::Forbidden)
        } else {
            Ok(())
        }
    }
    fn stamps(&self, now: u64) -> Result<(), RouteError> {
        if !self.config.registry.fresh(now) || !self.config.policy.fresh(now) {
            Err(RouteError::Stale)
        } else {
            Ok(())
        }
    }
    fn observe<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        s: &DataSession,
        pins: &[SourcePin],
        c: &mut RouteControl,
    ) -> Result<Vec<Observation>, RouteError> {
        self.live(data)?;
        if pins.len() > 128 {
            return Err(RouteError::Limit);
        }
        let mut out = Vec::new();
        c.reserve(128 * 512)?;
        for pin in pins {
            if let Some(old) = out
                .iter()
                .find(|old: &&Observation| old.pin.reference == pin.reference)
            {
                if old.pin.digest != pin.digest {
                    return Err(RouteError::Stale);
                }
                continue;
            }
            c.charge(262144)?;
            let m = data
                .authorized_metadata(s, &pin.reference, DataUse::Execute)
                .map_err(|_| RouteError::Forbidden)?;
            if m.digest != pin.digest {
                return Err(RouteError::Stale);
            }
            if m.kind == DataKind::TrainingCandidate {
                return Err(RouteError::Unavailable);
            }
            out.push(Observation {
                pin: pin.clone(),
                revision: m.revision,
                kind: m.kind,
                expires_at: m.expires_at,
            });
            c.check()?;
        }
        Ok(out)
    }
    fn recheck<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        s: &DataSession,
        observed: &[Observation],
        c: &mut RouteControl,
    ) -> Result<u64, RouteError> {
        self.live(data)?;
        c.check()?;
        let now = data.trusted_time().map_err(|_| RouteError::Forbidden)?;
        self.stamps(now)?;
        for o in observed {
            c.charge(262144)?;
            let m = data
                .authorized_metadata_at_observed_time(s, &o.pin.reference, DataUse::Execute)
                .map_err(|_| RouteError::Forbidden)?;
            if m.digest != o.pin.digest || m.revision != o.revision {
                return Err(RouteError::Stale);
            }
        }
        c.check()?;
        Ok(now)
    }
    fn entries(&self, requested: &[RouteId], max: usize) -> Result<Vec<&Entry>, RouteError> {
        if requested.is_empty() || requested.len() > max {
            return Err(RouteError::Limit);
        }
        let mut out = vec![];
        for (n, id) in requested.iter().enumerate() {
            if requested[..n].contains(id) {
                return Err(RouteError::InvalidInput);
            }
            out.push(
                self.config
                    .entries
                    .iter()
                    .find(|e| &e.id == id)
                    .ok_or(RouteError::Forbidden)?,
            )
        }
        Ok(out)
    }
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn number(n: u64) -> Value {
    Value::Unsigned(n)
}
fn source_value(o: &Observation, body: Value) -> Value {
    Value::List(vec![
        text(o.pin.reference.project.as_str()),
        text(o.pin.reference.id.as_str()),
        Value::Bytes(o.pin.digest.to_vec()),
        number(o.revision.get()),
        text(o.kind.as_str()),
        number(o.expires_at),
        text("execute"),
        body,
    ])
}
fn value_size(v: &Value) -> Result<usize, RouteError> {
    let own = match v {
        Value::Null | Value::Bool(_) => 1,
        Value::Signed(_) | Value::Unsigned(_) => 9,
        Value::Bytes(b) => 5usize.checked_add(b.len()).ok_or(RouteError::Limit)?,
        Value::Text(s) => 5usize.checked_add(s.len()).ok_or(RouteError::Limit)?,
        Value::List(xs) => {
            let mut n = 5usize;
            for x in xs {
                n = n.checked_add(value_size(x)?).ok_or(RouteError::Limit)?;
            }
            n
        }
        Value::Map(_) => return Err(RouteError::InvalidInput),
    };
    Ok(own)
}
fn encode(v: &Value, max: usize, c: &mut RouteControl) -> Result<Vec<u8>, RouteError> {
    let len = value_size(v)?
        .checked_add(codec::HEADER.len())
        .ok_or(RouteError::Limit)?;
    if len > max {
        return Err(RouteError::Limit);
    }
    c.charge(len * 3)?;
    c.reserve(len * 4 + 32768)?;
    let bytes = codec::encode(v).map_err(|_| RouteError::InvalidInput)?;
    c.check()?;
    Ok(bytes)
}

fn entry_value(e: &Entry) -> Value {
    Value::List(vec![
        text(e.id.as_str()),
        number(e.revision.get()),
        text(match e.intent {
            Intent::Retrieval => "retrieval",
            Intent::TypedSkill => "typed_skill",
            Intent::Code => "code",
            Intent::Document => "document",
        }),
        text(match e.backend {
            RouteBackend::Retrieval => "retrieval",
            RouteBackend::Skill => "skill",
            RouteBackend::External => "external",
            RouteBackend::Unsupported => "unsupported",
        }),
        text(e.input_schema.as_str()),
        text(e.output_schema.as_str()),
    ])
}

impl From<crate::retrieval::HostRetrievalError> for RouteError {
    fn from(e: crate::retrieval::HostRetrievalError) -> Self {
        match e {
            crate::retrieval::HostRetrievalError::Forbidden
            | crate::retrieval::HostRetrievalError::Data(_) => Self::Forbidden,
            crate::retrieval::HostRetrievalError::Core(e) => e.into(),
        }
    }
}
