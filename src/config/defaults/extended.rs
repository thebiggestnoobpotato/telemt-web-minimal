use std::collections::HashMap;

use serde::Deserialize;

use super::*;

pub(crate) fn default_upstream_connect_retry_attempts() -> u32 {
    DEFAULT_UPSTREAM_CONNECT_RETRY_ATTEMPTS
}

pub(crate) fn default_upstream_connect_retry_backoff_ms() -> u64 {
    100
}

pub(crate) fn default_upstream_unhealthy_fail_threshold() -> u32 {
    DEFAULT_UPSTREAM_UNHEALTHY_FAIL_THRESHOLD
}

pub(crate) fn default_upstream_connect_budget_ms() -> u64 {
    DEFAULT_UPSTREAM_CONNECT_BUDGET_MS
}

pub(crate) fn default_upstream_connect_failfast_hard_errors() -> bool {
    false
}

pub(crate) fn default_crypto_pending_buffer() -> usize {
    256 * 1024
}

pub(crate) fn default_max_client_frame() -> usize {
    16 * 1024 * 1024
}

pub(crate) fn default_access_users() -> HashMap<String, String> {
    HashMap::from([(
        DEFAULT_ACCESS_USER.to_string(),
        DEFAULT_ACCESS_SECRET.to_string(),
    )])
}

pub(crate) fn default_user_max_unique_ips_window_secs() -> u64 {
    DEFAULT_USER_MAX_UNIQUE_IPS_WINDOW_SECS
}

pub(crate) fn default_user_max_tcp_conns_global_each() -> usize {
    0
}

pub(crate) fn default_user_max_unique_ips_global_each() -> usize {
    0
}

// Custom deserializer helpers

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

pub(crate) fn deserialize_dc_overrides<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, Vec<String>>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let raw: HashMap<String, OneOrMany> = HashMap::deserialize(deserializer)?;
    let mut out = HashMap::new();
    for (dc, val) in raw {
        let mut addrs = match val {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        };
        addrs.retain(|s| !s.trim().is_empty());
        if !addrs.is_empty() {
            out.insert(dc, addrs);
        }
    }
    Ok(out)
}
