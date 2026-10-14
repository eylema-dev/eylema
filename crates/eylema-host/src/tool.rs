//! Sealed own-tool facade: no raw scheduler escape, no implicit data grant.
use crate::decision::{DecisionEvidence, Measurement};
use crate::skill::{SkillEvidence, SkillWitness};
use crate::{
    project_data::{DataClock, DataError, DataSession, ProjectStore},
    *,
};
use eylema_contracts::decision::DecisionDeclaration;
use eylema_contracts::skill::SkillLoop;
use eylema_contracts::{project_data::DataUse, tool::ToolPlan};
use eylema_runtime::tool::ToolRegistry;
use std::time::Instant;
use std::{collections::BTreeMap, sync::Arc};
pub const MAX_TOOL_EXECUTIONS: usize = 128;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolError {
    Runtime(ErrorCode),
    Host(HostError),
    Data(DataError),
    Busy,
    InputMismatch,
    WrongBackingStore,
    ProvenanceUnavailable,
    InvalidEvidence,
    DeadlineNotBound,
}
impl From<HostError> for ToolError {
    fn from(e: HostError) -> Self {
        Self::Host(e)
    }
}
impl From<DataError> for ToolError {
    fn from(e: DataError) -> Self {
        Self::Data(e)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolStatus {
    Pending,
    Completed,
    Failed,
    Stopped,
    Partial,
    Unknown,
}
#[derive(Clone, Eq, PartialEq)]
pub struct ToolOutput {
    pub action_id: ActionId,
    pub evidence: EvidenceRef,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Eq, PartialEq)]
pub struct ToolReport {
    pub execution_id: ExecutionId,
    pub status: ToolStatus,
    pub outputs: Vec<ToolOutput>,
}
impl std::fmt::Debug for ToolReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolReport")
            .field("status", &self.status)
            .field("outputs", &self.outputs.len())
            .finish()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryInspection {
    pub status: ToolStatus,
    pub confirmed_actions: usize,
    pub unknown_actions: usize,
}
struct Provenance {
    plan: ToolPlan,
    session: DataSession,
    actor: DurableSession,
    decision: Option<DecisionDeclaration>,
    measurement: Measurement,
    skill: Option<SkillWitness>,
}
struct Pending {
    id: ExecutionId,
    cancel_requested: bool,
}
pub struct ToolHost<T: Storage, H: Sha256, S: HostServices> {
    host: DurableHost<T, H, S>,
    registry: ToolRegistry,
    records: BTreeMap<ExecutionId, Provenance>,
    active: Option<Pending>,
    data_identity: Option<Arc<()>>,
    measurement_identity: Arc<()>,
}
impl<T: Storage, H: Sha256, S: HostServices> ToolHost<T, H, S> {
    /// Fresh host only: importing recovered tasks without durable tool lineage
    /// would allow a replay to relabel a private source-backed input as literal.
    pub fn new(
        host: DurableHost<T, H, S>,
        registry: ToolRegistry,
    ) -> std::result::Result<Self, ToolError> {
        if host.attempt != 0 || host.has_pending_work()? {
            return Err(ToolError::ProvenanceUnavailable);
        }
        Ok(Self {
            host,
            registry,
            records: BTreeMap::new(),
            active: None,
            data_identity: None,
            measurement_identity: Arc::new(()),
        })
    }
    pub fn enroll(
        &mut self,
        o: &Owner,
        g: Grant,
    ) -> std::result::Result<DurableSession, ToolError> {
        Ok(self.host.enroll(o, g)?)
    }
    pub fn enroll_verified_human(
        &mut self,
        o: &Owner,
        g: Grant,
    ) -> std::result::Result<DurableHuman, ToolError> {
        Ok(self.host.enroll_verified_human(o, g)?)
    }
    pub fn create_task(
        &mut self,
        s: &DurableSession,
        t: TaskSpec,
    ) -> std::result::Result<(), ToolError> {
        Ok(self.host.create_task(s, t)?)
    }
    pub fn preview_approval(
        &mut self,
        s: &DurableSession,
        plan: &ToolPlan,
        expires: Timestamp,
    ) -> std::result::Result<ApprovalBinding, ToolError> {
        self.registry.validate(plan).map_err(ToolError::Runtime)?;
        Ok(self
            .host
            .preview_approval(s, plan.task(), plan.proposal(), expires)?)
    }
    /// Approval is for concrete effect bytes, not source data-use authority.
    pub fn create_approval(
        &mut self,
        h: &DurableHuman,
        plan: &ToolPlan,
        binding: ApprovalBinding,
    ) -> std::result::Result<EventId, ToolError> {
        self.registry.validate(plan).map_err(ToolError::Runtime)?;
        Ok(self
            .host
            .create_approval(h, plan.task(), plan.proposal(), binding)?)
    }
    pub fn revoke_delegation(
        &mut self,
        h: &DurableHuman,
        task: &TaskId,
        delegated: &DurableSession,
    ) -> std::result::Result<Revocation, ToolError> {
        Ok(self.host.revoke_delegation(h, task, delegated)?)
    }
    fn binding<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &ProjectStore<D, F, C>,
    ) -> std::result::Result<(), ToolError> {
        if self
            .data_identity
            .as_ref()
            .is_some_and(|bound| !Arc::ptr_eq(bound, data.memory_binding().1))
        {
            return Err(ToolError::WrongBackingStore);
        }
        Ok(())
    }
    fn source_checks<D: Storage, F: Sha256, C: DataClock>(
        data: &mut ProjectStore<D, F, C>,
        row: &Provenance,
    ) -> std::result::Result<(), ToolError> {
        if let Some(skill) = &row.skill {
            for source in &skill.controls {
                let metadata = data.authorized_metadata(&row.session, source, DataUse::Execute)?;
                if skill
                    .uses
                    .iter()
                    .any(|u| &u.reference == source && u.source_put_digest != metadata.digest)
                {
                    return Err(ToolError::InvalidEvidence);
                }
            }
        }
        for (b, a) in row
            .plan
            .bindings()
            .iter()
            .zip(&row.plan.proposal().fields().actions)
        {
            if let Some(r) = &b.source {
                let bytes = data.read(&row.session, r, DataUse::Execute)?;
                if bytes != a.fields().payload {
                    return Err(ToolError::InputMismatch);
                }
            }
        }
        Ok(())
    }
    fn snapshot_deadline<D: Storage, F: Sha256, C: DataClock>(
        data: &ProjectStore<D, F, C>,
        row: &Provenance,
    ) -> std::result::Result<Option<u64>, ToolError> {
        let mut deadline: Option<u64> = None;
        if let Some(skill) = &row.skill {
            for source in &skill.controls {
                let bound = data.authorized_deadline_at_observed_time(
                    &row.session,
                    source,
                    DataUse::Execute,
                )?;
                deadline = Some(deadline.map_or(bound, |old| old.min(bound)));
            }
        }
        for b in row.plan.bindings() {
            if let Some(r) = &b.source {
                let bound =
                    data.authorized_deadline_at_observed_time(&row.session, r, DataUse::Execute)?;
                deadline = Some(deadline.map_or(bound, |old| old.min(bound)));
            }
        }
        Ok(deadline)
    }
    /// Includes every past input to the cumulative task value, not just the
    /// current plan. Records never evict or silently widen source permissions.
    fn task_sources<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        task: &TaskId,
        extra: Option<&Provenance>,
        executing: Option<&ToolPlan>,
    ) -> std::result::Result<(), ToolError> {
        self.binding(data)?;
        let rows = self
            .records
            .values()
            .filter(|r| &r.plan.task().fields().task_id == task);
        for row in rows.clone().chain(extra) {
            Self::source_checks(data, row)?;
        }
        data.trusted_time()?;
        let mut deadline: Option<u64> = None;
        for row in rows.chain(extra) {
            if let Some(bound) = Self::snapshot_deadline(data, row)? {
                deadline = Some(deadline.map_or(bound, |old| old.min(bound)));
            }
        }
        if let (Some(deadline), Some(plan)) = (deadline, executing)
            && plan
                .proposal()
                .fields()
                .actions
                .iter()
                .any(|a| a.fields().budget.deadline().unix_ms() > deadline)
        {
            return Err(ToolError::DeadlineNotBound);
        }
        Ok(())
    }
    pub fn task_spec<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        s: &DurableSession,
        id: &TaskId,
    ) -> std::result::Result<TaskSpec, ToolError> {
        let task = self.host.task(s, id)?;
        self.task_sources(data, id, None, None)?;
        Ok(task.task)
    }
    pub fn admit<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
        ds: &DataSession,
        s: &DurableSession,
        plan: ToolPlan,
        approvals: &[EventId],
    ) -> std::result::Result<ExecutionId, ToolError> {
        self.admit_inner(data, ds, s, (plan, None), approvals)
    }
    pub fn admit_skill<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
        ds: &DataSession,
        s: &DurableSession,
        request: (ToolPlan, SkillLoop),
        approvals: &[EventId],
    ) -> std::result::Result<ExecutionId, ToolError> {
        let (plan, definition) = request;
        self.binding(data)?;
        self.host.task(s, &plan.task().fields().task_id)?;
        let skill = crate::skill::resolve(data, ds, &plan, definition)?;
        self.admit_inner(data, ds, s, (plan, Some(skill)), approvals)
    }
    fn admit_inner<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
        ds: &DataSession,
        s: &DurableSession,
        request: (ToolPlan, Option<SkillWitness>),
        approvals: &[EventId],
    ) -> std::result::Result<ExecutionId, ToolError> {
        let (plan, skill) = request;
        let started = Instant::now();
        self.registry.validate(&plan).map_err(ToolError::Runtime)?;
        self.host.task(s, &plan.task().fields().task_id)?;
        let task = &plan.task().fields().task_id;
        if self
            .records
            .values()
            .any(|old| old.plan == plan && old.skill != skill)
        {
            return Err(ToolError::ProvenanceUnavailable);
        }
        let source_constrained = skill.is_some()
            || plan.bindings().iter().any(|b| b.source.is_some())
            || self.records.values().any(|row| {
                &row.plan.task().fields().task_id == task
                    && (row.skill.is_some()
                        || row.plan.bindings().iter().any(|b| b.source.is_some()))
            });
        // A source denial can block normal stepping. Require explicit actual
        // intervention so G11 reserves a real scoped cancellation path and
        // checks controller availability again before every effect.
        if source_constrained
            && plan
                .proposal()
                .fields()
                .actions
                .iter()
                .any(|a| !a.fields().authority.oversight().requires_intervention())
        {
            return Err(ToolError::Runtime(ErrorCode::CapabilityUnavailable));
        }

        if self
            .active
            .as_ref()
            .is_some_and(|active| self.records[&active.id].plan != plan)
        {
            return Err(ToolError::Busy);
        }
        let row = Provenance {
            plan,
            session: ds.clone(),
            actor: s.clone(),
            decision: None,
            measurement: Measurement::new(started, self.records.is_empty()),
            skill,
        };
        self.task_sources(
            data,
            &row.plan.task().fields().task_id,
            Some(&row),
            Some(&row.plan),
        )?;
        if self.records.len() >= MAX_TOOL_EXECUTIONS
            && !self
                .records
                .values()
                .any(|old| old.plan == row.plan && old.skill == row.skill)
        {
            return Err(ToolError::Runtime(ErrorCode::BudgetExceeded));
        }
        let id = self.host.admit_with_approvals(
            s,
            row.plan.task().clone(),
            row.plan.proposal().clone(),
            approvals,
        )?;
        if let Some(old) = self.records.get(&id) {
            if old.plan != row.plan || old.skill != row.skill {
                return Err(ToolError::ProvenanceUnavailable);
            }
        } else {
            self.records.insert(id.clone(), row);
            self.records
                .get_mut(&id)
                .expect("inserted own row")
                .measurement
                .timing
                .admission = started.elapsed();
        }
        self.data_identity
            .get_or_insert_with(|| data.memory_binding().1.clone());
        if !self.host.execution(s, &id)?.fields().state.is_terminal() && self.active.is_none() {
            self.active = Some(Pending {
                id: id.clone(),
                cancel_requested: false,
            });
        }
        Ok(id)
    }
    pub fn step<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
    ) -> std::result::Result<Option<ExecutionId>, ToolError> {
        let started = Instant::now();
        let Some(p) = &self.active else {
            return Ok(None);
        };
        let id = p.id.clone();
        if !p.cancel_requested {
            let row = &self.records[&id];
            self.task_sources(
                data,
                &row.plan.task().fields().task_id,
                None,
                Some(&row.plan),
            )?;
        }
        let result = self.host.step()?;
        if result.as_ref() != Some(&id) {
            return Err(ToolError::InvalidEvidence);
        }
        // This facade owns the only queued job. A revoked actor may no longer
        // read its terminal record; use the trusted scheduler's actual idle flag.
        let terminal = !self.host.has_pending_work()?;
        self.records
            .get_mut(&id)
            .expect("owned active row")
            .measurement
            .step(started, terminal);
        if terminal {
            self.active = None;
        }
        Ok(result)
    }
    pub fn request_cancellation(
        &mut self,
        h: &DurableHuman,
        id: &ExecutionId,
    ) -> std::result::Result<CancellationOutcome, ToolError> {
        let started = Instant::now();
        let result = self.host.request_cancellation(h, id)?;
        if matches!(&result,CancellationOutcome::Requested(receipt) if &receipt.execution_id==id)
            && let Some(p) = &mut self.active
            && &p.id == id
        {
            p.cancel_requested = true;
            self.records
                .get_mut(id)
                .expect("owned active row")
                .measurement
                .cancellation_started
                .get_or_insert(started);
        }
        Ok(result)
    }
    pub fn attach_decision<D: Storage, F: Sha256, C: DataClock>(
        &mut self,
        data: &mut ProjectStore<D, F, C>,
        s: &DurableSession,
        id: &ExecutionId,
        declaration: DecisionDeclaration,
    ) -> std::result::Result<(), ToolError> {
        self.report(data, s, id)?;
        let row = self
            .records
            .get_mut(id)
            .ok_or(ToolError::ProvenanceUnavailable)?;
        if !Arc::ptr_eq(&row.actor.identity, &s.identity) || row.actor.slot != s.slot {
            return Err(ToolError::Runtime(ErrorCode::Forbidden));
        }
        if let Some(old) = &row.decision {
            return if old == &declaration {
                Ok(())
            } else {
                Err(ToolError::InvalidEvidence)
            };
        }
        if !row.measurement.timing.steps.is_empty() || row.measurement.ended.is_some() {
            return Err(ToolError::InvalidEvidence);
        }
        let c = declaration.selected();
        if !row
            .plan
            .bindings()
            .iter()
            .any(|b| b.action_id == c.action && b.profile == c.profile && b.revision == c.revision)
        {
            return Err(ToolError::InvalidEvidence);
        }
        row.decision = Some(declaration);
        Ok(())
    }
    pub fn decision_evidence<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        s: &DurableSession,
        id: &ExecutionId,
    ) -> std::result::Result<DecisionEvidence, ToolError> {
        let outcome = self.report(data, s, id)?;
        let row = self
            .records
            .get(id)
            .ok_or(ToolError::ProvenanceUnavailable)?;
        let declaration = row
            .decision
            .clone()
            .ok_or(ToolError::ProvenanceUnavailable)?;
        let values = row
            .plan
            .bindings()
            .iter()
            .zip(&row.plan.proposal().fields().actions)
            .map(|(b, a)| {
                let a = a.fields();
                codec::Value::List(vec![
                    codec::Value::Text(b.profile.as_str().into()),
                    codec::Value::Unsigned(b.revision.get()),
                    codec::Value::Text(a.operation_id.as_str().into()),
                    codec::Value::Bytes(a.payload.clone()),
                    codec::Value::Unsigned(a.budget.max_payload_bytes() as u64),
                ])
            })
            .collect();
        let workload = codec::encode(&codec::Value::List(vec![
            codec::Value::List(values),
            row.skill
                .as_ref()
                .map_or(codec::Value::Null, |w| w.workload_key()),
        ]))
        .map_err(|_| ToolError::InvalidEvidence)?;
        Ok(DecisionEvidence {
            task: row.plan.task().fields().task_id.clone(),
            declaration,
            outcome,
            timing: row.measurement.timing.clone(),
            identity: self.measurement_identity.clone(),
            started: row.measurement.started,
            ended: row.measurement.ended,
            workload,
            declared_payload_budget: row.plan.task().fields().budget.max_payload_bytes(),
        })
    }
    pub fn skill_evidence<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        s: &DurableSession,
        id: &ExecutionId,
    ) -> std::result::Result<SkillEvidence, ToolError> {
        let outcome = self.report(data, s, id)?;
        let witness = self
            .records
            .get(id)
            .and_then(|row| row.skill.clone())
            .ok_or(ToolError::ProvenanceUnavailable)?;
        Ok(SkillEvidence { witness, outcome })
    }
    pub fn report<D: Storage, F: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<D, F, C>,
        s: &DurableSession,
        id: &ExecutionId,
    ) -> std::result::Result<ToolReport, ToolError> {
        let e = self.host.execution(s, id)?;
        let row = self
            .records
            .get(id)
            .ok_or(ToolError::ProvenanceUnavailable)?;
        self.task_sources(data, &row.plan.task().fields().task_id, None, None)?;
        let mut outputs = vec![];
        for (index, effect) in e.fields().effects.iter().enumerate() {
            if row
                .plan
                .bindings()
                .get(index)
                .is_none_or(|b| b.action_id != effect.fields().action_id)
            {
                return Err(ToolError::InvalidEvidence);
            }
            for reference in &effect.fields().evidence {
                let observation = self.host.observation(s, reference)?;
                if observation.execution_id.as_ref() != Some(id)
                    || observation.task_id != row.plan.task().fields().task_id
                {
                    return Err(ToolError::InvalidEvidence);
                }
                outputs.push(ToolOutput {
                    action_id: effect.fields().action_id.clone(),
                    evidence: reference.clone(),
                    bytes: codec::encode(&Value::Unsigned(observation.after))
                        .map_err(|_| ToolError::InvalidEvidence)?,
                });
            }
        }
        let applied = e
            .fields()
            .effects
            .iter()
            .any(|e| e.fields().outcome == EffectOutcome::Applied);
        let status = match e.fields().state {
            ExecutionState::OutcomeUnknown => ToolStatus::Unknown,
            ExecutionState::Succeeded => ToolStatus::Completed,
            ExecutionState::Failed | ExecutionState::Cancelled if applied => ToolStatus::Partial,
            ExecutionState::Failed => ToolStatus::Failed,
            ExecutionState::Cancelled => ToolStatus::Stopped,
            _ => ToolStatus::Pending,
        };
        Ok(ToolReport {
            execution_id: id.clone(),
            status,
            outputs,
        })
    }
    /// Reconciliation inspection only. Lost typed lineage cannot authorize a
    /// restart, expose private numeric outputs or invent confirmed success.
    pub fn inspect_recovery(
        host: &DurableHost<T, H, S>,
        s: &DurableSession,
        id: &ExecutionId,
    ) -> std::result::Result<RecoveryInspection, ToolError> {
        let e = host.execution(s, id)?;
        if e.fields().state != ExecutionState::OutcomeUnknown {
            return Err(ToolError::ProvenanceUnavailable);
        }
        Ok(RecoveryInspection {
            status: ToolStatus::Unknown,
            confirmed_actions: e
                .fields()
                .effects
                .iter()
                .filter(|e| {
                    e.fields().outcome != EffectOutcome::Unknown && !e.fields().evidence.is_empty()
                })
                .count(),
            unknown_actions: e
                .fields()
                .effects
                .iter()
                .filter(|e| e.fields().outcome == EffectOutcome::Unknown)
                .count(),
        })
    }
    pub fn close(self) {
        self.host.close()
    }
}
