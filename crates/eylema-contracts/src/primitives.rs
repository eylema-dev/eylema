use std::collections::BTreeSet;
use std::fmt;

/// Structural protocol limits, not measured capacity or performance claims.
pub const MAX_ID_BYTES: usize = 96;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_REFERENCES: usize = 64;
pub const MAX_ACTIONS: usize = 1024;
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidRequest,
    Forbidden,
    PreconditionFailed,
    ApprovalRequired,
    ApprovalExpired,
    ApprovalMismatch,
    RevisionConflict,
    CapabilityUnavailable,
    BudgetExceeded,
    DeadlineExceeded,
    IdempotencyConflict,
    CursorExpired,
    OutcomeUnknown,
}

impl ErrorCode {
    pub const ALL: [Self; 13] = [
        Self::InvalidRequest,
        Self::Forbidden,
        Self::PreconditionFailed,
        Self::ApprovalRequired,
        Self::ApprovalExpired,
        Self::ApprovalMismatch,
        Self::RevisionConflict,
        Self::CapabilityUnavailable,
        Self::BudgetExceeded,
        Self::DeadlineExceeded,
        Self::IdempotencyConflict,
        Self::CursorExpired,
        Self::OutcomeUnknown,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::Forbidden => "FORBIDDEN",
            Self::PreconditionFailed => "PRECONDITION_FAILED",
            Self::ApprovalRequired => "APPROVAL_REQUIRED",
            Self::ApprovalExpired => "APPROVAL_EXPIRED",
            Self::ApprovalMismatch => "APPROVAL_MISMATCH",
            Self::RevisionConflict => "REVISION_CONFLICT",
            Self::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            Self::BudgetExceeded => "BUDGET_EXCEEDED",
            Self::DeadlineExceeded => "DEADLINE_EXCEEDED",
            Self::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
            Self::CursorExpired => "CURSOR_EXPIRED",
            Self::OutcomeUnknown => "OUTCOME_UNKNOWN",
        }
    }
}

/// Local construction failure; a future request adapter adds correlation context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: &'static str,
}

