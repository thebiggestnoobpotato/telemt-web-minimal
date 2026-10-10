use super::*;

/// Fields that are safe to swap without restarting listeners.
#[derive(Debug, Clone, PartialEq)]
pub struct HotFields {
    pub log_level: LogLevel,
    pub telemetry_core_enabled: bool,
    pub telemetry_user_enabled: bool,
    pub direct_relay_copy_buf_c2s_bytes: usize,
    pub direct_relay_copy_buf_s2c_bytes: usize,
    pub users: std::collections::HashMap<String, String>,
    pub user_enabled: std::collections::HashMap<String, bool>,
    pub user_max_unique_ips: std::collections::HashMap<String, usize>,
    pub global_user_max_unique_ips: usize,
    pub user_max_unique_ips_mode: crate::config::UserMaxUniqueIpsMode,
    pub user_max_unique_ips_window_secs: u64,
    pub web_debug: WebDebugConfig,
}

impl HotFields {
    pub fn from_config(cfg: &ProxyConfig) -> Self {
        Self {
            log_level: cfg.logging.log_level.clone(),
            telemetry_core_enabled: cfg.general.telemetry_core_enabled,
            telemetry_user_enabled: cfg.general.telemetry_user_enabled,
            direct_relay_copy_buf_c2s_bytes: cfg.general.direct_relay_copy_buf_c2s_bytes,
            direct_relay_copy_buf_s2c_bytes: cfg.general.direct_relay_copy_buf_s2c_bytes,
            users: cfg.access.users.clone(),
            user_enabled: cfg.access.user_enabled.clone(),
            user_max_unique_ips: cfg.access.user_max_unique_ips.clone(),
            global_user_max_unique_ips: cfg.access.global_user_max_unique_ips,
            user_max_unique_ips_mode: cfg.access.user_max_unique_ips_mode,
            user_max_unique_ips_window_secs: cfg.access.user_max_unique_ips_window_secs,
            web_debug: cfg.web.debug.clone(),
        }
    }
}

pub(super) fn overlay_hot_fields(old: &ProxyConfig, new: &ProxyConfig) -> ProxyConfig {
    let mut cfg = old.clone();

    cfg.logging.log_level = new.logging.log_level.clone();
    cfg.general.telemetry_core_enabled = new.general.telemetry_core_enabled;
    cfg.general.telemetry_user_enabled = new.general.telemetry_user_enabled;
    cfg.general.direct_relay_copy_buf_c2s_bytes = new.general.direct_relay_copy_buf_c2s_bytes;
    cfg.general.direct_relay_copy_buf_s2c_bytes = new.general.direct_relay_copy_buf_s2c_bytes;

    cfg.access.users = new.access.users.clone();
    cfg.access.user_enabled = new.access.user_enabled.clone();
    cfg.access.user_max_unique_ips = new.access.user_max_unique_ips.clone();
    cfg.access.global_user_max_unique_ips = new.access.global_user_max_unique_ips;
    cfg.access.user_max_unique_ips_mode = new.access.user_max_unique_ips_mode;
    cfg.access.user_max_unique_ips_window_secs = new.access.user_max_unique_ips_window_secs;
    let process_limits = cfg.web.limits.clone();
    let decoy_fasttrack_mode = cfg.web.decoy_fasttrack_mode;
    cfg.web = new.web.clone();
    cfg.web.limits = process_limits;
    cfg.web.decoy_fasttrack_mode = decoy_fasttrack_mode;
    if cfg.web.carrier_negotiation_enabled()
        && cfg.web.carrier_learning
        && cfg.web.limits.max_carrier_learning_entries < WEB_CARRIER_LEARNING_MIN_ENTRIES
    {
        if old.web.carrier_learning != new.web.carrier_learning {
            cfg.web.carrier_learning = old.web.carrier_learning;
        } else {
            cfg.web.carriers = old.web.carriers.clone();
        }
    }
    if !web_debug_fits_limits(&cfg.web.debug, &cfg.web.limits) {
        cfg.web.debug = old.web.debug.clone();
    }
    if cfg.rebuild_runtime_user_auth().is_err() {
        cfg.runtime_user_auth = None;
    }

    cfg
}
