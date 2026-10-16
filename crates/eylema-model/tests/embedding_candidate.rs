//! Explicit G68 tokenizer input freeze; model execution is a separate bounded test.
use eylema_contracts::model_contract::ModelManifest;
use eylema_model::{
    LoadBudget, LoadedWeights, WeightLayout,
    cpu::{CpuControl, CpuWorkGroup},
    embedding::{EmbeddingBudget, EmbeddingCpuModel, embedding_layout},
    load,
    tokenizer::{AdmissionBudget, EmbeddingTokenizer, TextBudget},
};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

const PAIRS: [(&str, &str); 4] = [
    (
        "What is the capital of France?",
        "Paris is the capital of France.",
    ),
    (
        "What is the capital of France?",
        "The Pacific Ocean is the largest ocean.",
    ),
    (
        "Türkiye'nin başkenti neresidir?",
        "Türkiye'nin başkenti Ankara'dır.",
    ),
    (
        "Türkiye'nin başkenti neresidir?",
        "Pasifik Okyanusu en büyük okyanustur.",
    ),
];

fn manifest() -> ModelManifest {
    ModelManifest::from_bytes(include_bytes!("fixtures/g68/embedding-profile.v1.eyl"))
        .expect("pinned G67 embedding manifest")
}

fn pinned_file(root: &Path, name: &str) -> std::io::Result<File> {
    let path = root.join(name);
    let before = fs::symlink_metadata(&path)?;
    if !before.is_file() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x100);
    } // O_NOFOLLOW
    let f = options.open(&path)?;
    let after = f.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
    }
    if !after.is_file() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    Ok(f)
}

#[test]
#[cfg(unix)]
fn artifact_open_rejects_symbolic_links() {
    use std::os::unix::fs::symlink;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("eyleme-g68-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("original"), b"fixture").unwrap();
    symlink(root.join("original"), root.join("alias")).unwrap();
    assert!(pinned_file(&root, "original").is_ok());
    assert!(pinned_file(&root, "alias").is_err());
    fs::remove_dir_all(&root).unwrap();
}

fn tokenizer(root: &Path) -> EmbeddingTokenizer {
    let mut vocab = pinned_file(root, "tokenizer.json").expect("private pinned tokenizer");
    let mut config = pinned_file(root, "tokenizer_config.json")
        .expect("previously verified pinned tokenizer config");
    EmbeddingTokenizer::load(
        &mut vocab,
        &mut config,
        &manifest(),
        AdmissionBudget {
            max_resident_bytes: 134_217_728,
            max_read_calls: 1024,
            max_interruptions: 16,
            max_work_per_artifact: 134_217_728,
        },
    )
    .expect("exact G68 tokenizer admission")
}

fn text_budget() -> TextBudget {
    TextBudget {
        max_resident_bytes: 134_217_728,
        max_tokens: 4096,
        max_work: 2_000_000,
    }
}

#[test]
#[ignore = "requires explicitly approved private G68 tokenizer/config artifacts; freezes only token IDs for the independent local reference"]
fn pinned_embedding_tokenizer_inputs() {
    let root = std::env::var_os("G68_ASSETS").expect("private artifact directory");
    let root = Path::new(&root);
    let tokenizer = tokenizer(root);
    assert_eq!(
        tokenizer.manifest_sha256(),
        [
            0xca, 0x3c, 0xde, 0x45, 0x94, 0x37, 0x69, 0x2c, 0x9a, 0xe8, 0x77, 0xb6, 0x6c, 0xf4,
            0xd1, 0x5b, 0xab, 0x58, 0xc0, 0x85, 0x2e, 0xc2, 0xf0, 0xfc, 0x7d, 0xc0, 0x8e, 0x70,
            0x03, 0xfd, 0xc7, 0x1f,
        ]
    );
    let mut sequences = Vec::<(u8, Vec<u32>)>::new();
    let control = CpuControl::new(Instant::now() + Duration::from_secs(60)).unwrap();
    for (query, document) in PAIRS {
        let q = tokenizer
            .encode_query(query, text_budget(), &control)
            .unwrap();
        let d = tokenizer
            .encode_document(document, text_budget(), &control)
            .unwrap();
        assert!(!q.ids().is_empty() && q.ids().len() <= 32);
        assert!(!d.ids().is_empty() && d.ids().len() <= 32);
        sequences.push((0, q.ids().to_vec()));
        sequences.push((1, d.ids().to_vec()));
    }
    let medium = "Local systems keep private data on the device. ".repeat(16);
    let medium_tokens = tokenizer
        .encode_document(&medium, text_budget(), &control)
        .unwrap();
    assert!(!medium_tokens.ids().is_empty() && medium_tokens.ids().len() <= 512);
    sequences.push((1, medium_tokens.ids().to_vec()));

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"G68TOK01");
    bytes.extend_from_slice(&(sequences.len() as u32).to_le_bytes());
    let mut total_tokens = 0usize;
    for (role, ids) in sequences {
        let count = ids.len();
        bytes.push(role);
        bytes.extend_from_slice(&(count as u32).to_le_bytes());
        bytes.extend(ids.iter().flat_map(|id| id.to_le_bytes()));
        total_tokens = total_tokens.checked_add(count).expect("total tokens");
    }
    let output = std::env::var_os("G68_TOKEN_FILE").expect("private token freeze path");
    let output = Path::new(&output);
    let parent = output.parent().expect("private output parent");
    fs::create_dir_all(parent).expect("private output directory");
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output)
            .expect("one private token freeze")
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .expect("one private token freeze");
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    let digest = eylema_platform::sha256(&bytes).unwrap();
    eprintln!(
        "G68 tokenizer frozen: cases=9 tokens={total_tokens} sha256={}",
        hex(&digest)
    );
}

