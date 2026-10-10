use super::*;

/// Normalizes source policy and authentication data before external fallback preparation.
pub(super) fn apply(config: &mut ProxyConfig) -> Result<()> {
    validate_network_cfg(&mut config.general)?;

    // Migration: Populate upstreams if empty (Default Direct).
    if config.upstreams.is_empty() {
        config.upstreams.push(UpstreamConfig {
            upstream_type: UpstreamType::Direct {
                interface: None,
                bind_addresses: None,
                bind_device: None,
            },
            weight: 1,
            enabled: true,
        });
    }

    // Ensure default DC203 override is present.
    config
        .general.dc_overrides
        .entry("203".to_string())
        .or_insert_with(|| vec!["91.105.192.100:443".to_string()]);

    validate_logging_config(&config.logging)?;
    config.rebuild_runtime_user_auth()?;
    Ok(())
}
