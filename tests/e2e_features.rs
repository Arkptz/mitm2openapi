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
fn e2e_all_features() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("openapi.yaml");

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
            "--tag-rules",
            fixture("tag-rules-test.yaml").to_str().unwrap(),
            "--envelope-discriminator",
            "success",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();

    // 1. Every operation should have operationId (--operation-id-strategy path)
    assert!(
        content.contains("operationId:"),
        "expected operationId in output:\n{content}"
    );

    // 2. Tags from rules: Contract and Private
    assert!(
        content.contains("- Contract"),
        "expected 'Contract' tag:\n{content}"
    );
    assert!(
        content.contains("- Private"),
        "expected 'Private' tag:\n{content}"
    );

    // 3. ApiError schema in components (from envelope detection)
    assert!(
        content.contains("ApiError:"),
        "expected ApiError in components:\n{content}"
    );

    // 4. oneOf present (ticker + fair_price both had mixed bodies)
    assert!(
        content.contains("oneOf:"),
        "expected oneOf schema:\n{content}"
    );

    // 5. Output is valid YAML
    let parsed: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&content).expect("generated spec must be valid YAML");
    assert!(parsed.get("openapi").is_some());
    assert!(parsed.get("paths").is_some());
    assert!(parsed.get("components").is_some());
}

#[test]
fn e2e_no_flags_backward_compat() {
    // Without new flags: no operationId, no components
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("openapi.yaml");

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
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(
        !content.contains("operationId:"),
        "operationId should NOT appear without --operation-id-strategy:\n{content}"
    );
    assert!(
        !content.contains("components:"),
        "components should NOT appear without --envelope-discriminator:\n{content}"
    );
}