#[test]
#[ignore = "requires the explicitly approved private G68 weight and tokenizer artifacts; validates layout/admission only, no model forward"]
fn pinned_embedding_artifact_structure() {
    let root = std::env::var_os("G68_ASSETS").expect("private artifact directory");
    let root = Path::new(&root);
    let manifest = manifest();
    let layout: WeightLayout = embedding_layout(&manifest).expect("exact G68 layout");
    let mut source = pinned_file(root, "model.safetensors").expect("private pinned weights");
    let weights: LoadedWeights = load(
        &mut source,
        &manifest,
        &layout,
        LoadBudget {
            max_artifact_bytes: 1_191_586_416,
            max_resident_content_bytes: 2_151_677_952,
            max_read_calls: 20_000,
            max_interruptions: 16,
        },
    )
    .expect("complete G68 safetensors header, ranges and digest");
    assert_eq!(weights.artifact_len(), 1_191_586_416);
    assert_eq!(
        weights.artifact_sha256(),
        [
            0x04, 0x37, 0xe4, 0x5c, 0x94, 0x56, 0x3b, 0x09, 0xe1, 0x3c, 0xb7, 0xa6, 0x44, 0x78,
            0xfc, 0x40, 0x69, 0x47, 0xa9, 0x3c, 0xb3, 0x4a, 0x7e, 0x05, 0x87, 0x0f, 0xc8, 0xdc,
            0xd4, 0x8e, 0x23, 0xfd,
        ]
    );
    assert_eq!(layout.tensors().len(), 310);
    let tokenizer = tokenizer(root);
    let group = CpuWorkGroup::new(2_500_000_000_000).unwrap();
    let control = CpuControl::new_job(
        Instant::now() + Duration::from_secs(600),
        2_500_000_000_000,
        group,
    )
    .unwrap();
    let model = EmbeddingCpuModel::admit(
        &manifest,
        &weights,
        &tokenizer,
        EmbeddingBudget {
            context: 4096,
            max_resident_bytes: 2_684_354_560,
            max_work: 2_500_000_000_000,
        },
        &control,
    )
    .expect("bounded exact-profile admission without forward");
    assert!(model.admission_work() <= 2_500_000_000_000);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|x| format!("{x:02x}")).collect()
}

