use super::*;

#[test]
fn serde_defaults_remain_unchanged_for_present_sections() {
    let yaml = r#"
general:
"#;
    let cfg: ProxyConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(cfg.logging, LoggingConfig::default());
    assert_eq!(cfg.general.network_ipv6, default_network_ipv6());
    assert_eq!(
        cfg.general.upstream_connect_retry_attempts,
        default_upstream_connect_retry_attempts()
    );
    assert_eq!(
        cfg.general.upstream_connect_retry_backoff_ms,
        default_upstream_connect_retry_backoff_ms()
    );
    assert_eq!(
        cfg.general.upstream_unhealthy_fail_threshold,
        default_upstream_unhealthy_fail_threshold()
    );
    assert_eq!(
        cfg.general.upstream_connect_failfast_hard_errors,
        default_upstream_connect_failfast_hard_errors()
    );
    assert_eq!(cfg.general.telemetry_core_enabled, default_true());
    assert_eq!(cfg.general.telemetry_user_enabled, default_true());
    assert_eq!(cfg.api.listen_address, default_api_listen_address());
    assert_eq!(cfg.api.whitelist, default_api_whitelist());
    assert_eq!(cfg.api.gray_action, ApiGrayAction::Drop);
    assert_eq!(
        cfg.api.request_body_limit_bytes,
        default_api_request_body_limit_bytes()
    );
    assert_eq!(
        cfg.api.minimal_runtime_enabled,
        default_api_minimal_runtime_enabled()
    );
    assert_eq!(
        cfg.api.minimal_runtime_cache_ttl_ms,
        default_api_minimal_runtime_cache_ttl_ms()
    );
    assert_eq!(
        cfg.api.runtime_edge_enabled,
        default_api_runtime_edge_enabled()
    );
    assert_eq!(
        cfg.api.runtime_edge_cache_ttl_ms,
        default_api_runtime_edge_cache_ttl_ms()
    );
    assert_eq!(
        cfg.api.runtime_edge_top_n,
        default_api_runtime_edge_top_n()
    );
    assert_eq!(
        cfg.api.runtime_edge_events_capacity,
        default_api_runtime_edge_events_capacity()
    );
    assert_eq!(cfg.users, default_access_users());
}

#[test]
fn logging_config_is_loaded_from_strict_config() {
    let cfg = load_config_from_temp_yaml(
        r#"
general:
  config_strict: true

logging:
  destination: "file"
  path: "/tmp/telemt.log"
  log_level: "verbose"
  show_users_links: true
  unknown_dc_log_enabled: true

users:
  user:
    secret: "00000000000000000000000000000000"
"#,
    );

    assert_eq!(cfg.logging.destination, LoggingDestination::File);
    assert_eq!(cfg.logging.path.as_deref(), Some("/tmp/telemt.log"));
    assert_eq!(cfg.logging.log_level, LogLevel::Verbose);
    assert!(cfg.logging.show_users_links);
    assert!(cfg.logging.unknown_dc_log_enabled);
}

