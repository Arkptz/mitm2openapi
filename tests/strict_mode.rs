use assert_cmd::Command;
use tempfile::TempDir;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("flows")
        .join(name)
}

#[test]
fn strict_exits_nonzero_on_corrupt_input() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

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
            "--strict",
        ])
        .assert()
        .code(2);
}

#[test]
fn strict_exits_zero_on_clean_input() {
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
            "--strict",
        ])
        .assert()
        .success();
}

#[test]
fn non_strict_exits_zero_on_corrupt() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.yaml");

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
        ])
        .assert()
        .success();
}
