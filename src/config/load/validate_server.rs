use super::*;

pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    if !(1..=MAX_API_REQUEST_BODY_LIMIT_BYTES).contains(&config.server.api.request_body_limit_bytes)
    {
        return Err(ProxyError::Config(
            "server.api.request_body_limit_bytes must be within [1, 1048576]".to_string(),
        ));
    }

    if config.server.api.minimal_runtime_cache_ttl_ms > 60_000 {
        return Err(ProxyError::Config(
            "server.api.minimal_runtime_cache_ttl_ms must be within [0, 60000]".to_string(),
        ));
    }

    if config.server.api.runtime_edge_cache_ttl_ms > 60_000 {
        return Err(ProxyError::Config(
            "server.api.runtime_edge_cache_ttl_ms must be within [0, 60000]".to_string(),
        ));
    }

    if !(1..=1000).contains(&config.server.api.runtime_edge_top_n) {
        return Err(ProxyError::Config(
            "server.api.runtime_edge_top_n must be within [1, 1000]".to_string(),
        ));
    }

    if !(16..=4096).contains(&config.server.api.runtime_edge_events_capacity) {
        return Err(ProxyError::Config(
            "server.api.runtime_edge_events_capacity must be within [16, 4096]".to_string(),
        ));
    }

    if config.server.api.listen.parse::<SocketAddr>().is_err() {
        return Err(ProxyError::Config(
            "server.api.listen must be in IP:PORT format".to_string(),
        ));
    }

    if config.server.accept_permit_timeout_ms > 60_000 {
        return Err(ProxyError::Config(
            "server.accept_permit_timeout_ms must be within [0, 60000]".to_string(),
        ));
    }

    // Validate secrets.
    for (user, secret) in &config.access.users {
        if !secret.chars().all(|c| c.is_ascii_hexdigit()) || secret.len() != 32 {
            return Err(ProxyError::InvalidSecret {
                user: user.clone(),
                reason: "Must be 32 hex characters".to_string(),
            });
        }
    }

    Ok(())
}
