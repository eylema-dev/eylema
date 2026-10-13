#![cfg(target_os = "macos")]
mod support;
use eylema_development::{AcceptedTask, AuthorityResolver, Limits, NeedSpec};
use eylema_development_host::{CancelHandle, adapters::*};
use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use support::*;

fn fixture() -> Fixture {
    Fixture::with_limits(Limits {
        model_calls: 4,
        repairs: 3,
        wall_ms: 60_000,
        usage_units: 300_000,
    })
}
fn tool(f: &Fixture) -> CodexTool {
    CodexTool::fixture(Path::new(env!("CARGO_BIN_EXE_codex-probe")), &f.root).unwrap()
}
fn invoke(mode: &str, wall_ms: u64, bytes: usize) -> CallOutcome {
    let mut f = fixture();
    let mut w = f.workspace();
    let tool = tool(&f);
    let start = f.started;
    tool.invoke(
        &mut w,
        f.registry.get_mut(&f.key).unwrap(),
        &f.auth,
        || 1 + start.elapsed().as_millis() as u64,
        CodexRequest {
            prompt: mode,
            limits: CallLimits {
                wall_ms,
                output_bytes: bytes,
            },
            cancel: &CancelHandle::default(),
            unavailable_probe: false,
            private_diagnostic: false,
        },
    )
    .unwrap()
}
#[test]
fn explicit_fixture_returns_valid_proposal_usage_and_no_workspace_edit() {
    let mut f = fixture();
    let mut w = f.workspace();
    let tool = tool(&f);
    let start = f.started;
    let result = tool
        .invoke(
            &mut w,
            f.registry.get_mut(&f.key).unwrap(),
            &f.auth,
            || 1 + start.elapsed().as_millis() as u64,
            CodexRequest {
                prompt: "good",
                limits: CallLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &CancelHandle::default(),
                unavailable_probe: false,
                private_diagnostic: false,
            },
        )
        .unwrap();
    assert_eq!(result.status, CallStatus::Completed);
    assert_eq!(result.transport, Transport::ExplicitFixture);
    assert_eq!(result.reported_total_tokens, Some(16));
    assert_eq!(result.proposal.unwrap().changes[0].text, "Türkçe result\n");
    assert_eq!(
        fs::read_to_string(w.path().join("allowed.txt")).unwrap(),
        "base\n"
    );
    let run = f.registry.get_mut(&f.key).unwrap();
    assert!(run.active_call().is_none());
    assert_eq!(run.usage().model_calls, 1);
    assert_eq!(run.usage().reserved_units, 7000);
}
#[test]
fn synthetic_quota_unavailable_authentication_and_bad_output_are_explicit() {
    for (mode, expected) in [
        ("quota", CallStatus::Quota),
        ("unavailable", CallStatus::ModelUnavailable),
        ("auth", CallStatus::AuthenticationUnavailable),
        ("malformed", CallStatus::MalformedOutput),
        ("unknown-event", CallStatus::MalformedOutput),
        ("outside", CallStatus::MalformedOutput),
        ("tool", CallStatus::UnexpectedTool),
    ] {
        assert_eq!(invoke(mode, 5000, 8192).status, expected, "{mode}");
    }
}
#[test]
fn actual_timeout_partial_line_does_not_replace_stop_reason_with_parse_failure() {
    assert_eq!(
        invoke("partial-wait", 100, 8192).status,
        CallStatus::TimedOut
    );
}
#[test]
fn output_flood_is_bounded_and_reaped() {
    assert_eq!(invoke("flood", 5000, 123).status, CallStatus::OutputLimit);
}
#[test]
fn denied_or_exhausted_call_does_not_start_provider() {
    let mut f = Fixture::new();
    let mut w = f.workspace();
    let tool = tool(&f);
    let result = tool.invoke(
        &mut w,
        f.registry.get_mut(&f.key).unwrap(),
        &f.auth,
        || 1 + f.started.elapsed().as_millis() as u64,
        CodexRequest {
            prompt: "good",
            limits: CallLimits {
                wall_ms: 1000,
                output_bytes: 8192,
            },
            cancel: &CancelHandle::default(),
            unavailable_probe: false,
            private_diagnostic: false,
        },
    );
    assert!(result.is_err());
    assert!(
        !w.path()
            .parent()
            .unwrap()
            .join("provider/started.txt")
            .exists()
    );
}
#[test]
fn changed_tool_cannot_claim_installed_transport() {
    let f = fixture();
    assert!(
        CodexTool::installed(
            Path::new(env!("CARGO_BIN_EXE_codex-probe")),
            &f.root,
            &f.root
        )
        .is_err()
    );
}
#[test]
fn authority_revocation_stops_group_preserves_reason_and_acknowledges_charged_ticket() {
    struct Revocable {
        accepted: AcceptedTask,
        revoked: Arc<AtomicBool>,
    }
    impl AuthorityResolver for Revocable {
        fn resolve(&self, _: &NeedSpec) -> Option<AcceptedTask> {
            if self.revoked.load(Ordering::Acquire) {
                None
            } else {
                Some(self.accepted.clone())
            }
        }
    }
    let mut f = fixture();
    let mut w = f.workspace();
    let tool = tool(&f);
    let revoked = Arc::new(AtomicBool::new(false));
    let auth = Revocable {
        accepted: f.auth.0.clone(),
        revoked: revoked.clone(),
    };
    let path = w.path().parent().unwrap().join("provider/started.txt");
    let revoke = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !path.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(path.exists());
        revoked.store(true, Ordering::Release);
    });
    let start = f.started;
    let result = tool.invoke(
        &mut w,
        f.registry.get_mut(&f.key).unwrap(),
        &auth,
        || 1 + start.elapsed().as_millis() as u64,
        CodexRequest {
            prompt: "wait",
            limits: CallLimits {
                wall_ms: 5000,
                output_bytes: 8192,
            },
            cancel: &CancelHandle::default(),
            unavailable_probe: false,
            private_diagnostic: false,
        },
    );
    revoke.join().unwrap();
    assert_eq!(result.unwrap().status, CallStatus::AuthorityDenied);
    let run = f.registry.get_mut(&f.key).unwrap();
    assert!(run.active_call().is_none());
    assert_eq!(run.usage().reserved_units, 7000);
}

