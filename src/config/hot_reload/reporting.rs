use super::*;

/// Log all detected changes and emit WEB links for new users.
pub(super) fn log_changes(
    old_hot: &HotFields,
    new_hot: &HotFields,
    new_cfg: &ProxyConfig,
    log_tx: &watch::Sender<LogLevel>,
) {
    if old_hot.log_level != new_hot.log_level {
        info!(
            "config reload: log_level: '{}' → '{}'",
            old_hot.log_level, new_hot.log_level
        );
        log_tx.send(new_hot.log_level.clone()).ok();
    }

    if old_hot.telemetry_core_enabled != new_hot.telemetry_core_enabled
        || old_hot.telemetry_user_enabled != new_hot.telemetry_user_enabled
    {
        info!(
            "config reload: telemetry_core_enabled={} telemetry_user_enabled={}",
            new_hot.telemetry_core_enabled,
            new_hot.telemetry_user_enabled,
        );
    }


    if old_hot.direct_relay_copy_buf_c2s_bytes != new_hot.direct_relay_copy_buf_c2s_bytes
        || old_hot.direct_relay_copy_buf_s2c_bytes != new_hot.direct_relay_copy_buf_s2c_bytes
    {
        info!(
            "config reload: direct relay buffers: c2s={} s2c={}",
            new_hot.direct_relay_copy_buf_c2s_bytes,
            new_hot.direct_relay_copy_buf_s2c_bytes,
        );
    }

    if old_hot.users != new_hot.users {
        let mut added: Vec<&String> = new_hot
            .users
            .keys()
            .filter(|u| !old_hot.users.contains_key(*u))
            .collect();
        added.sort();

        let mut removed: Vec<&String> = old_hot
            .users
            .keys()
            .filter(|u| !new_hot.users.contains_key(*u))
            .collect();
        removed.sort();

        let mut changed: Vec<&String> = new_hot
            .users
            .keys()
            .filter(|u| {
                old_hot
                    .users
                    .get(*u)
                    .map(|s| s != &new_hot.users[*u])
                    .unwrap_or(false)
            })
            .collect();
        changed.sort();

        if !added.is_empty() {
            info!(
                "config reload: users added: [{}]",
                added
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for user in &added {
                info!(target: "telemt::links", "--- New user: {user} ---");
                let links = crate::web::links::web_links_for_user(new_cfg, user);
                if links.is_empty() {
                    info!(
                        target: "telemt::links",
                        "  (no WEB links: user is not assigned to a [web] profile)"
                    );
                }
                for link in links {
                    info!(target: "telemt::links", "  WEB: {link}");
                }
                info!(target: "telemt::links", "--------------------");
            }
        }
        if !removed.is_empty() {
            info!(
                "config reload: users removed: [{}]",
                removed
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !changed.is_empty() {
            info!(
                "config reload: users secret changed: [{}]",
                changed
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    if old_hot.user_enabled != new_hot.user_enabled {
        info!(
            "config reload: user_enabled updated ({} disabled overrides)",
            new_hot
                .user_enabled
                .values()
                .filter(|enabled| !**enabled)
                .count()
        );
    }
    if old_hot.user_max_tcp_conns != new_hot.user_max_tcp_conns {
        info!(
            "config reload: user_max_tcp_conns updated ({} entries)",
            new_hot.user_max_tcp_conns.len()
        );
    }
    if old_hot.global_user_max_tcp_conns != new_hot.global_user_max_tcp_conns {
        info!(
            "config reload: global_user_max_tcp_conns={}",
            new_hot.global_user_max_tcp_conns
        );
    }
    if old_hot.user_expirations != new_hot.user_expirations {
        info!(
            "config reload: user_expirations updated ({} entries)",
            new_hot.user_expirations.len()
        );
    }
    if old_hot.user_data_quota != new_hot.user_data_quota {
        info!(
            "config reload: user_data_quota updated ({} entries)",
            new_hot.user_data_quota.len()
        );
    }
    if old_hot.user_rate_limits != new_hot.user_rate_limits {
        info!(
            "config reload: user_rate_limits updated ({} entries)",
            new_hot.user_rate_limits.len()
        );
    }
    if old_hot.cidr_rate_limits != new_hot.cidr_rate_limits {
        info!(
            "config reload: cidr_rate_limits updated ({} entries)",
            new_hot.cidr_rate_limits.len()
        );
    }
    if old_hot.user_max_unique_ips != new_hot.user_max_unique_ips {
        info!(
            "config reload: user_max_unique_ips updated ({} entries)",
            new_hot.user_max_unique_ips.len()
        );
    }
    if old_hot.global_user_max_unique_ips != new_hot.global_user_max_unique_ips
        || old_hot.user_max_unique_ips_mode != new_hot.user_max_unique_ips_mode
        || old_hot.user_max_unique_ips_window_secs != new_hot.user_max_unique_ips_window_secs
    {
        info!(
            "config reload: global_user_max_unique_ips={} mode={:?} window={}s",
            new_hot.global_user_max_unique_ips,
            new_hot.user_max_unique_ips_mode,
            new_hot.user_max_unique_ips_window_secs
        );
    }
    if old_hot.web_debug != new_hot.web_debug {
        info!(
            "config reload: web.debug updated: enabled={} body_capture={:?} window={}..={}s",
            new_hot.web_debug.enabled,
            new_hot.web_debug.body_capture,
            new_hot.web_debug.default_window_secs,
            new_hot.web_debug.max_window_secs,
        );
    }
}
