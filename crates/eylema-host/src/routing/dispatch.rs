use super::*;
use crate::{
    DurableSession,
    retrieval::{RetrievalHost, RetrievalResponse},
    tool::ToolHost,
};
use eylema_contracts::{
    EventId, ExecutionId, project_data::DataRef, skill::SkillLoop, tool::ToolPlan,
};
use eylema_retrieval::SearchQuery;
use eylema_runtime::{
    HostServices,
    resource_estimate::{Claim, Formula, Snapshot},
};
use std::collections::BTreeSet;
#[derive(Clone, Copy)]
pub enum CallPayload<'a> {
    Retrieval(&'a SearchQuery),
    Skill {
        plan: &'a ToolPlan,
        definition: &'a SkillLoop,
        approvals: &'a [EventId],
    },
}
pub enum Selection {
    Selected(SelectionTicket),
    Abstain,
    Unavailable,
}
pub struct SelectionTicket {
    identity: Arc<()>,
    data_identity: Arc<()>,
    session: DataSession,
    route: RouteId,
    digest: [u8; 32],
    observed: Vec<Observation>,
    expires_at: u64,
}
pub enum DispatchReceipt {
    NoMatch,
    Retrieval {
        response: RetrievalResponse,
        no_match: bool,
    },
    SkillAdmitted {
        execution: ExecutionId,
    },
}
impl std::fmt::Debug for SelectionTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SelectionTicket(<opaque>)")
    }
}
impl std::fmt::Debug for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Selected(_) => "Selected(<opaque>)",
            Self::Abstain => "Abstain",
            Self::Unavailable => "Unavailable",
        })
    }
}
impl std::fmt::Debug for DispatchReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch => f.write_str("NoMatch"),
            Self::Retrieval { response, .. } => f
                .debug_struct("RetrievalReceipt")
                .field("hits", &response.hit_count())
                .finish(),
            Self::SkillAdmitted { .. } => f.write_str("SkillAdmitted(<opaque>)"),
        }
    }
}
fn until<T>(c: Claim<'_, T>) -> u64 {
    match c {
        Claim::Known { evidence, .. } => evidence.valid_until,
        Claim::Unknown { .. } => 0,
    }
}
fn formula_until(c: Claim<'_, Formula<'_>>) -> u64 {
    let outer = match &c {
        Claim::Known { evidence, .. } => evidence.valid_until,
        Claim::Unknown { .. } => 0,
    };
    match c {
        Claim::Known {
            value: Formula::Weights { metadata, .. },
            ..
        } => outer.min(until(metadata)),
        _ => outer,
    }
}
fn snapshot_until(s: &Snapshot<'_>) -> u64 {
    let mut end = u64::MAX;
    for p in s.pools {
        end = end
            .min(until(p.capacity))
            .min(until(p.available))
            .min(until(p.reserve));
    }
    for r in s.residents {
        end = end.min(formula_until(r.weights));
    }
    for i in s.invocations {
        end = end
            .min(formula_until(i.state))
            .min(formula_until(i.workspace));
    }
    for a in s.auxiliary {
        end = end.min(formula_until(a.size));
    }
    end
}
impl Router<'_> {
    fn resource(
        &self,
        e: &Entry,
        r: &Request,
        now: u64,
        c: &mut RouteControl,
    ) -> Result<u64, RouteError> {
        let v = self
            .config
            .resources
            .iter()
            .find(|v| {
                v.snapshot.id == e.resource.id.as_str()
                    && v.snapshot.revision == e.resource.revision.get()
            })
            .ok_or(RouteError::Unavailable)?;
        c.charge(1_000_000)?;
        c.reserve(128 * 1024)?;
        let result = eylema_runtime::routing::check_resources(v, e, r, now);
        c.release(128 * 1024);
        result?;
        Ok(snapshot_until(v.snapshot)
            .min(v.valid_until)
            .min(e.quality.valid_until)
            .min(self.config.registry.valid_until)
            .min(self.config.policy.valid_until))
    }
    fn request(&self, r: &Request) -> Result<(), RouteError> {
        if r.project != self.project {
            return Err(RouteError::Forbidden);
        }
        if r.registry != self.config.registry.pin || r.policy != self.config.policy.pin {
            return Err(RouteError::Stale);
        }
        if !r.local_only || r.candidates.is_empty() || r.candidates.len() > 8 {
            return Err(RouteError::Unavailable);
        }
        Ok(())
    }
    pub fn select<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        s: &DataSession,
        description: &Description,
        r: &Request,
        a: &[Assessment],
        payload: CallPayload<'_>,
        c: &mut RouteControl,
    ) -> Result<Selection, RouteError> {
        self.live(data)?;
        self.request(r)?;
        c.check()?;
        if !Arc::ptr_eq(&description.identity, &self.identity) {
            return Err(RouteError::Forbidden);
        }
        if a.len() != r.candidates.len()
            || r.candidates
                .iter()
                .any(|id| !description.entries.contains(id))
        {
            return Err(RouteError::InvalidInput);
        }
        for assessment in a {
            let p = assessment
                .provenance
                .as_ref()
                .ok_or(RouteError::Forbidden)?;
            if !description.observed.iter().any(|o| o.pin == *p) {
                return Err(RouteError::Forbidden);
            }
        }
        let pins: Vec<_> = description.observed.iter().map(|o| o.pin.clone()).collect();
        let observed = self.observe(data, s, &pins, c)?;
        if observed
            .iter()
            .zip(&description.observed)
            .any(|(a, b)| a.revision != b.revision)
        {
            return Err(RouteError::Stale);
        }
        let now = data.trusted_time().map_err(|_| RouteError::Forbidden)?;
        self.stamps(now)?;
        let entries = self.entries(&r.candidates, 8)?;
        let mut admitted = vec![];
        let mut expiries = vec![];
        for e in entries {
            let result = self.resource(e, r, now, c);
            let rejection = match result {
                Ok(end) => {
                    expiries.push((e.id.clone(), end));
                    None
                }
                Err(RouteError::Limit) => Some(RoutingError::ResourceLimit),
                Err(RouteError::Stale) => Some(RoutingError::Stale),
                Err(RouteError::Unavailable) => Some(RoutingError::Unavailable),
                Err(e) => return Err(e),
            };
            admitted.push(Admission {
                route: e.id.clone(),
                rejection,
            });
        }
        let choice = eylema_runtime::routing::select(&self.config.entries, r, a, &admitted)?;
        let Choice::Selected(id) = choice else {
            self.recheck(data, s, &observed, c)?;
            return Ok(match choice {
                Choice::Abstain => Selection::Abstain,
                _ => Selection::Unavailable,
            });
        };
        let e = self
            .config
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or(RouteError::InvalidInput)?;
        match payload {
            CallPayload::Retrieval(_) if e.backend != RouteBackend::Retrieval => {
                return Err(RouteError::Unavailable);
            }
            CallPayload::Skill { .. } if e.backend != RouteBackend::Skill => {
                return Err(RouteError::Unavailable);
            }
            _ => {}
        }
        let digest = super::wire::fingerprint(e, r, a, payload, c)?;
        let end = expiries
            .iter()
            .find(|(route, _)| *route == id)
            .map(|(_, end)| *end)
            .ok_or(RouteError::Unavailable)?;
        let now = self.recheck(data, s, &observed, c)?;
        if now > end {
            return Err(RouteError::Stale);
        }
        Ok(Selection::Selected(SelectionTicket {
            identity: self.identity.clone(),
            data_identity: self.data_identity.clone(),
            session: s.clone(),
            route: id,
            digest,
            observed,
            expires_at: end,
        }))
    }
    fn prepare<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        t: &SelectionTicket,
        r: &Request,
        a: &[Assessment],
        payload: CallPayload<'_>,
        c: &mut RouteControl,
    ) -> Result<u64, RouteError> {
        self.live(data)?;
        self.request(r)?;
        if !Arc::ptr_eq(&t.identity, &self.identity)
            || !Arc::ptr_eq(&t.data_identity, &self.data_identity)
        {
            return Err(RouteError::Forbidden);
        }
        let now = self.recheck(data, &t.session, &t.observed, c)?;
        if now > t.expires_at {
            return Err(RouteError::Stale);
        }
        let e = self
            .config
            .entries
            .iter()
            .find(|e| e.id == t.route)
            .ok_or(RouteError::Stale)?;
        if super::wire::fingerprint(e, r, a, payload, c)? != t.digest {
            return Err(RouteError::Stale);
        }
        let end = self.resource(e, r, now, c)?;
        Ok(end.min(t.expires_at))
    }
    pub fn call_retrieval<T: Storage, H: Sha256, C: DataClock, IT: Storage, IH: Sha256>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        t: SelectionTicket,
        r: &Request,
        a: &[Assessment],
        q: &SearchQuery,
        index: &RetrievalHost<IT, IH>,
        c: &mut RouteControl,
    ) -> Result<DispatchReceipt, RouteError> {
        let end = self.prepare(data, &t, r, a, CallPayload::Retrieval(q), c)?;
        let e = self
            .config
            .entries
            .iter()
            .find(|e| e.id == t.route)
            .ok_or(RouteError::Stale)?;
        if e.backend != RouteBackend::Retrieval
            || q.sources
                .iter()
                .any(|r| !e.sources.iter().any(|s| s.reference == *r))
        {
            return Err(RouteError::Forbidden);
        }
        let response = index.search(data, &t.session, q, &mut c.inner)?;
        let no_match = response.hit_count() == 0
            || (matches!(&q.input, eylema_retrieval::QueryInput::Lexical(_))
                && super::wire::lexical_no_match(&response, c)?);
        let now = self.recheck(data, &t.session, &t.observed, c)?;
        if now > end {
            return Err(RouteError::Stale);
        }
        if no_match {
            return Ok(DispatchReceipt::NoMatch);
        }
        Ok(DispatchReceipt::Retrieval {
            no_match: false,
            response,
        })
    }
    pub fn call_skill<
        T: Storage,
        H: Sha256,
        C: DataClock,
        HT: Storage,
        HH: Sha256,
        S: HostServices,
    >(
        &self,
        data: &mut ProjectStore<T, H, C>,
        t: SelectionTicket,
        r: &Request,
        a: &[Assessment],
        plan: ToolPlan,
        definition: SkillLoop,
        approvals: &[EventId],
        tools: &mut ToolHost<HT, HH, S>,
        task_session: &DurableSession,
        c: &mut RouteControl,
    ) -> Result<DispatchReceipt, RouteError> {
        let end = self.prepare(
            data,
            &t,
            r,
            a,
            CallPayload::Skill {
                plan: &plan,
                definition: &definition,
                approvals,
            },
            c,
        )?;
        if !self
            .config
            .entries
            .iter()
            .any(|e| e.id == t.route && e.backend == RouteBackend::Skill)
        {
            return Err(RouteError::Unavailable);
        }
        let mut seen = BTreeSet::<DataRef>::new();
        let mut stack: Vec<_> = definition
            .calls()
            .iter()
            .map(|s| (s.reference.clone(), 0usize))
            .collect();
        while let Some((reference, depth)) = stack.pop() {
            if seen.contains(&reference) {
                continue;
            }
            if depth > 32 || seen.len() >= 64 || stack.len() > 1024 {
                return Err(RouteError::Limit);
            }
            c.charge(262144)?;
            let m = data
                .authorized_metadata(&t.session, &reference, DataUse::Execute)
                .map_err(|_| RouteError::Forbidden)?;
            seen.insert(reference);
            for p in m.parents {
                stack.push((p, depth + 1));
            }
        }
        if t.observed.iter().any(|o| !seen.contains(&o.pin.reference)) {
            return Err(RouteError::Unavailable);
        }
        let now = self.recheck(data, &t.session, &t.observed, c)?;
        let mut deadline = end;
        for o in &t.observed {
            deadline = deadline.min(
                data.authorized_deadline_at_observed_time(
                    &t.session,
                    &o.pin.reference,
                    DataUse::Execute,
                )
                .map_err(|_| RouteError::Forbidden)?,
            );
        }
        if now > deadline
            || plan
                .proposal()
                .fields()
                .actions
                .iter()
                .any(|a| a.fields().budget.deadline().unix_ms() > deadline)
        {
            return Err(RouteError::Stale);
        }
        c.charge(4_000_000)?;
        c.reserve(16 * 1024 * 1024)?;
        c.check()?;
        let result = tools.admit_skill(
            data,
            &t.session,
            task_session,
            (plan, definition),
            approvals,
        );
        c.release(16 * 1024 * 1024);
        let execution = result.map_err(RouteError::Tool)?;
        let now = self
            .recheck(data, &t.session, &t.observed, c)
            .map_err(|_| RouteError::AdmissionOutcomeUnknown)?;
        if now > deadline || c.check().is_err() {
            return Err(RouteError::AdmissionOutcomeUnknown);
        }
        Ok(DispatchReceipt::SkillAdmitted { execution })
    }
}
