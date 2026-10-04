//! Append-only file appender for log output.
//!
//! The target file is opened once at startup and kept for the process
//! lifetime; no rotation or retention is performed.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[cfg(not(unix))]
use std::fs::{self, OpenOptions};

#[cfg(test)]
mod tests;

/// File appender that opens the target path in append mode.
pub(crate) struct AppendFileAppender {
    file: File,
}

impl AppendFileAppender {
    /// Opens the appender using the process-level Unix parent-path policy.
    pub(crate) fn new(path: &str, strict_runtime_paths: bool) -> io::Result<Self> {
        let path = Path::new(path);
        #[cfg(unix)]
        {
            let dir = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let name = path
                .file_name()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "log path has no file name")
                })?;
            let dir_fd = if strict_runtime_paths {
                crate::util::secure_fs::open_trusted_dir_nofollow_or_create(&dir, 0o750)?
            } else {
                crate::util::secure_fs::open_compatible_dir(&dir, Some(0o750))?
            };
            let file = crate::util::secure_fs::open_append_regular_at(&dir_fd, name, 0o640)?;
            Ok(Self { file })
        }
        #[cfg(not(unix))]
        {
            let _ = strict_runtime_paths;
            let mut options = OpenOptions::new();
            options.create(true).append(true);

            let file = match options.open(path) {
                Ok(file) => file,
                Err(error) => {
                    let Some(parent) = path
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                    else {
                        return Err(error);
                    };
                    fs::create_dir_all(parent)?;
                    options.open(path)?
                }
            };
            Ok(Self { file })
        }
    }
}

impl Write for AppendFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
