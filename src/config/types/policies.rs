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

