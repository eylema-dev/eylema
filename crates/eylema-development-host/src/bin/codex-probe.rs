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
fn number<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or("null".into(), |v| v.to_string())
}
fn diagnostics_json(value: Option<CallDiagnostics>) -> String {
    let Some(d) = value else {
        return "null".into();
    };
    let usage = d.reported_usage.map_or("null".into(), |u| format!(
        r#"{{"kind":"cli_turn_aggregate","input_tokens":{},"cached_input_tokens":{},"cache_write_input_tokens":{},"output_tokens":{},"reasoning_output_tokens":{}}}"#,
        number(u.input_tokens), number(u.cached_input_tokens), number(u.cache_write_input_tokens), number(u.output_tokens), number(u.reasoning_output_tokens)));
    format!(
        r#"{{"failure_path":"{:?}","child_exit_code":{},"child_exit_success":{},"stdin_write_success":{},"completed_turn_seen":{},"final_message_seen":{},"provider_error_seen":{},"message_category":"{:?}","reported_usage":{},"invalid_usage_seen":{}}}"#,
        d.failure_path,
        number(d.child_exit_code),
        d.child_exit_success,
        d.stdin_write_success,
        d.completed_turn_seen,
        d.final_message_seen,
        d.provider_error_seen,
        d.message_category,
        usage,
        d.invalid_usage_seen
    )
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
    assert!(
        args.windows(2)
            .any(|v| v == ["-c", "forced_login_method=\"chatgpt\""])
    );
    assert!(
        args.iter()
            .any(|v| v.starts_with("permissions.g40.filesystem={"))
    );
    fs::write("fixture-args.txt", args.join("\n")).unwrap();
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
        "diag-invalid-usage" => {
            println!(
                r#"{{"type":"turn.completed","usage":{{"input_tokens":10,"cached_input_tokens":8,"cache_write_input_tokens":8,"output_tokens":5}}}}"#
            );
        }
        "diag-schema" => {
            println!(
                r#"{{"type":"item.completed","item":{{"type":"error","message":"schema rejection G40_DIAGNOSTIC_SECRET_MARKER"}}}}"#
            );
            println!(
                r#"{{"type":"turn.completed","usage":{{"input_tokens":123,"cached_input_tokens":70,"output_tokens":7,"reasoning_output_tokens":3}}}}"#
            );
            std::process::exit(1);
        }
        "item-error" => {
            println!(
                r#"{{"type":"item.completed","item":{{"type":"error","message":"Requested model unavailable"}}}}"#
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
            if mode != "diag-incomplete" {
                if mode == "diag-missing-usage" {
                    println!(r#"{{"type":"turn.completed"}}"#);
                } else {
                    println!(
                        r#"{{"type":"turn.completed","usage":{{"input_tokens":11,"output_tokens":5}}}}"#
                    );
                }
            }
            if mode == "diag-exit" {
                std::process::exit(7);
            }
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!("g40-provider-fixture/1");
        return;
    }
    if args.ends_with(&["login".into(), "status".into()]) {
        assert!(
            args.windows(2)
                .any(|v| v == ["-c", "forced_login_method=\"chatgpt\""])
        );
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let mode =
            fs::read_to_string(home.join("fixture-auth-mode.txt")).unwrap_or("chatgpt".into());
        if mode == "chatgpt" {
            println!("Logged in using ChatGPT");
        } else if mode == "apikey" {
            println!("Logged in using an API key");
        } else {
            eprintln!("Not logged in");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "exec") {
        fixture(&args);
        return;
    }
    let synthetic = args.first().map(String::as_str) == Some("--fixture-observation");
    assert!(
        synthetic || args.first().map(String::as_str) == Some("--live"),
        "explicit observation mode required"
    );
    assert!(args.len() <= 2, "unexpected arguments");
    let mode = args.get(1).map(String::as_str).unwrap_or("success");
    let expected = match mode {
        "success" | "permissions" => CallStatus::Completed,
        "diag-schema" | "diag-exit" | "diag-incomplete" if synthetic => CallStatus::TransportFailed,
        "diag-missing-usage" if synthetic => CallStatus::Completed,
        "diag-invalid-usage" if synthetic => CallStatus::MalformedOutput,
        "timeout" => CallStatus::TimedOut,
        "unavailable" => CallStatus::ModelUnavailable,
        _ => panic!("unknown mode"),
    };
    let mut fixture = support::Fixture::with_limits(Limits {
        model_calls: 4,
        repairs: 3,
        wall_ms: 120_000,
        usage_units: 300_000,
    });
    let mut workspace = fixture.workspace();
    let tool = if synthetic {
        CodexTool::fixture(&std::env::current_exe().unwrap(), &fixture.root).unwrap()
    } else {
        let home = PathBuf::from(std::env::var_os("HOME").expect("operator HOME"));
        let auth = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        CodexTool::installed(Path::new("/opt/homebrew/bin/codex"), &home, &auth)
            .expect("pinned installed CLI")
    };
    if mode == "permissions" {
        assert!(!synthetic, "installed local policy control required");
        tool.check_permissions(&workspace)
            .expect("actual local permission controls");
        println!(
            r#"{{"kind":"codex_adapter_local_permission_controls","status":"passed","inside_control":"actual_allowed_read","outside_canary":"actual_denied_read","provider_calls":0,"cli_version":"{}","cli_sha256":"{}"}}"#,
            CODEX_VERSION, CODEX_SHA256
        );
        return;
    }
    let prompt = "Return only the structured proposal. Do not call any tools. This is a synthetic public acceptance task: replace allowed.txt with the text 'G40 bounded proposal\\n'. Include one change with deleted=false and a short summary. No execution, credentials, operational data or GitHub effect is requested.";
    let limits = CallLimits {
        wall_ms: if mode == "timeout" { 1 } else { 60_000 },
        output_bytes: 1024 * 1024,
    };
    let start = fixture.started;
    let fixture_prompt = if synthetic && mode.starts_with("diag-") {
        mode
    } else {
        prompt
    };
    let request = CodexRequest {
        prompt: fixture_prompt,
        limits,
        cancel: &CancelHandle::default(),
        unavailable_probe: mode == "unavailable",
    };
    let requested_model = request.requested_model();
    let result = tool
        .invoke(
            &mut workspace,
            fixture.registry.get_mut(&fixture.key).unwrap(),
            &fixture.auth,
            || 1 + start.elapsed().as_millis() as u64,
            request,
        )
        .expect("controlled provider observation");
    let count = result.proposal.as_ref().map_or(0, |p| p.changes.len());
    println!(
        r#"{{"kind":"{}","status":"{:?}","transport":"{:?}","cli_version":"{}","cli_sha256":"{}","requested_model":"{}","requested_effort":"{}","elapsed_ms":{},"proposal_changes":{},"reported_total_tokens":{},"authority":"synthetic_host_fixture","filesystem_process":"actual_native","remote_cancellation":"unknown_after_local_stop","hidden_model_identity":"not_established","rejected_item_kind":{},"diagnostics":{}}}"#,
        if synthetic {
            "codex_adapter_fixture_observation"
        } else {
            "codex_adapter_live_observation"
        },
        result.status,
        result.transport,
        if synthetic {
            "g40-provider-fixture/1"
        } else {
            CODEX_VERSION
        },
        if synthetic {
            "not_claimed_for_fixture"
        } else {
            CODEX_SHA256
        },
        requested_model,
        EFFORT,
        result.elapsed_ms,
        count,
        result
            .reported_total_tokens
            .map_or("null".into(), |tokens| tokens.to_string()),
        result
            .rejected_item_kind
            .as_deref()
            .map_or("null".into(), quote),
        diagnostics_json(result.diagnostics)
    );
    assert_eq!(result.status, expected);
}
