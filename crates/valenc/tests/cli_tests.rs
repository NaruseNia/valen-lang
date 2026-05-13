use assert_cmd::Command;

fn valenc() -> Command {
    Command::cargo_bin("valenc").expect("valenc binary not found")
}

fn fixture(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/fixtures/{name}")
}

#[test]
fn build_valid_file_succeeds() {
    let tmp = std::env::temp_dir().join("valenc_test_build");
    let _ = std::fs::remove_dir_all(&tmp);

    valenc()
        .args(["build", &fixture("valid.vln"), "-o"])
        .arg(&tmp)
        .assert()
        .success();

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn build_invalid_syntax_fails() {
    let tmp = std::env::temp_dir().join("valenc_test_build_invalid");
    let _ = std::fs::remove_dir_all(&tmp);

    valenc()
        .args(["build", &fixture("invalid_syntax.vln"), "-o"])
        .arg(&tmp)
        .assert()
        .failure();

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn check_valid_file_succeeds() {
    valenc()
        .args(["check", &fixture("valid.vln")])
        .assert()
        .success();
}

#[test]
fn check_invalid_syntax_fails() {
    valenc()
        .args(["check", &fixture("invalid_syntax.vln")])
        .assert()
        .failure();
}

#[test]
fn version_subcommand() {
    valenc()
        .arg("version")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("valenc "));
}

#[test]
fn no_args_shows_help() {
    valenc().assert().failure();
}

#[test]
fn diagnostic_uses_line_col_format() {
    let output = valenc()
        .args(["check", &fixture("invalid_syntax.vln")])
        .output()
        .expect("failed to run valenc");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Diagnostics should contain line:col format (e.g. ":3:") not raw byte offsets
    assert!(
        stderr.contains(":3:"),
        "diagnostic should contain line number, got: {stderr}"
    );
    // Should NOT contain the old ".." byte-offset format
    assert!(
        !stderr.contains(".."),
        "diagnostic should not contain byte-offset '..' format, got: {stderr}"
    );
}
