use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ropero"))
}

#[test]
fn validate_ok() {
    let out = bin().arg("validate").output().unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OK:"), "stdout={}", stdout);
}

#[test]
fn list_prefix() {
    let out = bin().arg("list").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        assert!(line.starts_with("  "), "line={}", line);
    }
    assert!(stdout.lines().count() >= 1);
}

#[test]
fn list_all_count() {
    let out = bin().args(["list", "--all"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().count(), 45);
}

#[test]
fn template_outputs() {
    let out = bin().arg("template").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[model]"));
}

#[test]
fn levels_lines() {
    let out = bin().args(["levels", "qwen38"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.lines().any(|l| l == "low"));
    assert!(!stdout.contains("nivel"));
}

#[test]
fn args_lines() {
    let out = bin().args(["args", "qwen38"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.lines().count() > 0);
}

#[test]
fn path_lines() {
    let out = bin().args(["path", "qwen38"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.lines().any(|l| l.starts_with("GGUF=")));
}

#[test]
fn usage_error_exit2() {
    let out = bin().args(["list", "--bogus"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
