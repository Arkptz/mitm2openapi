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
fn cli_enrichments_flag_applies_overlay() {
    let dir = TempDir::new().unwrap();
    let overlay_path = dir.path().join("overlay.yaml");
    std::fs::write(
        &overlay_path,
        r#"
operations:
  listTicker:
    summary: Get ticker data
    x-requires-auth: false
"#,
    )
    .unwrap();
    let output = dir.path().join("spec.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("envelope_test.har").to_str().unwrap(),
            "-t",
            fixture("envelope_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--operation-id-strategy",
            "path",
            "--enrichments",
            overlay_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(
        content.contains("Get ticker data"),
        "enrichment summary should be applied:\n{content}"
    );
    assert!(
        content.contains("x-requires-auth"),
        "enrichment extension should be present:\n{content}"
    );
}

#[test]
fn cli_enrichments_requires_operation_id_strategy() {
    let dir = TempDir::new().unwrap();
    let overlay_path = dir.path().join("overlay.yaml");
    std::fs::write(&overlay_path, "operations: {}").unwrap();
    let output = dir.path().join("spec.yaml");

    let cmd = Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("envelope_test.har").to_str().unwrap(),
            "-t",
            fixture("envelope_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--enrichments",
            overlay_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        !cmd.status.success(),
        "should fail without --operation-id-strategy"
    );
    let stderr = String::from_utf8_lossy(&cmd.stderr);
    assert!(
        stderr.contains("--enrichments requires --operation-id-strategy")
            || stderr.contains("operation-id-strategy"),
        "error should mention --operation-id-strategy:\n{stderr}"
    );
}

#[test]
fn cli_enrichments_missing_file_errors() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("spec.yaml");

    let cmd = Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("envelope_test.har").to_str().unwrap(),
            "-t",
            fixture("envelope_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--operation-id-strategy",
            "path",
            "--enrichments",
            "/nonexistent/overlay.yaml",
        ])
        .output()
        .unwrap();

    assert!(
        !cmd.status.success(),
        "should fail for missing enrichments file"
    );
}

#[test]
fn cli_enrichments_invalid_yaml_errors() {
    let dir = TempDir::new().unwrap();
    let overlay_path = dir.path().join("overlay.yaml");
    std::fs::write(
        &overlay_path,
        "operations:\n  : invalid\n  broken yaml {{{{",
    )
    .unwrap();
    let output = dir.path().join("spec.yaml");

    let cmd = Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("envelope_test.har").to_str().unwrap(),
            "-t",
            fixture("envelope_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--operation-id-strategy",
            "path",
            "--enrichments",
            overlay_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!cmd.status.success(), "should fail for invalid YAML");
}

#[test]
fn cli_enrichments_strict_mode_rejects_unknown_opid() {
    let dir = TempDir::new().unwrap();
    let overlay_path = dir.path().join("overlay.yaml");
    std::fs::write(
        &overlay_path,
        "operations:\n  doesNotExist:\n    summary: Ghost",
    )
    .unwrap();
    let output = dir.path().join("spec.yaml");

    let cmd = Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            fixture("envelope_test.har").to_str().unwrap(),
            "-t",
            fixture("envelope_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--operation-id-strategy",
            "path",
            "--enrichments",
            overlay_path.to_str().unwrap(),
            "--strict",
        ])
        .output()
        .unwrap();

    assert!(
        !cmd.status.success(),
        "strict mode should fail for unknown operationId"
    );
}
