//! Explicit private public-artifact acceptance. Ordinary CI compiles and skips it.
use eylema_contracts::{model_contract::ModelManifest, platform::RoleKind};
use eylema_model::{
    LoadBudget,
    cpu::{CpuBudget, CpuControl, CpuError, CpuModel, CpuRoleOutput, CpuRoleRequest, qwen_layout},
    load,
    tokenizer::{AdmissionBudget, PromptMessage, PromptRole, TextBudget, Tokenizer},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn numbers(text: &str, key: &str, count: usize) -> Result<Vec<f64>, &'static str> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("reference bound");
    }
    let marker = format!("\"{key}\": ");
    let start = text.find(&marker).ok_or("missing reference array")? + marker.len();
    let s = text.get(start..).ok_or("reference position")?;
    if !s.starts_with('[') {
        return Err("reference array");
    }
    let mut depth = 0;
    let mut end = None;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => (),
        }
    }
    let array = s
        .get(..=end.ok_or("reference extent")?)
        .ok_or("reference extent")?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| "reference allocation")?;
    for word in array
        .split(|c: char| c == '[' || c == ']' || c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
    {
        if values.len() == count {
            return Err("reference count");
        }
        let v: f64 = word.parse().map_err(|_| "reference number")?;
        if !v.is_finite() {
            return Err("reference finite");
        }
        values.push(v);
    }
    if values.len() != count {
        return Err("reference count");
    }
    Ok(values)
}
#[test]
fn reference_parser_rejects_nonfinite_and_wrong_counts() {
    assert_eq!(
        numbers("{\"x\": [[1, 2],[3]]}", "x", 3).unwrap(),
        [1., 2., 3.]
    );
    for text in [
        "{\"x\": [NaN]}",
        "{\"x\": [1,2]}",
        "{\"x\": []}",
        "{\"x\": 1}",
        "{\"x\": [1}",
    ] {
        assert!(numbers(text, "x", 1).is_err());
    }
}
fn control(start: Instant) -> CpuControl {
    let remaining = Duration::from_secs(1200)
        .checked_sub(start.elapsed())
        .expect("aggregate candidate deadline");
    CpuControl::new(Instant::now() + remaining.min(Duration::from_secs(600))).unwrap()
}
fn budget() -> CpuBudget {
    CpuBudget {
        context: 64,
        max_new_tokens: 8,
        max_resident_bytes: 2_684_354_560,
        max_work: 80_000_000_000,
    }
}
fn text_budget() -> TextBudget {
    TextBudget {
        max_resident_bytes: 134217728,
        max_tokens: 4096,
        max_work: 2_000_000,
    }
}
fn token(assets: &Path) -> Tokenizer {
    let parent = ModelManifest::from_bytes(include_bytes!(
        "../../eylema-contracts/tests/fixtures/qwen3-profile.v1.eyl"
    ))
    .unwrap();
    Tokenizer::load(
        &mut std::fs::File::open(assets.join("tokenizer.json")).unwrap(),
        &mut std::fs::File::open(assets.join("tokenizer_config.json")).unwrap(),
        &parent,
        AdmissionBudget {
            max_resident_bytes: 134217728,
            max_read_calls: 1024,
            max_interruptions: 16,
            max_work_per_artifact: 134217728,
        },
    )
    .unwrap()
}
fn compare(actual: &[f32], expected: &[f64]) -> f64 {
    assert_eq!(actual.len(), expected.len());
    let mut maximum = 0f64;
    for (i, (&a, &e)) in actual.iter().zip(expected).enumerate() {
        let error = (f64::from(a) - e).abs();
        assert!(
            a.is_finite() && error <= 0.001 + 0.0002 * e.abs(),
            "numerical mismatch at {i}"
        );
        maximum = maximum.max(error);
    }
    maximum
}
fn rss_sample() -> u64 {
    let output = std::process::Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    std::str::from_utf8(&output.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap()
        * 1024
}
fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
#[ignore = "requires explicit G30 weight/reference and accepted G29 tokenizer assets; six bounded local CPU jobs"]
fn pinned_model_numerics_four_roles_and_effective_stop() {
    let root = std::env::var_os("G30_ASSETS").expect("explicit G30 assets required");
    let root = Path::new(&root);
    let g29 = std::env::var_os("G29_ASSETS").expect("accepted G29 assets required");
    let start = Instant::now();
    let manifest =
        ModelManifest::from_bytes(include_bytes!("fixtures/g30/cpu-profile.v1.eyl")).unwrap();
    let layout = qwen_layout(&manifest).unwrap();
    let tokenizer = token(Path::new(&g29));
    let reference = std::fs::read_to_string(root.join("f64-reference.json")).unwrap();
    assert!(reference.len() <= 16 * 1024 * 1024);
    assert!(reference.contains("\"format\": \"g30.f64-reference.v1\""));
    assert!(reference.contains(
        "\"artifact_sha256\": \"f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b\""
    ));
    assert!(reference.contains("\"token\": 9707"));
    let layer_reference = numbers(&reference, "per_layer_hidden", 28 * 1024).unwrap();
    let logit_reference = numbers(&reference, "logits", 151936).unwrap();
    let weights = load(
        &mut std::fs::File::open(root.join("model.safetensors")).unwrap(),
        &manifest,
        &layout,
        LoadBudget {
            max_artifact_bytes: 1503300328,
            max_resident_content_bytes: 1503300328 + 4194304,
            max_read_calls: 65568,
            max_interruptions: 16,
        },
    )
    .unwrap();
    let model =
        CpuModel::admit(&manifest, &weights, &tokenizer, budget(), &control(start)).unwrap();
    let mut aggregate_work = model.admission_work();
    let mut jobs = Vec::new();
    // Candidate job 1: complete pretrained single-token numerical comparison.
    let hello = tokenizer.encode_text("Hello", text_budget()).unwrap();
    assert_eq!(hello.ids(), [9707]);
    let job_start = Instant::now();
    let mut session = model.begin(budget(), control(start)).unwrap();
    let logits = session.prefill(&hello).unwrap();
    let hidden_error = compare(session.last_layer_hidden().unwrap(), &layer_reference);
    let logit_error = compare(logits.values(), &logit_reference);
    let winner = logits
        .values()
        .iter()
        .enumerate()
        .max_by(|(ia, a), (ib, b)| a.total_cmp(b).then(ib.cmp(ia)))
        .unwrap()
        .0;
    let expected = logit_reference
        .iter()
        .enumerate()
        .max_by(|(ia, a), (ib, b)| a.total_cmp(b).then(ib.cmp(ia)))
        .unwrap()
        .0;
    assert_eq!(winner, expected);
    aggregate_work += session.observed_work();
    jobs.push(format!("{{\"job\":\"numerical\",\"comparisons\":180608,\"hidden_max_abs_error\":{hidden_error},\"logit_max_abs_error\":{logit_error},\"greedy\":{winner},\"work\":{},\"peak_logical_bytes\":{},\"elapsed_seconds\":{},\"sampled_rss_bytes\":{}}}",session.observed_work(),session.peak_logical_bytes(),job_start.elapsed().as_secs_f64(),rss_sample()));
    drop(logits);
    drop(session);
    // Candidate jobs 2–5: distinct typed proposals, each with a fresh session/control.
    for (index, role) in [
        RoleKind::Reasoning,
        RoleKind::Planning,
        RoleKind::Specialist,
        RoleKind::Critic,
    ]
    .into_iter()
    .enumerate()
    {
        let messages = [PromptMessage {
            role: PromptRole::User,
            content: [
                "Why is 2+2=4?",
                "Plan two checks for a sum.",
                "Türkçe: 2+2 kaçtır?",
                "",
            ][index],
        }];
        let request = match role {
            RoleKind::Reasoning => CpuRoleRequest::Reasoning(&messages),
            RoleKind::Planning => CpuRoleRequest::Planning(&messages),
            RoleKind::Specialist => CpuRoleRequest::Specialist(&messages),
            RoleKind::Critic => CpuRoleRequest::Critic {
                candidate: "2+2=5",
                evidence: "2+2=4",
            },
            _ => unreachable!(),
        };
        let job_start = Instant::now();
        let proposal = model.generate(request, budget(), control(start)).unwrap();
        assert_eq!(proposal.role(), role);
        assert_eq!(proposal.receipt().interface().role, role);
        assert!(proposal.receipt().proposal_only());
        assert_eq!(proposal.receipt().numeric_profile(), "qwen3.cpu-f32.v1");
        assert!(proposal.receipt().input_tokens() + proposal.receipt().generated_tokens() <= 64);
        assert!((1..=8).contains(&proposal.receipt().generated_tokens()));
        let output = match (role, proposal.output()) {
            (RoleKind::Reasoning, CpuRoleOutput::ReasoningSummary(s))
            | (RoleKind::Planning, CpuRoleOutput::PlanProposal(s))
            | (RoleKind::Specialist, CpuRoleOutput::DomainResult(s))
            | (RoleKind::Critic, CpuRoleOutput::EvaluationProposal(s)) => s,
            _ => panic!("wrong role output variant"),
        };
        assert!(!output.trim().is_empty());
        let digest = eylema_platform::sha256(output.as_bytes()).unwrap();
        aggregate_work += proposal.receipt().work();
        jobs.push(format!("{{\"job\":\"{}\",\"input_tokens\":{},\"generated_tokens\":{},\"output_utf8_bytes\":{},\"output_sha256\":\"{}\",\"work\":{},\"peak_logical_bytes\":{},\"elapsed_seconds\":{},\"sampled_rss_bytes\":{}}}",role.as_str(),proposal.receipt().input_tokens(),proposal.receipt().generated_tokens(),output.len(),hex(&digest),proposal.receipt().work(),proposal.receipt().peak_logical_bytes(),job_start.elapsed().as_secs_f64(),rss_sample()));
        assert!(aggregate_work <= 320_000_000_000);
        assert!(start.elapsed() < Duration::from_secs(1200));
    }
    // Candidate job 6: wait for numerical progress after initial workspace setup.
    let job_start = Instant::now();
    let c = control(start);
    let mut session = model.begin(budget(), c.clone()).unwrap();
    let threshold = c.observed_work() + 4096;
    let result = std::thread::scope(|scope| {
        let observer = c.clone();
        scope.spawn(move || {
            let end = Instant::now() + Duration::from_secs(30);
            while observer.observed_work() < threshold && Instant::now() < end {
                std::thread::yield_now();
            }
            observer.cancel();
        });
        session.prefill(&hello)
    });
    assert!(matches!(result, Err(CpuError::Cancelled)));
    assert!(c.observed_work() >= threshold);
    assert!(matches!(session.decode(9707), Err(CpuError::InvalidState)));
    assert!(matches!(
        session.last_layer_hidden(),
        Err(CpuError::InvalidState)
    ));
    aggregate_work += c.observed_work();
    jobs.push(format!("{{\"job\":\"effective_stop\",\"numerical_progress_observed\":true,\"proposal_returned\":false,\"cache_poisoned\":true,\"work\":{},\"elapsed_seconds\":{}}}",c.observed_work(),job_start.elapsed().as_secs_f64()));
    assert!(aggregate_work <= 320_000_000_000);
    assert!(start.elapsed() < Duration::from_secs(1200));
    let result = format!(
        "{{\"format\":\"g30.cpu-acceptance.v1\",\"candidate_jobs\":6,\"admission_work\":{},\"aggregate_work\":{aggregate_work},\"elapsed_seconds\":{},\"jobs\":[{}]}}\n",
        model.admission_work(),
        start.elapsed().as_secs_f64(),
        jobs.join(",")
    );
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join("candidate-result.json"))
        .unwrap();
    output.write_all(result.as_bytes()).unwrap();
}
