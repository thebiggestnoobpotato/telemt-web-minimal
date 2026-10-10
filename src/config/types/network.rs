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
        /// Optional alias: `force_bind`.
        #[serde(default, alias = "force_bind")]
        bindtodevice: Option<String>,
    },
    Socks {
        address: String,
        #[serde(default)]
        interface: Option<String>,
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
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
    #[serde(default)]
    pub scopes: String,
    #[serde(skip)]
    pub selected_scope: String,
    /// Allow IPv4 DC targets for this upstream.
    /// `None` means auto-detect from runtime connectivity state.
    #[serde(default)]
    pub ipv4: Option<bool>,
    /// Allow IPv6 DC targets for this upstream.
    /// `None` means auto-detect from runtime connectivity state.
    #[serde(default)]
    pub ipv6: Option<bool>,
    /// Per-upstream IP family preference for Telegram DC targets.
    /// `None` inherits the effective global `[general].network_prefer` decision.
    #[serde(default)]
    pub prefer: Option<u8>,
}

impl UpstreamConfig {
    pub fn prefer_ipv6(&self, default_prefer_ipv6: bool) -> bool {
        match self.prefer {
            Some(6) => true,
            Some(4) => false,
            _ => default_prefer_ipv6,
        }
    }
}
