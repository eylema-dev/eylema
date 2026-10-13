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
    let prefixes = d
        .error_prefixes
        .iter()
        .flatten()
        .map(|v| quote(&format!("{v:?}")))
        .collect::<Vec<_>>()
        .join(",");
    let identifiers = d
        .public_error_identifiers
        .iter()
        .flatten()
        .map(|v| quote(v))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"failure_path":"{:?}","child_exit_code":{},"child_exit_success":{},"stdin_write_success":{},"completed_turn_seen":{},"final_message_seen":{},"final_proposal_valid":{},"provider_error_seen":{},"error_event_count":{},"unclassified_error_events":{},"item_error_seen":{},"turn_failed_seen":{},"top_level_error_seen":{},"error_prefixes":[{}],"public_error_identifiers":[{}],"error_identifier_limit_reached":{},"message_category":"{:?}","reported_usage":{},"invalid_usage_seen":{}}}"#,
        d.failure_path,
        number(d.child_exit_code),
        d.child_exit_success,
        d.stdin_write_success,
        d.completed_turn_seen,
        d.final_message_seen,
        number(d.final_proposal_valid),
        d.provider_error_seen,
        d.error_event_count,
        d.unclassified_error_events,
        d.item_error_seen,
        d.turn_failed_seen,
        d.top_level_error_seen,
        prefixes,
        identifiers,
        d.error_identifier_limit_reached,
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
        mode if mode.starts_with("diag-projection-") => {
            let proposed = r#"{"summary":"bounded fixture","changes":[{"path":"allowed.txt","text":"fixture","deleted":false}]}"#;
            println!(
                r#"{{"type":"item.completed","item":{{"type":"agent_message","text":{}}}}}"#,
                quote(proposed)
            );
            let message = match mode {
                "diag-projection-safe" => "`code_mode_host` is disabled via configuration. Remove OPAQUE_CANARY_9X /fixture/local/value https://fixture.invalid/value person@fixture.invalid. \"/fixture/code mode host\"".to_owned(),
                "diag-projection-assigned" => "code_mode_host configuration value=\"old new\"".to_owned(),
                "diag-projection-parenthesized" => "code_mode_host configuration (\"old new\")".to_owned(),
                "diag-projection-unmatched" => "code_mode_host configuration value=\"old".to_owned(),
                "diag-projection-escaped" => "code_mode_host configuration value=\"old \\\"new\"".to_owned(),
                "diag-projection-embedded" => "code_mode_host configuration old\"new old\"new".to_owned(),
                "diag-projection-single" => "code_mode_host configuration value='old new'".to_owned(),
                "diag-projection-backtick" => "code_mode_host configuration value=`old new`".to_owned(),
                "diag-projection-plain" => "code_mode_host configuration old new".to_owned(),
                "diag-projection-sensitive" => "code_mode_host configuration credential sk-fixture-private-value".to_owned(),
                "diag-projection-words" => format!("code_mode_host configuration {}", "required ".repeat(64)),
                "diag-projection-bytes" => format!("code_mode_host configuration {}", "guardian_enhanced_node_repl_transcripts ".repeat(20)),
                _ => "code_mode_host is disabled via configuration".to_owned(),
            };
            if mode == "diag-projection-origin" {
                println!(r#"{{"type":"error","message":{}}}"#, quote(&message));
            } else {
                for _ in 0..if mode == "diag-projection-multiple" {
                    2
                } else {
                    1
                } {
                    println!(
                        r#"{{"type":"item.completed","item":{{"type":"error","message":{}}}}}"#,
                        quote(&message)
                    );
                }
            }
            println!(
                r#"{{"type":"turn.completed","usage":{{"input_tokens":11,"output_tokens":5}}}}"#
            );
        }
        "diag-many-identifiers" => {
            for _ in 0..2 {
                println!(
                    r#"{{"type":"error","message":"apps artifact browser_use code_mode code_mode_host computer_use goals hooks image_generation in_app_browser js_repl memories multi_agent plugins shell_snapshot shell_tool skill_search sleep_tool sqlite tool_suggest unified_exec G40_DIAGNOSTIC_SECRET_MARKER"}}"#
                );
            }
        }
        "diag-feature-notice" => {
            let proposed = r#"{"summary":"bounded fixture","changes":[{"path":"allowed.txt","text":"fixture","deleted":false}]}"#;
            println!(
                r#"{{"type":"item.completed","item":{{"type":"agent_message","text":{}}}}}"#,
                quote(proposed)
            );
            println!(
                r#"{{"type":"item.completed","item":{{"type":"error","message":"Under-development features enabled: memories, code_mode. Set suppress_unstable_features_warning in config.toml. G40_DIAGNOSTIC_SECRET_MARKER"}}}}"#
            );
            println!(
                r#"{{"type":"turn.completed","usage":{{"input_tokens":11,"output_tokens":5}}}}"#
            );
        }
        "diag-private-identifiers" => {
            println!(
                r#"{{"type":"error","message":"private-memories-secret unknown_private_feature G40_DIAGNOSTIC_SECRET_MARKER"}}"#
            );
            println!(
                r#"{{"type":"turn.failed","error":{{"message":"Reasoning effort `private_value` is not supported for model `private_model`. Supported reasoning efforts: private_other"}}}}"#
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
            if mode == "diag-late-stream" {
                print!("bad-json");
                std::io::stdout().flush().unwrap();
                let _owned_descendant = std::process::Command::new("/bin/sleep")
                    .arg("10")
                    .spawn()
                    .unwrap();
                // The adapter reaps the owned group after this fixture leader exits.
                std::process::exit(0);
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
    assert!(args.len() <= 2 || args.len() == 4, "unexpected arguments");
    let mode = args.get(1).map(String::as_str).unwrap_or("success");
    let expected = match mode {
        "success" | "permissions" => CallStatus::Completed,
        "diag-schema" | "diag-exit" | "diag-incomplete" | "diag-feature-notice" if synthetic => {
            CallStatus::TransportFailed
        }
        "diag-projection-safe"
        | "diag-projection-assigned"
        | "diag-projection-parenthesized"
        | "diag-projection-unmatched"
        | "diag-projection-escaped"
        | "diag-projection-embedded"
        | "diag-projection-single"
        | "diag-projection-backtick"
        | "diag-projection-plain"
        | "diag-projection-sensitive"
        | "diag-projection-words"
        | "diag-projection-bytes"
        | "diag-projection-origin"
        | "diag-projection-multiple"
            if synthetic =>
        {
            CallStatus::TransportFailed
        }
        "diag-many-identifiers" if synthetic => CallStatus::TransportFailed,
        "diag-private-identifiers" if synthetic => CallStatus::ModelUnavailable,
        "diag-missing-usage" if synthetic => CallStatus::Completed,
        "diag-invalid-usage" if synthetic => CallStatus::MalformedOutput,
        "timeout" => CallStatus::TimedOut,
        "unavailable" => CallStatus::ModelUnavailable,
        _ => panic!("unknown mode"),
    };
    let private_output = if args.len() == 4 {
        assert!(
            args[2] == "--private-diagnostic-output"
                && (mode == "success" || synthetic && mode.starts_with("diag-projection-")),
            "private output scope"
        );
        let path = PathBuf::from(&args[3]);
        assert!(
            fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "private output must be new"
        );
        use std::os::unix::fs::OpenOptionsExt;
        Some(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .expect("new private diagnostic file"),
        )
    } else {
        None
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
        private_diagnostic: private_output.is_some(),
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
    if let Some(mut file) = private_output {
        let projection = result
            .private_projection
            .as_ref()
            .map_or("null".into(), |v| quote(v.as_str()));
        writeln!(
            file,
            "{{\"status\":\"{:?}\",\"projection\":{}}}",
            result.private_projection_status, projection
        )
        .expect("private diagnostic write");
    }
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
