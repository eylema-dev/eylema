//! Synthetic CLI transport and explicit opt-in real provider acceptance driver.
#[path = "../../tests/support/mod.rs"]
mod support;
use eylema_development::Limits;
use eylema_development_host::{CancelHandle, adapters::*};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

fn quote(text: &str) -> String {
    let mut output = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => output.push_str(&format!("\\u{:04x}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}
fn fixture(args: &[String]) {
    for flag in [
        "--ignore-user-config",
        "--ignore-rules",
        "--ephemeral",
        "--strict-config",
        "--json",
        "--no-daemon",
    ] {
        assert!(
            args.iter().any(|arg| arg == flag),
            "broker must supply {flag}"
        );
    }
    assert!(std::env::var_os("G40_SECRET_CANARY").is_none());
    let mut prompt = String::new();
    std::io::stdin()
        .take(16385)
        .read_to_string(&mut prompt)
        .unwrap();
    assert!(prompt.len() <= 16384);
    fs::write("started.txt", "fixture started").unwrap();
    println!(r#"{{"type":"thread.started","thread_id":"fixture"}} "#);
    if args.iter().any(|arg| arg == "gpt-6.1-sol-g40-unavailable") || prompt == "unavailable" {
        println!(r#"{{"type":"turn.failed","error":{{"message":"Requested model not found"}}}}"#);
        std::process::exit(1);
    }
    match prompt.as_str() {
        "quota" => {
            println!(r#"{{"type":"error","message":"Usage limit: quota exceeded"}}"#);
            std::process::exit(1);
        }
        "auth" => {
            println!(
                r#"{{"type":"turn.failed","error":{{"message":"Authentication unavailable: not logged in"}}}}"#
            );
            std::process::exit(1);
        }
        "malformed" => println!("not-json"),
        "unknown-event" => println!(r#"{{"type":"unknown"}}"#),
        "partial-wait" => {
            print!(r#"{{"type":"item."#);
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_secs(20));
        }
        "wait" => std::thread::sleep(Duration::from_secs(20)),
        "tool" => {
            println!(
                r#"{{"type":"item.started","item":{{"type":"command_execution","command":"rejected"}}}}"#
            );
            std::thread::sleep(Duration::from_secs(20));
        }
        "flood" => {
            let mut out = std::io::stdout().lock();
            loop {
                if out.write_all(&[b'x'; 4096]).is_err() {
                    break;
                }
            }
        }
        mode => {
            let path = if mode == "outside" {
                "outside.txt"
            } else {
                "allowed.txt"
            };
            let proposed = format!(
                r#"{{"summary":"bounded fixture","changes":[{{"path":"{path}","text":"Türkçe result\n","deleted":false}}]}}"#
            );
            println!(
                r#"{{"type":"item.completed","item":{{"type":"agent_message","text":{}}}}}"#,
                quote(&proposed)
            );
            println!(
                r#"{{"type":"turn.completed","usage":{{"input_tokens":11,"output_tokens":5}}}}"#
            );
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!("g40-provider-fixture/1");
        return;
    }
    if args.iter().any(|arg| arg == "exec") {
        fixture(&args);
        return;
    }
    assert_eq!(
        args.first().map(String::as_str),
        Some("--live"),
        "opt-in --live MODE required"
    );
    let mode = args.get(1).map(String::as_str).unwrap_or("success");
    let mut fixture = support::Fixture::with_limits(Limits {
        model_calls: 4,
        repairs: 3,
        wall_ms: 120_000,
        usage_units: 300_000,
    });
    let mut workspace = fixture.workspace();
    let home = PathBuf::from(std::env::var_os("HOME").expect("operator HOME"));
    let auth = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let tool = CodexTool::installed(Path::new("/opt/homebrew/bin/codex"), &home, &auth)
        .expect("pinned installed CLI");
    let prompt = "Return only the structured proposal. Do not call any tools. This is a synthetic public acceptance task: replace allowed.txt with the text 'G40 bounded proposal\\n'. Include one change with deleted=false and a short summary. No execution, credentials, operational data or GitHub effect is requested.";
    let limits = CallLimits {
        wall_ms: if mode == "timeout" { 1 } else { 60_000 },
        output_bytes: 1024 * 1024,
    };
    let start = fixture.started;
    let result = tool
        .invoke(
            &mut workspace,
            fixture.registry.get_mut(&fixture.key).unwrap(),
            &fixture.auth,
            || 1 + start.elapsed().as_millis() as u64,
            CodexRequest {
                prompt,
                limits,
                cancel: &CancelHandle::default(),
                unavailable_probe: mode == "unavailable",
            },
        )
        .expect("controlled provider observation");
    let count = result.proposal.as_ref().map_or(0, |p| p.changes.len());
    println!(
        r#"{{"kind":"codex_adapter_live_observation","status":"{:?}","transport":"{:?}","cli_version":"{}","cli_sha256":"{}","requested_model":"{}","requested_effort":"{}","elapsed_ms":{},"proposal_changes":{},"reported_total_tokens":{},"authority":"synthetic_host_fixture","filesystem_process":"actual_native","remote_cancellation":"unknown_after_local_stop","hidden_model_identity":"not_established"}}"#,
        result.status,
        result.transport,
        CODEX_VERSION,
        CODEX_SHA256,
        MODEL,
        EFFORT,
        result.elapsed_ms,
        count,
        result
            .reported_total_tokens
            .map_or("null".into(), |tokens| tokens.to_string())
    );
    let expected = match mode {
        "success" => CallStatus::Completed,
        "timeout" => CallStatus::TimedOut,
        "unavailable" => CallStatus::ModelUnavailable,
        _ => panic!("unknown mode"),
    };
    assert_eq!(result.status, expected);
}
