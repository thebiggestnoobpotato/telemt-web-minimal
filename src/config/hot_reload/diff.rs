use super::*;

fn canonicalize_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let mut pairs: Vec<(String, serde_json::Value)> =
                std::mem::take(map).into_iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, item) in pairs.iter_mut() {
                canonicalize_json(item);
            }
            for (key, item) in pairs {
                map.insert(key, item);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                canonicalize_json(item);
            }
        }
        _ => {}
    }
}

pub(super) fn config_equal(lhs: &ProxyConfig, rhs: &ProxyConfig) -> bool {
    let mut left = match serde_json::to_value(lhs) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let mut right = match serde_json::to_value(rhs) {
        Ok(value) => value,
        Err(_) => return false,
    };
    canonicalize_json(&mut left);
    canonicalize_json(&mut right);
    left == right
}

/// Warns when the requested snapshot contains fields that require restart.
pub(super) fn warn_non_hot_changes(old: &ProxyConfig, new: &ProxyConfig, non_hot_changed: bool) {
    let mut warned = false;
    if old.api.enabled != new.api.enabled
        || old.api.listen != new.api.listen
        || old.api.whitelist != new.api.whitelist
        || old.api.gray_action != new.api.gray_action
        || old.api.auth_header != new.api.auth_header
        || old.api.request_body_limit_bytes != new.api.request_body_limit_bytes
        || old.api.minimal_runtime_enabled != new.api.minimal_runtime_enabled
        || old.api.minimal_runtime_cache_ttl_ms
            != new.api.minimal_runtime_cache_ttl_ms
        || old.api.runtime_edge_enabled != new.api.runtime_edge_enabled
        || old.api.runtime_edge_cache_ttl_ms != new.api.runtime_edge_cache_ttl_ms
        || old.api.runtime_edge_top_n != new.api.runtime_edge_top_n
        || old.api.runtime_edge_events_capacity
            != new.api.runtime_edge_events_capacity
        || old.api.read_only != new.api.read_only
    {
        warned = true;
        warn!("config reload: api changed; restart required");
    }
    if serde_json::to_value(&old.listener).ok() != serde_json::to_value(&new.listener).ok()
        || old.general.listen_backlog != new.general.listen_backlog
    {
        warned = true;
        warn!("config reload: server listener settings changed; restart required");
    }
    if old.web.decoy_fasttrack_mode != new.web.decoy_fasttrack_mode {
        warned = true;
        warn!("config reload: web.decoy_fasttrack_mode changed; restart required");
    }
    if old.general.network_ipv4 != new.general.network_ipv4
        || old.general.network_ipv6 != new.general.network_ipv6
    {
        warned = true;
        warn!("config reload: general.network_ipv4/network_ipv6 changed; restart required");
    }
    if old.general.network_prefer != new.general.network_prefer {
        warned = true;
        warn!("config reload: general.network_prefer changed; restart required");
    }
    if old.logging.unknown_dc_log_enabled != new.logging.unknown_dc_log_enabled {
        warned = true;
        warn!("config reload: logging.unknown_dc_log_enabled changed; restart required");
    }
    if old.logging.show_users_links != new.logging.show_users_links {
        warned = true;
        warn!("config reload: logging.show_users_links changed; restart required");
    }
    if old.general.upstream_connect_retry_attempts != new.general.upstream_connect_retry_attempts
        || old.general.upstream_connect_retry_backoff_ms
            != new.general.upstream_connect_retry_backoff_ms
        || old.general.upstream_connect_timeout
            != new.general.upstream_connect_timeout
        || old.general.upstream_unhealthy_fail_threshold
            != new.general.upstream_unhealthy_fail_threshold
        || old.general.upstream_connect_failfast_hard_errors
            != new.general.upstream_connect_failfast_hard_errors
    {
        warned = true;
        warn!("config reload: general.upstream_* changed; restart required");
    }
    if non_hot_changed && !warned {
        warn!("config reload: one or more non-hot fields changed; restart required");
    }
}

/// Which top-level config sections changed and whether any require a restart.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ChangeClassification {
    pub changed: Vec<String>,
    pub restart_required: bool,
}

/// Classify old->new using Telemt's OWN reload rule: overlay the hot fields and
/// see if anything non-hot remains different. This guarantees `restart_required`
/// matches actual runtime behavior and never drifts as new fields are added.
pub fn classify_config_changes(old: &ProxyConfig, new: &ProxyConfig) -> ChangeClassification {
    let applied = overlay_hot_fields(old, new);
    let restart_required = !config_equal(&applied, new);
    ChangeClassification {
        changed: changed_sections(old, new),
        restart_required,
    }
}

/// Top-level config sections whose canonical serialized form differs between
/// old and new. Uses the same serialize+canonicalize path as `config_equal`.
fn changed_sections(old: &ProxyConfig, new: &ProxyConfig) -> Vec<String> {
    let mut lhs = serde_json::to_value(old).unwrap_or(serde_json::Value::Null);
    let mut rhs = serde_json::to_value(new).unwrap_or(serde_json::Value::Null);
    canonicalize_json(&mut lhs);
    canonicalize_json(&mut rhs);

    let mut out = Vec::new();
    if let (Some(lo), Some(ro)) = (lhs.as_object(), rhs.as_object()) {
        let mut keys: std::collections::BTreeSet<&String> = lo.keys().collect();
        keys.extend(ro.keys());
        for key in keys {
            if lo.get(key) != ro.get(key) {
                out.push(key.clone());
            }
        }
    }
    out
}
