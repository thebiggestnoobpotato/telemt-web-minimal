use super::*;

pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    if config.general.upstream_connect_retry_attempts == 0 {
        return Err(ProxyError::Config(
            "general.upstream_connect_retry_attempts must be > 0".to_string(),
        ));
    }

    if config.general.upstream_connect_budget_ms == 0 {
        return Err(ProxyError::Config(
            "general.upstream_connect_budget_ms must be > 0".to_string(),
        ));
    }

    if config.general.upstream_connect_timeout == 0 {
        return Err(ProxyError::Config(
            "general.upstream_connect_timeout must be > 0".to_string(),
        ));
    }

    if config.general.upstream_unhealthy_fail_threshold == 0 {
        return Err(ProxyError::Config(
            "general.upstream_unhealthy_fail_threshold must be > 0".to_string(),
        ));
    }

    // The kernel treats a zero backlog as a literal accept-queue limit, not a
    // request for the OS default, so zero would drop every connection beyond
    // the first queued one; the upper bound is the i32 argument of listen(2).
    if config.general.listen_backlog == 0 || config.general.listen_backlog > i32::MAX as u32 {
        return Err(ProxyError::Config(format!(
            "general.listen_backlog must be within [1, {}]",
            i32::MAX
        )));
    }


    if config.timeouts.client_handshake == 0 {
        return Err(ProxyError::Config(
            "timeouts.client_handshake must be > 0".to_string(),
        ));
    }

    Ok(())
}
