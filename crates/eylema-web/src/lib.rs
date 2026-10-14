//! Owned, closed loopback HTTP profile. No remote authentication or TLS claim.
#![forbid(unsafe_code)]
pub use eylema_contracts::transport as protocol;
use eylema_contracts::{
    codec::{self, Value, WireRecord},
    *,
};
use eylema_host::{DeviceOwnerHuman, DurableHost, DurableHuman, DurableSession, HostError, Owner};
use eylema_journal::{Sha256, Storage};
use eylema_runtime::{CancellationOutcome, Grant, HostServices};
use protocol::Request;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
mod http;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebError {
    Malformed,
    Unauthorized,
    Forbidden,
    TooLarge,
    Timeout,
    Unsupported,
    Io,
    Configuration,
    Entropy,
    Host(HostError),
}
impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for WebError {}
impl From<HostError> for WebError {
    fn from(e: HostError) -> Self {
        Self::Host(e)
    }
}
impl WebError {
    pub fn status(self) -> u16 {
        match self {
            Self::Unauthorized => 401,
            Self::Forbidden | Self::Host(HostError::Runtime(ErrorCode::Forbidden)) => 403,
            Self::TooLarge => 413,
            Self::Timeout => 408,
            Self::Unsupported => 405,
            Self::Malformed => 400,
            Self::Host(HostError::Runtime(_)) => 409,
            _ => 503,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Malformed => "MALFORMED_REQUEST",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::TooLarge => "REQUEST_TOO_LARGE",
            Self::Timeout => "REQUEST_TIMEOUT",
            Self::Unsupported => "UNSUPPORTED_REQUEST",
            Self::Io => "TRANSPORT_UNAVAILABLE",
            Self::Configuration => "INVALID_CONFIGURATION",
            Self::Entropy => "ENTROPY_UNAVAILABLE",
            Self::Host(HostError::Runtime(e)) => e.as_str(),
            Self::Host(_) => "STORAGE_UNAVAILABLE",
        }
    }
}
#[derive(Clone)]
pub struct Config {
    pub address: SocketAddr,
    pub header_bytes: usize,
    pub body_bytes: usize,
    pub response_bytes: usize,
    pub request_timeout: Duration,
    pub response_timeout: Duration,
    pub session_lifetime: Duration,
    pub worker_interval: Duration,
    pub sessions: usize,
    pub oversight: OversightRequirements,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            address: (Ipv4Addr::LOCALHOST, 0).into(),
            header_bytes: 8192,
            body_bytes: 64 * 1024,
            response_bytes: codec::MAX_WIRE_BYTES,
            request_timeout: Duration::from_secs(5),
            response_timeout: Duration::from_secs(5),
            session_lifetime: Duration::from_secs(3600),
            worker_interval: Duration::from_millis(10),
            sessions: 16,
            oversight: OversightRequirements::new(vec![], true, false, None)
                .expect("constant oversight"),
        }
    }
}
impl Config {
    fn validate(&self) -> Result<(), WebError> {
        let timeout = |d: Duration| !d.is_zero() && d <= Duration::from_secs(10);
        if self.address.ip() != Ipv4Addr::LOCALHOST
            || !(128..=16384).contains(&self.header_bytes)
            || !(32..=64 * 1024).contains(&self.body_bytes)
            || !(256..=codec::MAX_WIRE_BYTES).contains(&self.response_bytes)
            || !timeout(self.request_timeout)
            || !timeout(self.response_timeout)
            || self.session_lifetime.is_zero()
            || self.session_lifetime > Duration::from_secs(86400)
            || !(1..=16).contains(&self.sessions)
            || self.worker_interval < Duration::from_millis(1)
            || self.worker_interval > Duration::from_millis(100)
        {
            return Err(WebError::Configuration);
        }
        Ok(())
    }
}
/// Returned only to the trusted embedding owner; never put in URLs or logs.
#[derive(Clone)]
pub struct Credentials {
    bearer: String,
    csrf: String,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials(<redacted>)")
    }
}
impl Credentials {
    pub fn bearer(&self) -> &str {
        &self.bearer
    }
    pub fn csrf(&self) -> &str {
        &self.csrf
    }
}
enum HumanAuthority {
    Attested(DurableHuman),
    DeviceOwner(DeviceOwnerHuman),
}
struct Actor {
    credentials: Credentials,
    session: DurableSession,
    human: Option<HumanAuthority>,
    grant: Grant,
    expires: Instant,
    last_console_execution: Option<ExecutionId>,
}
impl Actor {
    fn permission(&self, permission: Permission) -> Result<(), WebError> {
        if !self.grant.permissions.contains(&permission) {
            Err(WebError::Forbidden)
        } else {
            Ok(())
        }
    }
    fn scope(&self, scope: &ResourceScope) -> Result<(), WebError> {
        if !scope.is_subset_of(&self.grant.resource_scope) {
            Err(WebError::Forbidden)
        } else {
            Ok(())
        }
    }
}
pub struct Server<T: Storage, H: Sha256, S: HostServices> {
    listener: TcpListener,
    host: DurableHost<T, H, S>,
    owner: Owner,
    config: Config,
    authority: String,
    origin: String,
    actors: Vec<Actor>,
}
fn token() -> Result<String, WebError> {
    let mut result = String::with_capacity(64);
    for _ in 0..2 {
        let id = eylema_platform::random_id(eylema_platform::IdKind::Request)
            .map_err(|_| WebError::Entropy)?;
        let part = id.strip_prefix("request:").ok_or(WebError::Entropy)?;
        if part.len() != 32 || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(WebError::Entropy);
        }
        result.push_str(part);
    }
    Ok(result)
}
fn wire(v: &WireRecord) -> Result<Value, WebError> {
    codec::encode_record(v)
        .map(Value::Bytes)
        .map_err(|_| WebError::Malformed)
}
fn text(v: &str) -> Value {
    Value::Text(v.into())
}
fn list(v: Vec<Value>) -> Value {
    Value::List(v)
}
impl<T: Storage, H: Sha256, S: HostServices> Server<T, H, S> {
    pub fn bind(
        host: DurableHost<T, H, S>,
        owner: Owner,
        config: Config,
    ) -> Result<Self, WebError> {
        config.validate()?;
        let listener = TcpListener::bind(config.address).map_err(|_| WebError::Io)?;
        listener.set_nonblocking(true).map_err(|_| WebError::Io)?;
        let address = listener.local_addr().map_err(|_| WebError::Io)?;
        let authority = address.to_string();
        let origin = format!("http://{authority}");
        Ok(Self {
            listener,
            host,
            owner,
            config,
            authority,
            origin,
            actors: vec![],
        })
    }
    pub fn address(&self) -> Result<SocketAddr, WebError> {
        self.listener.local_addr().map_err(|_| WebError::Io)
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn register_session(&mut self, grant: Grant) -> Result<Credentials, WebError> {
        self.register(grant, false)
    }
    /// Trusted host attestation only after independent person verification.
    /// There is no HTTP registration or human-status field.
    pub fn register_verified_human(&mut self, grant: Grant) -> Result<Credentials, WebError> {
        self.register(grant, true)
    }
    fn new_credentials(&self) -> Result<Credentials, WebError> {
        if self.actors.len() >= self.config.sessions {
            return Err(WebError::Configuration);
        }
        let mut candidate = None;
        for _ in 0..8 {
            let credentials = Credentials {
                bearer: token()?,
                csrf: token()?,
            };
            if !self.actors.iter().any(|a| {
                a.credentials.bearer == credentials.bearer || a.credentials.csrf == credentials.csrf
            }) && credentials.bearer != credentials.csrf
            {
                candidate = Some(credentials);
                break;
            }
        }
        candidate.ok_or(WebError::Entropy)
    }
    fn register(&mut self, grant: Grant, human: bool) -> Result<Credentials, WebError> {
        let credentials = self.new_credentials()?;
        let expires = Instant::now()
            .checked_add(self.config.session_lifetime)
            .ok_or(WebError::Configuration)?;
        let (session, human) = if human {
            let h = self
                .host
                .enroll_verified_human(&self.owner, grant.clone())?;
            (h.session().clone(), Some(HumanAuthority::Attested(h)))
        } else {
            (self.host.enroll(&self.owner, grant.clone())?, None)
        };
        self.actors.push(Actor {
            credentials: credentials.clone(),
            session,
            human,
            grant,
            expires,
            last_console_execution: None,
        });
        Ok(credentials)
    }
    /// Explicit trusted-host entry point. Never exposed as an HTTP enrollment endpoint.
    pub fn register_device_owner(
        &mut self,
        grant: Grant,
        purpose: eylema_platform::human::HumanPurpose,
        ttl: Duration,
        timeout: Duration,
    ) -> Result<Credentials, WebError> {
        let credentials = self.new_credentials()?;
        let request = self
            .host
            .request_device_owner(&self.owner, grant.clone(), purpose, ttl)?;
        let proof = request.authenticate(timeout).map_err(|error| match error {
            eylema_platform::human::HumanError::Denied => WebError::Forbidden,
            eylema_platform::human::HumanError::Timeout => WebError::Timeout,
            _ => WebError::Configuration,
        })?;
        let human = self.host.enroll_device_owner(&self.owner, request, proof)?;
        let expires = human.expires().min(
            Instant::now()
                .checked_add(self.config.session_lifetime)
                .ok_or(WebError::Configuration)?,
        );
        if Instant::now() >= expires {
            self.host.revoke_device_owner(&human)?;
            return Err(WebError::Unauthorized);
        }
        let session = human.session().clone();
        self.actors.push(Actor {
            credentials: credentials.clone(),
            session,
            human: Some(HumanAuthority::DeviceOwner(human)),
            grant,
            expires,
            last_console_execution: None,
        });
        Ok(credentials)
    }
    fn authenticate(&self, headers: &http::Headers) -> Result<usize, WebError> {
        if headers.get("host") != Some(self.authority.as_str())
            || headers.get("origin") != Some(self.origin.as_str())
        {
            return Err(WebError::Forbidden);
        }
        if let Some(site) = headers.get("sec-fetch-site")
            && site != "same-origin"
        {
            return Err(WebError::Forbidden);
        }
        let supplied = headers
            .get("authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
            .or_else(|| {
                headers.get("cookie").and_then(|cookies| {
                    cookies
                        .split(';')
                        .map(str::trim)
                        .find_map(|cookie| cookie.strip_prefix("eylema_session="))
                })
            })
            .ok_or(WebError::Unauthorized)?;
        let index = self
            .actors
            .iter()
            .position(|a| a.credentials.bearer == supplied && Instant::now() < a.expires)
            .ok_or(WebError::Unauthorized)?;
        if headers.get("x-eylema-csrf") != Some(self.actors[index].credentials.csrf.as_str()) {
            return Err(WebError::Forbidden);
        }
        Ok(index)
    }
    fn dispatch(&mut self, actor: usize, request: Request) -> Result<Value, WebError> {
        let a = &self.actors[actor];
        match request {
            Request::Create(task) => {
                a.permission(Permission::CreateTask)?;
                a.scope(&task.fields().resource_scope)?;
                self.host.create_governed_task(
                    &self.owner,
                    &a.session,
                    task,
                    self.config.oversight.clone(),
                )?;
                Ok(Value::Null)
            }
            Request::Admit {
                task,
                proposal,
                approvals,
            } => {
                a.permission(Permission::ExecuteAction)?;
                a.scope(&task.fields().resource_scope)?;
                if proposal
                    .fields()
                    .actions
                    .iter()
                    .any(|x| x.fields().authority.principal_ref() != &a.grant.principal)
                {
                    return Err(WebError::Forbidden);
                }
                Ok(text(
                    self.host
                        .admit_with_approvals(&a.session, task, proposal, &approvals)?
                        .as_str(),
                ))
            }
            Request::Task(id) => {
                a.permission(Permission::Read)?;
                let snapshot = self.host.task(&a.session, &id)?;
                Ok(list(vec![
                    wire(&WireRecord::Task(snapshot.task))?,
                    Value::Unsigned(snapshot.value),
                    Value::Unsigned(snapshot.performed_steps),
                    Value::Unsigned(snapshot.charged_steps),
                    Value::Unsigned(snapshot.charged_events),
                ]))
            }
            Request::Execution(id) => {
                a.permission(Permission::Read)?;
                wire(&self.host.execution_record(&a.session, &id)?)
            }
            Request::Events { task, after, limit } => {
                a.permission(Permission::ObserveEvents)?;
                Ok(list(
                    self.host
                        .events(&a.session, &task, after.as_ref(), usize::from(limit))?
                        .into_iter()
                        .map(|e| wire(&WireRecord::Event(e)))
                        .collect::<Result<_, _>>()?,
                ))
            }
            Request::Observation(id) => {
                a.permission(Permission::ObserveEvents)?;
                let o = self.host.observation(&a.session, &id)?;
                Ok(list(vec![
                    text(o.evidence.as_str()),
                    text(o.event_id.as_str()),
                    text(o.task_id.as_str()),
                    o.execution_id.map_or(Value::Null, |id| text(id.as_str())),
                    Value::Unsigned(o.timestamp.unix_ms()),
                    Value::Unsigned(o.before),
                    Value::Unsigned(o.after),
                ]))
            }
            Request::Cancel(id) => {
                a.permission(Permission::RequestCancellation)?;
                let h = a.human.as_ref().ok_or(WebError::Forbidden)?;
                let outcome = match h {
                    HumanAuthority::Attested(h) => self.host.request_cancellation(h, &id)?,
                    HumanAuthority::DeviceOwner(h) => self.host.device_owner_cancel(h, &id)?,
                };
                match outcome {
                    CancellationOutcome::Requested(r) => Ok(list(vec![
                        text("requested"),
                        text(r.execution_id.as_str()),
                        text(r.receipt_event.as_str()),
                        text(r.verified_principal.as_str()),
                        Value::Unsigned(r.requested_at.unix_ms()),
                    ])),
                    CancellationOutcome::AlreadyTerminal(e) => Ok(list(vec![
                        text("terminal"),
                        text(e.fields().execution_id.as_str()),
                        text(match e.fields().state {
                            ExecutionState::Accepted => "accepted",
                            ExecutionState::Queued => "queued",
                            ExecutionState::Running => "running",
                            ExecutionState::CancelRequested => "cancelRequested",
                            ExecutionState::Succeeded => "succeeded",
                            ExecutionState::Failed => "failed",
                            ExecutionState::Cancelled => "cancelled",
                            ExecutionState::OutcomeUnknown => "outcomeUnknown",
                        }),
                    ])),
                }
            }
        }
    }

    pub(crate) fn console_session(&self) -> Result<(String, String), WebError> {
        let actor = self.actors.first().ok_or(WebError::Unauthorized)?;
        if Instant::now() >= actor.expires {
            return Err(WebError::Unauthorized);
        }
        Ok((
            actor.credentials.bearer.clone(),
            actor.credentials.csrf.clone(),
        ))
    }

    pub(crate) fn console_create(&mut self, actor: usize, delta: u64) -> Result<Value, WebError> {
        if !(1..=1_000_000).contains(&delta) {
            return Err(WebError::Malformed);
        }
        let id = |kind| eylema_platform::random_id(kind).map_err(|_| WebError::Entropy);
        let task_id =
            TaskId::new(id(eylema_platform::IdKind::Task)?).map_err(|_| WebError::Malformed)?;
        let context = RequestContext {
            request_id: RequestId::new(id(eylema_platform::IdKind::Request)?)
                .map_err(|_| WebError::Malformed)?,
            correlation_id: CorrelationId::new(id(eylema_platform::IdKind::Correlation)?)
                .map_err(|_| WebError::Malformed)?,
            contract_version: ContractVersion::CURRENT,
        };
        let scope = self
            .actors
            .get(actor)
            .ok_or(WebError::Unauthorized)?
            .grant
            .resource_scope
            .clone();
        let revision = Revision::new(1).map_err(|_| WebError::Malformed)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| WebError::Configuration)?
            .as_millis();
        let deadline = u64::try_from(now)
            .map_err(|_| WebError::Configuration)?
            .checked_add(60_000)
            .ok_or(WebError::Configuration)?;
        let budget = Budget::new(
            1,
            12,
            1024,
            1,
            Timestamp::new(deadline).map_err(|_| WebError::Malformed)?,
        )
        .map_err(|_| WebError::Malformed)?;
        let target = ExecutionTarget::new(
            ExecutionContext::NativeMacos,
            Backend::NativeCpu,
            GpuApi::None,
        )
        .map_err(|_| WebError::Malformed)?;
        let task = TaskSpec::new(TaskDraft {
            context,
            task_id: task_id.clone(),
            revision,
            objective: "Advance a bounded local task value".into(),
            resource_scope: scope,
            budget,
            target,
        })
        .map_err(|_| WebError::Malformed)?;
        self.dispatch(actor, Request::Create(task))?;
        self.actors[actor].last_console_execution = None;
        Ok(list(vec![text(task_id.as_str()), Value::Unsigned(delta)]))
    }

    pub(crate) fn console_run(
        &mut self,
        actor: usize,
        task_id: &str,
        delta: u64,
    ) -> Result<Value, WebError> {
        if !(1..=1_000_000).contains(&delta) {
            return Err(WebError::Malformed);
        }
        let task_id = TaskId::new(task_id.to_owned()).map_err(|_| WebError::Malformed)?;
        let snapshot = self.host.task(&self.actors[actor].session, &task_id)?;
        let task = snapshot.task;
        let d = task.fields();
        let id = |kind| eylema_platform::random_id(kind).map_err(|_| WebError::Entropy);
        let context = RequestContext {
            request_id: RequestId::new(id(eylema_platform::IdKind::Request)?)
                .map_err(|_| WebError::Malformed)?,
            correlation_id: CorrelationId::new(id(eylema_platform::IdKind::Correlation)?)
                .map_err(|_| WebError::Malformed)?,
            contract_version: ContractVersion::CURRENT,
        };
        let action = ActionSpec::new(ActionDraft {
            context: context.clone(),
            action_id: ActionId::new(id(eylema_platform::IdKind::Action)?)
                .map_err(|_| WebError::Malformed)?,
            task_id: task_id.clone(),
            operation_id: OperationId::new("addTaskValue").map_err(|_| WebError::Malformed)?,
            operation_version: ContractVersion::CURRENT,
            side_effect_class: SideEffectClass::RecordMutation,
            resource_scope: d.resource_scope.clone(),
            expected_revision: Some(d.revision),
            authority: AuthorityRequirement::new(
                self.actors[actor].grant.principal.clone(),
                self.actors[actor].grant.policy_revision,
                d.resource_scope.clone(),
                vec![Permission::ExecuteAction],
                OversightRequirements::new(vec![], true, false, None)
                    .map_err(|_| WebError::Malformed)?,
            )
            .map_err(|_| WebError::Malformed)?,
            required_capabilities: vec![
                CapabilityId::new("task:state").map_err(|_| WebError::Malformed)?,
            ],
            budget: d.budget,
            target: d.target,
            idempotency_key: IdempotencyKey::new(format!(
                "{}:console:{}",
                task_id.as_str(),
                id(eylema_platform::IdKind::Request)?
            ))
            .map_err(|_| WebError::Malformed)?,
            payload: codec::encode(&Value::Unsigned(delta)).map_err(|_| WebError::Malformed)?,
        })
        .map_err(|_| WebError::Malformed)?;
        let proposal = ActionProposal::new(
            ProposalDraft {
                context,
                proposal_id: ProposalId::new(id(eylema_platform::IdKind::Proposal)?)
                    .map_err(|_| WebError::Malformed)?,
                task_id: task_id.clone(),
                plan_revision: d.revision,
                state_revision: d.revision,
                policy_revision: self.actors[actor].grant.policy_revision,
                actions: vec![action],
            },
            &task,
        )
        .map_err(|_| WebError::Malformed)?;
        let execution = self.dispatch(
            actor,
            Request::Admit {
                task,
                proposal,
                approvals: vec![],
            },
        )?;
        if let Value::Text(ref value) = execution {
            self.actors[actor].last_console_execution =
                Some(ExecutionId::new(value.clone()).map_err(|_| WebError::Malformed)?);
        }
        Ok(execution)
    }
    /// One bounded connection and one authorized queued runtime step. Idle
    /// polling performs no durable mutation. Closing a client is not shutdown.
    pub fn poll(&mut self) -> Result<(), WebError> {
        match self.listener.accept() {
            Ok((stream, peer)) => {
                if peer.ip() == Ipv4Addr::LOCALHOST {
                    // Darwin accepted sockets inherit the listener nonblocking flag.
                    // Per-connection absolute read/write deadlines require blocking IO.
                    stream.set_nonblocking(false).map_err(|_| WebError::Io)?;
                    http::serve(self, stream);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return Err(WebError::Io),
        }
        if self.host.has_pending_work()? {
            self.host.step()?;
        }
        Ok(())
    }
    pub fn run(&mut self, stop: &AtomicBool) -> Result<(), WebError> {
        while !stop.load(Ordering::Acquire) {
            self.poll()?;
            std::thread::sleep(self.config.worker_interval);
        }
        Ok(())
    }
}
