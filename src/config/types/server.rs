use super::*;

/// Application protocol accepted by one process-owned TCP listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ListenerTransport {
    /// Plain HTTP WEB gateway behind a trusted TLS terminator.
    #[default]
    Web,
}

/// Trusted L7 source used to recover a WEB client's identity address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WebClientIpSource {
    /// Use one parseable `X-Forwarded-For` address or the trusted direct peer.
    #[default]
    XForwardedFor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Legacy listener port used for backward compatibility.
    /// For new configs prefer `[[server.listeners]].port`.
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default, alias = "admin_api")]
    pub api: ApiConfig,

    #[serde(default)]
    pub listeners: Vec<ListenerConfig>,

    /// TCP `listen(2)` backlog for client-facing sockets (also used for the metrics HTTP listener).
    /// The effective queue is capped by the kernel (for example `somaxconn` on Linux).
    #[serde(default = "default_listen_backlog")]
    pub listen_backlog: u32,

    /// Maximum number of concurrent client connections.
    /// 0 means unlimited.
    #[serde(default = "default_server_max_connections")]
    pub max_connections: u32,

    /// Maximum wait in milliseconds while acquiring a connection slot permit.
    /// `0` keeps legacy unbounded wait behavior.
    #[serde(default = "default_accept_permit_timeout_ms")]
    pub accept_permit_timeout_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            api: ApiConfig::default(),
            listeners: Vec::new(),
            listen_backlog: default_listen_backlog(),
            max_connections: default_server_max_connections(),
            accept_permit_timeout_ms: default_accept_permit_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutsConfig {
    /// Maximum idle wait in seconds for the first client byte before handshake parsing starts.
    /// `0` disables the separate idle phase and keeps legacy timeout behavior.
    #[serde(default = "default_client_first_byte_idle_secs")]
    pub client_first_byte_idle_secs: u64,

    /// Maximum active handshake duration in seconds after the first client byte is received.
    #[serde(default = "default_handshake_timeout")]
    pub client_handshake: u64,

    #[serde(default = "default_keepalive")]
    pub client_keepalive: u64,

    #[serde(default = "default_ack_timeout")]
    pub client_ack: u64,
}

impl Default for TimeoutsConfig {
    fn default() -> Self {
        Self {
            client_first_byte_idle_secs: default_client_first_byte_idle_secs(),
            client_handshake: default_handshake_timeout(),
            client_keepalive: default_keepalive(),
            client_ack: default_ack_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub ip: IpAddr,
    /// Application protocol accepted by this listener.
    #[serde(default)]
    pub transport: ListenerTransport,
    /// Per-listener TCP port. If omitted, falls back to legacy `server.port`.
    #[serde(default)]
    pub port: Option<u16>,
    /// L7 header policy used by WEB listeners.
    #[serde(default)]
    pub web_client_ip_source: WebClientIpSource,
    /// Immediate socket peers allowed to provide the WEB client identity header.
    #[serde(default)]
    pub web_trusted_proxy_cidrs: Vec<IpNetwork>,
}
