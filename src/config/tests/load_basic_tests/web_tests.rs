use super::*;

// Method selection must not alter capability identity or accepted config literals.
#[path = "web_tests/carrier_method_tests.rs"]
mod carrier_method_tests;

#[path = "web_tests/conveyor_tests.rs"]
mod conveyor_tests;
#[path = "web_tests/fallback_dns_tests.rs"]
mod fallback_dns_tests;

#[path = "web_tests/global_profiles_tests.rs"]
mod global_profiles_tests;

#[path = "web_tests/profile_host_tests.rs"]
mod profile_host_tests;

const WEB_CONFIG: &str = r#"
users:
  alice:
    secret: "000102030405060708090a0b0c0d0e0f"
    max_sessions: 4
    max_streams: 64
    max_streams_per_session: 16

listener:
  ip: "127.0.0.1"
  port: 18080
  web_client_ip_source: "x_forwarded_for"
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]

web:
  carrier: "https-lanes"
  secret_mode: "dd"
  vhost:
    host: "Proxy.Example.COM"
    public_addr: "203.0.113.10:443"
    upstream: "http://127.0.0.1:18081"
"#;

#[test]
fn web_config_builds_canonical_runtime_snapshot() {
    let config = load_config_from_temp_yaml(WEB_CONFIG);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.base, "/");
    assert_eq!(vhost.profiles.len(), 1);
    assert_eq!(vhost.capabilities.len(), vhost.profiles.len());
    assert_eq!(vhost.capabilities[0], vhost.profiles[0].capability);
    assert_eq!(runtime.capabilities.as_ref(), vhost.capabilities.as_ref());
    assert_eq!(vhost.fallback_fasttrack_mode, WebFallbackFastTrackMode::Off);
    assert_eq!(vhost.profiles[0].user, "alice");
    assert_eq!(vhost.profiles[0].secret_mode, WebSecretMode::Dd);
    assert_eq!(vhost.profiles[0].carrier, WebCarrier::HttpsLanes);
    assert_eq!(vhost.profiles[0].max_sessions, 4);
    assert_eq!(vhost.profiles[0].max_streams, 64);
    assert_eq!(vhost.profiles[0].max_streams_per_session, 16);
    assert_eq!(vhost.profiles[0].key_fingerprint.len(), 16);
    assert_ne!(vhost.profiles[0].key_fingerprint, "0001020304050607");
    assert!(!vhost.profiles[0].carrier_negotiation_enabled);
    assert_eq!(
        vhost.profiles[0].carriers.as_ref(),
        [WebCarrier::HttpsLanes]
    );
}

#[test]
fn web_base_path_is_canonical_and_precomputed() {
    let maximum = "a".repeat(128);
    for base_path in ["a", "a/b/c9_x-y", "Dobry-Cola/super_app", maximum.as_str()] {
        let configured = WEB_CONFIG.replace(
            "host: \"Proxy.Example.COM\"",
            &format!("host: \"Proxy.Example.COM\"\n    base_path: \"{base_path}\""),
        );
        let config = load_config_from_temp_yaml(&configured);
        assert_eq!(
            config.web.vhost.as_ref().expect("web vhost").base_path,
            base_path
        );
        assert_eq!(
            config.web.runtime.as_ref().unwrap().vhosts["proxy.example.com"].base,
            format!("/{base_path}/")
        );
    }

    let strict = format!(
        "general:\n  config_strict: true\n{}",
        WEB_CONFIG.replace(
            "host: \"Proxy.Example.COM\"",
            "host: \"Proxy.Example.COM\"\n    base_path: \"relay\"",
        )
    );
    assert_eq!(
        load_config_from_temp_yaml(&strict)
            .web
            .vhost
            .as_ref()
            .expect("web vhost")
            .base_path,
        "relay"
    );
}

