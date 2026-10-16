//! Bounded declared routing policy. Pure values never grant execution authority.
use crate::resource_estimate::{self as sizing, Claim, Formula, Verdict};
use eylema_contracts::{
    Revision,
    platform::{ModelProfileId, ProjectId, RoleKind, SemanticProfileId},
    project_data::DataRef,
};
pub type RouteId = SemanticProfileId;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pin {
    pub id: SemanticProfileId,
    pub revision: Revision,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub pin: Pin,
    pub valid_from: u64,
    pub valid_until: u64,
}
impl Stamp {
    pub fn fresh(&self, now: u64) -> bool {
        self.valid_from <= now && now <= self.valid_until
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intent {
    Retrieval,
    TypedSkill,
    Code,
    Document,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceStatus {
    DeclaredConsistent,
    Unknown,
    Contradictory,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteBackend {
    Retrieval,
    Skill,
    External,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePin {
    pub reference: DataRef,
    pub digest: [u8; 32],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelBinding {
    pub role: RoleKind,
    pub profile: ModelProfileId,
    pub revision: Revision,
    pub manifest: [u8; 32],
    pub artifact: [u8; 32],
    pub tokenizer: [u8; 32],
    pub preprocessing: [u8; 32],
    pub context_limit: u64,
    pub resident_parameters: u64,
    pub active_parameters: u64,
    pub resident: String,
    pub invocation: String,
    pub backend: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub id: RouteId,
    pub revision: Revision,
    pub intent: Intent,
    pub backend: RouteBackend,
    pub sources: Vec<SourcePin>,
    pub models: Vec<ModelBinding>,
    pub input_schema: SemanticProfileId,
    pub output_schema: SemanticProfileId,
    pub quality: Stamp,
    pub context_limit: u64,
    pub resource: Pin,
    pub grounding: SourceStatus,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub project: ProjectId,
    pub registry: Pin,
    pub policy: Pin,
    pub resource: Pin,
    pub quality: Pin,
    pub intent: Intent,
    pub candidates: Vec<RouteId>,
    pub local_only: bool,
    pub context: u64,
    pub response: u64,
    pub verified_tokens_required: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assessment {
    pub route: RouteId,
    pub utility: u32,
    pub score_profile: Pin,
    pub provenance: Option<SourcePin>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingError {
    InvalidInput,
    Stale,
    Unavailable,
    ResourceLimit,
    ContextLimit,
    TokenCounterUnavailable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Admission {
    pub route: RouteId,
    pub rejection: Option<RoutingError>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Choice {
    Selected(RouteId),
    Abstain,
    Unavailable,
}
pub struct ResourceView<'a> {
    pub snapshot: &'a sizing::Snapshot<'a>,
    pub required_invocations: &'a [&'a str],
    pub required_auxiliary: &'a [&'a str],
    pub valid_from: u64,
    pub valid_until: u64,
}
impl Entry {
    pub fn validate(&self) -> Result<(), RoutingError> {
        if self.sources.len() > 8
            || self.models.len() > 9
            || self.quality.valid_from > self.quality.valid_until
            || self.context_limit == 0
        {
            return Err(RoutingError::InvalidInput);
        }
        for (n, s) in self.sources.iter().enumerate() {
            if s.digest == [0; 32] || self.sources[..n].iter().any(|p| p.reference == s.reference) {
                return Err(RoutingError::InvalidInput);
            }
        }
        for (n, m) in self.models.iter().enumerate() {
            if self.models[..n].iter().any(|p| p.role == m.role)
                || [m.manifest, m.artifact, m.tokenizer, m.preprocessing].contains(&[0; 32])
                || m.context_limit == 0
                || m.resident_parameters == 0
                || m.active_parameters == 0
                || m.active_parameters > m.resident_parameters
                || [&m.resident, &m.invocation, &m.backend]
                    .iter()
                    .any(|s| s.is_empty() || s.len() > 128)
            {
                return Err(RoutingError::InvalidInput);
            }
        }
        Ok(())
    }
    pub fn supported(&self) -> bool {
        self.models.is_empty()
            && matches!(
                (self.intent, self.backend),
                (Intent::Retrieval, RouteBackend::Retrieval)
                    | (Intent::TypedSkill, RouteBackend::Skill)
            )
    }
}
pub fn select(
    entries: &[Entry],
    r: &Request,
    a: &[Assessment],
    ad: &[Admission],
) -> Result<Choice, RoutingError> {
    if entries.is_empty()
        || entries.len() > 16
        || r.candidates.is_empty()
        || r.candidates.len() > 8
        || a.len() != r.candidates.len()
        || ad.len() != r.candidates.len()
    {
        return Err(RoutingError::InvalidInput);
    }
    if r.verified_tokens_required {
        return Err(RoutingError::TokenCounterUnavailable);
    }
    let context = r
        .context
        .checked_add(r.response)
        .ok_or(RoutingError::ContextLimit)?;
    for (n, e) in entries.iter().enumerate() {
        e.validate()?;
        if entries[..n].iter().any(|p| p.id == e.id) {
            return Err(RoutingError::InvalidInput);
        }
    }
    let mut eligible = Vec::with_capacity(8);
    let mut uncertain = false;
    for (n, id) in r.candidates.iter().enumerate() {
        if r.candidates[..n].contains(id) {
            return Err(RoutingError::InvalidInput);
        }
        let e = entries
            .iter()
            .find(|e| &e.id == id)
            .ok_or(RoutingError::InvalidInput)?;
        let scores: Vec<_> = a.iter().filter(|v| &v.route == id).collect();
        let admissions: Vec<_> = ad.iter().filter(|v| &v.route == id).collect();
        if scores.len() != 1
            || admissions.len() != 1
            || scores[0].utility > 1_000_000
            || scores[0].score_profile != a[0].score_profile
        {
            return Err(RoutingError::InvalidInput);
        }
        if e.intent != r.intent
            || !e.supported()
            || e.quality.pin != r.quality
            || e.resource != r.resource
            || context > e.context_limit
            || admissions[0].rejection.is_some()
        {
            continue;
        }
        if e.grounding != SourceStatus::DeclaredConsistent {
            uncertain = true;
            continue;
        }
        eligible.push((scores[0].utility, e.id.clone()));
    }
    if uncertain {
        return Ok(Choice::Abstain);
    }
    if eligible.is_empty() {
        return Ok(Choice::Unavailable);
    }
    eligible.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if eligible[0].0 < 500_000
        || eligible
            .get(1)
            .is_some_and(|next| eligible[0].0 - next.0 < 100_000)
    {
        return Ok(Choice::Abstain);
    }
    Ok(Choice::Selected(eligible.remove(0).1))
}
fn formula_supported(c: Claim<'_, Formula<'_>>) -> bool {
    match c {
        Claim::Unknown { .. } => true,
        Claim::Known { value, .. } => matches!(
            value,
            Formula::Bytes(_)
                | Formula::Weights { .. }
                | Formula::Kv { .. }
                | Formula::Recurrent { .. }
                | Formula::RagVectors { .. }
                | Formula::NotApplicable
        ),
    }
}
/// Bound borrowed resource shapes and string extents without copying or scanning
/// their contents. Call before identity comparison or estimator admission.
/// G80 retains semantic identity/formula validation, including unknown evidence.
pub fn validate_resource_extents(v: &ResourceView<'_>) -> Result<(), RoutingError> {
    let s = v.snapshot;
    if s.pools.len() > 2
        || s.residents.len() > 4
        || s.invocations.len() > 8
        || s.auxiliary.len() > 8
        || s.overlaps
            .is_some_and(|g| g.len() > 4 || g.iter().any(|g| g.members.len() > 16))
        || v.required_invocations.len() > 8
        || v.required_auxiliary.len() > 8
    {
        return Err(RoutingError::ResourceLimit);
    }
    fn extent(s: &str) -> Result<(), RoutingError> {
        if s.len() > 128 {
            Err(RoutingError::ResourceLimit)
        } else {
            Ok(())
        }
    }
    fn claim<T>(c: Claim<'_, T>) -> Result<(), RoutingError> {
        extent(match c {
            Claim::Known { evidence, .. } => evidence.source,
            Claim::Unknown { source, .. } => source,
        })
    }
    fn formula(c: Claim<'_, Formula<'_>>) -> Result<(), RoutingError> {
        claim(c)?;
        if let Claim::Known { value, .. } = c {
            match value {
                Formula::Weights { metadata, .. } => claim(metadata)?,
                Formula::Bytes(_)
                | Formula::Kv { .. }
                | Formula::Recurrent { .. }
                | Formula::RagVectors { .. }
                | Formula::NotApplicable => {}
                // In particular, never traverse a borrowed LoRA target slice.
                _ => return Err(RoutingError::Unavailable),
            }
        }
        Ok(())
    }
    extent(s.id)?;
    extent(s.expected_id)?;
    for id in v.required_invocations.iter().chain(v.required_auxiliary) {
        extent(id)?;
    }
    for p in s.pools {
        extent(p.id)?;
        extent(p.physical_id)?;
        claim(p.capacity)?;
        claim(p.available)?;
        claim(p.reserve)?;
    }
    for r in s.residents {
        extent(r.id)?;
        for k in [r.key, r.expected_key] {
            for id in [k.profile, k.backend, k.owner, k.pool] {
                extent(id)?;
            }
        }
        formula(r.weights)?;
    }
    for i in s.invocations {
        for id in [i.id, i.resident, i.pool] {
            extent(id)?;
        }
        formula(i.state)?;
        formula(i.workspace)?;
    }
    for a in s.auxiliary {
        extent(a.id)?;
        extent(a.pool)?;
        formula(a.size)?;
    }
    if let Some(groups) = s.overlaps {
        for g in groups {
            extent(g.id)?;
            for m in g.members {
                extent(match m {
                    sizing::Member::Invocation(id) | sizing::Member::Auxiliary(id) => id,
                })?;
            }
        }
    }
    Ok(())
}
pub fn check_resources(
    v: &ResourceView<'_>,
    e: &Entry,
    r: &Request,
    now: u64,
) -> Result<(), RoutingError> {
    e.validate()?;
    validate_resource_extents(v)?;
    let s = v.snapshot;
    if now < v.valid_from
        || now > v.valid_until
        || !e.quality.fresh(now)
        || e.quality.pin != r.quality
        || e.resource != r.resource
        || s.id != r.resource.id.as_str()
        || s.revision != r.resource.revision.get()
    {
        return Err(RoutingError::Stale);
    }
    let total = r
        .context
        .checked_add(r.response)
        .ok_or(RoutingError::ContextLimit)?;
    if r.verified_tokens_required {
        return Err(RoutingError::TokenCounterUnavailable);
    }
    if total > e.context_limit {
        return Err(RoutingError::ContextLimit);
    }
    for (n, id) in v.required_invocations.iter().enumerate() {
        if v.required_invocations[..n].contains(id) || !s.invocations.iter().any(|i| i.id == *id) {
            return Err(RoutingError::InvalidInput);
        }
    }
    for (n, id) in v.required_auxiliary.iter().enumerate() {
        if v.required_auxiliary[..n].contains(id) || !s.auxiliary.iter().any(|i| i.id == *id) {
            return Err(RoutingError::InvalidInput);
        }
    }
    for m in &e.models {
        if total > m.context_limit {
            return Err(RoutingError::ContextLimit);
        }
        let resident = s
            .residents
            .iter()
            .find(|x| x.id == m.resident)
            .ok_or(RoutingError::Unavailable)?;
        if resident.key.profile != m.profile.as_str()
            || resident.key.revision != m.revision.get()
            || resident.key.artifact != m.artifact
            || resident.key.backend != m.backend
        {
            return Err(RoutingError::Stale);
        }
        if !matches!(resident.weights,Claim::Known{value:Formula::Weights{resident_parameters,active_parameters:Some(active),..},..}if resident_parameters==m.resident_parameters&&active==m.active_parameters)
        {
            return Err(RoutingError::Unavailable);
        }
        let inv = s
            .invocations
            .iter()
            .find(|x| x.id == m.invocation && x.resident == m.resident)
            .ok_or(RoutingError::Unavailable)?;
        if !v.required_invocations.contains(&inv.id) {
            return Err(RoutingError::Unavailable);
        }
        if matches!(inv.state,Claim::Known{value:Formula::Kv{context,..},..}if context<total) {
            return Err(RoutingError::ContextLimit);
        }
    }
    if s.residents.iter().any(|x| !formula_supported(x.weights))
        || s.invocations
            .iter()
            .any(|x| !formula_supported(x.state) || !formula_supported(x.workspace))
        || s.auxiliary.iter().any(|x| !formula_supported(x.size))
    {
        return Err(RoutingError::Unavailable);
    }
    let report = sizing::estimate(s, now).map_err(|_| RoutingError::InvalidInput)?;
    if report.verdict != Verdict::EstimatedFit {
        return Err(RoutingError::ResourceLimit);
    }
    Ok(())
}
