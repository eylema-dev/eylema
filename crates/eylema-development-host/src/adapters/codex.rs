use super::{
    Proposal,
    json::{self, Json},
    proposal,
};
use crate::{CancelHandle, Error, Result, Workspace, files};
use eylema_development::{AuthorityResolver, Coordinator};
use eylema_platform::process_group::OwnedGroup;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const CODEX_VERSION: &str = "codex-cli 0.162.1";
pub const CODEX_SHA256: &str = "74c6a6d263f40d65f99c27a815275f5b4ed0bcc2baecc396b4cf3255498bdfc0";
pub const MODEL: &str = "gpt-6.1-sol";
pub const EFFORT: &str = "high";
const SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["summary","changes"],"properties":{"summary":{"type":"string"},"changes":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["path","text","deleted"],"properties":{"path":{"type":"string"},"text":{"type":"string"},"deleted":{"type":"boolean"}}}}}}"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    InstalledCodex,
    ExplicitFixture,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallStatus {
    Completed,
    Quota,
    ModelUnavailable,
    AuthenticationUnavailable,
    TransportFailed,
    MalformedOutput,
    UnexpectedTool,
    TimedOut,
    Cancelled,
    OutputLimit,
    AuthorityDenied,
    CoordinatorStopped(eylema_development::Error),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePath {
    Completed,
    Stopped,
    ProviderError,
    ProcessOrInput,
    IncompleteTurn,
    ProposalRejected,
    StreamRejected,
    OutputRejected,
    ToolRejected,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MessageCategory {
    #[default]
    NoError,
    QuotaMention,
    ModelMention,
    AuthenticationMention,
    SchemaMention,
    PermissionsMention,
    ConnectivityMention,
    StreamMention,
    ConfigurationMention,
    Unclassified,
}
/// Observed CLI turn aggregates, not individual backend response/billing records.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReportedUsage {
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cache_write_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_output_tokens: Option<u64>,
}
#[derive(Clone, Copy, Debug)]
pub struct CallDiagnostics {
    pub failure_path: FailurePath,
    pub child_exit_code: Option<i32>,
    pub child_exit_success: bool,
    pub stdin_write_success: bool,
    pub completed_turn_seen: bool,
    pub final_message_seen: bool,
    pub provider_error_seen: bool,
    pub message_category: MessageCategory,
    pub reported_usage: Option<ReportedUsage>,
    pub invalid_usage_seen: bool,
}
#[derive(Debug)]
pub struct CallOutcome {
    pub status: CallStatus,
    pub proposal: Option<Proposal>,
    pub reported_total_tokens: Option<u64>,
    pub elapsed_ms: u64,
    pub transport: Transport,
    pub rejected_item_kind: Option<String>,
    pub diagnostics: Option<CallDiagnostics>,
}
#[derive(Clone, Copy)]
pub struct CallLimits {
    pub wall_ms: u64,
    pub output_bytes: usize,
}
pub struct CodexRequest<'a> {
    pub prompt: &'a str,
    pub limits: CallLimits,
    pub cancel: &'a CancelHandle,
    /// Deliberate negative acceptance probe; never a fallback model selection.
    pub unavailable_probe: bool,
}
impl CodexRequest<'_> {
    pub fn requested_model(&self) -> &'static str {
        if self.unavailable_probe {
            "gpt-6.1-sol-g40-unavailable"
        } else {
            MODEL
        }
    }
}
pub struct CodexTool {
    program: PathBuf,
    user_home: PathBuf,
    auth_home: PathBuf,
    transport: Transport,
    sha256: String,
}
impl CodexTool {
    /// Operator supplies trusted installed paths. Task/provider text cannot.
    pub fn installed(program: &Path, user_home: &Path, auth_home: &Path) -> Result<Self> {
        let mut tool = Self::paths(program, user_home, auth_home, Transport::InstalledCodex)?;
        tool.sha256 = tool.hash()?;
        if tool.sha256 != CODEX_SHA256 {
            return Err(Error::InvalidInput);
        }
        tool.version(CODEX_VERSION)?;
        Ok(tool)
    }
    /// Distinct synthetic transport; cannot claim installed-provider evidence.
    pub fn fixture(program: &Path, home: &Path) -> Result<Self> {
        let mut tool = Self::paths(program, home, home, Transport::ExplicitFixture)?;
        tool.sha256 = tool.hash()?;
        tool.version("g40-provider-fixture/1")?;
        Ok(tool)
    }
    fn paths(
        program: &Path,
        user_home: &Path,
        auth_home: &Path,
        transport: Transport,
    ) -> Result<Self> {
        if !cfg!(target_os = "macos") {
            return Err(Error::Unsupported);
        }
        let program = fs::canonicalize(program)?;
        files::regular(&program)?;
        let user_home = fs::canonicalize(user_home)?;
        let auth_home = fs::canonicalize(auth_home)?;
        if !user_home.is_dir() || !auth_home.is_dir() {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            program,
            user_home,
            auth_home,
            transport,
            sha256: String::new(),
        })
    }
    fn hash(&self) -> Result<String> {
        files::regular(&self.program)?;
        let mut command = Command::new("/usr/bin/shasum");
        command
            .env_clear()
            .args(["-a", "256", "--"])
            .arg(&self.program);
        let output = crate::process::host_output(&mut command, 20_000, 4096, 4096)?;
        let digest = output.stdout.get(..64).ok_or(Error::InvalidInput)?;
        if !output.status.success()
            || !digest.iter().all(u8::is_ascii_hexdigit)
            || output.stdout.get(64) != Some(&b' ')
        {
            return Err(Error::InvalidInput);
        }
        String::from_utf8(digest.to_vec()).map_err(|_| Error::InvalidInput)
    }
    fn revalidate(&self) -> Result<()> {
        if self.hash()? != self.sha256 {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    fn chatgpt_session(&self) -> Result<bool> {
        let mut command = self.command();
        command.args(["-c", "forced_login_method=\"chatgpt\"", "login", "status"]);
        let output = crate::process::host_output(&mut command, 20_000, 4096, 4096)?;
        let mut status = output.stdout;
        status.extend(output.stderr);
        Ok(output.status.success() && status.trim_ascii() == b"Logged in using ChatGPT")
    }
    fn filesystem_policy(&self, workspace: &Workspace) -> Result<String> {
        use std::collections::BTreeSet;
        let mut denied: BTreeSet<PathBuf> = [
            "/Users",
            "/Volumes",
            "/tmp",
            "/private/tmp",
            "/var/folders",
            "/private/var/folders",
            "/System/Volumes/Data/Users",
            "/System/Volumes/Data/Volumes",
            "/System/Volumes/Data/private/tmp",
            "/System/Volumes/Data/private/var/folders",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        denied.extend([
            self.user_home.clone(),
            self.auth_home.clone(),
            workspace
                .work
                .parent()
                .and_then(Path::parent)
                .ok_or(Error::Scope)?
                .to_path_buf(),
        ]);
        let mut policy = String::from("permissions.g40.filesystem={\"/\"=\"read\"");
        for path in denied {
            let text = path.to_str().ok_or(Error::InvalidInput)?;
            if text.chars().any(char::is_control) {
                return Err(Error::InvalidInput);
            }
            policy.push_str(&format!(
                ",\"{}\"=\"deny\"",
                text.replace('\\', "\\\\").replace('\"', "\\\"")
            ));
        }
        policy.push_str(",\":workspace_roots\"={\".\"=\"read\"}}");
        Ok(policy)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.user_home)
            .env("CODEX_HOME", &self.auth_home)
            .env("LANG", "C");
        command
    }
    fn version(&self, expected: &str) -> Result<()> {
        let mut command = self.command();
        command.arg("--version");
        let output = crate::process::host_output(&mut command, 20_000, 4096, 4096)?;
        if !output.status.success() || output.stdout != format!("{expected}\n").as_bytes() {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    /// Actual local CLI policy controls, without a provider request or secret read.
    pub fn check_permissions(&self, workspace: &Workspace) -> Result<()> {
        if self.transport != Transport::InstalledCodex {
            return Err(Error::InvalidInput);
        }
        self.revalidate()?;
        let schema = prepare_schema(workspace)?;
        let private = schema.parent().ok_or(Error::Scope)?;
        let outside = workspace
            .work
            .parent()
            .ok_or(Error::Scope)?
            .join("provider-permission-canary.txt");
        match fs::symlink_metadata(&outside) {
            Ok(_) => files::regular(&outside)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        fs::write(&outside, b"G40 synthetic protected canary\n")?;
        for (path, allowed) in [(&schema, true), (&outside, false)] {
            let mut command = self.command();
            command
                .args([
                    "-c",
                    "default_permissions=\"g40\"",
                    "-c",
                    &self.filesystem_policy(workspace)?,
                    "-c",
                    "permissions.g40.network.enabled=false",
                    "sandbox",
                    "-P",
                    "g40",
                    "-C",
                ])
                .arg(private)
                .args(["--", "/bin/cat"])
                .arg(path);
            let output = crate::process::host_output(&mut command, 20_000, 8192, 8192)?;
            let valid = if allowed {
                output.status.success() && output.stdout == SCHEMA.as_bytes()
            } else {
                output.status.code() == Some(1)
                    && output.stdout.is_empty()
                    && output
                        .stderr
                        .windows(b"Operation not permitted".len())
                        .any(|v| v == b"Operation not permitted")
            };
            if !valid {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
    }
    pub fn invoke(
        &self,
        workspace: &mut Workspace,
        run: &mut Coordinator,
        authority: &impl AuthorityResolver,
        mut now: impl FnMut() -> u64,
        request: CodexRequest<'_>,
    ) -> Result<CallOutcome> {
        let epoch = now();
        workspace.bind(run, epoch, authority)?;
        if request.prompt.is_empty()
            || request.prompt.len() > 16384
            || request.prompt.contains('\0')
            || !(1..=60_000).contains(&request.limits.wall_ms)
            || !(1..=1024 * 1024).contains(&request.limits.output_bytes)
        {
            return Err(Error::InvalidInput);
        }
        if request.cancel.is_cancelled() {
            run.cancel(epoch)?;
            return Ok(CallOutcome {
                status: CallStatus::Cancelled,
                proposal: None,
                reported_total_tokens: None,
                elapsed_ms: 0,
                transport: self.transport,
                rejected_item_kind: None,
                diagnostics: None,
            });
        }
        for path in [&self.program, &self.auth_home, &self.user_home] {
            if path.starts_with(&workspace.work) || path.starts_with(&workspace.scratch) {
                return Err(Error::Scope);
            }
        }
        self.revalidate()?;
        if !self.chatgpt_session()? {
            return Ok(CallOutcome {
                status: CallStatus::AuthenticationUnavailable,
                proposal: None,
                reported_total_tokens: None,
                elapsed_ms: 0,
                transport: self.transport,
                rejected_item_kind: None,
                diagnostics: None,
            });
        }
        files::tree(&workspace.work, 100_000)?;
        let schema = prepare_schema(workspace)?;
        let private = schema.parent().ok_or(Error::Scope)?;
        if self.transport == Transport::InstalledCodex {
            self.check_permissions(workspace)?;
        }
        let mut command = self.command();
        command.args([
            "--no-daemon",
            "-a",
            "never",
            "exec",
            "--strict-config",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--skip-git-repo-check",
            "--json",
            "--color",
            "never",
        ]);
        command.arg("--model").arg(request.requested_model());
        for flag in [
            "shell_tool",
            "unified_exec",
            "code_mode_host",
            "apps",
            "plugins",
            "hooks",
            "multi_agent",
            "goals",
            "shell_snapshot",
            "browser_use",
            "browser_use_external",
            "browser_use_full_cdp_access",
            "computer_use",
            "in_app_browser",
            "browser_annotation_api",
            "skill_search",
            "tool_suggest",
            "sleep_tool",
            "tool_call_mcp_elicitation",
        ] {
            command.args(["--disable", flag]);
        }
        for setting in [
            "model_reasoning_effort=\"high\"",
            "model_provider=\"openai\"",
            "forced_login_method=\"chatgpt\"",
            "web_search=\"disabled\"",
            "project_doc_max_bytes=0",
            "default_permissions=\"g40\"",
            "permissions.g40.network.enabled=false",
        ] {
            command.args(["-c", setting]);
        }
        command.args(["-c", &self.filesystem_policy(workspace)?]);
        command
            .arg("--output-schema")
            .arg(&schema)
            .arg("-C")
            .arg(private)
            .arg("-")
            .current_dir(private)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Usage units here are admitted CLI deadline milliseconds plus cleanup
        // allowance, not money/tokens or a count of hidden backend requests.
        // Trusted operator may still replace a path between this check and spawn.
        // No atomic executable binding or protection from a hostile operator is claimed.
        self.revalidate()?;
        let ticket = run.reserve_call(request.limits.wall_ms + 2000, epoch, authority)?;
        let mut acknowledged_epoch = epoch;
        let mut tracked_clock = || {
            let observed = now();
            acknowledged_epoch = acknowledged_epoch.max(observed);
            observed
        };
        let observed = execute(
            &mut command,
            workspace,
            run,
            authority,
            &mut tracked_clock,
            request,
        );
        // A failed clock is reported in the outcome. Acknowledge only at the last
        // validated epoch; never refund or advance verification/review readiness.
        let result = match observed {
            Ok((outcome, _)) => {
                if outcome.status == CallStatus::Cancelled {
                    run.cancel(acknowledged_epoch)?;
                }
                run.finish_call(&ticket, acknowledged_epoch)?;
                Ok(outcome)
            }
            Err(error) => {
                run.finish_call(&ticket, acknowledged_epoch)?;
                Err(error)
            }
        };
        result.map(|mut result| {
            result.transport = self.transport;
            result
        })
    }
}

fn prepare_schema(workspace: &Workspace) -> Result<PathBuf> {
    let private = workspace
        .work
        .parent()
        .ok_or(Error::Scope)?
        .join("provider");
    match fs::create_dir(&private) {
        Ok(()) => {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::symlink_metadata(&private)?.is_symlink() || !private.is_dir() {
                return Err(Error::Scope);
            }
        }
        Err(error) => return Err(error.into()),
    }
    let schema = private.join("proposal-schema.json");
    match fs::symlink_metadata(&schema) {
        Ok(_) => files::regular(&schema)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::write(&schema, SCHEMA)?;
    Ok(schema)
}

struct CaptureFlags {
    bytes: AtomicUsize,
    overflow: AtomicBool,
    malformed: AtomicBool,
    tool: AtomicBool,
    rejected_kind: Mutex<Option<String>>,
}
fn capture(
    mut pipe: impl Read + Send + 'static,
    flags: Arc<CaptureFlags>,
    limit: usize,
    inspect: bool,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut pending = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = pipe.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            let before = flags.bytes.fetch_add(count, Ordering::AcqRel);
            let retain = limit.saturating_sub(before).min(count);
            kept.extend_from_slice(&buffer[..retain]);
            if before.saturating_add(count) > limit {
                flags.overflow.store(true, Ordering::Release);
            }
            if inspect && !flags.overflow.load(Ordering::Acquire) {
                pending.extend_from_slice(&buffer[..count]);
                while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                    inspect_event(&pending[..end], &flags);
                    pending.drain(..=end);
                }
            }
        }
        if inspect && !pending.is_empty() {
            inspect_event(&pending, &flags);
        }
        Ok(kept)
    })
}
fn inspect_event(data: &[u8], flags: &CaptureFlags) {
    let Ok(value) = json::parse(data) else {
        flags.malformed.store(true, Ordering::Release);
        return;
    };
    let Some(kind) = value.field("type").and_then(Json::text) else {
        flags.malformed.store(true, Ordering::Release);
        return;
    };
    match kind {
        "thread.started" | "turn.started" | "turn.completed" | "turn.failed" | "error" => {}
        "item.started" | "item.updated" | "item.completed" => {
            let item = value
                .field("item")
                .and_then(|item| item.field("type"))
                .and_then(Json::text);
            if !matches!(item, Some("agent_message" | "reasoning" | "error")) {
                if let Ok(mut rejected) = flags.rejected_kind.lock() {
                    let safe = item
                        .filter(|v| {
                            v.len() <= 64
                                && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        })
                        .unwrap_or("unknown");
                    rejected.get_or_insert_with(|| safe.to_owned());
                }
                flags.tool.store(true, Ordering::Release);
            }
        }
        _ => {
            flags.malformed.store(true, Ordering::Release);
        }
    }
}
fn execute(
    command: &mut Command,
    workspace: &Workspace,
    run: &mut Coordinator,
    authority: &impl AuthorityResolver,
    now: &mut impl FnMut() -> u64,
    request: CodexRequest<'_>,
) -> Result<(CallOutcome, u64)> {
    let mut valid_epoch = now();
    workspace.bind(run, valid_epoch, authority)?;
    let mut group = OwnedGroup::spawn(command)?;
    let mut input = group.take_stdin().ok_or(Error::InvalidState)?;
    let prompt = request.prompt.as_bytes().to_vec();
    let writer = thread::spawn(move || input.write_all(&prompt));
    let flags = Arc::new(CaptureFlags {
        bytes: AtomicUsize::new(0),
        overflow: AtomicBool::new(false),
        malformed: AtomicBool::new(false),
        tool: AtomicBool::new(false),
        rejected_kind: Mutex::new(None),
    });
    let out = capture(
        group.take_stdout().ok_or(Error::InvalidState)?,
        flags.clone(),
        request.limits.output_bytes,
        true,
    );
    let err = capture(
        group.take_stderr().ok_or(Error::InvalidState)?,
        flags.clone(),
        request.limits.output_bytes,
        false,
    );
    let started = Instant::now();
    let mut stopped = None;
    let exit = loop {
        let epoch = now();
        let cause = match workspace.bind(run, epoch, authority) {
            Ok(()) => {
                valid_epoch = epoch;
                None
            }
            Err(Error::Coordinator(eylema_development::Error::Forbidden)) => {
                Some(CallStatus::AuthorityDenied)
            }
            Err(Error::Coordinator(cause)) => Some(CallStatus::CoordinatorStopped(cause)),
            Err(_) => Some(CallStatus::CoordinatorStopped(
                eylema_development::Error::InvalidState,
            )),
        };
        let stop = if request.cancel.is_cancelled() {
            Some(CallStatus::Cancelled)
        } else if flags.overflow.load(Ordering::Acquire) {
            Some(CallStatus::OutputLimit)
        } else if flags.tool.load(Ordering::Acquire) {
            Some(CallStatus::UnexpectedTool)
        } else if flags.malformed.load(Ordering::Acquire) {
            Some(CallStatus::MalformedOutput)
        } else if started.elapsed() >= Duration::from_millis(request.limits.wall_ms) {
            Some(CallStatus::TimedOut)
        } else {
            cause
        };
        if let Some(status) = stop {
            stopped = Some(status);
            break group.stop()?.ok_or(Error::InvalidState)?;
        }
        if let Some(status) = group.poll()? {
            break status;
        }
        thread::sleep(Duration::from_millis(2));
    };
    let stdout = out.join().map_err(|_| Error::InvalidState)??;
    let _private_stderr = err.join().map_err(|_| Error::InvalidState)??;
    let wrote = writer.join().map_err(|_| Error::InvalidState)?;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let (mut status, mut proposed, mut usage) = if let Some(stop) = stopped {
        (stop, None, None)
    } else {
        decode(
            &stdout,
            &workspace.need.draft().allowed_paths,
            exit.success() && wrote.is_ok(),
        )
    };
    if stopped.is_none() && flags.overflow.load(Ordering::Acquire) {
        status = CallStatus::OutputLimit;
        proposed = None;
        usage = None;
    } else if stopped.is_none() && flags.tool.load(Ordering::Acquire) {
        status = CallStatus::UnexpectedTool;
        proposed = None;
    } else if stopped.is_none() && flags.malformed.load(Ordering::Acquire) {
        status = CallStatus::MalformedOutput;
        proposed = None;
    }
    let diagnostics = diagnose(
        &stdout,
        status,
        stopped.is_some(),
        flags.malformed.load(Ordering::Acquire),
        exit.code(),
        exit.success(),
        wrote.is_ok(),
    );
    Ok((
        CallOutcome {
            status,
            proposal: proposed,
            reported_total_tokens: usage,
            elapsed_ms,
            transport: Transport::ExplicitFixture,
            diagnostics: Some(diagnostics),
            rejected_item_kind: flags
                .rejected_kind
                .lock()
                .map_err(|_| Error::InvalidState)?
                .clone(),
        },
        valid_epoch,
    ))
}
fn message_category(text: &str) -> MessageCategory {
    match failure_status(text) {
        CallStatus::Quota => return MessageCategory::QuotaMention,
        CallStatus::ModelUnavailable => return MessageCategory::ModelMention,
        CallStatus::AuthenticationUnavailable => return MessageCategory::AuthenticationMention,
        _ => {}
    }
    let text = text.to_ascii_lowercase();
    if text.contains("schema") || text.contains("response_format") {
        MessageCategory::SchemaMention
    } else if text.contains("permission")
        || text.contains("sandbox")
        || text.contains("not permitted")
    {
        MessageCategory::PermissionsMention
    } else if text.contains("stream") {
        MessageCategory::StreamMention
    } else if text.contains("connect")
        || text.contains("network")
        || text.contains("dns")
        || text.contains("timed out")
    {
        MessageCategory::ConnectivityMention
    } else if text.contains("config") || text.contains("feature") || text.contains("setting") {
        MessageCategory::ConfigurationMention
    } else {
        MessageCategory::Unclassified
    }
}
fn diagnose(
    data: &[u8],
    status: CallStatus,
    stopped: bool,
    stream_rejected: bool,
    exit_code: Option<i32>,
    exit_success: bool,
    wrote: bool,
) -> CallDiagnostics {
    let mut d = CallDiagnostics {
        failure_path: FailurePath::StreamRejected,
        child_exit_code: exit_code,
        child_exit_success: exit_success,
        stdin_write_success: wrote,
        completed_turn_seen: false,
        final_message_seen: false,
        provider_error_seen: false,
        message_category: MessageCategory::NoError,
        reported_usage: None,
        invalid_usage_seen: false,
    };
    for line in data.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let Ok(value) = json::parse(line) else {
            continue;
        };
        let mut error_text = None;
        match value.field("type").and_then(Json::text) {
            Some("item.completed") => {
                if let Some(item) = value.field("item") {
                    match item.field("type").and_then(Json::text) {
                        Some("agent_message") => {
                            d.final_message_seen |=
                                item.field("text").and_then(Json::text).is_some()
                        }
                        Some("error") => {
                            d.provider_error_seen = true;
                            error_text =
                                Some(item.field("message").and_then(Json::text).unwrap_or(""));
                        }
                        _ => {}
                    }
                }
            }
            Some("turn.completed") => {
                d.completed_turn_seen = true;
                if let Some(tokens) = value.field("usage") {
                    let observed = ReportedUsage {
                        input_tokens: tokens.field("input_tokens").and_then(Json::unsigned),
                        cached_input_tokens: tokens
                            .field("cached_input_tokens")
                            .and_then(Json::unsigned),
                        cache_write_input_tokens: tokens
                            .field("cache_write_input_tokens")
                            .and_then(Json::unsigned),
                        output_tokens: tokens.field("output_tokens").and_then(Json::unsigned),
                        reasoning_output_tokens: tokens
                            .field("reasoning_output_tokens")
                            .and_then(Json::unsigned),
                    };
                    let bad_input = observed
                        .input_tokens
                        .zip(observed.cached_input_tokens)
                        .is_some_and(|(a, b)| b > a)
                        || observed
                            .input_tokens
                            .zip(observed.cache_write_input_tokens)
                            .is_some_and(|(a, b)| b > a);
                    let bad_cache_sum = observed
                        .input_tokens
                        .zip(
                            observed
                                .cached_input_tokens
                                .zip(observed.cache_write_input_tokens),
                        )
                        .is_some_and(|(input, (read, write))| {
                            read.checked_add(write).is_none_or(|sum| sum > input)
                        });
                    let bad_type = [
                        "input_tokens",
                        "cached_input_tokens",
                        "cache_write_input_tokens",
                        "output_tokens",
                        "reasoning_output_tokens",
                    ]
                    .into_iter()
                    .any(|name| tokens.field(name).is_some_and(|v| v.unsigned().is_none()));
                    let bad_output = observed
                        .output_tokens
                        .zip(observed.reasoning_output_tokens)
                        .is_some_and(|(a, b)| b > a);
                    let bad_sum = observed
                        .input_tokens
                        .zip(observed.output_tokens)
                        .is_some_and(|(a, b)| a.checked_add(b).is_none());
                    if bad_input || bad_output || bad_sum || bad_cache_sum || bad_type {
                        d.invalid_usage_seen = true;
                        d.reported_usage = None;
                    } else {
                        d.reported_usage = Some(observed);
                    }
                }
            }
            Some("turn.failed" | "error") => {
                d.provider_error_seen = true;
                error_text = Some(
                    value
                        .field("error")
                        .and_then(|v| v.field("message"))
                        .or_else(|| value.field("message"))
                        .and_then(Json::text)
                        .unwrap_or(""),
                );
            }
            _ => {}
        }
        if let Some(text) = error_text {
            d.message_category = message_category(text);
        }
    }
    d.failure_path = if stopped {
        FailurePath::Stopped
    } else if status == CallStatus::UnexpectedTool {
        FailurePath::ToolRejected
    } else if status == CallStatus::OutputLimit {
        FailurePath::OutputRejected
    } else if stream_rejected {
        FailurePath::StreamRejected
    } else if d.provider_error_seen {
        FailurePath::ProviderError
    } else if !exit_success || !wrote {
        FailurePath::ProcessOrInput
    } else if !d.completed_turn_seen {
        FailurePath::IncompleteTurn
    } else if status == CallStatus::MalformedOutput {
        FailurePath::ProposalRejected
    } else if status == CallStatus::Completed {
        FailurePath::Completed
    } else {
        FailurePath::StreamRejected
    };
    d
}
fn failure_status(text: &str) -> CallStatus {
    let lower = text.to_ascii_lowercase();
    if lower.contains("usage limit") || lower.contains("rate limit") || lower.contains("quota") {
        CallStatus::Quota
    } else if lower.contains("model")
        && (lower.contains("not supported")
            || lower.contains("not found")
            || lower.contains("does not exist")
            || lower.contains("unavailable"))
    {
        CallStatus::ModelUnavailable
    } else if lower.contains("not logged in")
        || lower.contains("authentication")
        || lower.contains("unauthorized")
        || lower.contains("401")
    {
        CallStatus::AuthenticationUnavailable
    } else {
        CallStatus::TransportFailed
    }
}

fn decode(
    data: &[u8],
    scope: &[String],
    success: bool,
) -> (CallStatus, Option<Proposal>, Option<u64>) {
    let mut final_text = None;
    let mut completed = false;
    let mut failure = None;
    let mut usage = None;
    for line in data.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let Ok(value) = json::parse(line) else {
            return (CallStatus::MalformedOutput, None, None);
        };
        match value.field("type").and_then(Json::text) {
            Some("item.completed") => {
                if let Some(item) = value.field("item")
                    && item.field("type").and_then(Json::text) == Some("error")
                {
                    failure = Some(failure_status(
                        item.field("message").and_then(Json::text).unwrap_or(""),
                    ));
                }
                if let Some(item) = value.field("item")
                    && item.field("type").and_then(Json::text) == Some("agent_message")
                {
                    final_text = item.field("text").and_then(Json::text).map(str::to_owned);
                }
            }
            Some("turn.completed") => {
                completed = true;
                if let Some(tokens) = value.field("usage") {
                    usage = tokens
                        .field("input_tokens")
                        .and_then(Json::unsigned)
                        .zip(tokens.field("output_tokens").and_then(Json::unsigned))
                        .and_then(|(input, output)| input.checked_add(output));
                }
            }
            Some("turn.failed" | "error") => {
                let text = value
                    .field("error")
                    .and_then(|v| v.field("message"))
                    .or_else(|| value.field("message"))
                    .and_then(Json::text)
                    .unwrap_or("");
                failure = Some(failure_status(text));
            }
            _ => {}
        }
    }
    if let Some(failure) = failure {
        return (failure, None, usage);
    }
    if !success || !completed {
        return (CallStatus::TransportFailed, None, usage);
    }
    match final_text.and_then(|text| proposal(text.as_bytes(), scope).ok()) {
        Some(proposal) => (CallStatus::Completed, Some(proposal), usage),
        None => (CallStatus::MalformedOutput, None, usage),
    }
}
