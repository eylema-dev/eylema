use crate::need::{require, text, token, unique};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

/// Constructed only in the trusted control host after identity/policy resolution.
/// Its entire accepted NeedSpec must match; an opaque reference is insufficient.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedTask {
    need: NeedSpec,
    base: Commit,
    principal: String,
    policy: String,
    expires_ms: u64,
}

impl AcceptedTask {
    pub fn new(
        need: NeedSpec,
        base: Commit,
        principal: String,
        policy: String,
        expires_ms: u64,
    ) -> Result<Self> {
        require(token(&principal) && token(&policy) && expires_ms > 0)?;
        Ok(Self {
            need,
            base,
            principal,
            policy,
            expires_ms,
        })
    }
}

/// Trusted host boundary, not a provider-supplied authorization callback.
pub trait AuthorityResolver {
    fn resolve(&self, need: &NeedSpec) -> Option<AcceptedTask>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Received,
    SpecReady,
    Planned,
    Implementing,
    LocalVerification,
    IndependentReview,
    DraftPr,
    RepositoryChecks,
    ReadyForMaintainer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Running(Stage),
    NeedsInput(Stage),
    Repairing,
    CancelRequested,
    Cancelled,
    BudgetExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Usage {
    pub model_calls: u32,
    pub repairs: u32,
    pub reserved_units: u64,
}

/// Host-observed check evidence. Provider self-attestation must not be routed here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckEvidence {
    pub head: Commit,
    pub passed_checks: Vec<String>,
}

/// A fresh separate context and blocking-finding result observed by the host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewEvidence {
    pub head: Commit,
    pub context: String,
    pub blocking_findings: u32,
    pub passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallTicket {
    key: TaskKey,
    ordinal: u32,
}

