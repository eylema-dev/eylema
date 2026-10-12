use crate::{Error, Result, Workspace, files};
use eylema_development::{AuthorityResolver, Coordinator};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct ProcessLimits {
    pub wall_ms: u64,
    pub output_bytes: usize,
}
impl ProcessLimits {
    fn validate(self) -> Result<()> {
        if self.wall_ms == 0
            || self.wall_ms > 60_000
            || self.output_bytes == 0
            || self.output_bytes > 1024 * 1024
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct CancelHandle(Arc<AtomicBool>);
impl CancelHandle {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    OutputLimit,
    AuthorityDenied,
    CoordinatorStopped(eylema_development::Error),
}
#[derive(Debug)]
pub struct WorkerResult {
    pub reason: StopReason,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
}

/// Trusted host-owned worker configuration; no environment or sandbox override.
pub struct WorkerSpec<'a> {
    pub executable: &'a Path,
    pub args: &'a [String],
    pub limits: ProcessLimits,
    pub cancel: &'a CancelHandle,
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn quoted(path: &Path) -> Result<String> {
    let p = path.to_str().ok_or(Error::InvalidInput)?;
    if p.chars().any(|c| c.is_control()) {
        return Err(Error::InvalidInput);
    }
    Ok(format!(
        "\"{}\"",
        p.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
fn reader(
    mut input: impl Read + Send + 'static,
    bytes: Arc<AtomicUsize>,
    overflow: Arc<AtomicBool>,
    limit: usize,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let before = bytes.fetch_add(n, Ordering::AcqRel);
            let retain = limit.saturating_sub(before).min(n);
            kept.extend_from_slice(&buf[..retain]);
            if before.saturating_add(n) > limit {
                overflow.store(true, Ordering::Release);
            }
        }
        Ok(kept)
    })
}
impl Workspace {
    fn profile(&self, executable: &Path) -> Result<String> {
        let mut p = format!(
            "(version 1)\n(deny default)\n(allow process-exec (literal {}))\n(allow file-read-metadata)\n(allow sysctl-read (sysctl-name \"hw.pagesize\") (sysctl-name \"hw.pagesize_compat\") (sysctl-name \"kern.usrstack64\"))\n(allow file-read* (literal \"/\") (subpath \"/usr/lib\") (subpath \"/System/Library\") (subpath \"/System/Volumes/Preboot/Cryptexes/OS\") (literal {}) (subpath {}) (subpath {}))\n(allow file-write* (subpath {}))\n",
            quoted(executable)?,
            quoted(executable)?,
            quoted(&self.work)?,
            quoted(&self.scratch)?,
            quoted(&self.scratch)?
        );
        for path in &self.need.draft().allowed_paths {
            let kind = if path.ends_with('/') {
                "subpath"
            } else {
                "literal"
            };
            p.push_str(&format!(
                "(allow file-write* ({kind} {}))\n",
                quoted(&self.work.join(path.trim_end_matches('/')))?
            ));
        }
        // Explicit deny overrides scoped write aliases; hardlinks can bypass lexical scope.
        p.push_str(&format!(
            "(deny file-link)\n(deny file-read* file-write* (literal {}))\n",
            quoted(&self.work.join(".git"))?
        ));
        Ok(p)
    }
    /// A trusted host clock uses the coordinator's monotonic epoch. Task output is data.
    /// No custom environment, shell command, arbitrary PID or sandbox override is accepted.
    pub fn run_single(
        &mut self,
        run: &mut Coordinator,
        authority: &impl AuthorityResolver,
        mut now: impl FnMut() -> u64,
        worker: WorkerSpec<'_>,
    ) -> Result<WorkerResult> {
        let WorkerSpec {
            executable,
            args,
            limits,
            cancel,
        } = worker;
        limits.validate()?;
        self.bind(run, now(), authority)?;
        if args.len() > 64 || args.iter().any(|s| s.len() > 4096 || s.contains('\0')) {
            return Err(Error::InvalidInput);
        }
        files::tree(&self.work, 100_000)?;
        let executable = fs::canonicalize(executable)?;
        files::regular(&executable)?;
        if executable.starts_with(&self.work) || executable.starts_with(&self.scratch) {
            return Err(Error::Scope);
        }
        if !cfg!(target_os = "macos") || !Path::new("/usr/bin/sandbox-exec").is_file() {
            return Err(Error::Unsupported);
        }
        let profile = self.profile(&executable)?;
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .env_clear()
            .env("HOME", &self.scratch)
            .env("TMPDIR", &self.scratch)
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .args(["-p", &profile])
            .arg(&executable)
            .args(args)
            .current_dir(&self.work)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = OwnedChild(command.spawn()?);
        let bytes = Arc::new(AtomicUsize::new(0));
        let overflow = Arc::new(AtomicBool::new(false));
        let out = reader(
            child.0.stdout.take().ok_or(Error::InvalidState)?,
            bytes.clone(),
            overflow.clone(),
            limits.output_bytes,
        );
        let err = reader(
            child.0.stderr.take().ok_or(Error::InvalidState)?,
            bytes,
            overflow.clone(),
            limits.output_bytes,
        );
        let started = Instant::now();
        let reason;
        let status;
        loop {
            // Denial/revocation/clock regression all stop the exact unreaped child.
            let stop = if cancel.0.load(Ordering::Acquire) {
                Some(StopReason::Cancelled)
            } else if overflow.load(Ordering::Acquire) {
                Some(StopReason::OutputLimit)
            } else if started.elapsed() >= Duration::from_millis(limits.wall_ms) {
                Some(StopReason::TimedOut)
            } else {
                match self.bind(run, now(), authority) {
                    Ok(()) => None,
                    Err(Error::Coordinator(eylema_development::Error::Forbidden)) => {
                        Some(StopReason::AuthorityDenied)
                    }
                    Err(Error::Coordinator(cause)) => Some(StopReason::CoordinatorStopped(cause)),
                    Err(_) => Some(StopReason::CoordinatorStopped(
                        eylema_development::Error::InvalidState,
                    )),
                }
            };
            if let Some(stopped) = stop {
                child.0.kill()?;
                status = child.0.wait()?;
                reason = stopped;
                break;
            }
            if let Some(exit) = child.0.try_wait()? {
                status = exit;
                reason = if status.success() {
                    StopReason::Completed
                } else {
                    StopReason::Failed
                };
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        let stdout = out.join().map_err(|_| Error::InvalidState)??;
        let stderr = err.join().map_err(|_| Error::InvalidState)??;
        let reason = if overflow.load(Ordering::Acquire) {
            StopReason::OutputLimit
        } else {
            reason
        };
        Ok(WorkerResult {
            reason,
            stdout,
            stderr,
            exit_code: status.code(),
        })
    }
}

// Trusted control-host commands share finite capture/reaping behavior, but never
// reuse the untrusted worker's sandbox or accept provider-supplied commands.
pub(crate) fn host_output(
    command: &mut Command,
    wall_ms: u64,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<std::process::Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = OwnedChild(command.spawn()?);
    let overflow = Arc::new(AtomicBool::new(false));
    let out = reader(
        child.0.stdout.take().ok_or(Error::InvalidState)?,
        Arc::new(AtomicUsize::new(0)),
        overflow.clone(),
        stdout_limit,
    );
    let err = reader(
        child.0.stderr.take().ok_or(Error::InvalidState)?,
        Arc::new(AtomicUsize::new(0)),
        overflow.clone(),
        stderr_limit,
    );
    let started = Instant::now();
    let mut limited = false;
    let status = loop {
        if overflow.load(Ordering::Acquire) || started.elapsed() >= Duration::from_millis(wall_ms) {
            limited = true;
            child.0.kill()?;
            break child.0.wait()?;
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(2));
    };
    let stdout = out.join().map_err(|_| Error::InvalidState)??;
    let stderr = err.join().map_err(|_| Error::InvalidState)??;
    if limited || overflow.load(Ordering::Acquire) {
        return Err(Error::Git);
    }
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trusted_host_command_output_and_wall_bounds_are_enforced() {
        let mut flood = Command::new("/usr/bin/yes");
        flood.env_clear();
        assert!(matches!(
            host_output(&mut flood, 1000, 128, 128),
            Err(Error::Git)
        ));
        let mut wait = Command::new("/bin/sleep");
        wait.env_clear().arg("10");
        let started = Instant::now();
        assert!(matches!(
            host_output(&mut wait, 100, 128, 128),
            Err(Error::Git)
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        let mut finite = Command::new("/usr/bin/printf");
        finite.env_clear().arg("observed");
        let output = host_output(&mut finite, 1000, 128, 128).unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"observed");
    }
}