#[test]
fn explicit_cancel_after_provider_start_reaps_and_acknowledges_without_refund() {
    let mut f = fixture();
    let mut w = f.workspace();
    let tool = tool(&f);
    let cancel = CancelHandle::default();
    let signal = cancel.clone();
    let path = w.path().parent().unwrap().join("provider/started.txt");
    let requester = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !path.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(path.exists());
        signal.cancel();
    });
    let start = f.started;
    let result = tool
        .invoke(
            &mut w,
            f.registry.get_mut(&f.key).unwrap(),
            &f.auth,
            || 1 + start.elapsed().as_millis() as u64,
            CodexRequest {
                prompt: "wait",
                limits: CallLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &cancel,
                unavailable_probe: false,
                private_diagnostic: false,
            },
        )
        .unwrap();
    requester.join().unwrap();
    assert_eq!(result.status, CallStatus::Cancelled);
    let run = f.registry.get_mut(&f.key).unwrap();
    assert_eq!(run.state(), eylema_development::State::Cancelled);
    assert!(run.active_call().is_none());
    assert_eq!(run.usage().reserved_units, 7000);
}

#[test]
fn actual_clock_and_task_budget_stops_keep_cause_and_charged_acknowledgement() {
    for (clock, cause) in [
        (0, eylema_development::Error::ClockRegressed),
        (60_002, eylema_development::Error::BudgetExhausted),
    ] {
        let mut f = fixture();
        let mut w = f.workspace();
        let tool = tool(&f);
        let path = w.path().parent().unwrap().join("provider/started.txt");
        let before = 1 + f.started.elapsed().as_millis() as u64;
        let result = tool
            .invoke(
                &mut w,
                f.registry.get_mut(&f.key).unwrap(),
                &f.auth,
                || if path.exists() { clock } else { before },
                CodexRequest {
                    prompt: "wait",
                    limits: CallLimits {
                        wall_ms: 5000,
                        output_bytes: 8192,
                    },
                    cancel: &CancelHandle::default(),
                    unavailable_probe: false,
                    private_diagnostic: false,
                },
            )
            .unwrap();
        assert!(
            path.exists(),
            "provider must really start before clock/budget failure"
        );
        assert_eq!(result.status, CallStatus::CoordinatorStopped(cause));
        let run = f.registry.get_mut(&f.key).unwrap();
        assert!(run.active_call().is_none());
        assert_eq!(run.usage().reserved_units, 7000);
        if clock > 0 {
            assert_eq!(run.state(), eylema_development::State::BudgetExhausted);
        }
    }
}

