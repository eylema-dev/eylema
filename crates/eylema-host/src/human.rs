//! Local device-owner enrollment; OS proof and kernel data-use rights stay separate.
use crate::*;
use eylema_platform::human::{DeviceOwnerProof, HumanError, HumanPurpose};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
const MAX_REAL_HUMANS: usize = 16;
const MAX_REAL_TASKS: usize = 128;
fn forbidden() -> HostError {
    HostError::Runtime(ErrorCode::Forbidden)
}
fn validate_grant(g: &Grant, purpose: HumanPurpose) -> Result<()> {
    if g.resource_scope.resources().len() != 1
        || g.resource_scope
            .resources()
            .next()
            .is_none_or(|r| r.as_str() != purpose.resource())
        || !g.permissions.contains(&Permission::Read)
        || !g.permissions.contains(&Permission::RequestCancellation)
        || g.permissions.is_empty()
        || g.permissions.len() > 6
        || g.permissions.iter().any(|p| {
            !matches!(
                p,
                Permission::Read
                    | Permission::CreateTask
                    | Permission::ExecuteAction
                    | Permission::ObserveEvents
                    | Permission::RequestCancellation
                    | Permission::ApproveAction
            )
        })
        || g.permissions
            .iter()
            .enumerate()
            .any(|(i, p)| g.permissions[..i].contains(p))
        || (purpose == HumanPurpose::LocalCli && g.permissions.contains(&Permission::ApproveAction))
    {
        return Err(forbidden());
    }
    Ok(())
}
pub struct HumanEnrollment {
    identity: Arc<()>,
    grant: Grant,
    binding: [u8; 32],
    issued_at: Instant,
    expires: Instant,
    deadline_ms: u64,
}
impl std::fmt::Debug for HumanEnrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HumanEnrollment(<opaque>)")
    }
}
impl HumanEnrollment {
    fn validate(&self, identity: &Arc<()>, binding: [u8; 32], now: Instant) -> Result<()> {
        if !Arc::ptr_eq(identity, &self.identity)
            || self.binding != binding
            || now >= self.expires
            || now
                .checked_duration_since(self.issued_at)
                .is_none_or(|age| age >= Duration::from_secs(120))
        {
            return Err(forbidden());
        }
        Ok(())
    }
    pub(crate) fn purpose(&self) -> HumanPurpose {
        if self
            .grant
            .resource_scope
            .resources()
            .next()
            .is_some_and(|r| r.as_str() == HumanPurpose::LocalCli.resource())
        {
            HumanPurpose::LocalCli
        } else if self
            .grant
            .resource_scope
            .resources()
            .next()
            .is_some_and(|r| r.as_str() == HumanPurpose::LocalReport.resource())
        {
            HumanPurpose::LocalReport
        } else {
            HumanPurpose::AcceptanceProof
        }
    }
    /// Explicit OS UI operation; no authentication retry and no caller-provided attestation.
    pub fn authenticate(
        &self,
        timeout: Duration,
    ) -> std::result::Result<DeviceOwnerProof, HumanError> {
        if self
            .validate(&self.identity, self.binding, Instant::now())
            .is_err()
        {
            return Err(HumanError::Expired);
        }
        let purpose = self.purpose();
        eylema_platform::human::authenticate_device_owner(self.binding, purpose, timeout)
    }
}
pub(crate) struct Lease {
    slot: usize,
    scope: ResourceScope,
    expires: Instant,
    deadline_ms: u64,
    revoked: AtomicBool,
}
impl Lease {
    fn valid_at(&self, now: Instant) -> bool {
        !self.revoked.load(Ordering::Acquire) && now < self.expires
    }
}
pub struct DeviceOwnerHuman {
    human: DurableHuman,
    lease: Arc<Lease>,
}
impl std::fmt::Debug for DeviceOwnerHuman {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceOwnerHuman(<opaque>)")
    }
}
impl DeviceOwnerHuman {
    pub fn session(&self) -> &DurableSession {
        self.human.session()
    }
    pub fn expires(&self) -> Instant {
        self.lease.expires
    }
    pub fn remaining(&self) -> Duration {
        if self.lease.revoked.load(Ordering::Acquire) {
            Duration::ZERO
        } else {
            self.lease.expires.saturating_duration_since(Instant::now())
        }
    }
    pub fn deadline(&self) -> Timestamp {
        Timestamp::new(self.lease.deadline_ms).expect("validated deadline")
    }
}
impl<T: Storage, H: Sha256, S: HostServices> DurableHost<T, H, S> {
    pub fn request_device_owner(
        &self,
        owner: &Owner,
        grant: Grant,
        purpose: HumanPurpose,
        ttl: Duration,
    ) -> Result<HumanEnrollment> {
        if purpose == HumanPurpose::LocalReport {
            return Err(forbidden());
        }
        self.request_device_owner_bound(owner, grant, purpose, ttl, None)
    }
    pub(crate) fn request_device_owner_bound(
        &self,
        owner: &Owner,
        grant: Grant,
        purpose: HumanPurpose,
        ttl: Duration,
        report: Option<(&TaskId, [u8; 32])>,
    ) -> Result<HumanEnrollment> {
        if (purpose == HumanPurpose::LocalReport) != report.is_some() {
            return Err(forbidden());
        }
        self.owner(owner)?;
        validate_grant(&grant, purpose)?;
        if ttl < Duration::from_secs(1) || ttl > Duration::from_secs(300) {
            return Err(HostError::Runtime(ErrorCode::InvalidRequest));
        }
        if self.real_humans.len() >= MAX_REAL_HUMANS {
            return Err(HostError::Runtime(ErrorCode::BudgetExceeded));
        }
        let issued_at = Instant::now();
        let expires = issued_at.checked_add(ttl).ok_or(forbidden())?;
        let ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| forbidden())?
                .as_millis(),
        )
        .map_err(|_| forbidden())?;
        let deadline_ms = ms
            .checked_add(u64::try_from(ttl.as_millis()).map_err(|_| forbidden())?)
            .ok_or(forbidden())?;
        Timestamp::new(deadline_ms).map_err(|_| forbidden())?;
        let nonce = eylema_platform::random_id(eylema_platform::IdKind::Request)
            .map_err(|_| HostError::Runtime(ErrorCode::CapabilityUnavailable))?;
        let mut fields = vec![
            Value::Text("owned:device-owner-enrollment:v1".into()),
            encoding::grant(&grant),
            Value::Text(nonce),
            Value::Unsigned(deadline_ms),
        ];
        if let Some((task, digest)) = report {
            if digest == [0; 32] {
                return Err(forbidden());
            }
            fields.push(Value::Text(task.as_str().to_owned()));
            fields.push(Value::Bytes(digest.to_vec()));
        }
        let bytes = codec::encode(&Value::List(fields)).map_err(|_| HostError::InvalidRecord)?;
        let binding = eylema_platform::sha256(&bytes)
            .map_err(|_| HostError::Runtime(ErrorCode::CapabilityUnavailable))?;
        Ok(HumanEnrollment {
            identity: self.identity.clone(),
            grant,
            binding,
            issued_at,
            expires,
            deadline_ms,
        })
    }
    pub fn enroll_device_owner(
        &mut self,
        owner: &Owner,
        request: HumanEnrollment,
        proof: DeviceOwnerProof,
    ) -> Result<DeviceOwnerHuman> {
        if request.purpose() == HumanPurpose::LocalReport {
            return Err(forbidden());
        }
        self.enroll_device_owner_bound(owner, request, proof)
    }
    pub(crate) fn enroll_device_owner_bound(
        &mut self,
        owner: &Owner,
        request: HumanEnrollment,
        proof: DeviceOwnerProof,
    ) -> Result<DeviceOwnerHuman> {
        self.owner(owner)?;
        let challenge = proof
            .consume_binding_for(request.purpose(), Duration::from_secs(15))
            .map_err(|_| forbidden())?;
        self.finish_device_owner(owner, request, challenge)
    }
    // Private completion path; synthetic unit fixtures call this directly and cannot export proof.
    fn finish_device_owner(
        &mut self,
        owner: &Owner,
        request: HumanEnrollment,
        challenge: [u8; 32],
    ) -> Result<DeviceOwnerHuman> {
        self.owner(owner)?;
        request.validate(&self.identity, challenge, Instant::now())?;
        if self.real_humans.len() >= MAX_REAL_HUMANS {
            return Err(HostError::Runtime(ErrorCode::BudgetExceeded));
        }
        let human = self.enroll_verified_human(owner, request.grant.clone())?;
        let lease = Arc::new(Lease {
            slot: human.0.slot,
            scope: request.grant.resource_scope,
            expires: request.expires,
            deadline_ms: request.deadline_ms,
            revoked: AtomicBool::new(false),
        });
        self.real_humans.push(lease.clone());
        Ok(DeviceOwnerHuman { human, lease })
    }
    pub fn device_owner_approval(
        &mut self,
        human: &DeviceOwnerHuman,
        task: &TaskSpec,
        proposal: &ActionProposal,
        binding: ApprovalBinding,
    ) -> Result<EventId> {
        self.session(human.session())?;
        if binding.expires_at.unix_ms() > human.lease.deadline_ms {
            return Err(forbidden());
        }
        self.create_approval(&human.human, task, proposal, binding)
    }
    pub fn device_owner_cancel(
        &mut self,
        human: &DeviceOwnerHuman,
        id: &ExecutionId,
    ) -> Result<CancellationOutcome> {
        self.session(human.session())?;
        self.request_cancellation(&human.human, id)
    }
    pub fn revoke_device_owner(&mut self, human: &DeviceOwnerHuman) -> Result<()> {
        // Instance binding is required even when the lease has already expired; idempotent.
        if !Arc::ptr_eq(&self.identity, &human.human.0.identity)
            || !self
                .real_humans
                .iter()
                .any(|l| Arc::ptr_eq(l, &human.lease))
        {
            return Err(forbidden());
        }
        self.healthy()?;
        human.lease.revoked.store(true, Ordering::Release);
        Ok(())
    }
    pub(crate) fn real_session(&self, slot: usize) -> Result<()> {
        if self
            .real_humans
            .iter()
            .any(|h| h.slot == slot && !h.valid_at(Instant::now()))
        {
            return Err(forbidden());
        }
        Ok(())
    }
    pub(crate) fn real_command(&self, command: &Command) -> Result<Option<(TaskId, Arc<Lease>)>> {
        match command {
            Command::Create(_, task, _) | Command::Admit(_, task, _, _) => {
                let lease = self
                    .human_tasks
                    .get(&task.fields().task_id)
                    .cloned()
                    .or_else(|| {
                        self.real_humans
                            .iter()
                            .rev()
                            .find(|h| {
                                h.valid_at(Instant::now())
                                    && task.fields().resource_scope.is_subset_of(&h.scope)
                            })
                            .cloned()
                    });
                let Some(lease) = lease else {
                    if self.real_humans.iter().any(|h| {
                        h.scope
                            .resources()
                            .any(|r| task.fields().resource_scope.resources().any(|s| r == s))
                    }) {
                        return Err(forbidden());
                    }
                    return Ok(None);
                };
                if !lease.valid_at(Instant::now())
                    || task.fields().budget.deadline().unix_ms() > lease.deadline_ms
                {
                    return Err(forbidden());
                }
                if !self.human_tasks.contains_key(&task.fields().task_id)
                    && self.human_tasks.len() >= MAX_REAL_TASKS
                {
                    return Err(HostError::Runtime(ErrorCode::BudgetExceeded));
                }
                Ok(Some((task.fields().task_id.clone(), lease)))
            }
            Command::Step => {
                let Some(id) = self
                    .state
                    .runtime
                    .pending_execution(&self.state.owner)
                    .map_err(HostError::Runtime)?
                else {
                    return Ok(None);
                };
                let (task, _) = self
                    .state
                    .bindings
                    .get(&id)
                    .ok_or(HostError::ReplayMismatch)?;
                let Some(lease) = self.human_tasks.get(&task.fields().task_id) else {
                    return Ok(None);
                };
                if lease.valid_at(Instant::now()) {
                    return Ok(None);
                }
                let e = self
                    .state
                    .runtime
                    .execution(&self.state.slots[lease.slot].session, &id)
                    .map_err(HostError::Runtime)?;
                if matches!(
                    e.fields().state,
                    ExecutionState::Accepted | ExecutionState::Queued | ExecutionState::Running
                ) {
                    return Err(HostError::HumanPaused);
                }
                // Check the actual FIFO head only. A prior valid cancellation at the
                // head must become effective even if an expired later task remains queued.
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}
#[cfg(test)]
#[path = "../../eylema-runtime/tests/support/mod.rs"]
mod test_support;
#[cfg(test)]
pub(crate) fn simulate_report_person<T: Storage, H: Sha256, S: HostServices>(
    host: &mut DurableHost<T, H, S>,
    owner: &Owner,
    grant: Grant,
    task: &TaskId,
    need: [u8; 32],
) -> DeviceOwnerHuman {
    let request = host
        .request_device_owner_bound(
            owner,
            grant,
            HumanPurpose::LocalReport,
            Duration::from_secs(300),
            Some((task, need)),
        )
        .unwrap();
    let binding = request.binding;
    host.finish_device_owner(owner, request, binding).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_request_rejects_expansion_and_expired_or_foreign_challenge() {
        let owner = Arc::new(());
        let grant = Grant {
            principal: PrincipalRef::new("principal:test-device-owner").unwrap(),
            resource_scope: ResourceScope::new(vec![ResourceId::new("state:cli").unwrap()])
                .unwrap(),
            permissions: vec![Permission::Read, Permission::RequestCancellation],
            policy_revision: Revision::new(1).unwrap(),
        };
        assert!(validate_grant(&grant, HumanPurpose::LocalCli).is_ok());
        let mut expanded = grant.clone();
        expanded.permissions.push(Permission::GovernPolicy);
        assert!(validate_grant(&expanded, HumanPurpose::LocalCli).is_err());
        expanded = grant.clone();
        expanded.resource_scope =
            ResourceScope::new(vec![ResourceId::new("state:foreign").unwrap()]).unwrap();
        assert!(validate_grant(&expanded, HumanPurpose::LocalCli).is_err());
        let now = Instant::now();
        let req = HumanEnrollment {
            identity: owner.clone(),
            grant,
            binding: [3; 32],
            issued_at: now,
            expires: now + Duration::from_secs(2),
            deadline_ms: 2000,
        };
        assert!(req.validate(&owner, [3; 32], now).is_ok());
        assert!(req.validate(&Arc::new(()), [3; 32], now).is_err());
        assert!(req.validate(&owner, [9; 32], now).is_err());
        assert!(
            req.validate(&owner, [3; 32], now + Duration::from_secs(2))
                .is_err()
        );
    }
    use super::test_support as support;
    use std::cell::{Cell, RefCell};
    use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
    use std::rc::Rc;
    #[derive(Clone)]
    struct Memory(Rc<RefCell<Cursor<Vec<u8>>>>);
    impl Read for Memory {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.0.borrow_mut().read(b)
        }
    }
    impl Write for Memory {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Seek for Memory {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            self.0.borrow_mut().seek(p)
        }
    }
    impl Storage for Memory {
        fn byte_len(&mut self) -> io::Result<u64> {
            Ok(self.0.borrow().get_ref().len() as u64)
        }
        fn synchronize(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    type Host = DurableHost<Memory, crate::native::NativeHash, support::Services>;
    fn fixture() -> (Host, Owner, Memory) {
        let memory = Memory(Rc::new(RefCell::new(Cursor::new(vec![]))));
        let journal = Journal::create_new(
            memory.clone(),
            crate::native::NativeHash,
            eylema_journal::Limits::default(),
        )
        .unwrap();
        let service = support::Services {
            clock: Rc::new(Cell::new(1000)),
            next: 0,
            duplicate_event: false,
            fail_clock: Rc::new(Cell::new(false)),
        };
        let (h, o) = Host::create_new(journal, service, eylema_runtime::Limits::default()).unwrap();
        (h, o, memory)
    }
    fn grant() -> Grant {
        let mut g = support::grant();
        g.resource_scope =
            ResourceScope::new(vec![ResourceId::new("state:g73-proof").unwrap()]).unwrap();
        g.permissions
            .extend([Permission::RequestCancellation, Permission::ApproveAction]);
        g
    }
    fn simulated(h: &mut Host, o: &Owner, ttl: Duration) -> DeviceOwnerHuman {
        // Explicit unit-only simulation of a successful OS challenge, not person acceptance.
        let request = h
            .request_device_owner(o, grant(), HumanPurpose::AcceptanceProof, ttl)
            .unwrap();
        let binding = request.binding;
        h.finish_device_owner(o, request, binding).unwrap()
    }
    fn task_plan(name: &str, deltas: &[u64]) -> (TaskSpec, ActionProposal) {
        let old = support::task(name, 10, 40);
        let p = support::proposal(&old, deltas);
        let mut t = old.fields().clone();
        t.resource_scope = grant().resource_scope;
        let task = TaskSpec::new(t).unwrap();
        let mut draft = p.fields().clone();
        draft.actions = draft
            .actions
            .into_iter()
            .map(|a| {
                let mut f = a.fields().clone();
                f.resource_scope = grant().resource_scope;
                f.authority = AuthorityRequirement::new(
                    grant().principal,
                    grant().policy_revision,
                    grant().resource_scope,
                    vec![Permission::ExecuteAction],
                    f.authority.oversight().clone(),
                )
                .unwrap();
                ActionSpec::new(f).unwrap()
            })
            .collect();
        let proposal = ActionProposal::new(draft, &task).unwrap();
        (task, proposal)
    }
    #[test]
    fn simulated_lease_revocation_pauses_effects_and_fresh_person_can_stop_retained_work() {
        let (mut h, o, _) = fixture();
        let human = simulated(&mut h, &o, Duration::from_secs(300));
        let s = h.enroll(&o, grant()).unwrap();
        let (t, p) = task_plan("task:human-revoke", &[2, 3]);
        h.create_task(&s, t.clone()).unwrap();
        let e = h.admit(&s, t.clone(), p).unwrap();
        h.step().unwrap();
        h.revoke_device_owner(&human).unwrap();
        assert_eq!(h.step(), Err(HostError::HumanPaused));
        assert_eq!(h.task(&s, &t.fields().task_id).unwrap().value, 2);
        assert!(h.device_owner_cancel(&human, &e).is_err());
        let fresh = simulated(&mut h, &o, Duration::from_secs(300));
        h.device_owner_cancel(&fresh, &e).unwrap();
        h.step().unwrap();
        assert_eq!(
            h.execution(&s, &e).unwrap().fields().state,
            ExecutionState::Cancelled
        );
        assert_eq!(h.task(&s, &t.fields().task_id).unwrap().value, 2);
    }
    #[test]
    fn simulated_expiry_denies_new_human_action_but_received_stop_remains_effective() {
        let (mut h, o, _) = fixture();
        let human = simulated(&mut h, &o, Duration::from_secs(1));
        let s = h.enroll(&o, grant()).unwrap();
        let (t, p) = task_plan("task:human-expiry", &[2, 3]);
        h.create_task(&s, t.clone()).unwrap();
        let e = h.admit(&s, t.clone(), p).unwrap();
        h.step().unwrap();
        h.device_owner_cancel(&human, &e).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        assert!(h.device_owner_cancel(&human, &e).is_err());
        h.step().unwrap();
        assert_eq!(
            h.execution(&s, &e).unwrap().fields().state,
            ExecutionState::Cancelled
        );
        assert!(
            h.execution(&s, &e)
                .unwrap()
                .fields()
                .effective_stop_evidence
                .is_some()
        );
        assert_eq!(h.task(&s, &t.fields().task_id).unwrap().value, 2);
    }
    #[test]
    fn simulated_approval_checks_exact_binding_and_lease_bound_and_revocation() {
        let (mut h, o, _) = fixture();
        let human = simulated(&mut h, &o, Duration::from_secs(300));
        let s = h.enroll(&o, grant()).unwrap();
        let (t, p) = task_plan("task:human-approval", &[2, 3]);
        let overs = OversightRequirements::new(
            vec![CheckpointRef::new("checkpoint:person").unwrap()],
            true,
            true,
            None,
        )
        .unwrap();
        h.create_governed_task(&o, &s, t.clone(), overs).unwrap();
        assert_eq!(
            h.admit(&s, t.clone(), p.clone()),
            Err(HostError::Runtime(ErrorCode::ApprovalRequired))
        );
        let b = h
            .preview_approval(&s, &t, &p, Timestamp::new(1800).unwrap())
            .unwrap();
        let mut wrong = b.clone();
        wrong.payload_sha256 = [9; 32];
        assert!(h.device_owner_approval(&human, &t, &p, wrong).is_err());
        let mut long = b.clone();
        long.expires_at = Timestamp::new(human.deadline().unix_ms() + 1).unwrap();
        assert!(h.device_owner_approval(&human, &t, &p, long).is_err());
        let a = h.device_owner_approval(&human, &t, &p, b).unwrap();
        let e = h.admit_with_approvals(&s, t.clone(), p, &[a]).unwrap();
        h.step().unwrap();
        h.revoke_device_owner(&human).unwrap();
        assert_eq!(h.step(), Err(HostError::HumanPaused));
        assert_eq!(
            h.execution(&s, &e).unwrap().fields().state,
            ExecutionState::Running
        );
    }
    #[test]
    fn simulated_foreign_and_reopened_instance_cannot_reuse_human_handle() {
        let (mut h, o, memory) = fixture();
        let human = simulated(&mut h, &o, Duration::from_secs(300));
        let (mut other, other_owner, _) = fixture();
        assert!(other.revoke_device_owner(&human).is_err());
        let req = h
            .request_device_owner(
                &o,
                grant(),
                HumanPurpose::AcceptanceProof,
                Duration::from_secs(300),
            )
            .unwrap();
        let binding = req.binding;
        assert!(
            other
                .finish_device_owner(&other_owner, req, binding)
                .is_err()
        );
        h.close();
        let journal = Journal::open(
            memory,
            crate::native::NativeHash,
            eylema_journal::Limits::default(),
        )
        .unwrap();
        let service = support::Services {
            clock: Rc::new(Cell::new(1000)),
            next: 100,
            duplicate_event: false,
            fail_clock: Rc::new(Cell::new(false)),
        };
        let (mut reopened, _) = Host::open(journal, service).unwrap();
        assert!(reopened.revoke_device_owner(&human).is_err());
        assert!(
            reopened
                .task(human.session(), &TaskId::new("task:none").unwrap())
                .is_err()
        );
    }
    #[test]
    fn simulated_request_window_deadline_and_retained_task_binding_fail_closed() {
        let (mut h, o, _) = fixture();
        assert!(
            h.request_device_owner(&o, grant(), HumanPurpose::AcceptanceProof, Duration::ZERO)
                .is_err()
        );
        let mut req = h
            .request_device_owner(
                &o,
                grant(),
                HumanPurpose::AcceptanceProof,
                Duration::from_secs(300),
            )
            .unwrap();
        req.issued_at = Instant::now() - Duration::from_secs(121);
        let b = req.binding;
        assert!(h.finish_device_owner(&o, req, b).is_err());
        let human = simulated(&mut h, &o, Duration::from_secs(300));
        let s = h.enroll(&o, grant()).unwrap();
        let (t, _) = task_plan("task:human-deadline", &[1]);
        let mut d = t.fields().clone();
        let old = d.budget;
        d.budget = Budget::new(
            old.max_steps(),
            old.max_events(),
            old.max_payload_bytes(),
            1,
            Timestamp::new(human.deadline().unix_ms() + 1).unwrap(),
        )
        .unwrap();
        let long = TaskSpec::new(d).unwrap();
        assert!(h.create_task(&s, long).is_err());
        h.create_task(&s, t.clone()).unwrap();
        h.revoke_device_owner(&human).unwrap();
        let _fresh = simulated(&mut h, &o, Duration::from_secs(300));
        let (_, p) = task_plan("task:human-deadline", &[1]);
        assert!(h.admit(&s, t, p).is_err());
    }
    #[test]
    fn earlier_received_stop_progresses_before_later_revoked_queued_task() {
        let (mut h, o, _) = fixture();
        let person = simulated(&mut h, &o, Duration::from_secs(300));
        let actor = h.enroll(&o, grant()).unwrap();
        let (a, ap) = task_plan("task:stop-head", &[2, 3]);
        let (b, bp) = task_plan("task:pause-tail", &[7]);
        h.create_task(&actor, a.clone()).unwrap();
        h.create_task(&actor, b.clone()).unwrap();
        let ae = h.admit(&actor, a.clone(), ap).unwrap();
        let be = h.admit(&actor, b.clone(), bp).unwrap();
        h.step().unwrap();
        h.device_owner_cancel(&person, &ae).unwrap();
        h.revoke_device_owner(&person).unwrap();
        assert!(
            h.step().is_ok(),
            "received stop must become effective despite an expired tail"
        );
        assert_eq!(
            h.execution(&actor, &ae).unwrap().fields().state,
            ExecutionState::Cancelled
        );
        assert!(
            h.execution(&actor, &ae)
                .unwrap()
                .fields()
                .effective_stop_evidence
                .is_some()
        );
        assert_eq!(h.task(&actor, &a.fields().task_id).unwrap().value, 2);
        assert!(h.step().is_err());
        assert_eq!(
            h.execution(&actor, &be).unwrap().fields().state,
            ExecutionState::Queued
        );
        assert_eq!(h.task(&actor, &b.fields().task_id).unwrap().value, 0);
    }
    #[test]
    fn report_purpose_cannot_use_generic_enrollment() {
        let (h, o, _) = fixture();
        let mut g = grant();
        g.resource_scope =
            ResourceScope::new(vec![ResourceId::new("state:report").unwrap()]).unwrap();
        assert!(
            h.request_device_owner(&o, g, HumanPurpose::LocalReport, Duration::from_secs(300))
                .is_err()
        );
        assert_eq!(HumanPurpose::LocalReport.resource(), "state:report");
    }

    #[test]
    fn foreign_controller_and_worker_text_cannot_approve() {
        use crate::tool::ToolHost;
        use eylema_contracts::{
            platform::SemanticProfileId,
            tool::{OwnedTool, ToolProfile},
        };
        use eylema_runtime::tool::ToolRegistry;
        let (h, o, _) = fixture();
        let registry = ToolRegistry::new(vec![ToolProfile::new(
            SemanticProfileId::new("tool:add").unwrap(),
            Revision::new(1).unwrap(),
            OwnedTool::AddTaskValue,
        )])
        .unwrap();
        let mut g = grant();
        g.resource_scope =
            ResourceScope::new(vec![ResourceId::new("state:report").unwrap()]).unwrap();
        let task = TaskId::new("task:report-a").unwrap();
        let mut tools = ToolHost::new(h, registry).unwrap();
        assert!(
            tools
                .request_report_controller(
                    &o,
                    g.clone(),
                    task.clone(),
                    [7; 32],
                    Duration::from_secs(300)
                )
                .is_ok()
        );
        let controller = tools.simulate_report_controller(&o, g.clone(), &task, [7; 32]);
        assert!(
            tools
                .check_report_controller(&controller, &task, [7; 32])
                .is_ok()
        );
        assert!(
            tools
                .check_report_controller(&controller, &TaskId::new("task:other").unwrap(), [7; 32])
                .is_err()
        );
        assert!(
            tools
                .check_report_controller(&controller, &task, [8; 32])
                .is_err()
        );
        let (h2, _, _) = fixture();
        let r2 = ToolRegistry::new(vec![ToolProfile::new(
            SemanticProfileId::new("tool:add").unwrap(),
            Revision::new(1).unwrap(),
            OwnedTool::AddTaskValue,
        )])
        .unwrap();
        let other = ToolHost::new(h2, r2).unwrap();
        assert!(
            other
                .check_report_controller(&controller, &task, [7; 32])
                .is_err()
        );
        let actor = tools.enroll(&o, g.clone()).unwrap();
        let (foreign_task, _) = task_plan("task:report-a", &[1]);
        assert!(
            tools
                .create_report_task(&o, &actor, &controller, [7; 32], foreign_task)
                .is_err()
        );
        tools.revoke_report_controller(&controller).unwrap();
        assert!(
            tools
                .check_report_controller(&controller, &task, [7; 32])
                .is_err()
        );
    }
}
