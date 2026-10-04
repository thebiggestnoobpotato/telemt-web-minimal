//! Logging configuration for telemt.
//!
//! Supports multiple log destinations:
//! - stderr (default, works with systemd journald)
//! - syslog (Unix only, for traditional init systems)
//! - file (append-only)

// Infrastructure module used via CLI flags.
#![allow(dead_code)]

use crate::config::{LoggingConfig, LoggingDestination};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, reload};

// Submodules:
// - file: append-only file appender for file logging.
mod file;

#[cfg(test)]
mod tests;

/// Log destination configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LogDestination {
    /// Log to stderr (default, captured by systemd journald).
    #[default]
    Stderr,
    /// Log to syslog (Unix only).
    #[cfg(unix)]
    Syslog,
    /// Log to a file (append-only).
    File {
        /// Resolved log file path.
        path: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogCliDestination {
    Stderr,
    Syslog,
    File,
}

/// Logging-related CLI overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogCliOptions {
    destination: Option<LogCliDestination>,
    path: Option<String>,
}

/// Logging options parsed from CLI/config.
#[derive(Debug, Clone, Default)]
pub struct LoggingOptions {
    /// Where to send logs.
    pub destination: LogDestination,
    /// Disable ANSI colors.
    pub disable_colors: bool,
    /// Require trusted, symlink-free log parents on Unix. Disabled by default for compatibility.
    pub strict_runtime_paths: bool,
}

/// Guard that must be held to keep file logging active.
/// When dropped, flushes and closes log files.
pub struct LoggingGuard {
    _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

impl LoggingGuard {
    fn new(guard: Option<tracing_appender::non_blocking::WorkerGuard>) -> Self {
        Self { _guard: guard }
    }

    /// Creates a no-op guard for stderr/syslog logging.
    pub fn noop() -> Self {
        Self { _guard: None }
    }
}

/// Initialize the tracing subscriber with the specified options.
///
/// Returns a reload handle for dynamic log level changes and a guard
/// that must be kept alive for file logging.
pub fn init_logging(
    opts: &LoggingOptions,
    initial_filter: &str,
) -> (
    reload::Handle<EnvFilter, impl tracing::Subscriber + Send + Sync>,
    LoggingGuard,
) {
    let (filter_layer, filter_handle) = reload::Layer::new(EnvFilter::new(initial_filter));

    match &opts.destination {
        LogDestination::Stderr => {
            let fmt_layer = fmt::Layer::default()
                .with_ansi(!opts.disable_colors)
                .with_target(true);

            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt_layer)
                .init();

            (filter_handle, LoggingGuard::noop())
        }

        #[cfg(unix)]
        LogDestination::Syslog => {
            let fmt_layer = fmt::Layer::default()
                .with_ansi(false)
                .with_target(false)
                .with_level(false)
                .without_time()
                .with_writer(SyslogMakeWriter::new());

            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt_layer)
                .init();

            (filter_handle, LoggingGuard::noop())
        }

        LogDestination::File { path } => {
            let file_appender =
                file::AppendFileAppender::new(path, opts.strict_runtime_paths)
                    .expect("Failed to open log file");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            let fmt_layer = fmt::Layer::default()
                .with_ansi(false)
                .with_target(true)
                .with_writer(non_blocking);

            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt_layer)
                .init();

            (filter_handle, LoggingGuard::new(Some(guard)))
        }
    }
}

/// Syslog writer for tracing.
#[cfg(unix)]
#[derive(Clone, Copy)]
struct SyslogMakeWriter;

#[cfg(unix)]
#[derive(Clone, Copy)]
struct SyslogWriter {
    priority: libc::c_int,
}

#[cfg(unix)]
impl SyslogMakeWriter {
    fn new() -> Self {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| unsafe {
            let ident = b"telemt\0".as_ptr() as *const libc::c_char;
            libc::openlog(ident, libc::LOG_PID | libc::LOG_NDELAY, libc::LOG_DAEMON);
        });
        Self
    }
}

#[cfg(unix)]
fn syslog_priority_for_level(level: &tracing::Level) -> libc::c_int {
    match *level {
        tracing::Level::ERROR => libc::LOG_ERR,
        tracing::Level::WARN => libc::LOG_WARNING,
        tracing::Level::INFO => libc::LOG_INFO,
        tracing::Level::DEBUG => libc::LOG_DEBUG,
        tracing::Level::TRACE => libc::LOG_DEBUG,
    }
}

#[cfg(unix)]
impl std::io::Write for SyslogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let msg = String::from_utf8_lossy(buf);
        let msg = msg.trim_end();

        if msg.is_empty() {
            return Ok(buf.len());
        }

        let c_msg = std::ffi::CString::new(msg.as_bytes())
            .unwrap_or_else(|_| std::ffi::CString::new("(invalid utf8)").unwrap());

        unsafe {
            libc::syslog(
                self.priority,
                b"%s\0".as_ptr() as *const libc::c_char,
                c_msg.as_ptr(),
            );
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SyslogMakeWriter {
    type Writer = SyslogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SyslogWriter {
            priority: libc::LOG_INFO,
        }
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        SyslogWriter {
            priority: syslog_priority_for_level(meta.level()),
        }
    }
}

/// Parse logging overrides from CLI arguments.
pub fn parse_log_cli_options(args: &[String]) -> Result<LogCliOptions, String> {
    let mut options = LogCliOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            #[cfg(unix)]
            "--syslog" => {
                options.destination = Some(LogCliDestination::Syslog);
            }
            #[cfg(not(unix))]
            "--syslog" => {
                options.destination = Some(LogCliDestination::Syslog);
            }
            "--log-file" => {
                i += 1;
                if i < args.len() {
                    options.destination = Some(LogCliDestination::File);
                    options.path = Some(args[i].clone());
                } else {
                    return Err("Missing value for --log-file".to_string());
                }
            }
            s if s.starts_with("--log-file=") => {
                options.destination = Some(LogCliDestination::File);
                options.path = Some(s.trim_start_matches("--log-file=").to_string());
            }
            _ => {}
        }
        i += 1;
    }
    Ok(options)
}

/// Resolve effective logging destination from config and CLI overrides.
pub fn resolve_log_destination(
    config: &LoggingConfig,
    cli: &LogCliOptions,
) -> Result<LogDestination, String> {
    let destination = cli.destination.unwrap_or(match config.destination {
        LoggingDestination::Stderr => LogCliDestination::Stderr,
        LoggingDestination::Syslog => LogCliDestination::Syslog,
        LoggingDestination::File => LogCliDestination::File,
    });

    match destination {
        LogCliDestination::Stderr => Ok(LogDestination::Stderr),
        LogCliDestination::Syslog => {
            #[cfg(unix)]
            {
                Ok(LogDestination::Syslog)
            }
            #[cfg(not(unix))]
            {
                Err("Syslog logging is only supported on Unix platforms".to_string())
            }
        }
        LogCliDestination::File => {
            let path = cli.path.as_ref().or(config.path.as_ref()).ok_or_else(|| {
                "logging.path or --log-file must be set when file logging is enabled".to_string()
            })?;
            if path.trim().is_empty() {
                return Err("Log file path cannot be empty".to_string());
            }

            Ok(LogDestination::File {
                path: path.clone(),
            })
        }
    }
}