#[test]
fn dangling_provider_schema_cannot_overwrite_outside_private_storage() {
    use std::os::unix::fs::symlink;
    let mut f = fixture();
    let mut w = f.workspace();
    let tool = tool(&f);
    let private = w.path().parent().unwrap().join("provider");
    fs::create_dir(&private).unwrap();
    let outside = f.root.join("must-not-be-created.txt");
    symlink(&outside, private.join("proposal-schema.json")).unwrap();
    let start = f.started;
    let result = tool.invoke(
        &mut w,
        f.registry.get_mut(&f.key).unwrap(),
        &f.auth,
        || 1 + start.elapsed().as_millis() as u64,
        CodexRequest {
            prompt: "good",
            limits: CallLimits {
                wall_ms: 5000,
                output_bytes: 8192,
            },
            cancel: &CancelHandle::default(),
            unavailable_probe: false,
            private_diagnostic: false,
        },
    );
    assert!(result.is_err());
    assert!(!outside.exists());
    assert_eq!(f.registry.get_mut(&f.key).unwrap().usage().model_calls, 0);
}

#[test]
fn executable_replacement_after_construction_is_rejected_before_model_reservation() {
    use std::os::unix::fs::symlink;
    for replace_with_symlink in [false, true] {
        let mut f = fixture();
        let executable = f.root.join("mutable-cli");
        fs::copy(env!("CARGO_BIN_EXE_codex-probe"), &executable).unwrap();
        let tool = CodexTool::fixture(&executable, &f.root).unwrap();
        fs::remove_file(&executable).unwrap();
        if replace_with_symlink {
            symlink("/usr/bin/true", &executable).unwrap();
        } else {
            fs::copy("/usr/bin/true", &executable).unwrap();
        }
        let mut w = f.workspace();
        let start = f.started;
        let result = tool.invoke(
            &mut w,
            f.registry.get_mut(&f.key).unwrap(),
            &f.auth,
            || 1 + start.elapsed().as_millis() as u64,
            CodexRequest {
                prompt: "good",
                limits: CallLimits {
                    wall_ms: 5000,
                    output_bytes: 8192,
                },
                cancel: &CancelHandle::default(),
                unavailable_probe: false,
                private_diagnostic: false,
            },
        );
        assert!(result.is_err(), "changed executable must not launch");
        assert_eq!(f.registry.get_mut(&f.key).unwrap().usage().model_calls, 0);
    }
}

