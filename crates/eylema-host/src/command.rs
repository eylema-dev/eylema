use crate::*;
#[derive(Clone)]
pub(crate) enum Command {
    Enroll(Grant, bool),
    Create(usize, TaskSpec, Option<OversightRequirements>),
    Admit(usize, TaskSpec, ActionProposal, Vec<EventId>),
    Step,
    Approve(usize, TaskSpec, ActionProposal, ApprovalBinding),
    RevokeApproval(usize, EventId),
    Governance(usize, TaskId, GovernanceRef),
    Policy(usize, TaskId, Revision),
    RevokeDelegation(usize, TaskId, usize),
    Cancel(usize, ExecutionId),
    Recover(bool),
}
impl Command {
    pub(crate) fn encode(&self) -> Result<Value> {
        Ok(match self {
            Self::Enroll(g, h) => list(vec![num(0), grant(g), Value::Bool(*h)]),
            Self::Create(s, t, o) => list(vec![
                num(1),
                num(*s),
                wire(&codec::WireRecord::Task(t.clone()))?,
                o.as_ref().map_or(Value::Null, oversight),
            ]),
            Self::Admit(s, t, p, a) => list(vec![
                num(2),
                num(*s),
                wire(&codec::WireRecord::Proposal {
                    task: t.clone(),
                    proposal: p.clone(),
                })?,
                list(a.iter().map(|a| text(a.as_str())).collect()),
            ]),
            Self::Step => list(vec![num(3)]),
            Self::Approve(s, t, p, b) => list(vec![
                num(4),
                num(*s),
                wire(&codec::WireRecord::Proposal {
                    task: t.clone(),
                    proposal: p.clone(),
                })?,
                binding(b),
            ]),
            Self::RevokeApproval(s, a) => list(vec![num(5), num(*s), text(a.as_str())]),
            Self::Governance(s, t, g) => {
                list(vec![num(6), num(*s), text(t.as_str()), text(g.as_str())])
            }
            Self::Policy(s, t, r) => list(vec![
                num(7),
                num(*s),
                text(t.as_str()),
                Value::Unsigned(r.get()),
            ]),
            Self::RevokeDelegation(s, t, d) => {
                list(vec![num(8), num(*s), text(t.as_str()), num(*d)])
            }
            Self::Cancel(s, e) => list(vec![num(9), num(*s), text(e.as_str())]),
            Self::Recover(p) => list(vec![num(10), Value::Bool(*p)]),
        })
    }
    pub(crate) fn decode(value: &Value) -> Result<Self> {
        let v = values(value, 4)?;
        let tag = index(v.first().ok_or(HostError::InvalidRecord)?)?;
        Ok(match tag {
            0 => {
                let [_, g, h] = fields(value)?;
                Self::Enroll(decode_grant(g)?, boolean(h)?)
            }
            1 => {
                let [_, s, t, o] = fields(value)?;
                let codec::WireRecord::Task(t) = record(t)? else {
                    return Err(HostError::InvalidRecord);
                };
                Self::Create(
                    index(s)?,
                    t,
                    if *o == Value::Null {
                        None
                    } else {
                        Some(decode_oversight(o)?)
                    },
                )
            }
            2 | 4 => {
                let [_, s, p, extra] = fields(value)?;
                let codec::WireRecord::Proposal { task, proposal } = record(p)? else {
                    return Err(HostError::InvalidRecord);
                };
                if tag == 2 {
                    Self::Admit(
                        index(s)?,
                        task,
                        proposal,
                        values(extra, MAX_REFERENCES)?
                            .iter()
                            .map(|v| typed(EventId::new(string(v)?)))
                            .collect::<Result<_>>()?,
                    )
                } else {
                    Self::Approve(index(s)?, task, proposal, decode_binding(extra)?)
                }
            }
            3 => {
                let [_] = fields(value)?;
                Self::Step
            }
            5 => {
                let [_, s, a] = fields(value)?;
                Self::RevokeApproval(index(s)?, typed(EventId::new(string(a)?))?)
            }
            6 => {
                let [_, s, t, g] = fields(value)?;
                Self::Governance(
                    index(s)?,
                    typed(TaskId::new(string(t)?))?,
                    typed(GovernanceRef::new(string(g)?))?,
                )
            }
            7 => {
                let [_, s, t, r] = fields(value)?;
                Self::Policy(
                    index(s)?,
                    typed(TaskId::new(string(t)?))?,
                    typed(Revision::new(unsigned(r)?))?,
                )
            }
            8 => {
                let [_, s, t, d] = fields(value)?;
                Self::RevokeDelegation(index(s)?, typed(TaskId::new(string(t)?))?, index(d)?)
            }
            9 => {
                let [_, s, e] = fields(value)?;
                Self::Cancel(index(s)?, typed(ExecutionId::new(string(e)?))?)
            }
            10 => {
                let [_, p] = fields(value)?;
                Self::Recover(boolean(p)?)
            }
            _ => return Err(HostError::InvalidRecord),
        })
    }
}
pub(crate) struct Enrollment {
    pub(crate) session: Session,
    pub(crate) human: Option<HumanSession>,
}
pub(crate) struct State<S: HostServices> {
    pub(crate) runtime: Runtime<services::Services<S>>,
    pub(crate) owner: HostKey,
    pub(crate) slots: Vec<Enrollment>,
    bindings: std::collections::BTreeMap<ExecutionId, (TaskSpec, ActionProposal)>,
    pub(crate) last_approval: Option<ApprovalRecord>,
    pub(crate) last_policy: Option<PolicyChange>,
}
impl<S: HostServices> State<S> {
    pub(crate) fn execution_record(
        &self,
        id: &ExecutionId,
        execution: ExecutionRecord,
    ) -> Result<codec::WireRecord> {
        let (task, proposal) = self.bindings.get(id).ok_or(HostError::ReplayMismatch)?;
        Ok(codec::WireRecord::Execution {
            task: task.clone(),
            proposal: proposal.clone(),
            execution: Box::new(execution),
        })
    }
    pub(crate) fn new(
        services: services::Services<S>,
        limits: eylema_runtime::Limits,
    ) -> Result<Self> {
        let (runtime, owner) = Runtime::new(services, limits).map_err(HostError::Runtime)?;
        Ok(Self {
            runtime,
            owner,
            slots: vec![],
            bindings: Default::default(),
            last_approval: None,
            last_policy: None,
        })
    }
    fn session(&self, n: usize) -> eylema_runtime::Result<Session> {
        self.slots
            .get(n)
            .map(|s| s.session.clone())
            .ok_or(ErrorCode::Forbidden)
    }
    fn human(&self, n: usize) -> eylema_runtime::Result<HumanSession> {
        self.slots
            .get(n)
            .and_then(|s| s.human.clone())
            .ok_or(ErrorCode::Forbidden)
    }
    pub(crate) fn execute(&mut self, c: &Command) -> eylema_runtime::Result<Value> {
        self.last_approval = None;
        self.last_policy = None;
        Ok(match c {
            Command::Enroll(g, h) => {
                let human = if *h {
                    Some(self.runtime.enroll_verified_human(&self.owner, g.clone())?)
                } else {
                    None
                };
                let session = if let Some(h) = &human {
                    h.session().clone()
                } else {
                    self.runtime.enroll(&self.owner, g.clone())?
                };
                let n = self.slots.len();
                self.slots.push(Enrollment { session, human });
                num(n)
            }
            Command::Create(s, t, o) => {
                let s = self.session(*s)?;
                match o {
                    Some(o) => {
                        self.runtime
                            .create_governed_task(&self.owner, &s, t.clone(), o.clone())?
                    }
                    None => self.runtime.create_task(&s, t.clone())?,
                };
                Value::Null
            }
            Command::Admit(s, t, p, a) => {
                let s = self.session(*s)?;
                let id = self
                    .runtime
                    .admit_with_approvals(&s, t.clone(), p.clone(), a)?;
                self.bindings
                    .entry(id.clone())
                    .or_insert((t.clone(), p.clone()));
                text(id.as_str())
            }
            Command::Step => self
                .runtime
                .step()?
                .map_or(Value::Null, |id| text(id.as_str())),
            Command::Approve(s, t, p, b) => {
                let h = self.human(*s)?;
                text(self.runtime.create_approval(&h, t, p, b.clone())?.as_str())
            }
            Command::RevokeApproval(s, id) => {
                let h = self.human(*s)?;
                let result = self.runtime.revoke_approval(&h, id)?;
                let value = approval(&result);
                self.last_approval = Some(result);
                value
            }
            Command::Governance(s, t, g) => {
                let h = self.human(*s)?;
                let result = self.runtime.register_governance(&h, t, g.clone())?;
                let value = policy(&result);
                self.last_policy = Some(result);
                value
            }
            Command::Policy(s, t, r) => {
                let h = self.human(*s)?;
                let result = self.runtime.advance_policy(&h, t, *r)?;
                let value = policy(&result);
                self.last_policy = Some(result);
                value
            }
            Command::RevokeDelegation(s, t, d) => {
                let h = self.human(*s)?;
                let d = self.session(*d)?;
                revocation(&self.runtime.revoke_delegation(&h, t, &d)?)
            }
            Command::Cancel(s, id) => {
                let h = self.human(*s)?;
                match self.runtime.request_cancellation(&h, id)? {
                    CancellationOutcome::Requested(r) => list(vec![text("requested"), receipt(&r)]),
                    CancellationOutcome::AlreadyTerminal(e) => {
                        let (t, p) = self.bindings.get(id).ok_or(ErrorCode::OutcomeUnknown)?;
                        list(vec![
                            text("terminal"),
                            wire(&codec::WireRecord::Execution {
                                task: t.clone(),
                                proposal: p.clone(),
                                execution: e,
                            })
                            .map_err(|_| ErrorCode::OutcomeUnknown)?,
                        ])
                    }
                }
            }
            Command::Recover(p) => list(
                self.runtime
                    .recover_unfinished(&self.owner, *p)?
                    .iter()
                    .map(|id| text(id.as_str()))
                    .collect(),
            ),
        })
    }
    pub(crate) fn digest(&mut self) -> Result<[u8; 32]> {
        self.runtime
            .recovery_digest(&self.owner)
            .map_err(HostError::Runtime)
    }
}