impl CallTicket {
    pub fn key(&self) -> &TaskKey {
        &self.key
    }
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

/// One trusted host's bounded in-memory run registry. Same-key retries retain
/// counters/start time; persistence across host restart belongs to a later host.
pub struct Registry {
    capacity: usize,
    runs: BTreeMap<(String, String, u64), Coordinator>,
}

impl Registry {
    pub fn new(capacity: usize) -> Result<Self> {
        require((1..=128).contains(&capacity))?;
        Ok(Self {
            capacity,
            runs: BTreeMap::new(),
        })
    }
    fn key(key: &TaskKey) -> (String, String, u64) {
        (
            key.repository().into(),
            key.request_id().as_str().into(),
            key.revision().get(),
        )
    }
    pub fn receive(&mut self, need: NeedSpec, now_ms: u64) -> Result<&mut Coordinator> {
        let key = Self::key(need.key());
        if let Some(existing) = self.runs.get_mut(&key) {
            if existing.need != need {
                return Err(Error::StaleBinding);
            }
            existing.observe_time(now_ms)?;
        } else {
            if self.runs.len() >= self.capacity {
                return Err(Error::BudgetExhausted);
            }
            self.runs
                .insert(key.clone(), Coordinator::receive(need, now_ms)?);
        }
        self.runs.get_mut(&key).ok_or(Error::InvalidState)
    }
    pub fn get(&self, key: &TaskKey) -> Option<&Coordinator> {
        self.runs.get(&Self::key(key))
    }
    pub fn get_mut(&mut self, key: &TaskKey) -> Option<&mut Coordinator> {
        self.runs.get_mut(&Self::key(key))
    }
}

#[derive(Debug)]
pub struct Coordinator {
    need: NeedSpec,
    accepted: Option<AcceptedTask>,
    state: State,
    plan: Option<WorkPlan>,
    head: Option<Commit>,
    started_ms: u64,
    last_ms: u64,
    usage: Usage,
    active_call: Option<CallTicket>,
    reason: Option<String>,
    pr_number: Option<u64>,
}

impl Coordinator {
    fn receive(need: NeedSpec, now_ms: u64) -> Result<Self> {
        require(now_ms > 0 && now_ms.checked_add(need.limits().wall_ms).is_some())?;
        Ok(Self {
            need,
            accepted: None,
            state: State::Running(Stage::Received),
            plan: None,
            head: None,
            started_ms: now_ms,
            last_ms: now_ms,
            usage: Usage {
                model_calls: 0,
                repairs: 0,
                reserved_units: 0,
            },
            active_call: None,
            reason: None,
            pr_number: None,
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn usage(&self) -> Usage {
        self.usage
    }
    pub fn need(&self) -> &NeedSpec {
        &self.need
    }
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
    pub fn pr_number(&self) -> Option<u64> {
        self.pr_number
    }
    pub fn head(&self) -> Option<&Commit> {
        self.head.as_ref()
    }
    pub fn active_call(&self) -> Option<&CallTicket> {
        self.active_call.as_ref()
    }
    pub fn poll(&mut self, now: u64, authority: &impl AuthorityResolver) -> Result<()> {
        self.guard(now, authority)
    }

    fn clock(&mut self, now: u64) -> Result<()> {
        if now < self.last_ms {
            return Err(Error::ClockRegressed);
        }
        self.last_ms = now;
        Ok(())
    }
    fn observe_time(&mut self, now: u64) -> Result<()> {
        self.clock(now)?;
        if !matches!(self.state, State::Cancelled | State::CancelRequested)
            && now - self.started_ms >= self.need.limits().wall_ms
        {
            self.state = State::BudgetExhausted;
        }
        Ok(())
    }
    fn guard(&mut self, now: u64, authority: &impl AuthorityResolver) -> Result<()> {
        self.observe_time(now)?;
        if matches!(self.state, State::Cancelled | State::CancelRequested) {
            return Err(Error::Cancelled);
        }
        if self.state == State::BudgetExhausted
            || now - self.started_ms >= self.need.limits().wall_ms
        {
            self.state = State::BudgetExhausted;
            return Err(Error::BudgetExhausted);
        }
        let resolved = authority.resolve(&self.need).ok_or(Error::Forbidden)?;
        if resolved.need != self.need || now >= resolved.expires_ms {
            return Err(Error::Forbidden);
        }
        if let Some(accepted) = &self.accepted {
            if accepted != &resolved {
                return Err(Error::StaleBinding);
            }
        } else {
            self.accepted = Some(resolved);
        }
        Ok(())
    }
    fn at(&self, stage: Stage) -> Result<()> {
        if self.active_call.is_some() {
            return Err(Error::CallInFlight);
        }
        if self.state != State::Running(stage) {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    pub fn spec_ready(&mut self, now: u64, authority: &impl AuthorityResolver) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::Received)?;
        self.state = State::Running(Stage::SpecReady);
        Ok(())
    }
    pub fn set_plan(
        &mut self,
        plan: WorkPlan,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::SpecReady)?;
        plan.validate(
            &self.need,
            &self.accepted.as_ref().ok_or(Error::Forbidden)?.base,
        )?;
        self.plan = Some(plan);
        self.state = State::Running(Stage::Planned);
        Ok(())
    }
    pub fn begin(&mut self, now: u64, authority: &impl AuthorityResolver) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::Planned)?;
        self.state = State::Running(Stage::Implementing);
        Ok(())
    }
    pub fn submit_change(
        &mut self,
        head: Commit,
        paths: Vec<String>,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::Implementing)?;
        require(
            !paths.is_empty()
                && paths.len() <= 256
                && unique(&paths)
                && paths.iter().all(|p| self.need.permits_path(p)),
        )?;
        self.head = Some(head);
        self.state = State::Running(Stage::LocalVerification);
        Ok(())
    }
    fn checks(&self, evidence: &CheckEvidence) -> Result<()> {
        if self.head.as_ref() != Some(&evidence.head) {
            return Err(Error::StaleBinding);
        }
        require(
            !evidence.passed_checks.is_empty()
                && evidence.passed_checks.len() <= 64
                && evidence.passed_checks.iter().all(|c| token(c))
                && unique(&evidence.passed_checks),
        )?;
        let wanted: BTreeSet<_> = self
            .plan
            .as_ref()
            .ok_or(Error::InvalidState)?
            .checks
            .iter()
            .map(|c| &c.check)
            .collect();
        let actual: BTreeSet<_> = evidence.passed_checks.iter().collect();
        if wanted != actual {
            return Err(Error::FailedEvidence);
        }
        Ok(())
    }
    pub fn local_verified(
        &mut self,
        evidence: CheckEvidence,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::LocalVerification)?;
        self.checks(&evidence)?;
        self.state = State::Running(Stage::IndependentReview);
        Ok(())
    }
    pub fn reviewed(
        &mut self,
        evidence: ReviewEvidence,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::IndependentReview)?;
        if self.head.as_ref() != Some(&evidence.head) {
            return Err(Error::StaleBinding);
        }
        require(token(&evidence.context))?;
        if !evidence.passed
            || evidence.blocking_findings != 0
            || evidence.context
                == self
                    .plan
                    .as_ref()
                    .ok_or(Error::InvalidState)?
                    .implementer_context
        {
            return Err(Error::FailedEvidence);
        }
        self.state = State::Running(Stage::DraftPr);
        Ok(())
    }
    /// Records a trusted delivery observation; it does not create a PR.
    pub fn pr_observed(
        &mut self,
        repository: &str,
        head: &Commit,
        number: u64,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::DraftPr)?;
        if repository != self.need.key().repository() || self.head.as_ref() != Some(head) {
            return Err(Error::StaleBinding);
        }
        require(number > 0)?;
        if self.pr_number.is_some_and(|known| known != number) {
            return Err(Error::StaleBinding);
        }
        self.pr_number = Some(number);
        self.state = State::Running(Stage::RepositoryChecks);
        Ok(())
    }
    pub fn repository_verified(
        &mut self,
        evidence: CheckEvidence,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::RepositoryChecks)?;
        self.checks(&evidence)?;
        self.state = State::Running(Stage::ReadyForMaintainer);
        Ok(())
    }
    /// Rechecks revocation/expiry/wall budget. Readiness grants no merge authority.
    pub fn readiness(
        &mut self,
        observed_head: &Commit,
        observed_base: &Commit,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        self.at(Stage::ReadyForMaintainer)?;
        if self.head.as_ref() != Some(observed_head)
            || self.plan.as_ref().map(|p| &p.base) != Some(observed_base)
        {
            return Err(Error::StaleBinding);
        }
        Ok(())
    }
    /// Reserve before invoking any provider. Unknown/failed calls retain the full
    /// ceiling; no refund can create a retry loop outside the accepted budget.
    pub fn reserve_call(
        &mut self,
        units: u64,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<CallTicket> {
        self.guard(now, authority)?;
        if self.active_call.is_some() {
            return Err(Error::CallInFlight);
        }
        if !matches!(
            self.state,
            State::Running(Stage::SpecReady | Stage::Implementing | Stage::IndependentReview)
        ) {
            return Err(Error::InvalidState);
        }
        require(units > 0)?;
        let limits = self.need.limits();
        let total = self.usage.reserved_units.checked_add(units);
        if self.usage.model_calls >= limits.model_calls
            || total.is_none_or(|v| v > limits.usage_units)
        {
            self.state = State::BudgetExhausted;
            return Err(Error::BudgetExhausted);
        }
        self.usage.model_calls += 1;
        self.usage.reserved_units = total.ok_or(Error::BudgetExhausted)?;
        let ticket = CallTicket {
            key: self.need.key().clone(),
            ordinal: self.usage.model_calls,
        };
        self.active_call = Some(ticket.clone());
        Ok(ticket)
    }
    /// Host acknowledgement of a completed call, not model-generated text.
    /// Completion never advances a verification/review gate automatically.
    pub fn finish_call(&mut self, ticket: &CallTicket, now: u64) -> Result<()> {
        self.clock(now)?;
        if self.active_call.as_ref() != Some(ticket) {
            return Err(Error::UnknownCall);
        }
        self.active_call = None;
        if self.state == State::CancelRequested {
            self.state = State::Cancelled;
        } else if now - self.started_ms >= self.need.limits().wall_ms {
            self.state = State::BudgetExhausted;
        }
        Ok(())
    }
    pub fn needs_input(
        &mut self,
        reason: String,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        if self.active_call.is_some() {
            return Err(Error::CallInFlight);
        }
        let State::Running(stage) = self.state else {
            return Err(Error::InvalidState);
        };
        if stage == Stage::ReadyForMaintainer {
            return Err(Error::InvalidState);
        }
        require(text(&reason, 4096))?;
        self.reason = Some(reason);
        self.state = State::NeedsInput(stage);
        Ok(())
    }
    /// Resumes the identical accepted task; changing scope requires a new accepted
    /// task revision. Waiting consumes wall time and preserves all counters.
    pub fn input_received(&mut self, now: u64, authority: &impl AuthorityResolver) -> Result<()> {
        self.guard(now, authority)?;
        let State::NeedsInput(stage) = self.state else {
            return Err(Error::InvalidState);
        };
        self.reason = None;
        self.state = State::Running(stage);
        Ok(())
    }
    pub fn repair(
        &mut self,
        reason: String,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        self.guard(now, authority)?;
        if self.active_call.is_some() {
            return Err(Error::CallInFlight);
        }
        if !matches!(
            self.state,
            State::Running(
                Stage::Implementing
                    | Stage::LocalVerification
                    | Stage::IndependentReview
                    | Stage::RepositoryChecks
            )
        ) {
            return Err(Error::InvalidState);
        }
        require(text(&reason, 4096))?;
        if self.usage.repairs >= self.need.limits().repairs {
            self.state = State::BudgetExhausted;
            return Err(Error::BudgetExhausted);
        }
        self.usage.repairs += 1;
        self.head = None;
        self.reason = Some(reason);
        self.state = State::Repairing;
        Ok(())
    }
    pub fn resume_repair(&mut self, now: u64, authority: &impl AuthorityResolver) -> Result<()> {
        self.guard(now, authority)?;
        if self.state != State::Repairing {
            return Err(Error::InvalidState);
        }
        self.state = State::Running(Stage::Implementing);
        Ok(())
    }
    /// Trusted controller cancellation. In-flight effects require acknowledgement;
    /// this state alone cannot prove a process stopped or undo earlier effects.
    pub fn cancel(&mut self, now: u64) -> Result<()> {
        self.clock(now)?;
        if self.state == State::BudgetExhausted {
            return Err(Error::BudgetExhausted);
        }
        self.state = if self.active_call.is_some() {
            State::CancelRequested
        } else {
            State::Cancelled
        };
        Ok(())
    }
}