#[test]
fn web_base_path_rejects_noncanonical_forms() {
    let oversized = "a".repeat(129);
    for base_path in [
        "/relay",
        "relay/",
        "relay//nested",
        "-relay",
        "_relay",
        "a/-lead",
        "a/_lead",
        "dot.ted",
        "..",
        "/",
        "relay/.hidden",
        "relay/%2fhidden",
        "relay path",
        "relay/тест",
        oversized.as_str(),
    ] {
        let invalid = WEB_CONFIG.replace(
            "host: \"Proxy.Example.COM\"",
            &format!("host: \"Proxy.Example.COM\"\n    base_path: \"{base_path}\""),
        );
        assert!(
            load_config_error_from_temp_yaml(&invalid).contains("web.vhost.base_path"),
            "base path {base_path:?} was accepted"
        );
    }
}

#[test]
fn web_fallback_fasttrack_mode_is_typed_and_defaults_off() {
    let defaults = ProxyConfig::default();
    assert_eq!(
        defaults.web.fallback_fasttrack_mode,
        WebFallbackFastTrackMode::Off
    );

    for (token, expected) in [
        ("shadow", WebFallbackFastTrackMode::Shadow),
        ("enforce", WebFallbackFastTrackMode::Enforce),
    ] {
        let configured = WEB_CONFIG.replace(
            "carrier: \"https-lanes\"",
            &format!("carrier: \"https-lanes\"\n  fallback_fasttrack_mode: \"{token}\""),
        );
        let config = load_config_from_temp_yaml(&configured);
        assert_eq!(config.web.fallback_fasttrack_mode, expected);
        assert_eq!(
            config.web.runtime.as_ref().unwrap().vhosts["proxy.example.com"].fallback_fasttrack_mode,
            expected
        );
    }

    let invalid = WEB_CONFIG.replace(
        "carrier: \"https-lanes\"",
        "carrier: \"https-lanes\"\n  fallback_fasttrack_mode: \"automatic\"",
    );
    // serde_yaml names the offending variant, not the field path.
    assert!(load_config_error_from_temp_yaml(&invalid).contains("unknown variant `automatic`"));
}

#[test]
fn web_http_connection_capacity_policy_is_bounded_and_configurable() {
    let configured = WEB_CONFIG
        .replace(
            "carrier: \"https-lanes\"",
            "carrier: \"https-lanes\"\n  http_connection_capacity_action: \"wait\"",
        )
        .replace(
            "web:\n",
            "limits:\n  max_http_overload_connections: 23\n\nweb:\n  timeouts:\n    http_overload_timeout_ms: 731\n",
        );
    let config = load_config_from_temp_yaml(&configured);

    assert_eq!(
        config.web.http_connection_capacity_action,
        WebHttpConnectionCapacityAction::Wait
    );
    assert_eq!(config.limits.max_http_overload_connections, 23);
    assert_eq!(config.web.timeouts.http_overload_timeout_ms, 731);

    let defaults = ProxyConfig::default();
    assert_eq!(
        defaults.web.http_connection_capacity_action,
        WebHttpConnectionCapacityAction::Drop
    );
    assert_eq!(defaults.limits.max_http_overload_connections, 64);
    assert_eq!(defaults.web.timeouts.http_overload_timeout_ms, 250);
}

#[test]
fn web_http_connection_capacity_policy_rejects_unknown_or_unbounded_values() {
    let unknown = WEB_CONFIG.replace(
        "carrier: \"https-lanes\"",
        "carrier: \"https-lanes\"\n  http_connection_capacity_action: \"queue\"",
    );
    // serde_yaml names the offending variant, not the field path.
    assert!(load_config_error_from_temp_yaml(&unknown).contains("unknown variant `queue`"));

    for timeout in [0, 60_001] {
        let invalid = WEB_CONFIG.replace(
            "  vhost:",
            &format!("  timeouts:\n    http_overload_timeout_ms: {timeout}\n\n  vhost:"),
        );
        assert!(
            load_config_error_from_temp_yaml(&invalid)
                .contains("web.timeouts.http_overload_timeout_ms")
        );
    }

    let no_overload_slots = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  max_http_overload_connections: 0\n\nweb:\n",
    );
    assert!(
        load_config_error_from_temp_yaml(&no_overload_slots)
            .contains("limits.max_http_overload_connections")
    );
}

