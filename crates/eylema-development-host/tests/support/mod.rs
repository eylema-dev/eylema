#![allow(dead_code)]
use eylema_development::*;
use eylema_development_host::Workspace;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub struct FixtureAuthority(pub AcceptedTask);
impl AuthorityResolver for FixtureAuthority {
    fn resolve(&self, _: &NeedSpec) -> Option<AcceptedTask> {
        Some(self.0.clone())
    }
}
pub struct DeniedAuthority;
impl AuthorityResolver for DeniedAuthority {
    fn resolve(&self, _: &NeedSpec) -> Option<AcceptedTask> {
        None
    }
}
pub fn git(repo: &Path, args: &[&str]) -> Vec<u8> {
    let o = Command::new(git_program())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("fixture Git");
    assert!(o.status.success(), "fixture Git failed");
    o.stdout
}
pub fn git_program() -> PathBuf {
    if Path::new("/opt/homebrew/bin/git").exists() {
        PathBuf::from("/opt/homebrew/bin/git")
    } else {
        PathBuf::from("/usr/bin/git")
    }
}
pub fn temp() -> PathBuf {
    let id = eylema_platform::random_id(eylema_platform::IdKind::Request).unwrap();
    let root = std::env::temp_dir().join(format!("g39-{id}"));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::canonicalize(root).unwrap()
}
pub struct Fixture {
    pub started: std::time::Instant,
    pub root: PathBuf,
    pub source: PathBuf,
    pub control: PathBuf,
    pub base: Commit,
    pub registry: Registry,
    pub key: TaskKey,
    pub auth: FixtureAuthority,
}
impl Fixture {
    pub fn new() -> Self {
        Self::with_limits(Limits {
            model_calls: 1,
            repairs: 1,
            wall_ms: 60_000,
            usage_units: 1,
        })
    }
    pub fn with_limits(limits: Limits) -> Self {
        let root = temp();
        let source = root.join("source");
        let control = root.join("control");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&control).unwrap();
        fs::set_permissions(&control, fs::Permissions::from_mode(0o700)).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("allowed.txt"), "base\n").unwrap();
        fs::write(source.join("outside.txt"), "untouched\n").unwrap();
        git(&source, &["add", "."]);
        git(
            &source,
            &[
                "-c",
                "user.name=G39 Fixture",
                "-c",
                "user.email=g39@example.invalid",
                "commit",
                "-m",
                "fixture base",
            ],
        );
        let base = Commit::new(
            String::from_utf8(git(&source, &["rev-parse", "HEAD"]))
                .unwrap()
                .trim()
                .to_owned(),
        )
        .unwrap();
        let need = NeedSpec::new(NeedDraft {
            key: TaskKey::new(
                "eylema-dev/eylema".into(),
                eylema_contracts::RequestId::new("g39fixture").unwrap(),
                eylema_contracts::Revision::new(1).unwrap(),
            )
            .unwrap(),
            source_repository: "eylema-dev/eylema".into(),
            issue_number: Some(32),
            objective: "Native isolated-workspace fixture".into(),
            acceptance: vec![Acceptance {
                id: "G39-A1".into(),
                scenario: "single-process isolation".into(),
                expected: "scope and secrets protected".into(),
            }],
            allowed_paths: vec!["allowed.txt".into(), "generated/".into()],
            out_of_scope: vec!["Publication and provider access".into()],
            targets: vec!["native_macos".into()],
            authorization_ref: "synthetic-fixture".into(),
            limits,
        })
        .unwrap();
        let auth = FixtureAuthority(
            AcceptedTask::new(
                need.clone(),
                base.clone(),
                "fixture-control".into(),
                "fixture-policy".into(),
                u64::MAX,
            )
            .unwrap(),
        );
        let key = need.key().clone();
        let mut registry = Registry::new(1).unwrap();
        let run = registry.receive(need, 1).unwrap();
        run.spec_ready(1, &auth).unwrap();
        run.set_plan(
            WorkPlan {
                base: base.clone(),
                implementer_context: "fixture-implementer".into(),
                checks: vec![CheckMapping {
                    acceptance_id: "G39-A1".into(),
                    check: "isolation".into(),
                }],
            },
            1,
            &auth,
        )
        .unwrap();
        run.begin(1, &auth).unwrap();
        Self {
            started: std::time::Instant::now(),
            root,
            source,
            control,
            base,
            registry,
            key,
            auth,
        }
    }
    pub fn workspace(&mut self) -> Workspace {
        Workspace::create(
            self.registry.get_mut(&self.key).unwrap(),
            1 + self.started.elapsed().as_millis() as u64,
            &self.auth,
            &self.source,
            &self.control,
            &git_program(),
        )
        .unwrap()
    }
}
