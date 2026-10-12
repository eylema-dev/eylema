use crate::{Artifact, ArtifactLimits, Error, Result, files};
use eylema_development::{AuthorityResolver, Commit, Coordinator, NeedSpec, Stage, State};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub struct Workspace {
    pub(crate) work: PathBuf,
    pub(crate) scratch: PathBuf,
    pub(crate) need: NeedSpec,
    pub(crate) base: Commit,
    pub(crate) git: PathBuf,
}

fn clean_command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    c
}
fn git(git: &Path, repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let mut command = clean_command(git);
    command
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-C",
        ])
        .arg(repo)
        .args(args);
    let o = crate::process::host_output(&mut command, 20_000, 4 * 1024 * 1024, 64 * 1024)?;
    if !o.status.success() || o.stdout.len() > 4 * 1024 * 1024 || o.stderr.len() > 64 * 1024 {
        return Err(Error::Git);
    }
    Ok(o.stdout)
}
fn names(bytes: Vec<u8>) -> Result<Vec<String>> {
    let text = String::from_utf8(bytes).map_err(|_| Error::Scope)?;
    text.split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if files::relative(s) {
                Ok(s.to_owned())
            } else {
                Err(Error::Scope)
            }
        })
        .collect()
}

impl Workspace {
    /// Only the trusted control host calls this. No credentials enter the worker.
    /// Failure retains any created directory/metadata for explicit reconciliation.
    pub fn create(
        run: &mut Coordinator,
        now: u64,
        authority: &impl AuthorityResolver,
        source: &Path,
        control_root: &Path,
        git_program: &Path,
    ) -> Result<Self> {
        if !cfg!(target_os = "macos") {
            return Err(Error::Unsupported);
        }
        run.poll(now, authority)?;
        if run.state() != State::Running(Stage::Implementing) {
            return Err(Error::InvalidState);
        }
        let base = run.accepted_base().ok_or(Error::InvalidState)?.clone();
        let source = fs::canonicalize(source)?;
        if !control_root.is_absolute() || fs::symlink_metadata(control_root)?.is_symlink() {
            return Err(Error::InvalidInput);
        }
        let root = fs::canonicalize(control_root)?;
        if root.starts_with(&source) || source.starts_with(&root) {
            return Err(Error::Scope);
        }
        #[cfg(unix)]
        if fs::metadata(&root)?.permissions().mode() & 0o077 != 0 {
            return Err(Error::Scope);
        }
        let git_program = fs::canonicalize(git_program)?;
        files::regular(&git_program)?;
        let actual = git(&git_program, &source, &["rev-parse", "HEAD"])?;
        if actual != format!("{}\n", base.as_str()).as_bytes() {
            return Err(Error::Scope);
        }
        let actual_root = git(&git_program, &source, &["rev-parse", "--show-toplevel"])?;
        let actual_root = String::from_utf8(actual_root).map_err(|_| Error::Git)?;
        if fs::canonicalize(actual_root.trim())? != source {
            return Err(Error::Scope);
        }
        // Local filters can execute commands on checkout. Refuse all filter configuration.
        let mut filter_command = clean_command(&git_program);
        filter_command
            .arg("-C")
            .arg(&source)
            .args(["config", "--get-regexp", "^filter\\."]);
        let filters =
            crate::process::host_output(&mut filter_command, 20_000, 4 * 1024 * 1024, 64 * 1024)?;
        if filters.status.code() != Some(1) || !filters.stdout.is_empty() {
            return Err(Error::Scope);
        }
        let tracked = names(git(
            &git_program,
            &source,
            &["ls-tree", "-r", "--name-only", "-z", base.as_str()],
        )?)?;
        if tracked.len() > 100_000 {
            return Err(Error::Scope);
        }
        let modes = git(&git_program, &source, &["ls-tree", "-r", base.as_str()])?;
        if String::from_utf8(modes)
            .map_err(|_| Error::Scope)?
            .lines()
            .any(|s| !s.starts_with("100644 ") && !s.starts_with("100755 "))
        {
            return Err(Error::Scope);
        }
        let key = format!(
            "{}\n{}\n{}\n",
            run.need().key().repository(),
            run.need().key().request_id().as_str(),
            run.need().key().revision().get()
        );
        let id = eylema_platform::hex(
            &eylema_platform::sha256(key.as_bytes()).map_err(|_| Error::Unsupported)?,
        );
        let owned = root.join(id);
        match fs::create_dir(&owned) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Error::AlreadyExists);
            }
            Err(e) => return Err(e.into()),
        }
        #[cfg(unix)]
        fs::set_permissions(&owned, fs::Permissions::from_mode(0o700))?;
        fs::write(
            owned.join("binding.txt"),
            format!("G39-v1\n{key}{}\n", base.as_str()),
        )?;
        let work = owned.join("worktree");
        git(
            &git_program,
            &source,
            &[
                "worktree",
                "add",
                "--detach",
                work.to_str().ok_or(Error::InvalidInput)?,
                base.as_str(),
            ],
        )?;
        files::tree(&work, 100_000)?;
        let scratch = owned.join("scratch");
        fs::create_dir(&scratch)?;
        let workspace = Self {
            work,
            scratch,
            need: run.need().clone(),
            base,
            git: git_program,
        };
        // Pre-create accepted directory scopes in the trusted host, rejecting escapes.
        for scope in &workspace.need.draft().allowed_paths {
            let dir = if scope.ends_with('/') {
                workspace.work.join(scope.trim_end_matches('/'))
            } else {
                workspace
                    .work
                    .join(scope)
                    .parent()
                    .ok_or(Error::Scope)?
                    .to_path_buf()
            };
            if !dir.starts_with(&workspace.work) {
                return Err(Error::Scope);
            }
            fs::create_dir_all(dir)?;
        }
        Ok(workspace)
    }
    pub fn path(&self) -> &Path {
        &self.work
    }
    pub fn scratch(&self) -> &Path {
        &self.scratch
    }
    pub fn base(&self) -> &Commit {
        &self.base
    }
    pub(crate) fn bind(
        &self,
        run: &mut Coordinator,
        now: u64,
        authority: &impl AuthorityResolver,
    ) -> Result<()> {
        run.poll(now, authority)?;
        if run.state() != State::Running(Stage::Implementing)
            || run.need() != &self.need
            || run.accepted_base() != Some(&self.base)
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    /// After the owned worker is reaped, inspect only bounded scoped text changes.
    /// Contents remain untrusted data; this does not authorize publication or apply them.
    pub fn artifacts(&self, limits: ArtifactLimits) -> Result<Vec<Artifact>> {
        let head = git(&self.git, &self.work, &["rev-parse", "HEAD"])?;
        if head != format!("{}\n", self.base.as_str()).as_bytes() {
            return Err(Error::Scope);
        }
        let mut changed = names(git(
            &self.git,
            &self.work,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--name-only",
                "-z",
                self.base.as_str(),
                "--",
            ],
        )?)?;
        changed.extend(names(git(
            &self.git,
            &self.work,
            &["ls-files", "--others", "-z"],
        )?)?);
        changed.sort();
        changed.dedup();
        files::collect(
            &self.work,
            &self.need.draft().allowed_paths,
            &changed,
            limits,
        )
    }
}
