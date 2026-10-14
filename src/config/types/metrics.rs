use super::*;

/// Prometheus-compatible metrics endpoint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable or disable the Prometheus metrics endpoint.
    #[serde(default)]
    pub enabled: bool,

    /// Listen address for metrics in `IP:PORT` format (e.g. `"127.0.0.1:9090"`).
    /// Required when `enabled = true`; binds on the specified address only.
    #[serde(default)]
    pub listen_address: Option<String>,

    /// CIDR whitelist for the metrics endpoint.
    #[serde(default = "default_metrics_whitelist")]
    pub whitelist: Vec<IpNetwork>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen_address: None,
            whitelist: default_metrics_whitelist(),
        }
    }
}
