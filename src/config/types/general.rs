use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub data_path: Option<PathBuf>,
    /// JSON state file for runtime per-user quota consumption.
    #[serde(default = "default_quota_state_path")]
    pub quota_state_path: PathBuf,
    /// Reject unknown TOML config keys during load.
    /// Startup fails fast; hot-reload rejects the new snapshot and keeps the current config.
    #[serde(default)]
    pub config_strict: bool,
    #[serde(default)]
    pub prefer_ipv6: bool,
    /// Fast nonce mode: pre-fills the client enc key/iv into the relay nonce.
    #[serde(default = "default_true")]
    pub fast_mode: bool,
    /// Copy buffer ceiling for client->DC direction in direct relay.
    ///
    /// This is also the upper bound for one amortized upload rate-limit burst:
    /// upload debt is settled before the next relay read instead of blocking
    /// inside the completed read path.
    #[serde(default = "default_direct_relay_copy_buf_c2s_bytes")]
    pub direct_relay_copy_buf_c2s_bytes: usize,
    /// Copy buffer ceiling for DC->client direction in direct relay.
    ///
    /// This bounds one direct download rate-limit grant because writes are
    /// clipped to the currently available shaper budget.
    #[serde(default = "default_direct_relay_copy_buf_s2c_bytes")]
    pub direct_relay_copy_buf_s2c_bytes: usize,
    /// Process-wide hard ceiling for Direct relay copy buffers.
    /// `0` derives the ceiling from host and cgroup memory limits.
    #[serde(default = "default_direct_relay_buffer_budget_max_bytes")]
    pub direct_relay_buffer_budget_max_bytes: usize,
    /// Max pending ciphertext buffer per client writer (bytes).
    /// Controls FakeTLS backpressure vs throughput.
    #[serde(default = "default_crypto_pending_buffer")]
    pub crypto_pending_buffer: usize,
    /// Maximum allowed client MTProto frame size (bytes).
    #[serde(default = "default_max_client_frame")]
    pub max_client_frame: usize,
    /// Connect attempts for the selected upstream before returning error/fallback.
    #[serde(default = "default_upstream_connect_retry_attempts")]
    pub upstream_connect_retry_attempts: u32,
    /// Delay in milliseconds between upstream connect attempts.
    #[serde(default = "default_upstream_connect_retry_backoff_ms")]
    pub upstream_connect_retry_backoff_ms: u64,
    /// Total wall-clock budget in milliseconds for one upstream connect request across retries.
    #[serde(default = "default_upstream_connect_budget_ms")]
    pub upstream_connect_budget_ms: u64,
    /// Per-attempt TCP connect timeout to Telegram DC servers (seconds).
    #[serde(default = "default_upstream_connect_timeout")]
    pub upstream_connect_timeout: u64,
    /// Consecutive failed requests before upstream is marked unhealthy.
    #[serde(default = "default_upstream_unhealthy_fail_threshold")]
    pub upstream_unhealthy_fail_threshold: u32,
    /// Skip additional retries for hard non-transient upstream connect errors.
    #[serde(default = "default_upstream_connect_failfast_hard_errors")]
    pub upstream_connect_failfast_hard_errors: bool,
    /// Enable core hot-path telemetry counters (process, buffer, traffic).
    #[serde(default = "default_true")]
    pub telemetry_core_enabled: bool,
    /// Enable per-user telemetry counters (bounded per-user metrics).
    #[serde(default = "default_true")]
    pub telemetry_user_enabled: bool,
    /// DC address overrides for non-standard DCs (CDN, media, test, etc.)
    /// Keys are DC indices as strings, values are one or more "ip:port" addresses.
    /// Matches the C implementation's `proxy_for <dc_id> <ip>:<port>` config directive.
    /// Example in config.toml:
    ///   [general.dc_overrides]
    ///   "203" = ["149.154.175.100:443", "91.105.192.100:443"]
    #[serde(default, deserialize_with = "deserialize_dc_overrides")]
    pub dc_overrides: HashMap<String, Vec<String>>,
    /// Default DC index (1-5) for unmapped non-standard DCs.
    /// Matches the C implementation's `default <dc_id>` config directive.
    /// If not set, defaults to 2 (matching Telegram's official `default 2;` in proxy-multi.conf).
    #[serde(default)]
    pub default_dc: Option<u8>,
}