impl ValidationError {
    pub const fn code(self) -> ErrorCode {
        ErrorCode::InvalidRequest
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}
impl std::error::Error for ValidationError {}

pub(crate) fn require(
    ok: bool,
    field: &'static str,
    message: &'static str,
) -> Result<(), ValidationError> {
    if ok {
        Ok(())
    } else {
        Err(ValidationError { field, message })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContractVersion;

impl ContractVersion {
    pub const CURRENT: Self = Self;
    pub const fn components(self) -> (u16, u16, u16) {
        (0, 1, 0)
    }
    pub fn new(major: u16, minor: u16, patch: u16) -> Result<Self, ValidationError> {
        require(
            (major, minor, patch) == (0, 1, 0),
            "contract_version",
            "unsupported runtime contract version",
        )?;
        Ok(Self)
    }
}

macro_rules! identifiers {
    ($($name:ident),+ $(,)?) => {$ (
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
                let value = value.into();
                require(!value.is_empty() && value.len() <= MAX_ID_BYTES
                    && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-:.".contains(&b)),
                    stringify!($name), "expected a bounded opaque ASCII identifier")?;
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
        }
    )+};
}

identifiers!(
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

/// Positive record revisions; creation requests do not fabricate revision zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Revision(u64);
impl Revision {
    pub fn new(value: u64) -> Result<Self, ValidationError> {
        require(value > 0, "revision", "revision must be positive")?;
        Ok(Self(value))
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Declared Unix milliseconds; construction does not consult a clock or prove expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timestamp(u64);
impl Timestamp {
    pub fn new(value: u64) -> Result<Self, ValidationError> {
        require(value > 0, "timestamp", "Unix milliseconds must be positive")?;
        Ok(Self(value))
    }
    pub const fn unix_ms(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContext {
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub contract_version: ContractVersion,
}

/// Exact logical resource IDs, never filesystem paths, wildcard grants or authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceScope(BTreeSet<ResourceId>);
impl ResourceScope {
    pub fn new(resources: Vec<ResourceId>) -> Result<Self, ValidationError> {
        require(
            !resources.is_empty() && resources.len() <= MAX_REFERENCES,
            "resource_scope",
            "scope must be nonempty and bounded",
        )?;
        let count = resources.len();
        let resources: BTreeSet<_> = resources.into_iter().collect();
        require(
            resources.len() == count,
            "resource_scope",
            "duplicate resource identifier",
        )?;
        Ok(Self(resources))
    }
    pub fn resources(&self) -> impl ExactSizeIterator<Item = &ResourceId> {
        self.0.iter()
    }
    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    max_steps: u64,
    max_events: u64,
    max_payload_bytes: u32,
    max_concurrency: u16,
    deadline: Timestamp,
}
impl Budget {
    pub fn new(
        max_steps: u64,
        max_events: u64,
        max_payload_bytes: u32,
        max_concurrency: u16,
        deadline: Timestamp,
    ) -> Result<Self, ValidationError> {
        require(
            max_steps > 0,
            "budget.max_steps",
            "step budget must be positive",
        )?;
        require(
            max_events > 0,
            "budget.max_events",
            "event budget must be positive",
        )?;
        require(
            max_payload_bytes > 0 && max_payload_bytes as usize <= MAX_PAYLOAD_BYTES,
            "budget.max_payload_bytes",
            "payload budget must be positive and protocol bounded",
        )?;
        require(
            max_concurrency > 0,
            "budget.max_concurrency",
            "concurrency must be positive",
        )?;
        Ok(Self {
            max_steps,
            max_events,
            max_payload_bytes,
            max_concurrency,
            deadline,
        })
    }
    pub const fn max_steps(self) -> u64 {
        self.max_steps
    }
    pub const fn max_events(self) -> u64 {
        self.max_events
    }
    pub const fn max_payload_bytes(self) -> u32 {
        self.max_payload_bytes
    }
    pub const fn max_concurrency(self) -> u16 {
        self.max_concurrency
    }
    pub const fn deadline(self) -> Timestamp {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionContext {
    NativeMacos,
    Browser,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backend {
    NativeCpu,
    NativeGpu,
    WasmCpu,
    WebGpu,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuApi {
    None,
    Metal,
    WebGpu,
}

/// A requested pair; no constructor establishes backend availability or privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionTarget {
    context: ExecutionContext,
    backend: Backend,
    gpu_api: GpuApi,
}
impl ExecutionTarget {
    pub fn new(
        context: ExecutionContext,
        backend: Backend,
        gpu_api: GpuApi,
    ) -> Result<Self, ValidationError> {
        let valid = matches!(
            (context, backend, gpu_api),
            (
                ExecutionContext::NativeMacos,
                Backend::NativeCpu,
                GpuApi::None
            ) | (
                ExecutionContext::NativeMacos,
                Backend::NativeGpu,
                GpuApi::Metal
            ) | (ExecutionContext::Browser, Backend::WasmCpu, GpuApi::None)
                | (ExecutionContext::Browser, Backend::WebGpu, GpuApi::WebGpu)
        );
        require(
            valid,
            "execution_target",
            "context/backend/GPU API combination is invalid",
        )?;
        Ok(Self {
            context,
            backend,
            gpu_api,
        })
    }
    pub const fn context(self) -> ExecutionContext {
        self.context
    }
    pub const fn backend(self) -> Backend {
        self.backend
    }
    pub const fn gpu_api(self) -> GpuApi {
        self.gpu_api
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Permission {
    Read,
    CreateTask,
    ProposeAction,
    ExecuteAction,
    ObserveEvents,
    RequestCancellation,
    ApproveAction,
    GovernPolicy,
}

/// Independently composable obligations, not mutually exclusive human roles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OversightRequirements {
    checkpoints: Vec<CheckpointRef>,
    observation: bool,
    intervention: bool,
    governance: Option<GovernanceRef>,
}
impl OversightRequirements {
    pub fn new(
        checkpoints: Vec<CheckpointRef>,
        observation: bool,
        intervention: bool,
        governance: Option<GovernanceRef>,
    ) -> Result<Self, ValidationError> {
        unique(&checkpoints, "oversight.checkpoints")?;
        Ok(Self {
            checkpoints,
            observation,
            intervention,
            governance,
        })
    }
    pub fn checkpoints(&self) -> &[CheckpointRef] {
        &self.checkpoints
    }
    pub const fn requires_observation(&self) -> bool {
        self.observation
    }
    pub const fn requires_intervention(&self) -> bool {
        self.intervention
    }
    pub fn governance(&self) -> Option<&GovernanceRef> {
        self.governance.as_ref()
    }
}

/// Untrusted required authority declaration. The controller must resolve and enforce it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityRequirement {
    principal_ref: PrincipalRef,
    policy_revision: Revision,
    resource_scope: ResourceScope,
    permissions: Vec<Permission>,
    oversight: OversightRequirements,
}
impl AuthorityRequirement {
    pub fn new(
        principal_ref: PrincipalRef,
        policy_revision: Revision,
        resource_scope: ResourceScope,
        permissions: Vec<Permission>,
        oversight: OversightRequirements,
    ) -> Result<Self, ValidationError> {
        require(
            !permissions.is_empty(),
            "permissions",
            "required permissions cannot be empty",
        )?;
        unique(&permissions, "permissions")?;
        Ok(Self {
            principal_ref,
            policy_revision,
            resource_scope,
            permissions,
            oversight,
        })
    }
    pub fn principal_ref(&self) -> &PrincipalRef {
        &self.principal_ref
    }
    pub const fn policy_revision(&self) -> Revision {
        self.policy_revision
    }
    pub fn resource_scope(&self) -> &ResourceScope {
        &self.resource_scope
    }
    pub fn permissions(&self) -> &[Permission] {
        &self.permissions
    }
    pub fn oversight(&self) -> &OversightRequirements {
        &self.oversight
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolError {
    code: ErrorCode,
    message: String,
    field_path: Option<String>,
    retryable: bool,
    correlation_id: CorrelationId,
    retry_after_ms: Option<u64>,
}
impl ProtocolError {
    pub fn new(
        code: ErrorCode,
        message: String,
        field_path: Option<String>,
        retryable: bool,
        correlation_id: CorrelationId,
        retry_after_ms: Option<u64>,
    ) -> Result<Self, ValidationError> {
        text(&message, "error.message")?;
        if let Some(path) = &field_path {
            require(
                !path.is_empty() && path.len() <= 256 && !path.chars().any(char::is_control),
                "error.field_path",
                "invalid bounded field path",
            )?;
        }
        require(
            retryable || retry_after_ms.is_none(),
            "error.retry_after_ms",
            "retry delay requires explicitly retryable error",
        )?;
        Ok(Self {
            code,
            message,
            field_path,
            retryable,
            correlation_id,
            retry_after_ms,
        })
    }
    pub const fn code(&self) -> ErrorCode {
        self.code
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn field_path(&self) -> Option<&str> {
        self.field_path.as_deref()
    }
    pub const fn retryable(&self) -> bool {
        self.retryable
    }
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }
    pub const fn retry_after_ms(&self) -> Option<u64> {
        self.retry_after_ms
    }
}

pub(crate) fn text(value: &str, field: &'static str) -> Result<(), ValidationError> {
    require(
        !value.trim().is_empty()
            && value.len() <= MAX_TEXT_BYTES
            && !value
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t'),
        field,
        "expected nonempty bounded text without disallowed controls",
    )
}

pub(crate) fn unique<T: Ord>(items: &[T], field: &'static str) -> Result<(), ValidationError> {
    require(
        items.len() <= MAX_REFERENCES,
        field,
        "reference list exceeds protocol bound",
    )?;
    require(
        items.iter().collect::<BTreeSet<_>>().len() == items.len(),
        field,
        "duplicate reference",
    )
}
