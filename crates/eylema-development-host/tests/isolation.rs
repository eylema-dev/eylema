mod support;
use eylema_development_host::*;
use std::{
    fs,
    process::{Command, Stdio},
    time::Duration,
};
use support::*;

fn exe() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_BIN_EXE_workspace-probe"))
}
fn probe(
    f: &mut Fixture,
    w: &mut Workspace,
    mode: &str,
    args: &[String],
    limit: u64,
) -> WorkerResult {
    let start = f.started;
    let mut a = vec!["--worker".into(), mode.into()];
    a.extend_from_slice(args);
    w.run_single(
        f.registry.get_mut(&f.key).unwrap(),
        &f.auth,
        || 1 + start.elapsed().as_millis() as u64,
        WorkerSpec {
            executable: exe(),
            args: &a,
            limits: ProcessLimits {
                wall_ms: limit,
                output_bytes: 8192,
            },
            cancel: &CancelHandle::default(),
        },
    )
    .unwrap()
}
fn done(r: WorkerResult) -> WorkerResult {
    assert_eq!(
        r.reason,
        StopReason::Completed,
        "stderr: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    r
}

#[test]
fn actual_detached_worktree_preserves_staged_unstaged_untracked_dirty_caller() {
    let mut f = Fixture::new();
    fs::write(f.source.join("allowed.txt"), "staged\n").unwrap();
    git(&f.source, &["add", "allowed.txt"]);
    fs::write(f.source.join("allowed.txt"), "unstaged\n").unwrap();
    fs::write(f.source.join("private-untracked.txt"), "dirty canary\n").unwrap();
    let before = git(&f.source, &["status", "--porcelain=v1", "-z"]);
    let index = git(&f.source, &["show", ":allowed.txt"]);
    let mut w = f.workspace();
    assert_eq!(
        fs::read_to_string(w.path().join("allowed.txt")).unwrap(),
        "base\n"
    );
    done(probe(&mut f, &mut w, "write", &[], 5000));
    let a = w
        .artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192,
        })
        .unwrap();
    assert_eq!(a.len(), 2);
    assert_eq!(git(&f.source, &["status", "--porcelain=v1", "-z"]), before);
    assert_eq!(git(&f.source, &["show", ":allowed.txt"]), index);
    assert_eq!(
        fs::read_to_string(f.source.join("allowed.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(
        fs::read_to_string(f.source.join("private-untracked.txt")).unwrap(),
        "dirty canary\n"
    );
}
#[test]
fn actual_os_scope_denies_write_outside_allowed_paths() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    done(probe(&mut f, &mut w, "deny-write", &[], 5000));
    assert_eq!(
        fs::read_to_string(w.path().join("outside.txt")).unwrap(),
        "untouched\n"
    );
    assert!(
        w.artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192
        })
        .unwrap()
        .is_empty()
    );
}
#[test]
fn actual_secret_control_source_git_and_sibling_reads_are_denied() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let value = "SYNTHETIC_G39_SECRET_NOT_A_CREDENTIAL";
    for p in [
        f.control.join("credential-canary"),
        f.root.join("home-canary"),
    ] {
        fs::write(&p, value).unwrap();
        let r = done(probe(
            &mut f,
            &mut w,
            "read-secret",
            &[p.to_string_lossy().into_owned()],
            5000,
        ));
        assert!(!String::from_utf8_lossy(&r.stdout).contains(value));
        assert!(!String::from_utf8_lossy(&r.stderr).contains(value));
    }
    git(&f.source, &["config", "g39.secret", value]);
    let config = f.source.join(".git/config").to_string_lossy().into_owned();
    done(probe(&mut f, &mut w, "read-secret", &[config], 5000));
    done(probe(&mut f, &mut w, "git-denied", &[], 5000));
}
#[test]
fn actual_worker_has_only_fixed_environment_and_private_home() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    done(probe(&mut f, &mut w, "env", &[], 5000));
}
#[test]
fn native_parent_environment_canary_is_removed_before_worker_exec() {
    let o = Command::new(exe())
        .env("G39_SECRET_CANARY", "SYNTHETIC_PARENT_ENV_CANARY")
        .arg("--env-proof")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!String::from_utf8_lossy(&o.stdout).contains("SYNTHETIC_PARENT_ENV_CANARY"));
}
#[test]
fn actual_hardlink_escape_is_denied_before_secret_read() {
    let mut f = Fixture::new();
    let secret = f.root.join("secret");
    fs::write(&secret, "secret canary").unwrap();
    let mut w = f.workspace();
    done(probe(
        &mut f,
        &mut w,
        "link",
        &[secret.to_string_lossy().into_owned()],
        5000,
    ));
    assert!(!w.path().join("generated/alias.txt").exists());
}
#[test]
fn actual_symlink_cannot_read_secret_and_artifact_export_rejects_it() {
    let mut f = Fixture::new();
    let secret = f.root.join("secret");
    fs::write(&secret, "secret canary").unwrap();
    let mut w = f.workspace();
    done(probe(
        &mut f,
        &mut w,
        "symlink",
        &[secret.to_string_lossy().into_owned()],
        5000,
    ));
    assert!(matches!(
        w.artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192
        }),
        Err(Error::Scope)
    ));
}
#[test]
fn actual_network_fork_and_foreign_signal_probes_are_denied() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    done(probe(&mut f, &mut w, "network", &[address], 5000));
    done(probe(
        &mut f,
        &mut w,
        "fork-signal",
        &[std::process::id().to_string()],
        5000,
    ));
}
#[test]
fn timeout_reaps_owned_child_and_preserves_unrelated_live_child_and_artifacts() {
    let mut sibling = Command::new("/bin/sleep")
        .arg("10")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let r = probe(&mut f, &mut w, "wait", &[], 1000);
    assert_eq!(r.reason, StopReason::TimedOut);
    assert!(sibling.try_wait().unwrap().is_none());
    assert_eq!(
        fs::read_to_string(w.path().join("generated/started.txt")).unwrap(),
        "started"
    );
    let a = w
        .artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192,
        })
        .unwrap();
    assert_eq!(a.len(), 1);
    sibling.kill().unwrap();
    sibling.wait().unwrap();
}
#[test]
fn explicit_cancel_reaps_exact_owned_child_without_arbitrary_pid_api() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let cancel = CancelHandle::default();
    let c = cancel.clone();
    let started_path = w.path().join("generated/started.txt");
    let timer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !started_path.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            started_path.exists(),
            "actual worker must start before cancellation"
        );
        c.cancel();
    });
    let start = f.started;
    let r = w
        .run_single(
            f.registry.get_mut(&f.key).unwrap(),
            &f.auth,
            || 1 + start.elapsed().as_millis() as u64,
            WorkerSpec {
                executable: exe(),
                args: &["--worker".into(), "wait".into()],
                limits: ProcessLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &cancel,
            },
        )
        .unwrap();
    timer.join().unwrap();
    assert_eq!(r.reason, StopReason::Cancelled);
    assert!(w.path().join("generated/started.txt").exists());
}
#[test]
fn finite_combined_output_cap_stops_actual_flood_worker() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let start = f.started;
    let r = w
        .run_single(
            f.registry.get_mut(&f.key).unwrap(),
            &f.auth,
            || 1 + start.elapsed().as_millis() as u64,
            WorkerSpec {
                executable: exe(),
                args: &["--worker".into(), "flood".into()],
                limits: ProcessLimits {
                    wall_ms: 5000,
                    output_bytes: 123,
                },
                cancel: &CancelHandle::default(),
            },
        )
        .unwrap();
    assert_eq!(r.reason, StopReason::OutputLimit);
    assert!(r.stdout.len() + r.stderr.len() <= 123);
}
#[test]
fn revoked_authority_stops_effect_and_cannot_start_new_worker() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let r = w.run_single(
        f.registry.get_mut(&f.key).unwrap(),
        &DeniedAuthority,
        || 1,
        WorkerSpec {
            executable: exe(),
            args: &["--worker".into(), "write".into()],
            limits: ProcessLimits {
                wall_ms: 5000,
                output_bytes: 8192,
            },
            cancel: &CancelHandle::default(),
        },
    );
    assert!(matches!(r, Err(Error::Coordinator(_))));
    assert_eq!(
        fs::read_to_string(w.path().join("allowed.txt")).unwrap(),
        "base\n"
    );
}
#[test]
fn stale_base_rejects_workspace_without_mutating_caller() {
    let mut f = Fixture::new();
    git(
        &f.source,
        &[
            "-c",
            "user.name=G39 Fixture",
            "-c",
            "user.email=g39@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "changed base",
        ],
    );
    assert!(matches!(
        Workspace::create(
            f.registry.get_mut(&f.key).unwrap(),
            1,
            &f.auth,
            &f.source,
            &f.control,
            &git_program()
        ),
        Err(Error::Scope)
    ));
    assert_eq!(fs::read_dir(&f.control).unwrap().count(), 0);
}
#[test]
fn retry_does_not_replace_existing_owned_workspace_or_reset_changes() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    done(probe(&mut f, &mut w, "write", &[], 5000));
    let now = f.registry.get_mut(&f.key).unwrap();
    assert!(matches!(
        Workspace::create(
            now,
            1 + f.started.elapsed().as_millis() as u64,
            &f.auth,
            &f.source,
            &f.control,
            &git_program()
        ),
        Err(Error::AlreadyExists)
    ));
    assert_eq!(
        fs::read_to_string(w.path().join("allowed.txt")).unwrap(),
        "worker changed\n"
    );
}
#[test]
fn artifact_escape_hardlinks_count_and_byte_limits_fail_closed() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    done(probe(&mut f, &mut w, "write", &[], 5000));
    assert!(
        w.artifacts(ArtifactLimits {
            files: 1,
            bytes: 8192
        })
        .is_err()
    );
    assert!(w.artifacts(ArtifactLimits { files: 8, bytes: 1 }).is_err());
    fs::write(
        w.path().join("outside.txt"),
        "trusted-host injected out-of-scope modification",
    )
    .unwrap();
    assert!(
        w.artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192
        })
        .is_err()
    );
    fs::write(w.path().join("outside.txt"), "untouched\n").unwrap();
    fs::hard_link(
        w.path().join("allowed.txt"),
        w.path().join("generated/hard.txt"),
    )
    .unwrap();
    assert!(
        w.artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192
        })
        .is_err()
    );
}
#[test]
fn configured_checkout_filters_cannot_execute_in_control_host() {
    let mut f = Fixture::new();
    git(
        &f.source,
        &["config", "filter.bad.smudge", "echo synthetic-config-code"],
    );
    assert!(matches!(
        Workspace::create(
            f.registry.get_mut(&f.key).unwrap(),
            1,
            &f.auth,
            &f.source,
            &f.control,
            &git_program()
        ),
        Err(Error::Scope)
    ));
    assert_eq!(fs::read_dir(&f.control).unwrap().count(), 0);
}