#[test]
fn non_chatgpt_or_missing_session_is_refused_before_model_request() {
    for mode in ["apikey", "missing"] {
        let mut f = fixture();
        fs::write(f.root.join("fixture-auth-mode.txt"), mode).unwrap();
        let tool = tool(&f);
        let mut w = f.workspace();
        let start = f.started;
        let result = tool
            .invoke(
                &mut w,
                f.registry.get_mut(&f.key).unwrap(),
                &f.auth,
                || 1 + start.elapsed().as_millis() as u64,
                CodexRequest {
                    prompt: "good",
                    limits: CallLimits {
                        wall_ms: 5000,
                        output_bytes: 8192,
                    },
                    cancel: &CancelHandle::default(),
                    unavailable_probe: false,
                    private_diagnostic: false,
                },
            )
            .unwrap();
        assert_eq!(result.status, CallStatus::AuthenticationUnavailable);
        assert_eq!(f.registry.get_mut(&f.key).unwrap().usage().model_calls, 0);
        assert!(
            !w.path()
                .parent()
                .unwrap()
                .join("provider/started.txt")
                .exists()
        );
    }
}

#[test]
fn invalid_live_probe_mode_is_rejected_before_tool_or_home_access() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
        .args(["--live", "invalid-mode"])
        .env_clear()
        .env("HOME", "/g40-nonexistent-home")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown mode"));
}

#[test]
fn synthetic_probe_evidence_reports_actual_selected_model() {
    for (mode, model, status) in [
        ("success", "gpt-6.1-sol", "Completed"),
        (
            "unavailable",
            "gpt-6.1-sol-g40-unavailable",
            "ModelUnavailable",
        ),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
            .args(["--fixture-observation", mode])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&format!("\"requested_model\":\"{model}\"")));
        assert!(text.contains(&format!("\"status\":\"{status}\"")));
        assert!(text.contains("\"transport\":\"ExplicitFixture\""));
    }
}

#[test]
fn passive_provider_error_item_is_a_failure_and_never_a_tool_or_proposal() {
    let result = invoke("item-error", 5000, 8192);
    assert_eq!(result.status, CallStatus::ModelUnavailable);
    assert!(result.proposal.is_none());
}

#[test]
fn bounded_diagnostics_distinguish_provider_exit_and_incomplete_failures() {
    for (mode, path, code) in [
        ("diag-schema", "ProviderError", 1),
        ("diag-exit", "ProcessOrInput", 7),
        ("diag-incomplete", "IncompleteTurn", 0),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
            .args(["--fixture-observation", mode])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains(&format!("\"failure_path\":\"{path}\"")),
            "{mode}"
        );
        assert!(text.contains(&format!("\"child_exit_code\":{code}")));
        assert!(!text.contains("G40_DIAGNOSTIC_SECRET_MARKER"));
        if mode == "diag-schema" {
            assert!(text.contains("\"message_category\":\"SchemaMention\""));
            assert!(text.contains("\"input_tokens\":123"));
            assert!(text.contains("\"cached_input_tokens\":70"));
            assert!(text.contains("\"output_tokens\":7"));
            assert!(text.contains("\"reasoning_output_tokens\":3"));
        }
        if mode == "diag-exit" {
            assert!(text.contains("\"completed_turn_seen\":true"));
        }
        if mode == "diag-incomplete" {
            assert!(text.contains("\"completed_turn_seen\":false"));
        }
    }
}

#[test]
fn diagnostics_keep_missing_usage_unknown_and_transport_synthetic() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
        .args(["--fixture-observation", "diag-missing-usage"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"reported_usage\":null"));
    assert!(text.contains("\"failure_path\":\"Completed\""));
    assert!(text.contains("\"transport\":\"ExplicitFixture\""));
}

#[test]
fn diagnostics_do_not_export_overlapping_input_cache_buckets_as_valid_usage() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
        .args(["--fixture-observation", "diag-invalid-usage"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"invalid_usage_seen\":true"));
    assert!(text.contains("\"reported_usage\":null"));
}

