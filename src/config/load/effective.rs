use super::*;

pub(super) fn apply(config: &mut ProxyConfig) -> Result<()> {
    // Migration: prefer_ipv6 -> network.prefer.
    if config.general.prefer_ipv6 {
        if config.network.prefer == 4 {
            config.network.prefer = 6;
        }
        warn!("prefer_ipv6 is deprecated, use [network].prefer = 6");
    }

    validate_network_cfg(&mut config.network)?;
    crate::network::dns_overrides::validate_entries(&config.network.dns_overrides)?;

    // Migration: listeners[].port fallback to legacy server.port.
    for listener in &mut config.server.listeners {
        if listener.port.is_none() {
            listener.port = Some(config.server.port);
        }
    }

    // Migration: Populate upstreams if empty (Default Direct).
    if config.upstreams.is_empty() {
        config.upstreams.push(UpstreamConfig {
            upstream_type: UpstreamType::Direct {
                interface: None,
                bind_addresses: None,
                bindtodevice: None,
            },
            weight: 1,
            enabled: true,
            scopes: String::new(),
            selected_scope: String::new(),
            ipv4: None,
            ipv6: None,
            prefer: None,
        });
    }
    normalize_upstream_family_policy(config);

    // Ensure default DC203 override is present.
    config
        .dc_overrides
        .entry("203".to_string())
        .or_insert_with(|| vec!["91.105.192.100:443".to_string()]);

    validate_logging_config(&config.logging)?;
    validate_upstreams(config)?;
    config.rebuild_runtime_user_auth()?;
    config.rebuild_runtime_web()?;
    Ok(())
}
