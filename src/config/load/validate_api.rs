use super::*;

pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    if !(1..=MAX_API_REQUEST_BODY_LIMIT_BYTES).contains(&config.api.request_body_limit_bytes) {
        return Err(ProxyError::Config(
            "api.request_body_limit_bytes must be within [1, 1048576]".to_string(),
        ));
    }

    if config.api.minimal_runtime_cache_ttl_ms > 60_000 {
        return Err(ProxyError::Config(
            "api.minimal_runtime_cache_ttl_ms must be within [0, 60000]".to_string(),
        ));
    }

    if config.api.runtime_edge_cache_ttl_ms > 60_000 {
        return Err(ProxyError::Config(
            "api.runtime_edge_cache_ttl_ms must be within [0, 60000]".to_string(),
        ));
    }

    if !(1..=1000).contains(&config.api.runtime_edge_top_n) {
        return Err(ProxyError::Config(
            "api.runtime_edge_top_n must be within [1, 1000]".to_string(),
        ));
    }

    if !(16..=4096).contains(&config.api.runtime_edge_events_capacity) {
        return Err(ProxyError::Config(
            "api.runtime_edge_events_capacity must be within [16, 4096]".to_string(),
        ));
    }

    if config.api.listen.parse::<SocketAddr>().is_err() {
        return Err(ProxyError::Config(
            "api.listen must be in IP:PORT format".to_string(),
        ));
    }

    Ok(())
}