#[test]
fn web_fallback_rejects_direct_and_wildcard_listener_loops() {
    let direct = WEB_CONFIG.replace("http://127.0.0.1:18081", "http://127.0.0.1:18080");
    assert!(
        load_config_error_from_temp_yaml(&direct)
            .contains("web.vhost.upstream overlaps WEB listener")
    );

    let wildcard = direct.replace("ip: \"127.0.0.1\"", "ip: \"0.0.0.0\"");
    assert!(
        load_config_error_from_temp_yaml(&wildcard)
            .contains("web.vhost.upstream overlaps WEB listener")
    );
}

#[test]
fn web_profile_user_labels_are_bounded_for_runtime_status() {
    let user = "a".repeat(65);
    let invalid = WEB_CONFIG.replace("alice", &user);

    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains(&format!("users.{user}")), "{error}");
    assert!(error.contains("must contain 1..64 bytes"), "{error}");
}

#[test]
fn web_carriers_missing_or_false_disable_negotiation() {
    let missing = load_config_from_temp_yaml(WEB_CONFIG);
    assert!(!missing.web.carrier_negotiation_enabled());
    assert!(!missing.web.runtime.unwrap().profiles[0].carrier_learning);

    let disabled = WEB_CONFIG.replace(
        "carrier: \"https-lanes\"",
        "carrier: \"https-lanes\"\n  carriers: false",
    );
    let disabled = load_config_from_temp_yaml(&disabled);
    assert!(!disabled.web.carrier_negotiation_enabled());
    assert!(!disabled.web.runtime.as_ref().unwrap().profiles[0].carrier_learning);
    assert_eq!(
        disabled.web.runtime.unwrap().profiles[0].carriers.as_ref(),
        [WebCarrier::HttpsLanes]
    );
}

#[test]
fn web_carrier_array_enables_ordered_negotiation_and_appends_fallback() {
    let configured = WEB_CONFIG.replace(
        "carrier: \"https-lanes\"",
        "carrier: \"https-lanes\"\n  carriers: [\"websocket\", \"https\"]\n  carrier_learning: false",
    );
    let config = load_config_from_temp_yaml(&configured);
    assert!(config.web.carrier_negotiation_enabled());
    assert!(!config.web.carrier_learning);
    let profile = &config.web.runtime.unwrap().profiles[0];
    assert_eq!(
        profile.carriers.as_ref(),
        [
            WebCarrier::Websocket,
            WebCarrier::Https,
            WebCarrier::HttpsLanes
        ]
    );
    assert!(!profile.carrier_learning);
}

#[test]
fn web_carriers_reject_true_empty_and_duplicates() {
    for value in ["true", "[]", "[\"https\", \"https\"]"] {
        let invalid = WEB_CONFIG.replace(
            "carrier: \"https-lanes\"",
            &format!("carrier: \"https-lanes\"\n  carriers: {value}"),
        );
        assert!(load_config_error_from_temp_yaml(&invalid).contains("web.carriers"));
    }
}

#[test]
fn web_carrier_and_bridge_deadlines_are_configurable() {
    let configured = WEB_CONFIG.replace(
        "  vhost:",
        "  timeouts:\n    carrier_negotiation_deadlines_secs: [1, 2, 4, 9]\n    carrier_learning_secs: 30\n    bridge_request_secs: 7\n    bridge_retry_secs: 41\n    bridge_recovery_secs: 13\n    carrier_probe_coalesce_ms: 4\n\n  vhost:",
    );
    let config = load_config_from_temp_yaml(&configured);
    assert_eq!(
        config.web.timeouts.carrier_negotiation_deadlines_secs,
        [1, 2, 4, 9]
    );
    assert_eq!(config.web.timeouts.carrier_learning_secs, 30);
    assert_eq!(config.web.timeouts.bridge_request_secs, 7);
    assert_eq!(config.web.timeouts.bridge_retry_secs, 41);
    assert_eq!(config.web.timeouts.bridge_recovery_secs, 13);
    assert_eq!(config.web.timeouts.carrier_probe_coalesce_ms, 4);
}

