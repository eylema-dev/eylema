use super::*;

trait Wire: Sized {
    fn value(&self) -> Value;
    fn read(value: Value) -> Result<Self>;
}
impl Wire for String {
    fn value(&self) -> Value {
        Value::Text(self.clone())
    }
    fn read(value: Value) -> Result<Self> {
        if let Value::Text(s) = value {
            Ok(s)
        } else {
            Err(CodecError::InvalidData)
        }
    }
}
impl Wire for u64 {
    fn value(&self) -> Value {
        Value::Unsigned(*self)
    }
    fn read(value: Value) -> Result<Self> {
        if let Value::Unsigned(n) = value {
            Ok(n)
        } else {
            Err(CodecError::InvalidData)
        }
    }
}
impl Wire for bool {
    fn value(&self) -> Value {
        Value::Bool(*self)
    }
    fn read(value: Value) -> Result<Self> {
        if let Value::Bool(b) = value {
            Ok(b)
        } else {
            Err(CodecError::InvalidData)
        }
    }
}
impl<T: Wire> Wire for Option<T> {
    fn value(&self) -> Value {
        self.as_ref().map_or(Value::Null, Wire::value)
    }
    fn read(value: Value) -> Result<Self> {
        if value == Value::Null {
            Ok(None)
        } else {
            Ok(Some(T::read(value)?))
        }
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn value(&self) -> Value {
        Value::List(self.iter().map(Wire::value).collect())
    }
    fn read(value: Value) -> Result<Self> {
        if let Value::List(items) = value {
            items.into_iter().map(T::read).collect()
        } else {
            Err(CodecError::InvalidData)
        }
    }
}
macro_rules! ids {
    ($($t:ty),+)=>{$(impl Wire for $t {
        fn value(&self)->Value { Value::Text(self.as_str().into()) }
        fn read(value:Value)->Result<Self> { Ok(Self::new(String::read(value)?)?) }
    })+};
}
ids!(
    TaskId,
    ActionId,
    ProposalId,
    ExecutionId,
    EventId,
    RequestId,
    CorrelationId,
    ResourceId,
    PrincipalRef,
    OperationId,
    CapabilityId,
    EvidenceRef,
    CheckpointRef,
    GovernanceRef,
    IdempotencyKey
);
impl Wire for Revision {
    fn value(&self) -> Value {
        self.get().value()
    }
    fn read(value: Value) -> Result<Self> {
        Ok(Self::new(u64::read(value)?)?)
    }
}
impl Wire for Timestamp {
    fn value(&self) -> Value {
        self.unix_ms().value()
    }
    fn read(value: Value) -> Result<Self> {
        Ok(Self::new(u64::read(value)?)?)
    }
}
impl Wire for ContractVersion {
    fn value(&self) -> Value {
        Value::Text("0.1.0".into())
    }
    fn read(value: Value) -> Result<Self> {
        if String::read(value)? == "0.1.0" {
            Ok(Self::CURRENT)
        } else {
            Err(CodecError::UnsupportedVersion)
        }
    }
}
macro_rules! enumeration {
    ($t:ty {$($variant:ident=>$name:literal),+ $(,)?})=>{
        impl Wire for $t {
            fn value(&self)->Value { Value::Text(match self { $(Self::$variant=>$name),+ }.into()) }
            fn read(value:Value)->Result<Self> { match String::read(value)?.as_str() { $($name=>Ok(Self::$variant)),+, _=>Err(CodecError::InvalidData) } }
        }
    };
}
enumeration!(ExecutionContext {NativeMacos=>"native_macos",Browser=>"browser"});
enumeration!(Backend {NativeCpu=>"native_cpu",NativeGpu=>"native_gpu",WasmCpu=>"wasm_cpu",WebGpu=>"webgpu"});
enumeration!(GpuApi {None=>"none",Metal=>"metal",WebGpu=>"webgpu"});
enumeration!(Permission {Read=>"read",CreateTask=>"create_task",ProposeAction=>"propose_action",ExecuteAction=>"execute_action",ObserveEvents=>"observe_events",RequestCancellation=>"request_cancellation",ApproveAction=>"approve_action",GovernPolicy=>"govern_policy"});
enumeration!(SideEffectClass {Read=>"read",RecordMutation=>"record_mutation",Compute=>"compute",ExternalEffect=>"external_effect"});
enumeration!(ExecutionState {Accepted=>"accepted",Queued=>"queued",Running=>"running",CancelRequested=>"cancel_requested",Succeeded=>"succeeded",Failed=>"failed",Cancelled=>"cancelled",OutcomeUnknown=>"outcome_unknown"});
enumeration!(EffectOutcome {Applied=>"applied",NotApplied=>"not_applied",Unknown=>"unknown"});
enumeration!(EventKind {TaskCreated=>"task_created",ActionProposed=>"action_proposed",ExecutionAccepted=>"execution_accepted",ExecutionProgress=>"execution_progress",CancellationRequested=>"cancellation_requested",CancellationEffective=>"cancellation_effective",ExecutionCompleted=>"execution_completed",ExecutionFailed=>"execution_failed",ExecutionOutcomeUnknown=>"execution_outcome_unknown"});
impl Wire for ErrorCode {
    fn value(&self) -> Value {
        Value::Text(self.as_str().into())
    }
    fn read(value: Value) -> Result<Self> {
        let s = String::read(value)?;
        Self::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or(CodecError::InvalidData)
    }
}

macro_rules! get {
    ($f:ident,$key:literal) => {
        Wire::read($f.take($key)?)?
    };
}
impl Wire for RequestContext {
    fn value(&self) -> Value {
        map(vec![
            ("request_id", self.request_id.value()),
            ("correlation_id", self.correlation_id.value()),
            ("contract_version", self.contract_version.value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let v = Self {
            request_id: get!(f, "request_id"),
            correlation_id: get!(f, "correlation_id"),
            contract_version: get!(f, "contract_version"),
        };
        f.finish()?;
        Ok(v)
    }
}
impl Wire for ResourceScope {
    fn value(&self) -> Value {
        Value::List(self.resources().map(Wire::value).collect())
    }
    fn read(value: Value) -> Result<Self> {
        let items = Vec::<ResourceId>::read(value)?;
        if items.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(CodecError::NonCanonical);
        }
        Ok(Self::new(items)?)
    }
}
impl Wire for Budget {
    fn value(&self) -> Value {
        map(vec![
            ("max_steps", self.max_steps().value()),
            ("max_events", self.max_events().value()),
            (
                "max_payload_bytes",
                (self.max_payload_bytes() as u64).value(),
            ),
            ("max_concurrency", (self.max_concurrency() as u64).value()),
            ("deadline", self.deadline().value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let steps = get!(f, "max_steps");
        let events = get!(f, "max_events");
        let bytes = u32::try_from(u64::read(f.take("max_payload_bytes")?)?)
            .map_err(|_| CodecError::TooLarge)?;
        let parallel = u16::try_from(u64::read(f.take("max_concurrency")?)?)
            .map_err(|_| CodecError::TooLarge)?;
        let deadline = get!(f, "deadline");
        f.finish()?;
        Ok(Self::new(steps, events, bytes, parallel, deadline)?)
    }
}
impl Wire for ExecutionTarget {
    fn value(&self) -> Value {
        map(vec![
            ("execution_context", self.context().value()),
            ("backend", self.backend().value()),
            ("gpu_api", self.gpu_api().value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let v = Self::new(
            get!(f, "execution_context"),
            get!(f, "backend"),
            get!(f, "gpu_api"),
        )?;
        f.finish()?;
        Ok(v)
    }
}
fn set_value<T: Wire>(values: &[T]) -> Value {
    let mut items: Vec<_> = values.iter().map(Wire::value).collect();
    items.sort_by(|a, b| match (a, b) {
        (Value::Text(a), Value::Text(b)) => a.as_bytes().cmp(b.as_bytes()),
        _ => std::cmp::Ordering::Equal,
    });
    Value::List(items)
}
fn set_read<T: Wire>(value: Value) -> Result<Vec<T>> {
    let Value::List(items) = value else {
        return Err(CodecError::InvalidData);
    };
    let mut last: Option<&str> = None;
    for item in &items {
        let Value::Text(current) = item else {
            return Err(CodecError::InvalidData);
        };
        if last.is_some_and(|last| last.as_bytes() >= current.as_bytes()) {
            return Err(CodecError::NonCanonical);
        }
        last = Some(current);
    }
    items.into_iter().map(Wire::read).collect()
}
impl Wire for OversightRequirements {
    fn value(&self) -> Value {
        map(vec![
            ("checkpoints", set_value(self.checkpoints())),
            ("observation", self.requires_observation().value()),
            ("intervention", self.requires_intervention().value()),
            ("governance", self.governance().cloned().value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let v = Self::new(
            set_read(f.take("checkpoints")?)?,
            get!(f, "observation"),
            get!(f, "intervention"),
            get!(f, "governance"),
        )?;
        f.finish()?;
        Ok(v)
    }
}
impl Wire for AuthorityRequirement {
    fn value(&self) -> Value {
        map(vec![
            ("principal_ref", self.principal_ref().value()),
            ("policy_revision", self.policy_revision().value()),
            ("resource_scope", self.resource_scope().value()),
            ("permissions", set_value(self.permissions())),
            ("oversight", self.oversight().value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let v = Self::new(
            get!(f, "principal_ref"),
            get!(f, "policy_revision"),
            get!(f, "resource_scope"),
            set_read(f.take("permissions")?)?,
            get!(f, "oversight"),
        )?;
        f.finish()?;
        Ok(v)
    }
}
impl Wire for ProtocolError {
    fn value(&self) -> Value {
        map(vec![
            ("code", self.code().value()),
            ("message", self.message().to_owned().value()),
            ("field_path", self.field_path().map(str::to_owned).value()),
            ("retryable", self.retryable().value()),
            ("correlation_id", self.correlation_id().value()),
            ("retry_after_ms", self.retry_after_ms().value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let v = Self::new(
            get!(f, "code"),
            get!(f, "message"),
            get!(f, "field_path"),
            get!(f, "retryable"),
            get!(f, "correlation_id"),
            get!(f, "retry_after_ms"),
        )?;
        f.finish()?;
        Ok(v)
    }
}
impl Wire for TaskSpec {
    fn value(&self) -> Value {
        let t = self.fields();
        map(vec![
            ("context", t.context.value()),
            ("task_id", t.task_id.value()),
            ("revision", t.revision.value()),
            ("objective", t.objective.value()),
            ("resource_scope", t.resource_scope.value()),
            ("budget", t.budget.value()),
            ("target", t.target.value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let t = Self::new(TaskDraft {
            context: get!(f, "context"),
            task_id: get!(f, "task_id"),
            revision: get!(f, "revision"),
            objective: get!(f, "objective"),
            resource_scope: get!(f, "resource_scope"),
            budget: get!(f, "budget"),
            target: get!(f, "target"),
        })?;
        f.finish()?;
        Ok(t)
    }
}
impl Wire for ActionSpec {
    fn value(&self) -> Value {
        let a = self.fields();
        map(vec![
            ("context", a.context.value()),
            ("action_id", a.action_id.value()),
            ("task_id", a.task_id.value()),
            ("operation_id", a.operation_id.value()),
            ("operation_version", a.operation_version.value()),
            ("side_effect_class", a.side_effect_class.value()),
            ("resource_scope", a.resource_scope.value()),
            ("expected_revision", a.expected_revision.value()),
            ("authority", a.authority.value()),
            ("required_capabilities", set_value(&a.required_capabilities)),
            ("budget", a.budget.value()),
            ("target", a.target.value()),
            ("idempotency_key", a.idempotency_key.value()),
            ("payload", Value::Bytes(a.payload.clone())),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let a = Self::new(ActionDraft {
            context: get!(f, "context"),
            action_id: get!(f, "action_id"),
            task_id: get!(f, "task_id"),
            operation_id: get!(f, "operation_id"),
            operation_version: get!(f, "operation_version"),
            side_effect_class: get!(f, "side_effect_class"),
            resource_scope: get!(f, "resource_scope"),
            expected_revision: get!(f, "expected_revision"),
            authority: get!(f, "authority"),
            required_capabilities: set_read(f.take("required_capabilities")?)?,
            budget: get!(f, "budget"),
            target: get!(f, "target"),
            idempotency_key: get!(f, "idempotency_key"),
            payload: bytes(f.take("payload")?)?,
        })?;
        f.finish()?;
        Ok(a)
    }
}
fn bytes(value: Value) -> Result<Vec<u8>> {
    if let Value::Bytes(bytes) = value {
        Ok(bytes)
    } else {
        Err(CodecError::InvalidData)
    }
}
impl Wire for ProposalDraft {
    fn value(&self) -> Value {
        map(vec![
            ("context", self.context.value()),
            ("proposal_id", self.proposal_id.value()),
            ("task_id", self.task_id.value()),
            ("plan_revision", self.plan_revision.value()),
            ("state_revision", self.state_revision.value()),
            ("policy_revision", self.policy_revision.value()),
            ("actions", self.actions.value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let p = Self {
            context: get!(f, "context"),
            proposal_id: get!(f, "proposal_id"),
            task_id: get!(f, "task_id"),
            plan_revision: get!(f, "plan_revision"),
            state_revision: get!(f, "state_revision"),
            policy_revision: get!(f, "policy_revision"),
            actions: get!(f, "actions"),
        };
        f.finish()?;
        Ok(p)
    }
}
impl Wire for EffectRecord {
    fn value(&self) -> Value {
        let e = self.fields();
        map(vec![
            ("action_id", e.action_id.value()),
            ("outcome", e.outcome.value()),
            ("evidence", set_value(&e.evidence)),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let e = Self::new(EffectDraft {
            action_id: get!(f, "action_id"),
            outcome: get!(f, "outcome"),
            evidence: set_read(f.take("evidence")?)?,
        })?;
        f.finish()?;
        Ok(e)
    }
}
impl Wire for ExecutionDraft {
    fn value(&self) -> Value {
        map(vec![
            ("context", self.context.value()),
            ("execution_id", self.execution_id.value()),
            ("task_id", self.task_id.value()),
            ("proposal_id", self.proposal_id.value()),
            ("plan_revision", self.plan_revision.value()),
            ("state_revision", self.state_revision.value()),
            ("state", self.state.value()),
            ("effects", self.effects.value()),
            ("result_evidence", set_value(&self.result_evidence)),
            (
                "effective_stop_evidence",
                self.effective_stop_evidence.value(),
            ),
            ("error", self.error.value()),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let e = Self {
            context: get!(f, "context"),
            execution_id: get!(f, "execution_id"),
            task_id: get!(f, "task_id"),
            proposal_id: get!(f, "proposal_id"),
            plan_revision: get!(f, "plan_revision"),
            state_revision: get!(f, "state_revision"),
            state: get!(f, "state"),
            effects: get!(f, "effects"),
            result_evidence: set_read(f.take("result_evidence")?)?,
            effective_stop_evidence: get!(f, "effective_stop_evidence"),
            error: get!(f, "error"),
        };
        f.finish()?;
        Ok(e)
    }
}
impl Wire for EventRecord {
    fn value(&self) -> Value {
        let e = self.fields();
        map(vec![
            ("contract_version", e.contract_version.value()),
            ("event_id", e.event_id.value()),
            ("task_id", e.task_id.value()),
            ("execution_id", e.execution_id.value()),
            ("state_revision", e.state_revision.value()),
            ("timestamp", e.timestamp.value()),
            ("correlation_id", e.correlation_id.value()),
            ("kind", e.kind.value()),
            ("evidence", set_value(&e.evidence)),
        ])
    }
    fn read(value: Value) -> Result<Self> {
        let mut f = Fields::new(value)?;
        let e = Self::new(EventDraft {
            contract_version: get!(f, "contract_version"),
            event_id: get!(f, "event_id"),
            task_id: get!(f, "task_id"),
            execution_id: get!(f, "execution_id"),
            state_revision: get!(f, "state_revision"),
            timestamp: get!(f, "timestamp"),
            correlation_id: get!(f, "correlation_id"),
            kind: get!(f, "kind"),
            evidence: set_read(f.take("evidence")?)?,
        })?;
        f.finish()?;
        Ok(e)
    }
}

/// Context is included where needed to revalidate typed record bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireRecord {
    Task(TaskSpec),
    Proposal {
        task: TaskSpec,
        proposal: ActionProposal,
    },
    Execution {
        task: TaskSpec,
        proposal: ActionProposal,
        execution: Box<ExecutionRecord>,
    },
    Event(EventRecord),
    Error(ProtocolError),
}

pub fn encode_record(record: &WireRecord) -> Result<Vec<u8>> {
    let value = match record {
        WireRecord::Task(t) => map(vec![
            ("kind", Value::Text("task".into())),
            ("task", t.value()),
        ]),
        WireRecord::Proposal { task, proposal } => {
            ActionProposal::new(proposal.fields().clone(), task)?;
            map(vec![
                ("kind", Value::Text("proposal".into())),
                ("task", task.value()),
                ("proposal", proposal.fields().value()),
            ])
        }
        WireRecord::Execution {
            task,
            proposal,
            execution,
        } => {
            ActionProposal::new(proposal.fields().clone(), task)?;
            ExecutionRecord::new(execution.fields().clone(), proposal)?;
            map(vec![
                ("kind", Value::Text("execution".into())),
                ("task", task.value()),
                ("proposal", proposal.fields().value()),
                ("execution", execution.fields().value()),
            ])
        }
        WireRecord::Event(event) => map(vec![
            ("kind", Value::Text("event".into())),
            ("event", event.value()),
        ]),
        WireRecord::Error(error) => map(vec![
            ("kind", Value::Text("error".into())),
            ("error", error.value()),
        ]),
    };
    encode(&value)
}

pub fn decode_record(bytes: &[u8]) -> Result<WireRecord> {
    let mut f = Fields::new(decode(bytes)?)?;
    let kind = String::read(f.take("kind")?)?;
    let record = match kind.as_str() {
        "task" => WireRecord::Task(get!(f, "task")),
        "proposal" | "execution" => {
            let task: TaskSpec = get!(f, "task");
            let proposal = ActionProposal::new(get!(f, "proposal"), &task)?;
            if kind == "proposal" {
                WireRecord::Proposal { task, proposal }
            } else {
                let execution = ExecutionRecord::new(get!(f, "execution"), &proposal)?;
                WireRecord::Execution {
                    task,
                    proposal,
                    execution: Box::new(execution),
                }
            }
        }
        "event" => WireRecord::Event(get!(f, "event")),
        "error" => WireRecord::Error(get!(f, "error")),
        _ => return Err(CodecError::InvalidData),
    };
    f.finish()?;
    Ok(record)
}

/// Exact effect-relevant task/proposal bytes. Only transport request/correlation
/// contexts are excluded; opaque payload bytes are never interpreted or rewritten.
pub fn canonical_approval_payload(task: &TaskSpec, proposal: &ActionProposal) -> Result<Vec<u8>> {
    ActionProposal::new(proposal.fields().clone(), task)?;
    let mut value = map(vec![
        ("kind", Value::Text("approval_payload".into())),
        ("task", task.value()),
        ("proposal", proposal.fields().value()),
    ]);
    fn strip_context(value: &mut Value) {
        match value {
            Value::Map(fields) => {
                fields.retain(|(key, _)| key != "context");
                for (_, value) in fields {
                    strip_context(value)
                }
            }
            Value::List(items) => {
                for item in items {
                    strip_context(item)
                }
            }
            _ => {}
        }
    }
    strip_context(&mut value);
    encode(&value)
}
