use assert_cmd::Command;
use tempfile::TempDir;

fn har_fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("har")
        .join(name)
}

const PREFIX: &str = "https://api.example.com";

#[test]
fn discover_param_regex_detects_crypto_pair() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("crypto_pairs.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
            "--param-regex",
            "[A-Z]{3}_[A-Z]{3,}",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&templates).unwrap();
    assert!(
        content.contains("{id}"),
        "BTC_USDT/ETH_USDT should be collapsed into {{id}} template, got:\n{content}"
    );
}

#[test]
fn discover_invalid_param_regex_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "discover",
            "-i",
            har_fixture("crypto_pairs.har").to_str().unwrap(),
            "-o",
            templates.to_str().unwrap(),
            "-p",
            PREFIX,
            "--param-regex",
            "[invalid(regex",
        ])
        .assert()
        .failure();
}