#[test]
fn web_bridge_deadlines_are_known_in_strict_mode() {
    let configured = WEB_CONFIG.replace(
        "  vhost:",
        "  timeouts:\n    bridge_request_secs: 7\n    bridge_retry_secs: 41\n    bridge_recovery_secs: 13\n    carrier_probe_coalesce_ms: 4\n\n  vhost:",
    );
    let configured = format!("general:\n  config_strict: true\n{configured}");
    let config = load_config_from_temp_yaml(&configured);

    assert_eq!(config.web.timeouts.bridge_request_secs, 7);
    assert_eq!(config.web.timeouts.bridge_retry_secs, 41);
    assert_eq!(config.web.timeouts.bridge_recovery_secs, 13);
    assert_eq!(config.web.timeouts.carrier_probe_coalesce_ms, 4);
}

#[test]
fn web_bridge_deadlines_are_bounded_and_ordered() {
    for (field, value) in [
        ("bridge_request_secs", "0"),
        ("bridge_request_secs", "61"),
        ("bridge_retry_secs", "0"),
        ("bridge_retry_secs", "301"),
        ("bridge_recovery_secs", "0"),
        ("bridge_recovery_secs", "61"),
        ("carrier_probe_coalesce_ms", "11"),
    ] {
        let invalid = WEB_CONFIG.replace(
            "  vhost:",
            &format!("  timeouts:\n    {field}: {value}\n\n  vhost:"),
        );
        assert!(
            load_config_error_from_temp_yaml(&invalid).contains(&format!("web.timeouts.{field}"))
        );
    }

    let reversed = WEB_CONFIG.replace(
        "  vhost:",
        "  timeouts:\n    bridge_request_secs: 20\n    bridge_retry_secs: 10\n\n  vhost:",
    );
    assert!(
        load_config_error_from_temp_yaml(&reversed)
            .contains("bridge_request_secs must not exceed bridge_retry_secs")
    );
}

#[test]
fn web_carrier_learning_capacity_must_remain_nonzero() {
    let invalid = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  max_carrier_learning_entries: 0\n\nweb:\n",
    );
    assert!(
        load_config_error_from_temp_yaml(&invalid)
            .contains("limits.max_carrier_learning_entries")
    );
}

#[test]
fn web_debug_table_uses_debug_name_and_bounded_defaults() {
    assert!(!crate::config::WebDebugConfig::default().sideband);
    let mut ineffective = crate::config::WebDebugConfig {
        enabled: true,
        sideband: true,
        ..Default::default()
    };
    ineffective.capture_lifecycle = false;
    assert!(!ineffective.bridge_diagnostics_enabled());

    let configured = WEB_CONFIG.replace(
        "  vhost:",
        "  debug:\n    enabled: true\n    sideband: true\n    body_capture: \"prefix\"\n    body_prefix_bytes: 2048\n    default_window_secs: 180\n    max_window_secs: 900\n\n  vhost:",
    );
    let config = load_config_from_temp_yaml(&configured);
    assert!(config.web.debug.enabled);
    assert!(config.web.debug.sideband);
    assert!(config.web.debug.bridge_diagnostics_enabled());
    assert_eq!(config.web.debug.body_capture, WebDebugBodyCapture::Prefix);
    assert_eq!(config.web.debug.body_prefix_bytes, 2048);
    assert_eq!(config.web.debug.default_window_secs, 180);
    assert_eq!(config.web.debug.max_window_secs, 900);

    let strict = format!("general:\n  config_strict: true\n{configured}");
    assert!(load_config_from_temp_yaml(&strict).web.debug.sideband);

    let old_name = format!(
        "general:\n  config_strict: true\n{}",
        WEB_CONFIG.replace(
            "  vhost:",
            "  trace:\n    enabled: true\n\n  vhost:",
        )
    );
    let error = load_config_error_from_temp_yaml(&old_name);
    assert!(error.contains("web.trace"));

    let old_parameter = format!(
        "general:\n  config_strict: true\n{}",
        WEB_CONFIG.replace(
            "  vhost:",
            "  debug:\n    bridge_diagnostics: true\n\n  vhost:",
        )
    );
    let error = load_config_error_from_temp_yaml(&old_parameter);
    assert!(error.contains("web.debug.bridge_diagnostics"));
}

