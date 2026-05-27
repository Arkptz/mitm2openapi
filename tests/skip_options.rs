use assert_cmd::Command;
use tempfile::TempDir;

fn har_fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("har")
        .join(name)
}

const PREFIX: &str = "https://api.example.com";

/// Discover with --skip-options: OPTIONS path should NOT appear in templates.
#[test]
fn discover_skip_options_excludes_options_method() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("with_options.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
            "--skip-options",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&templates).unwrap();
    assert!(
        content.contains("/api/users"),
        "GET /api/users should be discovered"
    );
    assert!(
        !content.contains("/api/preflight"),
        "OPTIONS-only path /api/preflight should not appear when --skip-options is set"
    );
}

/// Discover WITHOUT --skip-options: OPTIONS path SHOULD appear in templates.
#[test]
fn discover_without_skip_options_includes_options_method() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("with_options.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&templates).unwrap();
    assert!(
        content.contains("/api/preflight"),
        "OPTIONS path /api/preflight should appear when --skip-options is NOT set"
    );
}

/// Generate with --skip-options: OPTIONS operation should NOT appear in OpenAPI spec.
#[test]
fn generate_skip_options_excludes_options_operation() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");
    let output = dir.path().join("openapi.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("with_options.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
        ])
        .assert()
        .success();

    let tmpl_content = std::fs::read_to_string(&templates).unwrap();
    let activated = tmpl_content.replace("ignore:", "");
    std::fs::write(&templates, activated).unwrap();

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            har_fixture("with_options.har").to_str().unwrap(),
            "-t",
            templates.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
            "--skip-options",
        ])
        .assert()
        .success();

    let spec = std::fs::read_to_string(&output).unwrap();
    assert!(
        !spec.contains("options:"),
        "OPTIONS operation should not appear in spec when --skip-options is set"
    );
    assert!(
        spec.contains("get:"),
        "GET operation should still be present"
    );
}

/// Generate WITHOUT --skip-options: OPTIONS operation SHOULD appear in OpenAPI spec.
#[test]
fn generate_without_skip_options_includes_options_operation() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");
    let output = dir.path().join("openapi.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("with_options.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
        ])
        .assert()
        .success();

    let tmpl_content = std::fs::read_to_string(&templates).unwrap();
    let activated = tmpl_content.replace("ignore:", "");
    std::fs::write(&templates, activated).unwrap();

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            har_fixture("with_options.har").to_str().unwrap(),
            "-t",
            templates.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
        ])
        .assert()
        .success();

    let spec = std::fs::read_to_string(&output).unwrap();
    assert!(
        spec.contains("options:"),
        "OPTIONS operation should appear in spec when --skip-options is NOT set"
    );
}
