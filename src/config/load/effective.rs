use super::*;

pub(super) fn apply(config: &mut ProxyConfig) -> Result<()> {
    // Migration: prefer_ipv6 -> general.network_prefer.
    if config.general.prefer_ipv6 {
        if config.general.network_prefer == 4 {
            config.general.network_prefer = 6;
        }
        warn!("prefer_ipv6 is deprecated, use [general].network_prefer = 6");
    }

    validate_network_cfg(&mut config.general)?;

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
        .general.dc_overrides
        .entry("203".to_string())
        .or_insert_with(|| vec!["91.105.192.100:443".to_string()]);

    validate_logging_config(&config.logging)?;
    validate_upstreams(config)?;
    config.rebuild_runtime_user_auth()?;
    config.rebuild_runtime_web()?;
    Ok(())
}
