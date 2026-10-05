use tracing::warn;

use crate::error::{ProxyError, Result};

use super::super::types::{GeneralConfig, LoggingConfig, LoggingDestination};
use super::ProxyConfig;

pub(super) fn validate_network_cfg(net: &mut GeneralConfig) -> Result<()> {
    if !net.network_ipv4 && matches!(net.network_ipv6, Some(false)) {
        return Err(ProxyError::Config(
            "Both network_ipv4 and network_ipv6 are disabled in [general]".to_string(),
        ));
    }

    if net.network_prefer != 4 && net.network_prefer != 6 {
        return Err(ProxyError::Config(
            "general.network_prefer must be 4 or 6".to_string(),
        ));
    }

    if !net.network_ipv4 && net.network_prefer == 4 {
        warn!("network_prefer=4 but network_ipv4=false; forcing network_prefer=6");
        net.network_prefer = 6;
    }

    if matches!(net.network_ipv6, Some(false)) && net.network_prefer == 6 {
        warn!("network_prefer=6 but network_ipv6=false; forcing network_prefer=4");
        net.network_prefer = 4;
    }

    Ok(())
}

pub(super) fn validate_logging_config(logging: &LoggingConfig) -> Result<()> {
    if let Some(path) = logging.path.as_ref()
        && path.trim().is_empty()
    {
        return Err(ProxyError::Config(
            "logging.path cannot be empty when provided".to_string(),
        ));
    }

    if matches!(logging.destination, LoggingDestination::File) && logging.path.is_none() {
        return Err(ProxyError::Config(
            "logging.path must be set when logging.destination=\"file\"".to_string(),
        ));
    }

    Ok(())
}

pub(super) fn validate_upstreams(config: &ProxyConfig) -> Result<()> {
    for upstream in &config.upstreams {
        if matches!(upstream.ipv4, Some(false)) && matches!(upstream.ipv6, Some(false)) {
            return Err(ProxyError::Config(
                "upstream.ipv4 and upstream.ipv6 cannot both be false".to_string(),
            ));
        }
        if let Some(prefer) = upstream.prefer
            && prefer != 4
            && prefer != 6
        {
            return Err(ProxyError::Config(
                "upstream.prefer must be 4 or 6".to_string(),
            ));
        }

    }

    Ok(())
}

pub(super) fn normalize_upstream_family_policy(config: &mut ProxyConfig) {
    for (idx, upstream) in config.upstreams.iter_mut().enumerate() {
        if matches!(upstream.ipv4, Some(false)) && upstream.prefer == Some(4) {
            warn!(
                upstream = idx,
                "upstream.prefer=4 but upstream.ipv4=false; forcing prefer=6"
            );
            upstream.prefer = Some(6);
        }

        if matches!(upstream.ipv6, Some(false)) && upstream.prefer == Some(6) {
            warn!(
                upstream = idx,
                "upstream.prefer=6 but upstream.ipv6=false; forcing prefer=4"
            );
            upstream.prefer = Some(4);
        }
    }
}
