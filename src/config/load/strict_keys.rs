use tracing::warn;

use crate::error::{ProxyError, Result};

const TOP_LEVEL_CONFIG_KEYS: &[&str] = &[
    "api",
    "general",
    "logging",
    "listener",
    "metrics",
    "web",
    "timeouts",
    "access",
    "upstreams",
    "include",
];

const GENERAL_CONFIG_KEYS: &[&str] = &[
    "data_path",
    "quota_state_path",
    "config_strict",
    "network_ipv4",
    "network_ipv6",
    "network_prefer",
    "fast_mode",
    "direct_relay_copy_buf_c2s_bytes",
    "direct_relay_copy_buf_s2c_bytes",
    "direct_relay_buffer_budget_max_bytes",
    "crypto_pending_buffer",
    "max_client_frame",
    "upstream_connect_retry_attempts",
    "upstream_connect_retry_backoff_ms",
    "upstream_connect_budget_ms",
    "upstream_connect_timeout",
    "upstream_unhealthy_fail_threshold",
    "upstream_connect_failfast_hard_errors",
    "telemetry_core_enabled",
    "telemetry_user_enabled",
    "dc_overrides",
    "default_dc",
    "listen_backlog",
    "max_connections",
];

const METRICS_CONFIG_KEYS: &[&str] = &["port", "listen", "whitelist"];

const API_CONFIG_KEYS: &[&str] = &[
    "enabled",
    "listen",
    "whitelist",
    "gray_action",
    "auth_header",
    "request_body_limit_bytes",
    "minimal_runtime_enabled",
    "minimal_runtime_cache_ttl_ms",
    "runtime_edge_enabled",
    "runtime_edge_cache_ttl_ms",
    "runtime_edge_top_n",
    "runtime_edge_events_capacity",
    "read_only",
];

const LISTENER_CONFIG_KEYS: &[&str] = &[
    "ip",
    "transport",
    "port",
    "socket_path",
    "socket_perm",
    "web_client_ip_source",
    "web_trusted_proxy_cidrs",
];

const WEB_CONFIG_KEYS: &[&str] = &[
    "enabled",
    "carrier",
    "carrier_method",
    "conveyor",
    "carriers",
    "carrier_learning",
    "carrier_negotiation_aggressiveness",
    "decoy_fasttrack_mode",
    "http_connection_capacity_action",
    "debug",
    "limits",
    "timeouts",
    "vhosts",
];

const WEB_LIMITS_CONFIG_KEYS: &[&str] = &[
    "max_header_bytes",
    "max_body_bytes",
    "max_frame_payload_bytes",
    "carrier_batch_bytes",
    "max_frames_per_body",
    "max_http_connections",
    "max_http_overload_connections",
    "max_http_handlers",
    "max_lane_open_waits_per_session",
    "pending_bytes_per_lane",
    "pending_items_per_lane",
    "websocket_bytes_global",
    "websocket_admission_watermark_pct",
    "websocket_eviction_watermark_pct",
    "websocket_http_connection_reserve",
    "max_websocket_evictions_in_flight",
    "max_carrier_learning_entries",
    "max_body_readers",
    "max_body_bytes_global",
    "max_sessions_global",
    "max_sessions_per_ip",
    "max_streams_per_session",
    "max_streams_global",
    "max_stream_handshakes",
    "max_tombstones_per_session",
    "pending_bytes_per_session",
    "pending_bytes_global",
    "pending_items_per_session",
    "pending_items_global",
    "control_bytes_per_session",
    "control_bytes_global",
    "max_bootstraps_global",
    "max_bootstraps_per_ip",
    "max_vhosts",
    "max_profiles",
    "max_static_files",
    "max_static_file_bytes",
    "max_static_bytes",
    "debug_records_capacity",
    "debug_bytes_global",
    "memory_envelope_bytes",
    "new_bootstraps_per_minute",
    "new_bootstraps_burst",
    "new_sessions_per_minute",
    "new_sessions_burst",
    "new_streams_per_minute",
    "new_streams_burst",
];