#[test]
fn empty_replacement_and_deletion_have_distinct_native_artifacts() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    done(probe(&mut f, &mut w, "empty", &[], 5000));
    let empty = w
        .artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192,
        })
        .unwrap();
    assert_eq!(empty.len(), 1);
    assert!(!empty[0].deleted);
    assert_eq!(empty[0].text, "");
    done(probe(&mut f, &mut w, "delete", &[], 5000));
    let deleted = w
        .artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192,
        })
        .unwrap();
    assert_eq!(deleted.len(), 1);
    assert!(deleted[0].deleted);
    assert_eq!(deleted[0].text, "");
}

#[test]
fn authority_revocation_during_actual_worker_run_reaps_child() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Revocable {
        accepted: eylema_development::AcceptedTask,
        revoked: Arc<AtomicBool>,
    }
    impl eylema_development::AuthorityResolver for Revocable {
        fn resolve(
            &self,
            _: &eylema_development::NeedSpec,
        ) -> Option<eylema_development::AcceptedTask> {
            if self.revoked.load(Ordering::Acquire) {
                None
            } else {
                Some(self.accepted.clone())
            }
        }
    }
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let revoked = Arc::new(AtomicBool::new(false));
    let auth = Revocable {
        accepted: f.auth.0.clone(),
        revoked: revoked.clone(),
    };
    let started_path = w.path().join("generated/started.txt");
    let timer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !started_path.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            started_path.exists(),
            "actual worker must start before revocation"
        );
        revoked.store(true, Ordering::Release);
    });
    let start = f.started;
    let result = w
        .run_single(
            f.registry.get_mut(&f.key).unwrap(),
            &auth,
            || 1 + start.elapsed().as_millis() as u64,
            WorkerSpec {
                executable: exe(),
                args: &["--worker".into(), "wait".into()],
                limits: ProcessLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &CancelHandle::default(),
            },
        )
        .unwrap();
    timer.join().unwrap();
    assert_eq!(result.reason, StopReason::AuthorityDenied);
    assert!(w.path().join("generated/started.txt").exists());
}

fn actual_coordinator_stop_cause(cause: eylema_development::Error, stop_clock: u64) {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let started_path = w.path().join("generated/started.txt");
    let before = 1 + f.started.elapsed().as_millis() as u64;
    let result = w
        .run_single(
            f.registry.get_mut(&f.key).unwrap(),
            &f.auth,
            || {
                if started_path.exists() {
                    stop_clock
                } else {
                    before
                }
            },
            WorkerSpec {
                executable: exe(),
                args: &["--worker".into(), "wait".into()],
                limits: ProcessLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &CancelHandle::default(),
            },
        )
        .unwrap();
    assert!(
        started_path.exists(),
        "actual worker must start before coordinator stop"
    );
    assert_eq!(result.reason, StopReason::CoordinatorStopped(cause));
    assert!(
        result.exit_code.is_none(),
        "owned worker was killed and reaped"
    );
}

#[test]
fn actual_task_budget_exhaustion_is_preserved_in_worker_result() {
    actual_coordinator_stop_cause(eylema_development::Error::BudgetExhausted, 60_002);
}

#[test]
fn actual_clock_regression_is_preserved_in_worker_result() {
    actual_coordinator_stop_cause(eylema_development::Error::ClockRegressed, 0);
}