#[test]
fn general_links_key_is_stripped_from_general() {
    // strict: the old general.links location is rejected after the move
    // to logging.show_users_links.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n  links:\n    show: \"*\"\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("general.links"), "{error}");

    // non-strict: the old key is silently ignored and
    // logging.show_users_links keeps its default.
    let cfg = load_config_from_temp_yaml(
        "general:\n  links:\n    show: \"*\"\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(cfg.logging.show_users_links);
}

#[test]
fn logging_show_users_key_is_stripped() {
    // strict: the renamed logging.show_users key is rejected after the
    // move to the boolean logging.show_users_links.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n\
         logging:\n  show_users: \"*\"\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("logging.show_users"), "{error}");

    // non-strict: the old key is ignored and logging.show_users_links
    // keeps its default.
    let cfg = load_config_from_temp_yaml(
        "general:\n\
         logging:\n  show_users: \"*\"\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(cfg.logging.show_users_links);
}

#[test]
fn general_telemetry_key_is_stripped_from_general() {
    // strict: the old general.telemetry location is rejected after the
    // move to flat general.telemetry_*_enabled keys.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n  telemetry:\n    core_enabled: true\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("general.telemetry"), "{error}");

    // non-strict: the old sub-table is silently ignored and the flat keys
    // keep their defaults.
    let cfg = load_config_from_temp_yaml(
        "general:\n  telemetry:\n    core_enabled: false\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.telemetry_core_enabled, default_true());
    assert_eq!(cfg.general.telemetry_user_enabled, default_true());
}

#[test]
fn prefer_ipv6_key_is_stripped_from_general() {
    // strict: the removed prefer_ipv6 alias is rejected; the family
    // preference now lives in network_prefer.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n  prefer_ipv6: true\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("prefer_ipv6"), "{error}");

    // non-strict: the removed key is silently ignored and no longer
    // affects the family preference.
    let cfg = load_config_from_temp_yaml(
        "general:\n  prefer_ipv6: true\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.network_prefer, default_prefer_4());
}

#[test]
fn legacy_general_direct_relay_keys_are_stripped() {
    // strict: the renamed legacy keys are rejected.
    let error = load_config_error_from_temp_yaml(
        r#"
general:
  config_strict: true
  direct_relay_copy_buf_c2s_bytes: 65536
  direct_relay_copy_buf_s2c_bytes: 262144
  direct_relay_buffer_budget_max_bytes: 0
  default_dc: 2
"#,
    );
    assert!(error.contains("general.direct_relay_copy_buf_c2s_bytes"), "{error}");
    assert!(error.contains("general.direct_relay_copy_buf_s2c_bytes"), "{error}");
    assert!(error.contains("general.direct_relay_buffer_budget_max_bytes"), "{error}");
    assert!(error.contains("general.default_dc"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_yaml(
        r#"
general:
  direct_relay_copy_buf_c2s_bytes: 65536
  direct_relay_copy_buf_s2c_bytes: 262144
  direct_relay_buffer_budget_max_bytes: 0
  default_dc: 2
"#,
    );
    assert_eq!(cfg.general.dc_copy_buf_c2s_bytes, default_dc_copy_buf_c2s_bytes());
    assert_eq!(cfg.general.dc_default, None);
}

#[test]
fn network_section_is_stripped_into_general() {
    // strict: the removed network section is rejected; its keys now live
    // in general as network_ipv4 / network_ipv6 / network_prefer.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\nnetwork:\n  ipv4: false\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("network"), "{error}");

    // non-strict: the removed section is silently ignored and general
    // keeps its defaults.
    let cfg = load_config_from_temp_yaml(
        "network:\n  ipv4: false\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.network_ipv4, default_true());

    // the new general location loads explicit values.
    let cfg = load_config_from_temp_yaml(
        "general:\n  network_ipv4: false\n  network_ipv6: true\n  network_prefer: 6\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.network_ipv4, false);
    assert_eq!(cfg.general.network_ipv6, Some(true));
    assert_eq!(cfg.general.network_prefer, 6);
}

#[test]
fn server_metrics_keys_are_moved_to_metrics() {
    // strict: the stripped server section is rejected after the move to the
    // metrics section.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\nserver:\n  metrics_port: 9090\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("server"), "{error}");

    // non-strict: the old keys are silently ignored and metrics keeps
    // its defaults.
    let cfg = load_config_from_temp_yaml(
        "server:\n  metrics_port: 9090\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(!cfg.metrics.enabled);
    assert_eq!(cfg.metrics.listen_address, None);

    // strict: the stripped port key is rejected after the move to
    // enabled/listen_address.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\nmetrics:\n  port: 9090\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("port"), "{error}");

    // the new metrics section loads explicit values and defaults.
    let cfg = load_config_from_temp_yaml(
        "metrics:\n  enabled: true\n  listen_address: \"127.0.0.1:9090\"\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(cfg.metrics.enabled);
    assert_eq!(cfg.metrics.listen_address.as_deref(), Some("127.0.0.1:9090"));
    assert_eq!(cfg.metrics.whitelist, default_metrics_whitelist());
}

#[test]
fn server_port_is_removed_and_listener_endpoints_are_explicit() {
    // strict: the stripped server section is rejected after the move to the
    // per-listener listener entry.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\nserver:\n  port: 443\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("server"), "{error}");

    // non-strict: the legacy section is ignored and no listener is set.
    let cfg = load_config_from_temp_yaml(
        "server:\n  port: 443\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(cfg.listener.is_none());

    // a listener entry requires both ip and port, or socket_path.
    let error = load_config_error_from_temp_yaml(
        "listener: {}\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("port"), "{error}");

    // a unix socket listener entry loads with the socket file as its trust
    // boundary; no proxy CIDRs are required.
    let cfg = load_config_from_temp_yaml(
        r#"
listener:
  socket_path: "/run/telemt/listener.sock"
web:
  secret_mode: "plain"
  host: "proxy.example.com"
  public_addr: "203.0.113.10:443"
  upstream: "http://127.0.0.1:18090"
users:
  user:
    secret: "00000000000000000000000000000000"
"#,
    );
    let listener = cfg.listener.as_ref().unwrap();
    assert_eq!(
        listener.socket_path.as_deref(),
        Some("/run/telemt/listener.sock")
    );

    // socket_path cannot be combined with ip or port.
    let error = load_config_error_from_temp_yaml(
        "listener:\n  socket_path: \"/run/telemt/listener.sock\"\n  ip: \"127.0.0.1\"\n  port: 443\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("socket_path"), "{error}");

    // socket_path must be an absolute path.
    let error = load_config_error_from_temp_yaml(
        "listener:\n  socket_path: \"run/telemt/listener.sock\"\n\
         users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("absolute path"), "{error}");
}

#[test]
fn listener_section_is_known_in_strict_config() {
    // strict: listener is a known top-level section.
    let cfg = load_config_from_temp_yaml(
        r#"
general:
  config_strict: true
listener:
  ip: "127.0.0.1"
  port: 443
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]
web:
  secret_mode: "plain"
  host: "proxy.example.com"
  public_addr: "203.0.113.10:443"
  upstream: "http://127.0.0.1:18090"
users:
  user:
    secret: "00000000000000000000000000000000"
"#,
    );
    assert!(cfg.listener.is_some());

    // strict: unknown keys inside listener are rejected with the full path.
    let error = load_config_error_from_temp_yaml(
        r#"
general:
  config_strict: true
listener:
  ip: "127.0.0.1"
  port: 443
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]
  bogus: 1
web:
  secret_mode: "plain"
  host: "proxy.example.com"
  public_addr: "203.0.113.10:443"
  upstream: "http://127.0.0.1:18090"
users:
  user:
    secret: "00000000000000000000000000000000"
"#,
    );
    assert!(error.contains("listener.bogus"), "{error}");
}

#[test]
fn file_logging_requires_path() {
    let error = load_config_error_from_temp_yaml(
        r#"
logging:
  destination: "file"

users:
  user:
    secret: "00000000000000000000000000000000"
"#,
    );

    assert!(error.contains("logging.path must be set"));
}

#[test]
fn impl_defaults_are_sourced_from_default_helpers() {
    let general = GeneralConfig::default();
    assert_eq!(general.network_ipv6, default_network_ipv6());
    assert_eq!(general.network_prefer, default_prefer_4());
    assert_eq!(
        general.upstream_connect_retry_attempts,
        default_upstream_connect_retry_attempts()
    );
    assert_eq!(
        general.upstream_connect_retry_backoff_ms,
        default_upstream_connect_retry_backoff_ms()
    );
    assert_eq!(
        general.upstream_unhealthy_fail_threshold,
        default_upstream_unhealthy_fail_threshold()
    );
    assert_eq!(
        general.upstream_connect_failfast_hard_errors,
        default_upstream_connect_failfast_hard_errors()
    );

    let api = ApiConfig::default();
    assert_eq!(api.listen_address, default_api_listen_address());
    assert_eq!(api.whitelist, default_api_whitelist());
    assert_eq!(api.gray_action, ApiGrayAction::Drop);
    assert_eq!(
        api.request_body_limit_bytes,
        default_api_request_body_limit_bytes()
    );
    assert_eq!(
        api.minimal_runtime_enabled,
        default_api_minimal_runtime_enabled()
    );
    assert_eq!(
        api.minimal_runtime_cache_ttl_ms,
        default_api_minimal_runtime_cache_ttl_ms()
    );
    assert_eq!(
        api.runtime_edge_enabled,
        default_api_runtime_edge_enabled()
    );
    assert_eq!(
        api.runtime_edge_cache_ttl_ms,
        default_api_runtime_edge_cache_ttl_ms()
    );
    assert_eq!(
        api.runtime_edge_top_n,
        default_api_runtime_edge_top_n()
    );
    assert_eq!(
        api.runtime_edge_events_capacity,
        default_api_runtime_edge_events_capacity()
    );

}

#[test]
fn upstream_type_value_socks_loads_as_socks_upstream() {
    let yaml = r#"
upstreams:
  - type: "socks"
    socks_address: "1.2.3.4:1080"
"#;
    let cfg = load_config_from_temp_yaml(yaml);
    assert_eq!(cfg.upstreams.len(), 1);
    assert!(matches!(
        cfg.upstreams[0].upstream_type,
        UpstreamType::Socks { .. }
    ));
}

#[test]
fn legacy_upstream_type_value_socks5_is_rejected() {
    let yaml = r#"
upstreams:
  - type: "socks5"
    socks_address: "1.2.3.4:1080"
"#;
    let err = load_config_error_from_temp_yaml(yaml);
    assert!(err.contains("socks5"), "error should name the rejected value: {err}");
}

#[test]
fn upstream_family_keys_are_stripped_from_upstreams() {
    // strict: the removed per-upstream family keys are rejected; the
    // family policy now lives only in the general network_* keys.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n\
         upstreams:\n  - type: \"direct\"\n    ipv4: false\n    ipv6: false\n    prefer: 4\n",
    );
    assert!(error.contains("upstreams[0].ipv4"), "{error}");
    assert!(error.contains("upstreams[0].ipv6"), "{error}");
    assert!(error.contains("upstreams[0].prefer"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_yaml(
        "upstreams:\n  - type: \"direct\"\n    ipv4: false\n    ipv6: false\n    prefer: 4\n",
    );
    assert_eq!(cfg.upstreams.len(), 1);
}

#[test]
fn upstream_scopes_key_is_stripped() {
    // strict: the removed scopes key is rejected.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n\
         upstreams:\n  - type: \"direct\"\n    scopes: \"me, fetch\"\n",
    );
    assert!(error.contains("upstreams[0].scopes"), "{error}");

    // non-strict: the removed key is ignored and the config loads.
    let cfg = load_config_from_temp_yaml(
        "upstreams:\n  - type: \"direct\"\n    scopes: \"me, fetch\"\n",
    );
    assert_eq!(cfg.upstreams.len(), 1);
}

#[test]
fn upstream_bind_device_key_replaces_bindtodevice() {
    let cfg = load_config_from_temp_yaml(
        "upstreams:\n  - type: \"direct\"\n    bind_device: \"eth0\"\n",
    );
    assert!(
        matches!(
            cfg.upstreams[0].upstream_type,
            UpstreamType::Direct {
                bind_device: Some(_),
                ..
            }
        ),
        "bind_device must load into the Direct variant"
    );
}

#[test]
fn legacy_upstream_bind_keys_are_stripped() {
    // strict: the removed bindtodevice and force_bind keys are rejected.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n\
         upstreams:\n  - type: \"direct\"\n    bindtodevice: \"eth0\"\n    force_bind: \"eth0\"\n",
    );
    assert!(error.contains("upstreams[0].bindtodevice"), "{error}");
    assert!(error.contains("upstreams[0].force_bind"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_yaml(
        "upstreams:\n  - type: \"direct\"\n    bindtodevice: \"eth0\"\n    force_bind: \"eth0\"\n",
    );
    assert_eq!(cfg.upstreams.len(), 1);
}

#[test]
fn upstream_socks_keys_load_into_socks_variant() {
    let cfg = load_config_from_temp_yaml(
        "upstreams:\n  - type: \"socks\"\n    socks_address: \"127.0.0.1:9050\"\n    socks_username: \"alice\"\n    socks_password: \"secret\"\n",
    );
    let UpstreamType::Socks {
        socks_address,
        socks_username,
        socks_password,
        ..
    } = &cfg.upstreams[0].upstream_type
    else {
        panic!("expected a socks upstream");
    };
    assert_eq!(socks_address, "127.0.0.1:9050");
    assert_eq!(socks_username.as_deref(), Some("alice"));
    assert_eq!(socks_password.as_deref(), Some("secret"));
}

#[test]
fn legacy_timeouts_keys_are_stripped() {
    // The top-level timeouts table is the WEB lifecycle deadline table now:
    // the removed telegram timeout keys are unknown keys for strict loads.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n\
         timeouts:\n  client_handshake: 30\n",
    );
    assert!(error.contains("timeouts"), "{error}");

    // non-strict: the legacy keys are ignored and the defaults apply.
    let cfg = load_config_from_temp_yaml(
        "users:\n  user:\n    secret: \"00000000000000000000000000000000\"\n\
         timeouts:\n  client_handshake: 30\n  client_ack: 90\n",
    );
    assert_eq!(cfg.users.len(), 1);
    assert_eq!(cfg.timeouts.fallback_resolve_secs, 5);
}

#[test]
fn legacy_upstream_socks_keys_are_stripped() {
    // strict: the removed address/username/password keys are rejected.
    let error = load_config_error_from_temp_yaml(
        "general:\n  config_strict: true\n\
         upstreams:\n  - type: \"socks\"\n    socks_address: \"127.0.0.1:9050\"\n    address: \"127.0.0.1:9050\"\n    username: \"alice\"\n    password: \"secret\"\n",
    );
    assert!(error.contains("upstreams[0].address"), "{error}");
    assert!(error.contains("upstreams[0].username"), "{error}");
    assert!(error.contains("upstreams[0].password"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_yaml(
        "upstreams:\n  - type: \"socks\"\n    socks_address: \"127.0.0.1:9050\"\n    address: \"127.0.0.1:9050\"\n    username: \"alice\"\n    password: \"secret\"\n",
    );
    assert!(matches!(
        cfg.upstreams[0].upstream_type,
        UpstreamType::Socks {
            socks_username: None,
            socks_password: None,
            ..
        }
    ));
}
