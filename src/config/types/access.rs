use serde::{Deserialize, Serialize};

/// One access user: the shared secret plus optional per-user WEB ceilings.
///
/// Every field except `secret` is optional; an absent ceiling inherits the
/// corresponding global `limits` value and an absent host inherits
/// `web.vhost.host`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserEntry {
    /// 32-hex-character secret authenticated by the inner MTProxy handshake.
    pub secret: String,
    /// Optional per-user hostname; `web.vhost.host` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Optional per-user live session ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sessions: Option<usize>,
    /// Optional per-user live logical-stream ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_streams: Option<usize>,
    /// Optional per-user stream ceiling for one session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_streams_per_session: Option<usize>,
}

impl From<String> for UserEntry {
    fn from(secret: String) -> Self {
        Self {
            secret,
            host: None,
            max_sessions: None,
            max_streams: None,
            max_streams_per_session: None,
        }
    }
}
