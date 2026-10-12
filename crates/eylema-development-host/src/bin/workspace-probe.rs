//! Actual native workspace/process proof; authorization is a declared fixture.
#[path = "../../tests/support/mod.rs"]
mod support;
use eylema_development_host::*;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn fork() -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
}

fn worker(args: &[String]) {
    match args[0].as_str() {
        "write" => {
            fs::write("allowed.txt", "worker changed\n").unwrap();
            fs::write("generated/new.txt", "scoped result\n").unwrap();
            println!("scoped-write");
        }
        "empty" => fs::write("allowed.txt", "").unwrap(),
        "delete" => fs::remove_file("allowed.txt").unwrap(),
        "deny-write" => assert!(fs::write("outside.txt", "bad").is_err()),
        "read-secret" => {
            assert!(fs::read(&args[1]).is_err());
            println!("secret-denied");
        }
        "git-denied" => assert!(fs::read(".git").is_err()),
        "env" => {
            let names: Vec<_> = std::env::vars().map(|(k, _)| k).collect();
            assert!(
                names
                    .iter()
                    .all(|k| ["HOME", "TMPDIR", "PATH", "LANG"].contains(&k.as_str()))
            );
            assert!(std::env::var_os("G39_SECRET_CANARY").is_none());
            println!("environment-cleared");
        }
        "link" => {
            assert!(fs::hard_link(&args[1], "generated/alias.txt").is_err());
            println!("hardlink-denied");
        }
        "symlink" => {
            std::os::unix::fs::symlink(&args[1], "generated/escape.txt").unwrap();
            assert!(fs::read("generated/escape.txt").is_err());
            println!("symlink-read-denied");
        }
        "network" => {
            assert!(std::net::TcpStream::connect(&args[1]).is_err());
            println!("network-denied");
        }
        "fork-signal" => {
            #[cfg(target_os = "macos")]
            {
                // SAFETY: test-only system probes. Default-deny must reject fork and signal;
                // if fork unexpectedly succeeds, terminate the child without doing work.
                let child = unsafe { fork() };
                if child == 0 {
                    std::process::exit(90);
                }
                assert_eq!(child, -1, "fork escaped sandbox");
                let pid = args[1].parse::<i32>().unwrap();
                assert_eq!(
                    unsafe { kill(pid, 0) },
                    -1,
                    "foreign process signal probe escaped sandbox"
                );
            }
            println!("fork-and-signal-denied");
        }
        "wait" => {
            fs::write("generated/started.txt", "started").unwrap();
            loop {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        "flood" => {
            let mut out = io::stdout().lock();
            loop {
                if out.write_all(&[b'x'; 4096]).is_err() {
                    break;
                }
            }
        }
        _ => panic!("unknown worker probe"),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--worker") {
        worker(&args[1..]);
        return;
    }
    let mut fixture = support::Fixture::new();
    fs::write(fixture.source.join("allowed.txt"), "caller dirty\n").unwrap();
    let mut work = fixture.workspace();
    let start = fixture.started;
    let mode = if args.first().map(String::as_str) == Some("--env-proof") {
        "env"
    } else {
        "write"
    };
    let result = work
        .run_single(
            fixture.registry.get_mut(&fixture.key).unwrap(),
            &fixture.auth,
            || 1 + start.elapsed().as_millis() as u64,
            WorkerSpec {
                executable: &std::env::current_exe().unwrap(),
                args: &["--worker".into(), mode.into()],
                limits: ProcessLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &CancelHandle::default(),
            },
        )
        .unwrap();
    if result.reason != StopReason::Completed {
        eprintln!(
            "worker exit {:?}; stderr {}",
            result.exit_code,
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(result.reason, StopReason::Completed);
    let artifacts = work
        .artifacts(ArtifactLimits {
            files: 8,
            bytes: 8192,
        })
        .unwrap();
    assert_eq!(artifacts.len(), if mode == "write" { 2 } else { 0 });
    assert_eq!(
        fs::read_to_string(fixture.source.join("allowed.txt")).unwrap(),
        "caller dirty\n"
    );
    println!(
        "{{\"kind\":\"native_development_workspace_proof\",\"status\":\"passed\",\"host_authority\":\"synthetic_fixture\",\"process_and_git\":\"actual_native\",\"scoped_artifacts\":{},\"dirty_caller_preserved\":true,\"provider_github\":\"not_run\"}}",
        artifacts.len()
    );
    // Retain the isolated worktree and artifacts for explicit host reconciliation.
    let _: PathBuf = fixture.root;
}
