use std::path::Path;

#[test]
fn successful_write_creates_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let output = dir.path().join("spec.yaml");
    mitm2openapi::output::write_yaml("hello: world\n", &output).unwrap();
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "hello: world\n");
}

#[test]
fn overwrite_existing_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let output = dir.path().join("spec.yaml");
    mitm2openapi::output::write_yaml("v1: yes\n", &output).unwrap();
    mitm2openapi::output::write_yaml("v2: yes\n", &output).unwrap();
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "v2: yes\n");
}

#[test]
fn creates_parent_directories() {
    let dir = tempfile::TempDir::new().unwrap();
    let output = dir.path().join("nested").join("dir").join("spec.yaml");
    mitm2openapi::output::write_yaml("nested: yes\n", &output).unwrap();
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "nested: yes\n");
}

#[cfg(target_os = "linux")]
#[test]
fn partial_write_preserves_target() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::TempDir::new().unwrap();
    let output = dir.path().join("spec.yaml");

    mitm2openapi::output::write_yaml("original: yes\n", &output).unwrap();
    let original = std::fs::read_to_string(&output).unwrap();
    assert_eq!(original, "original: yes\n");

    let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o500);
    std::fs::set_permissions(dir.path(), perms).unwrap();

    let result = mitm2openapi::output::write_yaml("new: content\n", &output);
    assert!(result.is_err(), "write to read-only dir should fail");

    let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(dir.path(), perms).unwrap();

    let after = std::fs::read_to_string(&output).unwrap();
    assert_eq!(
        after, original,
        "original file must remain untouched on write failure"
    );
}

#[test]
fn write_to_nonexistent_parent_fails_gracefully() {
    let result =
        mitm2openapi::output::write_yaml("test\n", Path::new("/nonexistent/dir/spec.yaml"));
    assert!(result.is_err());
}
