use assert_cmd::Command;

#[test]
fn param_regex_flag_rejected_as_unknown() {
    let mut cmd = Command::cargo_bin("mitm2openapi").unwrap();
    cmd.args([
        "generate",
        "-i",
        "nonexistent.flow",
        "-t",
        "nonexistent.yaml",
        "-o",
        "out.yaml",
        "-p",
        "https://example.com",
        "--param-regex",
        "foo",
    ]);
    cmd.assert().failure();
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument") || stderr.contains("unknown"),
        "expected 'unexpected argument' in stderr, got: {stderr}"
    );
}