#[test]
fn web_debug_prefix_and_window_validation_fail_closed() {
    let oversized_prefix = WEB_CONFIG.replace(
        "  vhost:",
        "  debug:\n    enabled: true\n    body_prefix_bytes: 2097153\n\n  vhost:",
    );
    let error = load_config_error_from_temp_yaml(&oversized_prefix);
    assert!(error.contains("web.debug.body_prefix_bytes"));

    let reversed_window = WEB_CONFIG.replace(
        "  vhost:",
        "  debug:\n    enabled: true\n    default_window_secs: 181\n    max_window_secs: 180\n\n  vhost:",
    );
    let error = load_config_error_from_temp_yaml(&reversed_window);
    assert!(error.contains("web.debug windows"));

    let undersized_store = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  debug_bytes_global: 4095\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&undersized_store);
    assert!(error.contains("debug_bytes_global must be at least 4096"));
}

#[test]
fn https_lanes_requires_separate_poll_and_control_handler_capacity() {
    let invalid = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  max_http_handlers: 1\n  max_body_readers: 1\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains("WEB https-lanes candidates require"));
}

#[test]
fn web_listener_requires_an_explicit_trusted_proxy() {
    let invalid = WEB_CONFIG.replace(
        "web_trusted_proxy_cidrs: [\"127.0.0.1/32\"]",
        "web_trusted_proxy_cidrs: []",
    );
    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains("web_trusted_proxy_cidrs must be non-empty"));
}

#[test]
fn web_queue_limits_preserve_control_and_uplink_progress() {
    let invalid = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  control_bytes_per_session: 1\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains("control reserves must cover bounded control frames"));
}

#[test]
fn web_semaphore_limits_are_rejected_before_runtime_construction() {
    let invalid = WEB_CONFIG.replace(
        "web:\n",
        &format!(
            "limits:\n  max_http_connections: {}\n\nweb:\n",
            tokio::sync::Semaphore::MAX_PERMITS + 1,
        ),
    );
    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains("exceeds Tokio semaphore capacity"));
}

#[test]
fn web_ipv6_fallback_uses_a_valid_http_authority() {
    let ipv6 = WEB_CONFIG.replace("http://127.0.0.1:18081", "http://[::1]:18081");
    let config = load_config_from_temp_yaml(&ipv6);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime.vhosts.get("proxy.example.com").unwrap();
    let WebRuntimeFallback::HttpUpstream { authority, .. } = &vhost.fallback;
    assert_eq!(authority, "[::1]:18081");
}

#[test]
fn vhost_mode_key_is_stripped() {
    // The fallback mode tag was removed with the flattened web.vhost
    // upstream: mode is now an unknown key for strict loads.
    let with_mode = WEB_CONFIG.replace(
        "upstream: \"http://127.0.0.1:18081\"",
        "mode: \"static_directory\"\n    upstream: \"http://127.0.0.1:18081\"",
    );
    let strict_error = load_config_error_from_temp_yaml(
        &format!("general:\n  config_strict: true\n{with_mode}"),
    );
    assert!(strict_error.contains("web.vhost.mode"), "{strict_error}");

    // non-strict: the stray key is ignored and the vhost loads untouched.
    let config = load_config_from_temp_yaml(&with_mode);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime.vhosts.get("proxy.example.com").unwrap();
    let WebRuntimeFallback::HttpUpstream { authority, .. } = &vhost.fallback;
    assert_eq!(authority, "127.0.0.1:18081");
}

