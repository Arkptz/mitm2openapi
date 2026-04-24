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

#[cfg(unix)]
#[test]
fn symlink_to_directory_rejected() {
    use std::os::unix::fs as unix_fs;

    let dir = TempDir::new().unwrap();
    let real_dir = dir.path().join("real_dir");
    std::fs::create_dir(&real_dir).unwrap();
    std::fs::write(real_dir.join("test.flow"), b"1:X,").unwrap();

    let link = dir.path().join("link_dir");
    unix_fs::symlink(&real_dir, &link).unwrap();

    assert!(link.is_dir(), "symlink should resolve to directory");

    let err = mitm2openapi::validate_input_path(&link, mitm2openapi::MAX_INPUT_SIZE, false);
    assert!(
        matches!(err, Err(mitm2openapi::error::Error::SymlinkRejected { .. })),
        "symlink to directory should be rejected, got {err:?}"
    );
}

#[cfg(unix)]
#[test]
fn symlink_dir_entry_rejected_in_mitmproxy() {
    use std::os::unix::fs as unix_fs;

    let dir = TempDir::new().unwrap();
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("flows")
        .join("simple_get.flow");
    let real_file = dir.path().join("real.flow");
    std::fs::copy(&src, &real_file).unwrap();

    let link_file = dir.path().join("linked.flow");
    unix_fs::symlink(&real_file, &link_file).unwrap();

    let iter = mitm2openapi::mitmproxy_reader::stream_mitmproxy_dir_no_symlinks(dir.path());
    assert!(iter.is_ok(), "should open directory");
    let results: Vec<_> = iter.unwrap().filter_map(|r| r.ok()).collect();

    assert!(
        !results.is_empty(),
        "real file should produce at least one flow"
    );

    let all_results: Vec<_> = mitm2openapi::mitmproxy_reader::stream_mitmproxy_dir(dir.path())
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        all_results.len() > results.len(),
        "without symlink rejection, both files should be processed"
    );
}

#[cfg(unix)]
#[test]
fn symlink_dir_entry_rejected_in_har() {
    use std::os::unix::fs as unix_fs;

    let dir = TempDir::new().unwrap();
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("har")
        .join("simple.har");
    let real_file = dir.path().join("real.har");
    std::fs::copy(&src, &real_file).unwrap();

    let link_file = dir.path().join("linked.har");
    unix_fs::symlink(&real_file, &link_file).unwrap();

    let iter = mitm2openapi::har_reader::stream_har_dir_no_symlinks(dir.path());
    assert!(iter.is_ok(), "should open directory");
    let results: Vec<_> = iter.unwrap().filter_map(|r| r.ok()).collect();

    assert_eq!(
        results.len(),
        1,
        "only the real HAR file should be processed, symlinked entry skipped"
    );
}