#[test]
fn late_capture_stream_rejection_is_distinct_from_proposal_rejection() {
    for (mode, route) in [
        ("diag-late-stream", FailurePath::StreamRejected),
        ("outside", FailurePath::ProposalRejected),
    ] {
        let mut f = fixture();
        let mut w = f.workspace();
        let tool = tool(&f);
        let start = f.started;
        let result = tool
            .invoke(
                &mut w,
                f.registry.get_mut(&f.key).unwrap(),
                &f.auth,
                || 1 + start.elapsed().as_millis() as u64,
                CodexRequest {
                    prompt: mode,
                    limits: CallLimits {
                        wall_ms: 5000,
                        output_bytes: 8192,
                    },
                    cancel: &CancelHandle::default(),
                    unavailable_probe: false,
                    private_diagnostic: false,
                },
            )
            .unwrap();
        assert_eq!(result.status, CallStatus::MalformedOutput, "{mode}");
        assert!(result.proposal.is_none());
        let diagnostics = result.diagnostics.unwrap();
        assert!(diagnostics.completed_turn_seen && diagnostics.final_message_seen);
        assert_eq!(diagnostics.failure_path, route, "{mode}");
        let run = f.registry.get_mut(&f.key).unwrap();
        assert!(run.active_call().is_none());
        assert_eq!(run.usage().model_calls, 1);
        assert_eq!(run.usage().reserved_units, 7000);
        assert_eq!(
            fs::read_to_string(w.path().join("allowed.txt")).unwrap(),
            "base\n"
        );
    }
}

#[test]
fn public_error_evidence_is_whitelisted_and_does_not_downgrade_failure() {
    for mode in ["diag-feature-notice", "diag-private-identifiers"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
            .args(["--fixture-observation", mode])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let expected = if mode == "diag-feature-notice" {
            "TransportFailed"
        } else {
            "ModelUnavailable"
        };
        assert!(text.contains(&format!("\"status\":\"{expected}\"")));
        assert!(text.contains("\"proposal_changes\":0"));
        for secret in [
            "G40_DIAGNOSTIC_SECRET_MARKER",
            "unknown_private_feature",
            "private_value",
            "private_model",
            "private_other",
        ] {
            assert!(!text.contains(secret), "{mode}");
        }
        if mode == "diag-feature-notice" {
            assert!(text.contains("\"error_event_count\":1"));
            assert!(text.contains("\"unclassified_error_events\":0"));
            assert!(text.contains("\"final_proposal_valid\":true"));
            assert!(text.contains("\"item_error_seen\":true"));
            assert!(text.contains("\"UnderDevelopmentFeatures\""));
            assert!(text.contains("\"memories\""));
            assert!(text.contains("\"code_mode\""));
            assert!(text.contains("\"suppress_unstable_features_warning\""));
        } else {
            assert!(text.contains("\"error_event_count\":2"));
            assert!(text.contains("\"unclassified_error_events\":1"));
            assert!(text.contains("\"top_level_error_seen\":true"));
            assert!(text.contains("\"turn_failed_seen\":true"));
            assert!(text.contains("\"UnsupportedReasoningEffort\""));
            assert!(text.contains("\"public_error_identifiers\":[]"));
        }
    }
}

