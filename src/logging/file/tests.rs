use std::fs;
use std::io::Write;

use tempfile::tempdir;

use super::*;

#[test]
fn appender_appends_to_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("telemt.log");
    fs::write(&path, "first\n").unwrap();

    let mut appender = AppendFileAppender::new(path.to_str().unwrap()).unwrap();
    appender.write_all(b"second\n").unwrap();
    appender.flush().unwrap();

    assert_eq!(fs::read_to_string(path).unwrap(), "first\nsecond\n");
}

#[test]
fn appender_creates_missing_file_in_existing_directory() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("telemt.log");

    let mut appender = AppendFileAppender::new(path.to_str().unwrap()).unwrap();
    appender.write_all(b"first\n").unwrap();
    appender.flush().unwrap();

    assert_eq!(fs::read_to_string(path).unwrap(), "first\n");
}

#[cfg(unix)]
#[test]
fn appender_rejects_group_writable_log_directory() {
    use std::os::unix::fs::PermissionsExt;

    let current = std::env::current_dir().unwrap();
    let dir = tempfile::Builder::new()
        .prefix("telemt-untrusted-log-")
        .tempdir_in(current)
        .unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o770)).unwrap();

    assert!(
        AppendFileAppender::new(dir.path().join("telemt.log").to_str().unwrap()).is_err()
    );
}
