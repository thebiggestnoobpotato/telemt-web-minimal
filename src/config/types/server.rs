use std::path::PathBuf;

use super::*;

/// Trusted L7 source used to recover a WEB client's identity address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WebClientIpSource {
    /// Use one parseable `X-Forwarded-For` address or the trusted direct peer.
    #[default]
    XForwardedFor,
}

/// Bind identity of one process-owned listener endpoint.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ListenerEndpoint {
    /// TCP endpoint with a concrete IP and port.
    Tcp(std::net::SocketAddr),
    /// Unix domain socket path.
    Unix(PathBuf),
}

impl ListenerEndpoint {
    /// Complete endpoint of a validated listener entry.
    pub(crate) fn from_listener(listener: &ListenerConfig) -> Option<ListenerEndpoint> {
        if let Some(path) = &listener.socket_path {
            return Some(ListenerEndpoint::Unix(PathBuf::from(path)));
        }
        Some(ListenerEndpoint::Tcp(std::net::SocketAddr::new(
            listener.ip?,
            listener.port?,
        )))
    }
}

impl fmt::Display for ListenerEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListenerEndpoint::Tcp(addr) => write!(f, "{addr}"),
            ListenerEndpoint::Unix(path) => write!(f, "unix:{}", path.display()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    /// TCP bind IP. Required for TCP listeners, omitted for unix socket listeners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<IpAddr>,
    /// TCP port. Required for TCP listeners, omitted for unix socket listeners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Unix domain socket path. Required for unix socket listeners,
    /// omitted for TCP listeners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<String>,
    /// Unix socket file permissions (octal, e.g. "0660"). Applied via chmod
    /// after bind. Omitted keeps the umask-derived mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_perm: Option<String>,
    /// L7 header policy used by WEB listeners.
    #[serde(default)]
    pub web_client_ip_source: WebClientIpSource,
    /// Immediate socket peers allowed to provide the WEB client identity header.
    #[serde(default)]
    pub web_trusted_proxy_cidrs: Vec<IpNetwork>,
}
