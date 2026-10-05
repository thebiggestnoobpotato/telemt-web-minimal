use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_true")]
    pub ipv4: bool,

    /// None = auto-detect IPv6 availability.
    #[serde(default = "default_network_ipv6")]
    pub ipv6: Option<bool>,

    /// 4 or 6.
    #[serde(default = "default_prefer_4")]
    pub prefer: u8,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            ipv4: default_true(),
            ipv6: default_network_ipv6(),
            prefer: default_prefer_4(),
        }
    }
}

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
    Socks5 {
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
    /// `None` inherits the effective global `[network].prefer` decision.
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
