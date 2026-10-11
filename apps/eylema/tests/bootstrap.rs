//! Execute the actual native host; these are not task-runtime acceptance tests.
use std::process::Command;

#[test]
fn native_version_consumes_the_portable_package_identity() {
    let output = Command::new(env!("CARGO_BIN_EXE_eylema"))
        .arg("--version")
        .output()
        .expect("execute native bootstrap");
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        format!(
            "eylema {} (workspace bootstrap)\n",
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes()
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn help_and_no_argument_expose_the_same_bounded_capability() {
    let implicit = Command::new(env!("CARGO_BIN_EXE_eylema"))
        .output()
        .expect("execute bootstrap");
    let explicit = Command::new(env!("CARGO_BIN_EXE_eylema"))
        .arg("--help")
        .output()
        .expect("execute bootstrap help");
    assert!(implicit.status.success() && explicit.status.success());
    assert_eq!(implicit.stdout, explicit.stdout);
    assert!(
        String::from_utf8(explicit.stdout)
            .unwrap()
            .contains("Task and service operations are not implemented.")
    );
    assert!(implicit.stderr.is_empty() && explicit.stderr.is_empty());
}

#[test]
fn unknown_and_excess_arguments_never_report_success() {
    for arguments in [
        vec!["run"],
        vec!["--version", "extra"],
        vec!["--help", "--version"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_eylema"))
            .args(arguments)
            .output()
            .expect("execute invalid bootstrap request");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .starts_with("unsupported bootstrap argument")
        );
    }
}

#[cfg(unix)]
#[test]
fn non_unicode_argument_is_rejected_without_a_panic() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let output = Command::new(env!("CARGO_BIN_EXE_eylema"))
        .arg(OsString::from_vec(vec![0xff]))
        .output()
        .expect("execute non-Unicode argument");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("panicked")
    );
}
