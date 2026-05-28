use assert_cmd::Command;
use tempfile::TempDir;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

const PREFIX: &str = "https://api.example.com";

#[test]
fn redact_pattern_with_quantifier_comma_is_not_split() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("openapi.yaml");

    // {8,64} quantifier contains a comma — clap must not split on it
    let cmd = Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("snapshot_input.har").to_str().unwrap(),
            "-t",
            fixture("snapshot_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--redact-patterns",
            "TOKEN[a-f0-9]{8,64}",
            "--strict",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&cmd.stderr);
    assert!(
        !stderr.contains("unclosed counted repetition"),
        "regex was truncated by clap comma-split:\n{stderr}"
    );
    assert!(
        cmd.status.success(),
        "should succeed — pattern is valid:\n{stderr}"
    );
}

#[test]
fn invalid_redact_pattern_fails_under_strict() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("openapi.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("snapshot_input.har").to_str().unwrap(),
            "-t",
            fixture("snapshot_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--redact-patterns",
            "[unclosed",
            "--strict",
        ])
        .assert()
        .failure();
}
