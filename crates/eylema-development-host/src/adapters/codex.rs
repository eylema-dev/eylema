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
        Arc,
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
#[derive(Debug)]
pub struct CallOutcome {
    pub status: CallStatus,
    pub proposal: Option<Proposal>,
    pub reported_total_tokens: Option<u64>,
    pub elapsed_ms: u64,
    pub transport: Transport,
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
pub struct CodexTool {
    program: PathBuf,
    user_home: PathBuf,
    auth_home: PathBuf,
    transport: Transport,
}
impl CodexTool {
    /// Operator supplies trusted installed paths. Task/provider text cannot.
    pub fn installed(program: &Path, user_home: &Path, auth_home: &Path) -> Result<Self> {
        let tool = Self::paths(program, user_home, auth_home, Transport::InstalledCodex)?;
        let mut hash = Command::new("/usr/bin/shasum");
        hash.env_clear()
            .args(["-a", "256", "--"])
            .arg(&tool.program);
        let output = crate::process::host_output(&mut hash, 20_000, 4096, 4096)?;
        if !output.status.success()
            || !output
                .stdout
                .starts_with(format!("{CODEX_SHA256} ").as_bytes())
        {
            return Err(Error::InvalidInput);
        }
        tool.version(CODEX_VERSION)?;
        Ok(tool)
    }
    /// Distinct synthetic transport; cannot claim installed-provider evidence.
    pub fn fixture(program: &Path, home: &Path) -> Result<Self> {
        let tool = Self::paths(program, home, home, Transport::ExplicitFixture)?;
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
        })
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
            });
        }
        for path in [&self.program, &self.auth_home, &self.user_home] {
            if path.starts_with(&workspace.work) || path.starts_with(&workspace.scratch) {
                return Err(Error::Scope);
            }
        }
        files::tree(&workspace.work, 100_000)?;
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
        command.arg("--model").arg(if request.unavailable_probe {
            "gpt-6.1-sol-g40-unavailable"
        } else {
            MODEL
        });
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
            "web_search=\"disabled\"",
            "project_doc_max_bytes=0",
            "default_permissions=\"g40\"",
            "permissions.g40.filesystem.\"/\"=\"deny\"",
            "permissions.g40.filesystem.\":workspace_roots\".\".\"=\"read\"",
            "permissions.g40.network.enabled=false",
        ] {
            command.args(["-c", setting]);
        }
        command
            .arg("--output-schema")
            .arg(&schema)
            .arg("-C")
            .arg(&private)
            .arg("-")
            .current_dir(&private)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Usage units here are admitted CLI deadline milliseconds plus cleanup
        // allowance, not money/tokens or a count of hidden backend requests.
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

struct CaptureFlags {
    bytes: AtomicUsize,
    overflow: AtomicBool,
    malformed: AtomicBool,
    tool: AtomicBool,
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
            if !matches!(item, Some("agent_message" | "reasoning")) {
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
    Ok((
        CallOutcome {
            status,
            proposal: proposed,
            reported_total_tokens: usage,
            elapsed_ms,
            transport: Transport::ExplicitFixture,
        },
        valid_epoch,
    ))
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
                let lower = text.to_ascii_lowercase();
                failure = Some(
                    if lower.contains("usage limit")
                        || lower.contains("rate limit")
                        || lower.contains("quota")
                    {
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
                    },
                );
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
