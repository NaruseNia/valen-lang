use assert_cmd::Command;

fn valenc() -> Command {
    Command::cargo_bin("valenc").expect("valenc binary not found")
}

fn has_byte_offset_pattern(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i].is_ascii_digit() && bytes[i + 1] == b'.' && bytes[i + 2] == b'.' {
            if let Some(&next) = bytes.get(i + 3) {
                if next.is_ascii_digit() {
                    return true;
                }
            }
        }
    }
    false
}

fn fixture(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/fixtures/{name}")
}

#[test]
fn compile_valid_file_succeeds() {
    let tmp = std::env::temp_dir().join("valenc_test_compile");
    let _ = std::fs::remove_dir_all(&tmp);

    valenc()
        .args(["compile", &fixture("valid.vln"), "-o"])
        .arg(&tmp)
        .assert()
        .success();

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn compile_invalid_syntax_fails() {
    let tmp = std::env::temp_dir().join("valenc_test_compile_invalid");
    let _ = std::fs::remove_dir_all(&tmp);

    valenc()
        .args(["compile", &fixture("invalid_syntax.vln"), "-o"])
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
fn compile_with_classpath_flag_succeeds() {
    let tmp = std::env::temp_dir().join("valenc_test_classpath");
    let _ = std::fs::remove_dir_all(&tmp);

    valenc()
        .args(["compile", &fixture("valid.vln"), "-o"])
        .arg(&tmp)
        .args(["--classpath", "/nonexistent/but/accepted"])
        .assert()
        .success();

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn check_with_classpath_flag_succeeds() {
    valenc()
        .args([
            "check",
            &fixture("valid.vln"),
            "--classpath",
            "/nonexistent/but/accepted",
        ])
        .assert()
        .success();
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
    // Diagnostic lines (containing V0xxx) should not use byte-offset format "N..M"
    for line in stderr.lines() {
        if line.contains("V0") {
            assert!(
                !has_byte_offset_pattern(line),
                "diagnostic should not contain byte-offset format, got: {line}"
            );
        }
    }
}
