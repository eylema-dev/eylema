//! Owned development fixtures, not production authentication or time evidence.
#![allow(dead_code)]
use eylema_contracts::{codec, *};
use eylema_runtime::*;
use std::cell::Cell;
use std::rc::Rc;

pub fn context() -> RequestContext {
    RequestContext {
        request_id: RequestId::new("request:1").unwrap(),
        correlation_id: CorrelationId::new("trace:1").unwrap(),
        contract_version: ContractVersion::CURRENT,
    }
}
pub fn rev(n: u64) -> Revision {
    Revision::new(n).unwrap()
}
pub fn scope() -> ResourceScope {
    ResourceScope::new(vec![ResourceId::new("state:1").unwrap()]).unwrap()
}
pub fn grant() -> Grant {
    Grant {
        principal: PrincipalRef::new("principal:local").unwrap(),
        resource_scope: scope(),
        permissions: vec![
            Permission::CreateTask,
            Permission::Read,
            Permission::ExecuteAction,
            Permission::ObserveEvents,
        ],
        policy_revision: rev(3),
    }
}
pub fn task(name: &str, steps: u64, events: u64) -> TaskSpec {
    TaskSpec::new(TaskDraft {
        context: context(),
        task_id: TaskId::new(name).unwrap(),
        revision: rev(2),
        objective: "Advance bounded owned task state".into(),
        resource_scope: scope(),
        budget: Budget::new(steps, events, 1024, 1, Timestamp::new(2000).unwrap()).unwrap(),
        target: ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeCpu,
            GpuApi::None,
        )
        .unwrap(),
    })
    .unwrap()
}
pub fn proposal(task: &TaskSpec, deltas: &[u64]) -> ActionProposal {
    let actions = deltas
        .iter()
        .enumerate()
        .map(|(i, n)| {
            ActionSpec::new(ActionDraft {
                context: context(),
                action_id: ActionId::new(format!("action:{i}")).unwrap(),
                task_id: task.fields().task_id.clone(),
                operation_id: OperationId::new("addTaskValue").unwrap(),
                operation_version: ContractVersion::CURRENT,
                side_effect_class: SideEffectClass::RecordMutation,
                resource_scope: scope(),
                expected_revision: Some(task.fields().revision),
                authority: AuthorityRequirement::new(
                    grant().principal,
                    rev(3),
                    scope(),
                    vec![Permission::ExecuteAction],
                    OversightRequirements::new(vec![], true, false, None).unwrap(),
                )
                .unwrap(),
                required_capabilities: vec![CapabilityId::new("task:state").unwrap()],
                budget: task.fields().budget,
                target: task.fields().target,
                idempotency_key: IdempotencyKey::new(format!(
                    "{}:step:{i}",
                    task.fields().task_id.as_str()
                ))
                .unwrap(),
                payload: codec::encode(&codec::Value::Unsigned(*n)).unwrap(),
            })
            .unwrap()
        })
        .collect();
    ActionProposal::new(
        ProposalDraft {
            context: context(),
            proposal_id: ProposalId::new(format!("{}:proposal:1", task.fields().task_id.as_str()))
                .unwrap(),
            task_id: task.fields().task_id.clone(),
            plan_revision: rev(1),
            state_revision: task.fields().revision,
            policy_revision: rev(3),
            actions,
        },
        task,
    )
    .unwrap()
}
pub struct Services {
    pub clock: Rc<Cell<u64>>,
    pub next: u64,
    pub duplicate_event: bool,
    pub fail_clock: Rc<Cell<bool>>,
}
impl HostServices for Services {
    fn now(&mut self) -> Result<Timestamp> {
        if self.fail_clock.get() {
            return Err(ErrorCode::CapabilityUnavailable);
        }
        Ok(Timestamp::new(self.clock.get()).unwrap())
    }
    fn execution_id(&mut self) -> Result<ExecutionId> {
        self.next += 1;
        Ok(ExecutionId::new(format!("execution:{}", self.next)).unwrap())
    }
    fn event_id(&mut self) -> Result<EventId> {
        self.next += 1;
        Ok(EventId::new(format!(
            "event:{}",
            if self.duplicate_event { 1 } else { self.next }
        ))
        .unwrap())
    }
    fn sha256(&mut self, bytes: &[u8]) -> Result<[u8; 32]> {
        eylema_platform::sha256(bytes).map_err(|_| ErrorCode::CapabilityUnavailable)
    }
}
pub type Fixture = (
    Runtime<Services>,
    HostKey,
    Session,
    Rc<Cell<u64>>,
    Rc<Cell<bool>>,
);
pub fn fixture(limits: Limits) -> Fixture {
    let clock = Rc::new(Cell::new(1000));
    let fail = Rc::new(Cell::new(false));
    let services = Services {
        clock: clock.clone(),
        fail_clock: fail.clone(),
        next: 0,
        duplicate_event: false,
    };
    let (mut runtime, host) = Runtime::new(services, limits).unwrap();
    let session = runtime.enroll(&host, grant()).unwrap();
    (runtime, host, session, clock, fail)
}
