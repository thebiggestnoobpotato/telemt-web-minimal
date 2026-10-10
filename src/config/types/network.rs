use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UpstreamType {
    Direct {
        #[serde(default)]
        interface: Option<String>,
        #[serde(default)]
        bind_addresses: Option<Vec<String>>,
        /// Linux-only hard interface pinning via `SO_BINDTODEVICE`.
        #[serde(default)]
        bind_device: Option<String>,
    },
    Socks {
        socks_address: String,
        #[serde(default)]
        interface: Option<String>,
        #[serde(default)]
        socks_username: Option<String>,
        #[serde(default)]
        socks_password: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    #[serde(flatten)]
    pub upstream_type: UpstreamType,
    #[serde(default = "default_weight")]
    pub weight: u16,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
