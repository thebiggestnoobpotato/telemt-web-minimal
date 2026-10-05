use super::*;

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            data_path: None,
            quota_state_path: default_quota_state_path(),
            config_strict: false,
            prefer_ipv6: false,
            fast_mode: default_true(),
            direct_relay_copy_buf_c2s_bytes: default_direct_relay_copy_buf_c2s_bytes(),
            direct_relay_copy_buf_s2c_bytes: default_direct_relay_copy_buf_s2c_bytes(),
            direct_relay_buffer_budget_max_bytes: default_direct_relay_buffer_budget_max_bytes(),
            crypto_pending_buffer: default_crypto_pending_buffer(),
            max_client_frame: default_max_client_frame(),
            upstream_connect_retry_attempts: default_upstream_connect_retry_attempts(),
            upstream_connect_retry_backoff_ms: default_upstream_connect_retry_backoff_ms(),
            upstream_connect_budget_ms: default_upstream_connect_budget_ms(),
            tg_connect: default_connect_timeout(),
            upstream_unhealthy_fail_threshold: default_upstream_unhealthy_fail_threshold(),
            upstream_connect_failfast_hard_errors: default_upstream_connect_failfast_hard_errors(),
            telemetry: TelemetryConfig::default(),
            links: LinksConfig::default(),
            rst_on_close: RstOnCloseMode::default(),
            dc_overrides: HashMap::new(),
            default_dc: None,
        }
    }
}