const WEB_DEBUG_CONFIG_KEYS: &[&str] = &[
    "enabled",
    "sideband",
    "capture_lifecycle",
    "capture_headers",
    "capture_timings",
    "capture_frames",
    "body_capture",
    "body_prefix_bytes",
    "decoy_body_prefix_bytes",
    "default_window_secs",
    "max_window_secs",
];

const WEB_TIMEOUTS_CONFIG_KEYS: &[&str] = &[
    "header_secs",
    "body_secs",
    "stream_handshake_secs",
    "stream_first_byte_secs",
    "long_poll_secs",
    "bridge_request_secs",
    "bridge_retry_secs",
    "bridge_recovery_secs",
    "carrier_probe_coalesce_ms",
    "lane_open_wait_secs",
    "carrier_health_secs",
    "websocket_upgrade_secs",
    "websocket_open_secs",
    "websocket_write_secs",
    "websocket_backpressure_secs",
    "websocket_eviction_secs",
    "carrier_negotiation_deadlines_secs",
    "carrier_learning_secs",
    "bootstrap_lifetime_secs",
    "reconnect_grace_secs",
    "http_idle_secs",
    "http_overload_timeout_ms",
    "shutdown_secs",
    "decoy_header_secs",
    "decoy_resolve_secs",
];

const WEB_VHOST_CONFIG_KEYS: &[&str] = &["host", "base_path", "public_addr", "decoy", "profiles"];
const WEB_DECOY_CONFIG_KEYS: &[&str] = &["mode", "upstream", "directory", "index", "resolve"];
const WEB_PROFILE_CONFIG_KEYS: &[&str] = &[
    "user",
    "secret_mode",
    "max_sessions",
    "max_streams",
    "max_streams_per_session",
];

const TIMEOUTS_CONFIG_KEYS: &[&str] = &[
    "client_first_byte_idle_secs",
    "client_handshake",
    "client_keepalive",
    "client_ack",
];

const ACCESS_CONFIG_KEYS: &[&str] = &[
    "users",
    "user_enabled",
    "user_max_tcp_conns",
    "user_max_tcp_conns_global_each",
    "user_expirations",
    "user_data_quota",
    "user_rate_limits",
    "cidr_rate_limits",
    "user_max_unique_ips",
    "user_max_unique_ips_global_each",
    "user_max_unique_ips_mode",
    "user_max_unique_ips_window_secs",
    "replay_check_len",
    "replay_window_secs",
    "ignore_time_skew",
];

const RATE_LIMIT_BPS_CONFIG_KEYS: &[&str] = &["up_bps", "down_bps"];

const UPSTREAM_CONFIG_KEYS: &[&str] = &[
    "type",
    "interface",
    "bind_addresses",
    "bind_device",
    "address",
    "user_id",
    "username",
    "password",
    "weight",
    "enabled",
];

const LOGGING_CONFIG_KEYS: &[&str] = &[
    "destination",
    "path",
    "log_level",
    "show_users",
    "unknown_dc_log_enabled",
];

// Recursive table traversal and key suggestion logic.
mod check;

/// Rejects or reports unknown configuration keys according to strict mode.
pub(super) fn handle_unknown_config_keys(parsed_toml: &toml::Value) -> Result<()> {
    let unknown = check::collect_unknown_config_keys(parsed_toml);
    if unknown.is_empty() {
        return Ok(());
    }

    for item in &unknown {
        if let Some(suggestion) = item.suggestion.as_deref() {
            warn!(
                key = %item.path,
                suggestion = %suggestion,
                "Unknown config key ignored; did you mean the suggested key?"
            );
        } else {
            warn!(key = %item.path, "Unknown config key ignored");
        }
    }

    if check::is_strict_config(parsed_toml) {
        let mut paths = Vec::with_capacity(unknown.len());
        for item in unknown {
            if let Some(suggestion) = item.suggestion {
                paths.push(format!("{} (did you mean `{}`?)", item.path, suggestion));
            } else {
                paths.push(item.path);
            }
        }
        return Err(ProxyError::Config(format!(
            "unknown config keys are not allowed when general.config_strict=true: {}",
            paths.join(", ")
        )));
    }

    Ok(())
}
