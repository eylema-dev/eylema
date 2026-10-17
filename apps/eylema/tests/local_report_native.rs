//! Admission check only; the physical/model proof runs once through the reviewed CLI.
#[test]
#[ignore = "manual G72 proof-entry admission; no model or human acceptance"]
fn missing_allocation_cannot_start_a_report_proof() {
    let root = std::env::temp_dir().join(format!("g72-missing-allocation-{}", std::process::id()));
    assert!(!root.exists());
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_eylema"))
        .args(["report", "proof"])
        .arg(root.join("absent.eyl"))
        .arg(root.join("output"))
        .stdin(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!root.exists());
}
