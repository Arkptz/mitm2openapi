use std::io::Write;
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn symlink_rejected_by_default() {
    use std::os::unix::fs as unix_fs;
    let dir = TempDir::new().unwrap();
    let real = dir.path().join("real.flow");
    std::fs::write(&real, b"1:X,").unwrap();
    let link = dir.path().join("link.flow");
    unix_fs::symlink(&real, &link).unwrap();

    let err = mitm2openapi::validate_input_path(&link, mitm2openapi::MAX_INPUT_SIZE, false);
    assert!(
        matches!(err, Err(mitm2openapi::error::Error::SymlinkRejected { .. })),
        "expected SymlinkRejected, got {err:?}"
    );
}

#[cfg(unix)]
#[test]
fn symlink_allowed_when_opted_in() {
    use std::os::unix::fs as unix_fs;
    let dir = TempDir::new().unwrap();
    let real = dir.path().join("real.flow");
    std::fs::write(&real, b"1:X,").unwrap();
    let link = dir.path().join("link.flow");
    unix_fs::symlink(&real, &link).unwrap();

    let result = mitm2openapi::validate_input_path(&link, mitm2openapi::MAX_INPUT_SIZE, true);
    assert!(
        result.is_ok(),
        "should allow symlinks when opted in: {result:?}"
    );
}

#[cfg(unix)]
#[test]
fn fifo_rejected() {
    let dir = TempDir::new().unwrap();
    let fifo_path = dir.path().join("input.fifo");

    let status = std::process::Command::new("mkfifo")
        .arg(&fifo_path)
        .status()
        .expect("mkfifo command failed");
    assert!(status.success(), "mkfifo should succeed");

    let err = mitm2openapi::validate_input_path(&fifo_path, mitm2openapi::MAX_INPUT_SIZE, false);
    assert!(
        matches!(err, Err(mitm2openapi::error::Error::NotRegularFile { .. })),
        "expected NotRegularFile for FIFO, got {err:?}"
    );
}

#[test]
fn oversize_input_rejected() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("big.flow");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0u8; 1024]).unwrap();
    }

    let err = mitm2openapi::validate_input_path(&path, 512, false);
    assert!(
        matches!(err, Err(mitm2openapi::error::Error::InputTooLarge { .. })),
        "expected InputTooLarge, got {err:?}"
    );
}

#[test]
fn normal_file_passes_validation() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ok.flow");
    std::fs::write(&path, b"1:X,").unwrap();

    let result = mitm2openapi::validate_input_path(&path, mitm2openapi::MAX_INPUT_SIZE, false);
    assert!(result.is_ok(), "normal file should pass: {result:?}");
}
