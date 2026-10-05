use super::*;

/// Per-user unique source IP limit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserMaxUniqueIpsMode {
    /// Count only currently active source IPs.
    #[default]
    ActiveWindow,
    /// Count source IPs seen within the recent time window.
    TimeWindow,
    /// Enforce both active and recent-window limits at the same time.
    Combined,
}

/// Telemetry controls for hot-path counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_true")]
    pub core_enabled: bool,
    #[serde(default = "default_true")]
    pub user_enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            core_enabled: default_true(),
            user_enabled: default_true(),
        }
    }
}
