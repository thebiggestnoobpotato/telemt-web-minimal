use super::*;

/// Prometheus-compatible metrics endpoint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Port for the metrics endpoint.
    /// Enables metrics when set; binds on loopback (dual-stack) by default.
    #[serde(default)]
    pub port: Option<u16>,

    /// Listen address for metrics in `IP:PORT` format (e.g. `"127.0.0.1:9090"`).
    /// When set, takes precedence over `port` and binds on the specified address only.
    #[serde(default)]
    pub listen: Option<String>,

    /// CIDR whitelist for the metrics endpoint.
    #[serde(default = "default_metrics_whitelist")]
    pub whitelist: Vec<IpNetwork>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            port: None,
            listen: None,
            whitelist: default_metrics_whitelist(),
        }
    }
}
