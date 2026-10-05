use super::*;

/// Logging verbosity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// All messages including trace (trace + debug + info + warn + error).
    Debug,
    /// Detailed operational logs (debug + info + warn + error).
    Verbose,
    /// Standard operational logs (info + warn + error).
    #[default]
    Normal,
    /// Minimal output: only warnings and errors (warn + error).
    /// Proxy links may still be emitted through their dedicated target.
    Silent,
}

impl LogLevel {
    /// Convert to tracing EnvFilter directive string.
    pub fn to_filter_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "trace",
            LogLevel::Verbose => "debug",
            LogLevel::Normal => "info",
            LogLevel::Silent => "warn",
        }
    }

    /// Parse from a loose string (CLI argument).
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" | "trace" => LogLevel::Debug,
            "verbose" => LogLevel::Verbose,
            "normal" | "info" => LogLevel::Normal,
            "silent" | "quiet" | "error" | "warn" => LogLevel::Silent,
            _ => LogLevel::Normal,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Verbose => write!(f, "verbose"),
            LogLevel::Normal => write!(f, "normal"),
            LogLevel::Silent => write!(f, "silent"),
        }
    }
}

/// Logging output destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LoggingDestination {
    /// Write logs to stderr.
    #[default]
    Stderr,
    /// Write logs to syslog on Unix platforms.
    Syslog,
    /// Write logs to a file.
    File,
}

/// Runtime logging settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Effective logging destination.
    #[serde(default)]
    pub destination: LoggingDestination,
    /// File path used when `destination = "file"`.
    #[serde(default)]
    pub path: Option<String>,
    /// Runtime logging verbosity level.
    #[serde(default)]
    pub log_level: LogLevel,
    /// Enable unknown-DC logging: distinct unknown DC indices are recorded
    /// once each in the main log destination.
    #[serde(default = "default_unknown_dc_log_enabled")]
    pub unknown_dc_log_enabled: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            destination: LoggingDestination::Stderr,
            path: None,
            log_level: LogLevel::Normal,
            unknown_dc_log_enabled: default_unknown_dc_log_enabled(),
        }
    }
}
