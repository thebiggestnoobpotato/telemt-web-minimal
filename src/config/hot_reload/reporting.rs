use super::*;

fn resolve_default_link_port(cfg: &ProxyConfig) -> u16 {
    cfg.server
        .listeners
        .first()
        .and_then(|listener| listener.port)
        .unwrap_or(cfg.server.port)
}

fn resolve_link_host(
    cfg: &ProxyConfig,
    detected_ip_v4: Option<IpAddr>,
    detected_ip_v6: Option<IpAddr>,
) -> String {
    if let Some(ref h) = cfg.general.links.public_host {
        return h.clone();
    }
    detected_ip_v4
        .or(detected_ip_v6)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| {
            warn!(
                "config reload: could not determine public IP for proxy links. \
                 Set [general.links] public_host in config."
            );
            "UNKNOWN".to_string()
        })
}

/// Print TG proxy links for a single user — mirrors print_proxy_links() in main.rs.
fn print_user_links(user: &str, secret: &str, host: &str, port: u16, cfg: &ProxyConfig) {
    info!(target: "telemt::links", "--- New user: {} ---", user);
    if cfg.general.modes.classic {
        info!(
            target: "telemt::links",
            "  Classic: tg://proxy?server={}&port={}&secret={}",
            host, port, secret
        );
    }
    if cfg.general.modes.secure {
        info!(
            target: "telemt::links",
            "  DD:      tg://proxy?server={}&port={}&secret=dd{}",
            host, port, secret
        );
    }
    if cfg.general.modes.tls {
        let mut domains = vec![cfg.censorship.tls_domain.clone()];
        for d in &cfg.censorship.tls_domains {
            if !domains.contains(d) {
                domains.push(d.clone());
            }
        }
        for domain in &domains {
            let domain_hex = hex::encode(domain.as_bytes());
            info!(
                target: "telemt::links",
                "  EE-TLS:  tg://proxy?server={}&port={}&secret=ee{}{}",
                host, port, secret, domain_hex
            );
        }
    }
    info!(target: "telemt::links", "--------------------");
}

/// Log all detected changes and emit TG links for new users.
pub(super) fn log_changes(
    old_hot: &HotFields,
    new_hot: &HotFields,
    new_cfg: &ProxyConfig,
    log_tx: &watch::Sender<LogLevel>,
    detected_ip_v4: Option<IpAddr>,
    detected_ip_v6: Option<IpAddr>,
) {
    if old_hot.log_level != new_hot.log_level {
        info!(
            "config reload: log_level: '{}' → '{}'",
            old_hot.log_level, new_hot.log_level
        );
        log_tx.send(new_hot.log_level.clone()).ok();
    }

    if old_hot.user_ad_tags != new_hot.user_ad_tags {
        info!(
            "config reload: user_ad_tags updated ({} entries)",
            new_hot.user_ad_tags.len(),
        );
    }

    if old_hot.ad_tag != new_hot.ad_tag {
        info!("config reload: general.ad_tag updated (applied on next connection)");
    }

    if old_hot.dns_overrides != new_hot.dns_overrides {
        info!(
            "config reload: network.dns_overrides updated ({} entries)",
            new_hot.dns_overrides.len()
        );
    }


    if old_hot.telemetry_core_enabled != new_hot.telemetry_core_enabled
        || old_hot.telemetry_user_enabled != new_hot.telemetry_user_enabled
    {
        info!(
            "config reload: telemetry: core_enabled={} user_enabled={}",
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
            let host = resolve_link_host(new_cfg, detected_ip_v4, detected_ip_v6);
            let port = new_cfg
                .general
                .links
                .public_port
                .unwrap_or(resolve_default_link_port(new_cfg));
            for user in &added {
                if let Some(secret) = new_hot.users.get(*user) {
                    print_user_links(user, secret, &host, port, new_cfg);
                }
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
    if old_hot.user_max_tcp_conns_global_each != new_hot.user_max_tcp_conns_global_each {
        info!(
            "config reload: user_max_tcp_conns policy global_each={}",
            new_hot.user_max_tcp_conns_global_each
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
    if old_hot.user_max_unique_ips_global_each != new_hot.user_max_unique_ips_global_each
        || old_hot.user_max_unique_ips_mode != new_hot.user_max_unique_ips_mode
        || old_hot.user_max_unique_ips_window_secs != new_hot.user_max_unique_ips_window_secs
    {
        info!(
            "config reload: user_max_unique_ips policy global_each={} mode={:?} window={}s",
            new_hot.user_max_unique_ips_global_each,
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
