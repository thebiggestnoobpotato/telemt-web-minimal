use super::*;

pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    if !(MIN_MAX_CLIENT_FRAME_BYTES..=MAX_MAX_CLIENT_FRAME_BYTES)
        .contains(&config.general.max_client_frame)
    {
        return Err(ProxyError::Config(format!(
            "general.max_client_frame must be within [{MIN_MAX_CLIENT_FRAME_BYTES}, {MAX_MAX_CLIENT_FRAME_BYTES}]"
        )));
    }


    if !(4096..=1024 * 1024).contains(&config.general.direct_relay_copy_buf_c2s_bytes) {
        return Err(ProxyError::Config(
            "general.direct_relay_copy_buf_c2s_bytes must be within [4096, 1048576]".to_string(),
        ));
    }

    if !(8192..=2 * 1024 * 1024).contains(&config.general.direct_relay_copy_buf_s2c_bytes) {
        return Err(ProxyError::Config(
            "general.direct_relay_copy_buf_s2c_bytes must be within [8192, 2097152]".to_string(),
        ));
    }

    if config.general.direct_relay_buffer_budget_max_bytes != 0 {
        if config.general.direct_relay_buffer_budget_max_bytes
            % DIRECT_RELAY_BUFFER_BUDGET_UNIT_BYTES
            != 0
        {
            return Err(ProxyError::Config(format!(
                "general.direct_relay_buffer_budget_max_bytes must be 0 or a multiple of {DIRECT_RELAY_BUFFER_BUDGET_UNIT_BYTES}"
            )));
        }
        if !(MIN_DIRECT_RELAY_BUFFER_BUDGET_BYTES..=MAX_DIRECT_RELAY_BUFFER_BUDGET_BYTES)
            .contains(&config.general.direct_relay_buffer_budget_max_bytes)
        {
            return Err(ProxyError::Config(format!(
                "general.direct_relay_buffer_budget_max_bytes must be 0 or within [{MIN_DIRECT_RELAY_BUFFER_BUDGET_BYTES}, {MAX_DIRECT_RELAY_BUFFER_BUDGET_BYTES}]"
            )));
        }
    }


    if config.access.user_max_unique_ips_window_secs == 0 {
        return Err(ProxyError::Config(
            "access.user_max_unique_ips_window_secs must be > 0".to_string(),
        ));
    }

    for (user, limit) in &config.access.user_rate_limits {
        if limit.up_bps == 0 && limit.down_bps == 0 {
            return Err(ProxyError::Config(format!(
                "access.user_rate_limits.{user} must set at least one non-zero direction"
            )));
        }
        for (direction, value) in [("up_bps", limit.up_bps), ("down_bps", limit.down_bps)] {
            if value > MAX_RATE_LIMIT_BPS {
                return Err(ProxyError::Config(format!(
                    "access.user_rate_limits.{user}.{direction} must be within [0, {MAX_RATE_LIMIT_BPS}]"
                )));
            }
        }
    }

    for (cidr, limit) in &config.access.cidr_rate_limits {
        if limit.up_bps == 0 && limit.down_bps == 0 {
            return Err(ProxyError::Config(format!(
                "access.cidr_rate_limits.{cidr} must set at least one non-zero direction"
            )));
        }
        for (direction, value) in [("up_bps", limit.up_bps), ("down_bps", limit.down_bps)] {
            if value > MAX_RATE_LIMIT_BPS {
                return Err(ProxyError::Config(format!(
                    "access.cidr_rate_limits.{cidr}.{direction} must be within [0, {MAX_RATE_LIMIT_BPS}]"
                )));
            }
        }
    }
    let mut cidr_auto_templates = HashSet::new();
    for cidr in config.access.cidr_rate_limits.keys() {
        for template in cidr.auto_templates().into_iter().flatten() {
            if !cidr_auto_templates.insert(template) {
                return Err(ProxyError::Config(format!(
                    "access.cidr_rate_limits.{cidr} duplicates normalized auto-template {template}"
                )));
            }
        }
    }

    Ok(())
}