fn vectors(bytes: &[u8]) -> Vec<(usize, Vec<f64>)> {
    assert_eq!(bytes.len(), 12 + 9 * (4 + 1024 * 8));
    assert_eq!(&bytes[..8], b"G68F6401");
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 9);
    bytes[12..]
        .as_chunks::<{ 4 + 1024 * 8 }>()
        .0
        .iter()
        .map(|row| {
            let tokens = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
            let vector: Vec<_> = row[4..]
                .as_chunks::<8>()
                .0
                .iter()
                .map(|v| f64::from_le_bytes(*v))
                .collect();
            assert!(vector.iter().all(|v| v.is_finite()));
            let norm = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
            assert!((norm - 1.).abs() <= 1e-12);
            (tokens, vector)
        })
        .collect()
}

#[test]
#[ignore = "one explicitly authorized six-job G68 CPU acceptance after independent F64 reference; no downloads or provider calls"]
fn actual_pinned_embeddings() {
    use eylema_model::{cpu::CpuError, embedding::cosine_similarity};
    let root = std::env::var_os("G68_ASSETS").unwrap();
    let root = Path::new(&root);
    let ref_path = std::env::var_os("G68_REFERENCE").unwrap();
    assert!(fs::metadata(&ref_path).unwrap().len() <= 100_000);
    let reference_bytes = fs::read(&ref_path).unwrap();
    let reference = vectors(&reference_bytes);
    let frozen = fs::read(root.join("tokens.v1.bin")).unwrap();
    assert_eq!(
        hex(&eylema_platform::sha256(&frozen).unwrap()),
        "814a6e864dbe7415d82937f45da27054287570ebf0e644b7e343f2eff3be9826"
    );
    let manifest = manifest();
    let layout = embedding_layout(&manifest).unwrap();
    let weights = load(
        &mut pinned_file(root, "model.safetensors").unwrap(),
        &manifest,
        &layout,
        LoadBudget {
            max_artifact_bytes: 1_191_586_416,
            max_resident_content_bytes: 2_151_677_952,
            max_read_calls: 20_000,
            max_interruptions: 16,
        },
    )
    .unwrap();
    let tokenizer = tokenizer(root);
    let start = Instant::now();
    let end = start + Duration::from_secs(1200);
    let group = CpuWorkGroup::new(2_500_000_000_000).unwrap();
    let job = || {
        CpuControl::new_job(
            end.min(Instant::now() + Duration::from_secs(600)),
            2_500_000_000_000,
            group.clone(),
        )
        .unwrap()
    };
    let b = EmbeddingBudget {
        context: 4096,
        max_resident_bytes: 2_684_354_560,
        max_work: 2_500_000_000_000,
    };
    let medium = "Local systems keep private data on the device. ".repeat(16);
    let texts: Vec<_> = PAIRS
        .iter()
        .flat_map(|(q, d)| [(true, *q), (false, *d)])
        .chain([(false, medium.as_str())])
        .collect();
    // Job 1: admission and the nine frozen inputs.
    let first = job();
    let model = EmbeddingCpuModel::admit(&manifest, &weights, &tokenizer, b, &first).unwrap();
    let mut results = Vec::new();
    let mut max_l2 = 0f64;
    let mut peak = 0u64;
    for (index, (query, text)) in texts.iter().enumerate() {
        let out = if *query {
            model.embed_query(text, b, first.clone())
        } else {
            model.embed_document(text, b, first.clone())
        }
        .unwrap();
        assert_eq!(out.receipt().input_tokens(), reference[index].0);
        assert_eq!(out.vector().len(), 1024);
        let norm = out
            .vector()
            .iter()
            .map(|v| f64::from(*v).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!((norm - 1.).abs() <= 1e-5);
        let l2 = out
            .vector()
            .iter()
            .zip(&reference[index].1)
            .map(|(v, w)| (f64::from(*v) - w).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(l2 <= 0.02, "frozen L2 tolerance");
        max_l2 = max_l2.max(l2);
        peak = peak.max(out.receipt().peak_logical_bytes());
        results.push(out.vector().to_vec());
    }
    let mut scores = Vec::new();
    let mut expected_scores = Vec::new();
    let mut max_cosine = 0f64;
    for pair in 0..4 {
        let a = pair * 2;
        let actual = f64::from(cosine_similarity(&results[a], &results[a + 1]).unwrap());
        let want = reference[a]
            .1
            .iter()
            .zip(&reference[a + 1].1)
            .map(|(a, b)| a * b)
            .sum::<f64>();
        max_cosine = max_cosine.max((actual - want).abs());
        assert!((actual - want).abs() <= 0.041);
        scores.push(actual);
        expected_scores.push(want);
    }
    for a in [0, 2] {
        if (expected_scores[a] - expected_scores[a + 1]).abs() > 0.083 {
            assert_eq!(
                scores[a] > scores[a + 1],
                expected_scores[a] > expected_scores[a + 1]
            );
        }
    }
    // Job 2: reverse ordering must not retain KV or workspace from another text.
    let reverse = job();
    for (index, (query, text)) in texts.iter().enumerate().rev() {
        let out = if *query {
            model.embed_query(text, b, reverse.clone())
        } else {
            model.embed_document(text, b, reverse.clone())
        }
        .unwrap();
        assert_eq!(out.vector(), results[index]);
    }
    // Job 3: explicit repeated-document reset.
    assert_eq!(
        model.embed_document(PAIRS[0].1, b, job()).unwrap().vector(),
        results[1]
    );
    // Job 4: exact 4096-token admission/preflight and rejection without a forward.
    let preflight = job();
    let mut boundary = None;
    for n in 4080..4100 {
        let text = "a ".repeat(n);
        if tokenizer
            .encode_document(&text, text_budget(), &preflight)
            .is_ok_and(|t| t.ids().len() == 4096)
        {
            boundary = Some(text);
            break;
        }
    }
    let boundary = boundary.expect("exact native 4096-token boundary");
    let low = EmbeddingBudget {
        max_work: 1_000_000,
        ..b
    };
    assert_eq!(
        model
            .embed_document(&boundary, low, preflight.clone())
            .unwrap_err(),
        CpuError::WorkLimit
    );
    assert_eq!(
        model.embed_document("", b, preflight.clone()).unwrap_err(),
        CpuError::InvalidInput
    );
    assert_eq!(
        model
            .embed_document(&"a ".repeat(4200), b, preflight)
            .unwrap_err(),
        CpuError::ContextLimit
    );
    // Job 5: cancellation is triggered only after >50M charged work, inside decoder forward.
    let stop = job();
    let observer = stop.clone();
    let stopper = std::thread::spawn(move || {
        let timeout = Instant::now() + Duration::from_secs(590);
        while observer.observed_work() <= 50_000_000 && Instant::now() < timeout {
            std::thread::yield_now();
        }
        let work = observer.observed_work();
        observer.cancel();
        work
    });
    let cancelled = model.embed_document(PAIRS[0].1, b, stop).unwrap_err();
    assert!(stopper.join().unwrap() > 50_000_000);
    assert_eq!(cancelled, CpuError::Cancelled);
    // Job 6: deadline after tokenization; no partial vector can be returned.
    let deadline = CpuControl::new_job(
        Instant::now() + Duration::from_secs(1),
        2_500_000_000_000,
        group.clone(),
    )
    .unwrap();
    assert_eq!(
        model
            .embed_document(&medium, b, deadline.clone())
            .unwrap_err(),
        CpuError::Deadline
    );
    assert!(deadline.observed_work() > 2_000_000);
    assert!(start.elapsed() < Duration::from_secs(1200));
    assert!(peak <= 2_684_354_560);
    eprintln!(
        "G68 PASS cases=9 jobs=6 tokens=305 max_l2={max_l2:.9} max_cosine={max_cosine:.9} work={} peak={peak} seconds={:.3} reference_sha256={}",
        group.observed_work(),
        start.elapsed().as_secs_f64(),
        hex(&eylema_platform::sha256(&reference_bytes).unwrap())
    );
}
