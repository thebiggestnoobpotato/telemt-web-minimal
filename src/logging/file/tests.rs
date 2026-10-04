use std::fs;
use std::io::Write;

use tempfile::tempdir;

use super::*;

#[cfg(unix)]
#[test]
fn compatibility_appender_accepts_writable_log_directories() {
    use std::os::unix::fs::PermissionsExt;

    for mode in [0o770, 0o777, 0o1777] {
        let dir = tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(mode)).unwrap();
        let path = dir.path().join("telemt.log");
        assert!(AppendFileAppender::new(path.to_str().unwrap(), true).is_err());
        assert!(!path.exists());
        let mut appender = AppendFileAppender::new(path.to_str().unwrap(), false).unwrap();
        appender.write_all(b"compatibility\n").unwrap();
        appender.flush().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"compatibility\n");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

#[cfg(unix)]
#[test]
fn compatibility_appender_follows_symlinked_parents() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let real = dir.path().join("real");
    let linked = dir.path().join("linked");
    fs::create_dir(&real).unwrap();
    symlink(&real, &linked).unwrap();
    let path = linked.join("nested/telemt.log");
    assert!(AppendFileAppender::new(path.to_str().unwrap(), true).is_err());
    assert!(!real.join("nested").exists());
    let mut appender = AppendFileAppender::new(path.to_str().unwrap(), false).unwrap();
    appender.write_all(b"compatibility\n").unwrap();
    appender.flush().unwrap();

    assert_eq!(
        fs::read(real.join("nested/telemt.log")).unwrap(),
        b"compatibility\n"
    );
}

#[cfg(unix)]
#[test]
fn appender_rejects_final_symlinks_and_hard_links_in_both_modes() {
    use std::os::unix::fs::symlink;

    for strict_runtime_paths in [false, true] {
        for hard_link in [false, true] {
            let dir = tempdir().unwrap();
            let target = dir.path().join("sentinel");
            let path = dir.path().join("telemt.log");
            fs::write(&target, b"preserve\n").unwrap();
            if hard_link {
                fs::hard_link(&target, &path).unwrap();
            } else {
                symlink(&target, &path).unwrap();
            }

            assert!(
                AppendFileAppender::new(path.to_str().unwrap(), strict_runtime_paths).is_err()
            );
            assert_eq!(fs::read(&target).unwrap(), b"preserve\n");
        }
    }
}

#[test]
fn appender_appends_to_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("telemt.log");
    fs::write(&path, "first\n").unwrap();

    let mut appender = AppendFileAppender::new(path.to_str().unwrap(), false).unwrap();
    appender.write_all(b"second\n").unwrap();
    appender.flush().unwrap();

    assert_eq!(fs::read_to_string(path).unwrap(), "first\nsecond\n");
}

#[test]
fn appender_creates_missing_file_in_existing_directory() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("telemt.log");

    let mut appender = AppendFileAppender::new(path.to_str().unwrap(), false).unwrap();
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
        AppendFileAppender::new(dir.path().join("telemt.log").to_str().unwrap(), true).is_err()
    );
}
