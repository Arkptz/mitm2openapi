use assert_cmd::Command;
use tempfile::TempDir;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("flows")
        .join(name)
}

#[test]
fn report_written_on_success() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");
    let report_path = dir.path().join("report.json");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            fixture("simple_get.flow").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            "https://api.example.com",
            "--report",
            report_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(report_path.exists(), "report file should be written");

    let content = std::fs::read_to_string(&report_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(report["report_version"], 1);
    assert!(report["tool_version"].is_string());
    assert!(report["input"]["path"].is_string());
    assert!(report["result"]["paths_in_spec"].is_number());
}

#[test]
fn report_written_on_corrupt_input() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");
    let report_path = dir.path().join("report.json");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            fixture("corrupt.flow").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            "https://api.example.com",
            "--report",
            report_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        report_path.exists(),
        "report file should be written even on corrupt input"
    );

    let content = std::fs::read_to_string(&report_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(report["report_version"], 1);
    assert!(report["tool_version"].is_string());
}

#[test]
fn report_not_written_without_flag() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            fixture("simple_get.flow").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            "https://api.example.com",
        ])
        .assert()
        .success();

    let report_path = dir.path().join("report.json");
    assert!(
        !report_path.exists(),
        "report should not be written without --report flag"
    );
}
