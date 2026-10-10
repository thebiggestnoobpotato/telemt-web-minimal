use tracing::warn;

use crate::error::{ProxyError, Result};

use super::super::types::{GeneralConfig, LoggingConfig, LoggingDestination};

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