#[test]
fn vhost_static_keys_are_stripped() {
    let stripped = WEB_CONFIG.replace(
        "upstream: \"http://127.0.0.1:18081\"",
        "directory: \"/var/lib/telemt/site\"\n    index: \"index.html\"\n    upstream: \"http://127.0.0.1:18081\"",
    );
    // strict: the removed static keys are unknown under web.vhost.
    let error =
        load_config_error_from_temp_yaml(&format!("general:\n  config_strict: true\n{stripped}"));
    assert!(error.contains("web.vhost.directory"), "{error}");

    // non-strict: the old keys are ignored and the upstream loads untouched.
    let config = load_config_from_temp_yaml(&stripped);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime.vhosts.get("proxy.example.com").unwrap();
    let WebRuntimeFallback::HttpUpstream { authority, .. } = &vhost.fallback;
    assert_eq!(authority, "127.0.0.1:18081");
}

#[test]
fn nested_web_limits_key_is_stripped() {
    // limits is a top-level directive now: a nested web.limits table is
    // an unknown key for strict loads.
    let nested = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  limits:\n    max_http_handlers: 96\n",
    );
    let strict_error = load_config_error_from_temp_yaml(
        &format!("general:\n  config_strict: true\n{nested}"),
    );
    assert!(strict_error.contains("web.limits"), "{strict_error}");

    // non-strict: the stray table is ignored and the default applies.
    let config = load_config_from_temp_yaml(&nested);
    assert_eq!(config.limits.max_http_handlers, 512);
}

#[test]
fn websocket_carriers_build_runtime_profiles_with_bounded_defaults() {
    for (name, carrier) in [
        ("websocket", WebCarrier::Websocket),
        ("websocket-lanes", WebCarrier::WebsocketLanes),
    ] {
        let configured = WEB_CONFIG.replace("https-lanes", name);
        let config = load_config_from_temp_yaml(&configured);
        let profile = &config.web.runtime.unwrap().profiles[0];
        assert_eq!(profile.carrier, carrier);
        assert_eq!(config.limits.websocket_bytes_global, 256 * 1024 * 1024);
        assert_eq!(config.limits.websocket_admission_watermark_pct, 75);
        assert_eq!(config.limits.websocket_eviction_watermark_pct, 90);
        assert_eq!(config.limits.websocket_http_connection_reserve, 64);
        assert_eq!(config.web.timeouts.websocket_write_secs, 30);
        assert_eq!(config.web.timeouts.websocket_backpressure_secs, 30);
        assert_eq!(config.web.timeouts.websocket_eviction_secs, 1);
    }
}

#[test]
fn websocket_limits_reject_ambiguous_or_nonprogressing_policy() {
    let reversed_watermarks = WEB_CONFIG.replace(
        "web:\n  carrier: \"https-lanes\"",
        "limits:\n  websocket_admission_watermark_pct: 90\n  websocket_eviction_watermark_pct: 75\n\nweb:\n  carrier: \"websocket\"",
    );
    assert!(
        load_config_error_from_temp_yaml(&reversed_watermarks).contains("WebSocket watermarks")
    );

    let no_http_reserve = WEB_CONFIG.replace(
        "web:\n  carrier: \"https-lanes\"",
        "limits:\n  websocket_http_connection_reserve: 0\n\nweb:\n  carrier: \"websocket\"",
    );
    assert!(
        load_config_error_from_temp_yaml(&no_http_reserve)
            .contains("websocket_http_connection_reserve")
    );

    let oversized_batch = WEB_CONFIG.replace(
        "web:\n  carrier: \"https-lanes\"",
        "limits:\n  max_body_bytes: 4194304\n  carrier_batch_bytes: 4194304\n  max_body_readers: 16\n\nweb:\n  carrier: \"websocket\"",
    );
    assert!(
        load_config_error_from_temp_yaml(&oversized_batch)
            .contains("carrier_batch_bytes <= 2097152")
    );
}