#[test]
fn public_error_identifier_union_is_bounded_and_deduplicated() {
    let result = invoke("diag-many-identifiers", 5000, 8192);
    assert_eq!(result.status, CallStatus::TransportFailed);
    assert!(result.proposal.is_none());
    let diagnostics = result.diagnostics.unwrap();
    let identifiers = diagnostics
        .public_error_identifiers
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(identifiers.len(), 16);
    assert_eq!(
        identifiers
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    assert!(diagnostics.error_identifier_limit_reached);
    assert!(diagnostics.top_level_error_seen);
    assert!(!diagnostics.item_error_seen && !diagnostics.turn_failed_seen);
    assert!(!format!("{diagnostics:?}").contains("G40_DIAGNOSTIC_SECRET_MARKER"));
}

#[test]
fn diagnostic_proposal_validation_keeps_missing_and_outside_scope_distinct() {
    for (mode, expected) in [("outside", Some(false)), ("diag-private-identifiers", None)] {
        let result = invoke(mode, 5000, 8192);
        assert!(result.proposal.is_none());
        assert_eq!(result.diagnostics.unwrap().final_proposal_valid, expected);
    }
}

#[test]
fn private_projection_is_opt_in_bounded_and_never_in_public_stdout() {
    for (mode, status) in [
        ("diag-projection-safe", "Available"),
        ("diag-projection-sensitive", "WithheldSensitive"),
        ("diag-projection-words", "LimitExceeded"),
        ("diag-projection-bytes", "LimitExceeded"),
        ("diag-projection-origin", "NotEligible"),
        ("diag-projection-multiple", "NotEligible"),
    ] {
        let f = fixture();
        let path = f.root.join("private-projection.json");
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
            .args(["--fixture-observation", mode, "--private-diagnostic-output"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{mode}");
        let public = String::from_utf8(output.stdout).unwrap();
        assert!(public.contains("\"status\":\"TransportFailed\""));
        assert!(public.contains("\"proposal_changes\":0"));
        assert!(!public.contains("is disabled via configuration"));
        let private = fs::read_to_string(&path).unwrap();
        assert!(
            private.contains(&format!("\"status\":\"{status}\"")),
            "{mode}"
        );
        for content in [
            "OPAQUE_CANARY_9X",
            "/fixture/local/value",
            "fixture.invalid",
            "person@",
            "sk-fixture",
        ] {
            assert!(
                !public.contains(content) && !private.contains(content),
                "{mode}"
            );
        }
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        if mode == "diag-projection-safe" {
            assert!(
                private.contains("code_mode_host is disabled via configuration remove <redacted>")
            );
        } else {
            assert!(private.contains("\"projection\":null"));
        }
    }
}

#[test]
fn private_output_existing_file_is_refused_before_tool_or_home_access() {
    let f = fixture();
    let path = f.root.join("preserved.txt");
    fs::write(&path, "preserve").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
        .args(["--live", "success", "--private-diagnostic-output"])
        .arg(&path)
        .env("HOME", f.root.join("absent-home"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("private output must be new")
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "preserve");
}

#[test]
fn quoted_configuration_values_are_masked_at_every_token_position() {
    for (mode, expected) in [
        (
            "diag-projection-assigned",
            Some("code_mode_host configuration <redacted>"),
        ),
        (
            "diag-projection-parenthesized",
            Some("code_mode_host configuration <redacted>"),
        ),
        ("diag-projection-unmatched", None),
        ("diag-projection-escaped", None),
        (
            "diag-projection-embedded",
            Some("code_mode_host configuration <redacted>"),
        ),
        (
            "diag-projection-single",
            Some("code_mode_host configuration <redacted>"),
        ),
        (
            "diag-projection-backtick",
            Some("code_mode_host configuration <redacted>"),
        ),
        (
            "diag-projection-plain",
            Some("code_mode_host configuration old new"),
        ),
    ] {
        let f = fixture();
        let path = f.root.join("private-quotes.json");
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_codex-probe"))
            .args(["--fixture-observation", mode, "--private-diagnostic-output"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{mode}");
        let public = String::from_utf8(output.stdout).unwrap();
        assert!(public.contains("\"status\":\"TransportFailed\""));
        assert!(public.contains("\"proposal_changes\":0"));
        assert!(!public.contains("code_mode_host configuration"), "{mode}");
        let private = fs::read_to_string(&path).unwrap();
        if let Some(expected) = expected {
            assert!(private.contains("\"status\":\"Available\""), "{mode}");
            assert!(
                private.contains(&format!("\"projection\":\"{expected}\"")),
                "{mode}: {private}"
            );
        } else {
            assert!(
                private.contains("\"status\":\"WithheldSensitive\""),
                "{mode}: {private}"
            );
            assert!(private.contains("\"projection\":null"), "{mode}");
        }
    }
}
