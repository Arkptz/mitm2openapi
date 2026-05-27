use assert_cmd::Command;
use tempfile::TempDir;

fn har_fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

const PREFIX: &str = "https://api.example.com";

#[test]
fn full_pipeline_all_features() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");
    let output = dir.path().join("openapi.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("full_pipeline.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
            "--skip-options",
            "--param-regex",
            "PERP_[A-Z_0-9]+",
        ])
        .assert()
        .success();

    let tmpl_content = std::fs::read_to_string(&templates).unwrap();
    assert!(
        tmpl_content.contains("{id}"),
        "4 distinct pair symbols should trigger parameterization, got:\n{tmpl_content}"
    );
    assert!(
        !tmpl_content.contains("OPTIONS"),
        "OPTIONS entries should not appear in templates with --skip-options"
    );

    let activated = tmpl_content
        .lines()
        .map(|line| {
            if line.contains("{id}") || line.contains("/orders") {
                line.replace("ignore:", "")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&templates, &activated).unwrap();

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            har_fixture("full_pipeline.har").to_str().unwrap(),
            "-t",
            templates.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--skip-options",
            "--max-examples",
            "3",
            "--redact-fields",
            "token",
            "--redact-patterns",
            "[0-9a-f]{32}",
        ])
        .assert()
        .success();

    let spec = std::fs::read_to_string(&output).unwrap();
    assert!(
        spec.contains("examples:"),
        "spec should contain examples section"
    );
    assert!(
        spec.contains("[REDACTED]"),
        "token values should be redacted"
    );
    assert!(
        !spec.contains("options:"),
        "OPTIONS operations should not appear in spec with --skip-options"
    );
    assert!(spec.contains("get:"), "spec should contain GET operations");
    assert!(
        spec.contains("post:"),
        "spec should contain POST operations"
    );
    assert!(
        spec.contains("{id}"),
        "parameterized path should appear in spec"
    );
}
