use assert_cmd::Command;
use tempfile::TempDir;

fn har_fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn expected_file(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("expected")
        .join(name)
}

const PREFIX: &str = "https://api.example.com";

#[test]
fn snapshot_compat() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("snapshot_output.yaml");

    std::fs::create_dir_all(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("expected"),
    )
    .unwrap();

    Command::cargo_bin("mitm2openapi")
        .unwrap()
        .args([
            "generate",
            "-i",
            har_fixture("snapshot_input.har").to_str().unwrap(),
            "-t",
            har_fixture("snapshot_templates.yaml").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-p",
            PREFIX,
        ])
        .assert()
        .success();

    let actual = std::fs::read_to_string(&output).unwrap();
    let baseline_path = expected_file("snapshot_baseline.yaml");
    let expected = std::fs::read_to_string(&baseline_path).unwrap_or_else(|e| {
        panic!(
            "Could not read baseline file {}: {}",
            baseline_path.display(),
            e
        )
    });

    if actual != expected {
        let actual_lines: Vec<&str> = actual.lines().collect();
        let expected_lines: Vec<&str> = expected.lines().collect();
        let max_lines = actual_lines.len().max(expected_lines.len());

        eprintln!("=== SNAPSHOT DIFF (expected vs actual) ===");
        for i in 0..max_lines {
            let exp_line = expected_lines.get(i).copied().unwrap_or("<missing>");
            let act_line = actual_lines.get(i).copied().unwrap_or("<missing>");
            if exp_line != act_line {
                eprintln!("Line {:>4}: expected: {:?}", i + 1, exp_line);
                eprintln!("           actual:   {:?}", act_line);
            }
        }
        eprintln!("=== END DIFF ===");
        panic!(
            "Snapshot mismatch: generate output differs from baseline.\n\
             If this change is intentional, regenerate the baseline with:\n\
             cargo run -- generate -i tests/fixtures/snapshot_input.har \\\n\
               -t tests/fixtures/snapshot_templates.yaml \\\n\
               -o tests/expected/snapshot_baseline.yaml \\\n\
               -p https://api.example.com"
        );
    }

    println!(
        "Snapshot test passed: output matches baseline ({} bytes)",
        actual.len()
    );
}
