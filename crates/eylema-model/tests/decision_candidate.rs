//! Explicit G57 pinned-artifact acceptance. Ordinary CI compiles and ignores it.
use eylema_contracts::model_contract::ModelManifest;
use eylema_model::{
    LoadBudget, LoadedWeights,
    cpu::CpuControl,
    decision::{
        DecisionBatch, DecisionBudget, DecisionModel, DecisionOption, DecisionQuestion,
        DecisionReason, decision_layout,
    },
    load,
    tokenizer::{AdmissionBudget, DecisionTokenizer, TextBudget},
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const MODEL_BYTES: u64 = 1_191_588_280;
const MODEL_SHA: &str = "27cd75a405b9c1b46b59abfd88aaa209e6fed2a1972cde9b70e7659537c5e65b";
const INSTRUCTION: &str = "Judge whether the candidate directly answers the query.";
const EN_QUERY: &str = "What is the capital of France?";
const EN_YES: &str = "Paris is the capital of France.";
const EN_NO: &str = "The Pacific Ocean is the largest ocean.";
const TR_QUERY: &str = "Türkiye'nin başkenti neresidir?";
const TR_YES: &str = "Türkiye'nin başkenti Ankara'dır.";
const TR_NO: &str = "Pasifik Okyanusu en büyük okyanustur.";
const EN_OPTIONS: [DecisionOption<'static>; 2] = [
    DecisionOption {
        id: "positive",
        candidate: EN_YES,
    },
    DecisionOption {
        id: "negative",
        candidate: EN_NO,
    },
];
const TR_OPTIONS: [DecisionOption<'static>; 2] = [
    DecisionOption {
        id: "positive",
        candidate: TR_YES,
    },
    DecisionOption {
        id: "negative",
        candidate: TR_NO,
    },
];

fn manifest() -> ModelManifest {
    ModelManifest::from_bytes(include_bytes!("fixtures/g57/cpu-profile.v1.eyl")).unwrap()
}
fn admission() -> AdmissionBudget {
    AdmissionBudget {
        max_resident_bytes: 134_217_728,
        max_read_calls: 1024,
        max_interruptions: 16,
        max_work_per_artifact: 134_217_728,
    }
}
fn budget(context: usize) -> DecisionBudget {
    DecisionBudget {
        context,
        max_resident_bytes: 2_560 * 1024 * 1024,
        max_work: 2_500_000_000_000,
    }
}
fn text_budget(context: usize) -> TextBudget {
    TextBudget {
        max_resident_bytes: 134_217_728,
        max_tokens: context,
        max_work: 2_000_000,
    }
}
fn pair_tokens(tokenizer: &DecisionTokenizer, control: &CpuControl) -> Vec<Vec<u32>> {
    [
        (EN_QUERY, EN_YES),
        (EN_QUERY, EN_NO),
        (TR_QUERY, TR_YES),
        (TR_QUERY, TR_NO),
    ]
    .into_iter()
    .map(|(query, candidate)| {
        tokenizer
            .encode_pair(INSTRUCTION, query, candidate, text_budget(128), control)
            .expect("frozen pair tokenizes within the approved bound")
            .ids()
            .to_vec()
    })
    .collect()
}
fn batches() -> [DecisionQuestion<'static>; 2] {
    [
        DecisionQuestion {
            id: "en",
            instruction: INSTRUCTION,
            query: EN_QUERY,
            options: &EN_OPTIONS,
        },
        DecisionQuestion {
            id: "tr",
            instruction: INSTRUCTION,
            query: TR_QUERY,
            options: &TR_OPTIONS,
        },
    ]
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn float_array(
    text: &str,
    key: &str,
    occurrence: usize,
    count: usize,
) -> Result<Vec<f64>, &'static str> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("reference output bound");
    }
    let marker = format!("\"{key}\":");
    let mut rest = text;
    for _ in 0..=occurrence {
        let at = rest.find(&marker).ok_or("missing reference field")? + marker.len();
        rest = rest.get(at..).ok_or("reference field extent")?;
    }
    let rest = rest.trim_start();
    if !rest.starts_with('[') {
        return Err("reference array");
    }
    let mut depth = 0usize;
    let mut end = None;
    for (index, ch) in rest.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1).ok_or("reference nesting")?;
                if depth == 0 {
                    end = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let array = rest
        .get(..=end.ok_or("reference array extent")?)
        .ok_or("reference array extent")?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| "reference allocation")?;
    for word in array
        .split(|ch: char| ch == '[' || ch == ']' || ch == ',' || ch.is_ascii_whitespace())
        .filter(|word| !word.is_empty())
    {
        if values.len() == count {
            return Err("reference item count");
        }
        let value = word.parse::<f64>().map_err(|_| "reference number")?;
        if !value.is_finite() {
            return Err("reference finite");
        }
        values.push(value);
    }
    if values.len() != count {
        return Err("reference item count");
    }
    Ok(values)
}
fn integer_field(text: &str, key: &str, occurrence: usize) -> Result<usize, &'static str> {
    let marker = format!("\"{key}\":");
    let mut rest = text;
    for _ in 0..=occurrence {
        let at = rest.find(&marker).ok_or("missing integer field")? + marker.len();
        rest = rest.get(at..).ok_or("integer field extent")?;
    }
    let rest = rest.trim_start();
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return Err("integer field format");
    }
    rest[..digits].parse().map_err(|_| "integer field overflow")
}
fn assert_job_work(control: &CpuControl, start: u64) {
    let delta = control
        .observed_work()
        .checked_sub(start)
        .expect("monotonic work counter");
    assert!(
        delta <= 800_000_000_000,
        "candidate job exceeded its approved work cap"
    );
}
fn job_control(group: &eylema_model::cpu::CpuWorkGroup, task_deadline: Instant) -> CpuControl {
    let deadline = (Instant::now() + Duration::from_secs(600)).min(task_deadline);
    CpuControl::new_job(deadline, 800_000_000_000, group.clone()).expect("candidate job admission")
}
fn rss_sample_bytes() -> u64 {
    let pid = std::process::id().to_string();
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .expect("macOS process RSS sampler");
    assert!(output.status.success());
    std::str::from_utf8(&output.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap()
        .checked_mul(1024)
        .unwrap()
}
fn write_tokens(path: &Path, tokens: &[Vec<u32>]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    for row in tokens {
        for (index, token) in row.iter().enumerate() {
            if index > 0 {
                file.write_all(b" ")?;
            }
            write!(file, "{token}")?;
        }
        file.write_all(b"\n")?;
    }
    file.sync_all()
}
struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn run_reference(assets: &Path, tokens_path: &Path, output_path: &Path) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(600);
    let oracle = std::env::var_os("G57_ORACLE").ok_or("G57_ORACLE is required")?;
    let source = File::open(assets.join("model.safetensors")).map_err(|_| "weight open failed")?;
    if source
        .metadata()
        .map_err(|_| "weight metadata failed")?
        .len()
        != MODEL_BYTES
    {
        return Err("pinned weight length mismatch".into());
    }
    let child = Command::new(oracle)
        .args(["--weights-stdin", "--tokens"])
        .arg(tokens_path)
        .arg("--output")
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "oracle spawn failed")?;
    let mut child = ChildGuard(Some(child));
    let input = child
        .0
        .as_mut()
        .and_then(|c| c.stdin.take())
        .ok_or("oracle stdin unavailable")?;
    let (send, receive) = mpsc::channel();
    let writer = thread::spawn(move || {
        let result = (|| -> Result<(u64, String), &'static str> {
            let mut input = input;
            let mut source = source;
            let mut digest = eylema_platform::sha256_stream::Sha256Stream::new(MODEL_BYTES)
                .map_err(|_| "hash init failed")?;
            let mut buffer = [0u8; 65536];
            let mut total = 0u64;
            loop {
                let count = source.read(&mut buffer).map_err(|_| "weight read failed")?;
                if count == 0 {
                    break;
                }
                total = total
                    .checked_add(count as u64)
                    .ok_or("weight length overflow")?;
                if total > MODEL_BYTES {
                    return Err("weight exceeds pinned size");
                }
                digest
                    .update(&buffer[..count])
                    .map_err(|_| "hash update failed")?;
                input
                    .write_all(&buffer[..count])
                    .map_err(|_| "oracle write failed")?;
            }
            drop(input);
            Ok((
                total,
                hex(&digest.finish().map_err(|_| "hash finish failed")?),
            ))
        })();
        let _ = send.send(result);
    });
    let mut transfer = None;
    let mut status = None;
    loop {
        if Instant::now() >= deadline {
            return Err("reference process deadline".into());
        }
        if transfer.is_none()
            && let Ok(value) = receive.try_recv()
        {
            transfer = Some(Ok(value?));
        }
        if status.is_none() {
            status = child
                .0
                .as_mut()
                .ok_or("oracle missing")?
                .try_wait()
                .map_err(|_| "oracle wait failed")?;
            if status.is_some_and(|s| !s.success()) {
                return Err("reference process failed".into());
            }
        }
        if transfer.is_some() && status.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let identity = transfer.unwrap().map_err(str::to_owned)?;
    writer.join().map_err(|_| "oracle writer panicked")?;
    if identity.0 != MODEL_BYTES || identity.1 != MODEL_SHA {
        return Err("transmitted weight identity mismatch".into());
    }
    child.0.take();
    if fs::metadata(output_path)
        .map_err(|_| "reference output metadata failed")?
        .len()
        > 16 * 1024 * 1024
    {
        return Err("reference output bound".into());
    }
    let text = fs::read_to_string(output_path).map_err(|_| "reference read failed")?;
    if !text.contains("\"format\":\"g57.f64-reference.v1\"") {
        return Err("reference format mismatch".into());
    }
    Ok(text)
}
fn compare(actual: &[f32], expected: &[f64]) -> f64 {
    assert_eq!(actual.len(), expected.len());
    let mut maximum = 0.0f64;
    for (index, (&got, &want)) in actual.iter().zip(expected).enumerate() {
        let error = (f64::from(got) - want).abs();
        assert!(
            got.is_finite() && error <= 0.001 + 0.0002 * want.abs(),
            "reference mismatch at element {index}"
        );
        maximum = maximum.max(error);
    }
    maximum
}
fn reference_probability(head: [f64; 2]) -> (f64, u32) {
    let delta = head[0] - head[1];
    let p = if delta >= 0.0 {
        1.0 / (1.0 + (-delta).exp())
    } else {
        let e = delta.exp();
        e / (1.0 + e)
    };
    (p, (p * 1_000_000.0).round_ties_even() as u32)
}
fn reference_reason(scores: &[u32]) -> DecisionReason {
    let mut sorted = scores.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    if sorted.len() > 1 && sorted[0] == sorted[1] {
        DecisionReason::Tie
    } else if sorted[0] < 750_000 {
        DecisionReason::BelowFloor
    } else if sorted.len() > 1 && sorted[0] - sorted[1] < 100_000 {
        DecisionReason::InsufficientMargin
    } else {
        DecisionReason::Chosen
    }
}
#[test]
fn independent_reference_scoring_vectors_cover_rounding_and_abstention_rules() {
    assert_eq!(
        reference_probability([2.0, 0.0]),
        (1.0 / (1.0 + (-2.0f64).exp()), 880_797)
    );
    assert_eq!(
        reference_reason(&[750_000, 650_000]),
        DecisionReason::Chosen
    );
    assert_eq!(
        reference_reason(&[750_000, 650_001]),
        DecisionReason::InsufficientMargin
    );
    assert_eq!(reference_reason(&[749_999, 0]), DecisionReason::BelowFloor);
    assert_eq!(reference_reason(&[880_797, 880_797]), DecisionReason::Tie);
}
fn pair_score<'a>(
    result: &'a eylema_model::decision::DecisionResult,
    question: &str,
    option: &str,
) -> &'a eylema_model::decision::OptionScore {
    result
        .questions()
        .iter()
        .find(|row| row.question_id() == question)
        .unwrap()
        .scores()
        .iter()
        .find(|score| score.option_id() == option)
        .unwrap()
}
#[test]
#[ignore = "requires the explicitly approved pinned G57 artifact and accepted G29 tokenizer assets; one bounded six-job CPU acceptance"]
fn actual_pinned_reranker() {
    let assets = std::env::var_os("G57_ASSETS").expect("private G57 asset directory required");
    let assets = Path::new(&assets);
    let g29_dir = std::env::var_os("G29_ASSETS").expect("accepted G29 tokenizer assets required");
    let g29 = Path::new(&g29_dir);
    let manifest = manifest();
    let start = Instant::now();
    let task_deadline = start + Duration::from_secs(1200);
    let group = eylema_model::cpu::CpuWorkGroup::new(2_500_000_000_000).unwrap();
    let control = CpuControl::new_job(
        start + Duration::from_secs(600),
        2_500_000_000_000,
        group.clone(),
    )
    .unwrap();
    let tokenizer = DecisionTokenizer::load(
        &mut File::open(g29.join("tokenizer.json")).unwrap(),
        &mut File::open(assets.join("tokenizer_config.json")).unwrap(),
        &manifest,
        admission(),
        &control,
    )
    .unwrap();
    let tokens = pair_tokens(&tokenizer, &control);
    assert!(tokens.iter().all(|row| (1..=128).contains(&row.len())));
    let tokens_path = assets.join("g57-pinned-tokens.txt");
    let reference_path = assets.join("g57-f64-reference.json");
    write_tokens(&tokens_path, &tokens).unwrap();

    // Job 1 reference: an isolated F64 process reads the verified transmitted bytes before native scoring.
    let reference_start = Instant::now();
    let reference = run_reference(assets, &tokens_path, &reference_path).unwrap();
    let reference_seconds = reference_start.elapsed().as_secs_f64();
    assert!(reference_seconds < 600.0);
    assert_eq!(reference.matches("\"tokens\":").count(), 4);
    for (index, row) in tokens.iter().enumerate() {
        assert_eq!(
            integer_field(&reference, "tokens", index).unwrap(),
            row.len()
        );
    }

    // Share one immutable loaded snapshot and one admitted frontend across candidate jobs.
    let layout = decision_layout(&manifest).unwrap();
    let weights: LoadedWeights = load(
        &mut File::open(assets.join("model.safetensors")).unwrap(),
        &manifest,
        &layout,
        LoadBudget {
            max_artifact_bytes: MODEL_BYTES,
            max_resident_content_bytes: MODEL_BYTES + 4_194_304,
            max_read_calls: 65568,
            max_interruptions: 16,
        },
    )
    .unwrap();
    let model =
        DecisionModel::admit(&manifest, &weights, &tokenizer, budget(128), &control).unwrap();
    let questions = batches();
    let batch = DecisionBatch::new(&questions).unwrap();
    let encoded_first = tokenizer
        .encode_pair(INSTRUCTION, EN_QUERY, EN_YES, text_budget(128), &control)
        .unwrap();

    // Candidate job 1: all final-position per-layer states and yes/no head match the independent oracle.
    let job1 = job_control(&group, task_deadline);
    let first_start = Instant::now();
    let first_work = job1.observed_work();
    let diagnostic = model
        .diagnose(&encoded_first, budget(128), job1.clone())
        .unwrap();
    let job1_seconds = first_start.elapsed().as_secs_f64();
    let job1_work = job1.observed_work() - first_work;
    let layer_error = compare(
        diagnostic.layers(),
        &float_array(&reference, "layers", 0, 28 * 1024).unwrap(),
    );
    let head_error = compare(
        &diagnostic.head(),
        &float_array(&reference, "head", 0, 2).unwrap(),
    );
    assert!(job1_seconds < 600.0);
    assert_job_work(&job1, first_work);

    // Candidate job 2: complete bilingual two-question/four-pair batch and fixed choice rules.
    let job2 = job_control(&group, task_deadline);
    let batch_start = Instant::now();
    let score_start_work = job2.observed_work();
    let result = model.score(&batch, budget(128), job2.clone()).unwrap();
    let job2_seconds = batch_start.elapsed().as_secs_f64();
    let job2_work = job2.observed_work() - score_start_work;
    assert_eq!(result.questions().len(), 2);
    assert_eq!(result.questions()[0].question_id(), "en");
    assert_eq!(result.questions()[1].question_id(), "tr");
    for question in result.questions() {
        assert_eq!(question.choice(), Some("positive"));
        assert_eq!(question.reason(), DecisionReason::Chosen);
    }
    let mut oracle_micros = Vec::new();
    for index in 0..4 {
        let hidden = float_array(&reference, "layers", index, 28 * 1024).unwrap();
        let head = float_array(&reference, "head", index, 2).unwrap();
        let (question, option) = [
            ("en", "positive"),
            ("en", "negative"),
            ("tr", "positive"),
            ("tr", "negative"),
        ][index];
        assert!(!hidden.is_empty());
        compare(&pair_score(&result, question, option).logits(), &head);
        let pair = pair_score(&result, question, option);
        let (probability, micros) = reference_probability([head[0], head[1]]);
        assert!(
            (pair.probability() - probability).abs() <= 1.0e-7,
            "probability differs from independent reference"
        );
        assert_eq!(
            pair.score_micros(),
            micros,
            "rounded score differs from independent reference"
        );
        oracle_micros.push(micros);
    }
    for (question_index, question) in result.questions().iter().enumerate() {
        let expected = reference_reason(&oracle_micros[question_index * 2..question_index * 2 + 2]);
        assert_eq!(
            question.reason(),
            expected,
            "choice floor/margin differs from independent reference"
        );
        if expected == DecisionReason::Chosen {
            let expected_option =
                if oracle_micros[question_index * 2] >= oracle_micros[question_index * 2 + 1] {
                    "positive"
                } else {
                    "negative"
                };
            assert_eq!(question.choice(), Some(expected_option));
        } else {
            assert_eq!(question.choice(), None);
        }
    }
    assert!(job2_seconds < 600.0);
    assert_job_work(&job2, score_start_work);
    let forward = result;

    // Candidate job 3: reverse question and option order; keyed pair scores prove per-pair reset.
    let reverse_tr = [
        DecisionOption {
            id: "negative",
            candidate: TR_NO,
        },
        DecisionOption {
            id: "positive",
            candidate: TR_YES,
        },
    ];
    let reverse_en = [
        DecisionOption {
            id: "negative",
            candidate: EN_NO,
        },
        DecisionOption {
            id: "positive",
            candidate: EN_YES,
        },
    ];
    let reverse_questions = [
        DecisionQuestion {
            id: "tr",
            instruction: INSTRUCTION,
            query: TR_QUERY,
            options: &reverse_tr,
        },
        DecisionQuestion {
            id: "en",
            instruction: INSTRUCTION,
            query: EN_QUERY,
            options: &reverse_en,
        },
    ];
    let reverse_batch = DecisionBatch::new(&reverse_questions).unwrap();
    let job3 = job_control(&group, task_deadline);
    let reverse_start = Instant::now();
    let reverse_start_work = job3.observed_work();
    let reverse = model
        .score(&reverse_batch, budget(128), job3.clone())
        .unwrap();
    let job3_seconds = reverse_start.elapsed().as_secs_f64();
    let job3_work = job3.observed_work() - reverse_start_work;
    assert!(job3_seconds < 600.0);
    assert_job_work(&job3, reverse_start_work);
    for (question, option) in [
        ("en", "positive"),
        ("en", "negative"),
        ("tr", "positive"),
        ("tr", "negative"),
    ] {
        assert_eq!(
            pair_score(&forward, question, option).logits(),
            pair_score(&reverse, question, option).logits()
        );
        assert_eq!(
            pair_score(&forward, question, option).score_micros(),
            pair_score(&reverse, question, option).score_micros()
        );
    }
    assert_eq!(reverse.questions()[0].choice(), Some("positive"));
    assert_eq!(reverse.questions()[1].choice(), Some("positive"));

    // Candidate job 5: negative preflight cases do no backbone work and publish no batch result.
    let job5 = job_control(&group, task_deadline);
    let before_resident = job5.observed_work();
    assert!(matches!(
        model.score(
            &batch,
            DecisionBudget {
                max_resident_bytes: 1,
                ..budget(128)
            },
            job5.clone()
        ),
        Err(eylema_model::cpu::CpuError::ResidentLimit)
    ));
    assert_eq!(job5.observed_work(), before_resident);
    assert_job_work(&job5, before_resident);
    assert!(DecisionBatch::new(&[]).is_err());
    let before_work = job5.observed_work();
    assert!(matches!(
        model.score(
            &batch,
            DecisionBudget {
                max_work: 200_000_000_000_001,
                ..budget(128)
            },
            job5.clone()
        ),
        Err(eylema_model::cpu::CpuError::WorkLimit)
    ));
    assert_eq!(job5.observed_work(), before_work);
    let job5_work = job5.observed_work() - before_resident;

    // Candidate job 6: exact token context succeeds; one below fails before a decoder forward.
    let exact = encoded_first.ids().len();
    let exact_q = [DecisionQuestion {
        id: "en",
        instruction: INSTRUCTION,
        query: EN_QUERY,
        options: &EN_OPTIONS[..1],
    }];
    let exact_batch = DecisionBatch::new(&exact_q).unwrap();
    let job6 = job_control(&group, task_deadline);
    let exact_start_work = job6.observed_work();
    let exact_start = Instant::now();
    assert_eq!(
        model
            .score(&exact_batch, budget(exact), job6.clone())
            .unwrap()
            .questions()
            .len(),
        1
    );
    assert_job_work(&job6, exact_start_work);
    let exact_seconds = exact_start.elapsed().as_secs_f64();
    let below_start = job6.observed_work();
    let below = model.score(&exact_batch, budget(exact - 1), job6.clone());
    assert!(matches!(
        below,
        Err(eylema_model::cpu::CpuError::ContextLimit)
    ));
    assert!(
        job6.observed_work() - below_start < 1_000_000_000,
        "one-below context unexpectedly reached decoder work"
    );
    assert_job_work(&job6, below_start);
    let below_work = job6.observed_work() - below_start;
    // Candidate job 4 runs last because cancellation invalidates the one shared execution identity.
    let first_pair_delta = pair_score(&forward, "en", "positive")
        .cumulative_work()
        .saturating_sub(score_start_work);
    let job4 = job_control(&group, task_deadline);
    let before_cancel = job4.observed_work();
    let cancel_start_work = before_cancel;
    let threshold = before_cancel
        .saturating_add(first_pair_delta)
        .saturating_add(4096);
    let watcher_control = job4.clone();
    let cancel_start = Instant::now();
    let watcher = thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(600);
        while Instant::now() < until {
            if watcher_control.observed_work() >= threshold {
                watcher_control.cancel();
                return true;
            }
            thread::yield_now();
        }
        false
    });
    let cancelled = model.score(&batch, budget(128), job4.clone());
    let observed_cancel = watcher.join().unwrap();
    assert!(observed_cancel, "cancellation threshold was not observed");
    assert!(matches!(
        cancelled,
        Err(eylema_model::cpu::CpuError::Cancelled)
    ));
    assert!(job4.observed_work() >= threshold);
    assert_job_work(&job4, cancel_start_work);
    let job4_seconds = cancel_start.elapsed().as_secs_f64();
    let job4_work = job4.observed_work() - cancel_start_work;
    assert!(job4_seconds < 600.0);
    assert!(start.elapsed() < Duration::from_secs(1200));
    assert!(
        group.observed_work() <= 2_500_000_000_000,
        "aggregate acceptance work cap exceeded"
    );

    let evidence = format!(
        "{{\"format\":\"g57.native-acceptance.v1\",\"artifact_sha256\":\"{}\",\"reference_sha256\":\"{}\",\"token_ids_sha256\":\"{}\",\"source_commit\":\"{}\",\"token_rows\":4,\"context_limit\":128,\"layer_max_abs_error\":{},\"head_max_abs_error\":{},\"reference_seconds\":{},\"job1_seconds\":{},\"job1_work\":{},\"job2_seconds\":{},\"job2_work\":{},\"job3_seconds\":{},\"job3_work\":{},\"job4_seconds\":{},\"job4_work\":{},\"job5_work\":{},\"job6_exact_seconds\":{},\"job6_below_work\":{},\"batch_questions\":2,\"batch_pairs\":4,\"english_choice\":\"positive\",\"turkish_choice\":\"positive\",\"reversed_order_match\":true,\"cancel_threshold_work\":{},\"cancel_no_result\":true,\"final_work\":{},\"peak_logical_bytes\":{},\"sampled_rss_bytes\":{},\"elapsed_seconds\":{}}}\n",
        MODEL_SHA,
        hex(&eylema_platform::sha256(reference.as_bytes()).unwrap()),
        hex(&eylema_platform::sha256(&fs::read(&tokens_path).unwrap()).unwrap()),
        hex(&manifest.fields().source_commit),
        layer_error,
        head_error,
        reference_seconds,
        job1_seconds,
        job1_work,
        job2_seconds,
        job2_work,
        job3_seconds,
        job3_work,
        job4_seconds,
        job4_work,
        job5_work,
        exact_seconds,
        below_work,
        threshold,
        group.observed_work(),
        forward.receipt().peak_logical_bytes(),
        rss_sample_bytes(),
        start.elapsed().as_secs_f64()
    );
    fs::write(assets.join("g57-native-acceptance.json"), evidence).unwrap();
}
