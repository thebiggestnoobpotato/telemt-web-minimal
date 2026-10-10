use super::*;

#[test]
fn serde_defaults_remain_unchanged_for_present_sections() {
    let toml = r#"
        [general]
        [access]
    "#;
    let cfg: ProxyConfig = toml::from_str(toml).unwrap();

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
    assert_eq!(cfg.api.listen, default_api_listen());
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
    assert_eq!(cfg.access.users, default_access_users());
    assert_eq!(
        cfg.access.global_user_max_tcp_conns,
        default_global_user_max_tcp_conns()
    );
    assert_eq!(
        cfg.access.user_max_unique_ips_mode,
        UserMaxUniqueIpsMode::default()
    );
    assert_eq!(
        cfg.access.user_max_unique_ips_window_secs,
        default_user_max_unique_ips_window_secs()
    );
}

#[test]
fn logging_config_is_loaded_from_strict_config() {
    let cfg = load_config_from_temp_toml(
        r#"
            [general]
            config_strict = true

            [logging]
            destination = "file"
            path = "/tmp/telemt.log"
            log_level = "verbose"
            show_users = ["user"]
            unknown_dc_log_enabled = true

            [access.users]
            user = "00000000000000000000000000000000"
        "#,
    );

    assert_eq!(cfg.logging.destination, LoggingDestination::File);
    assert_eq!(cfg.logging.path.as_deref(), Some("/tmp/telemt.log"));
    assert_eq!(cfg.logging.log_level, LogLevel::Verbose);
    assert_eq!(
        cfg.logging.show_users,
        ShowLink::Specific(vec!["user".to_string()])
    );
    assert!(cfg.logging.unknown_dc_log_enabled);
}

#[test]
fn general_links_key_is_stripped_from_general() {
    // strict: the old [general.links] location is rejected after the move
    // to [logging].show_users.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n[general.links]\nshow = \"*\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("general.links"), "{error}");

    // non-strict: the old key is silently ignored and [logging].show_users
    // keeps its default.
    let cfg = load_config_from_temp_toml(
        "[general]\n[general.links]\nshow = \"*\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.logging.show_users, ShowLink::All);
}

#[test]
fn general_telemetry_key_is_stripped_from_general() {
    // strict: the old [general.telemetry] location is rejected after the
    // move to flat [general].telemetry_*_enabled keys.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n[general.telemetry]\ncore_enabled = true\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("general.telemetry"), "{error}");

    // non-strict: the old sub-table is silently ignored and the flat keys
    // keep their defaults.
    let cfg = load_config_from_temp_toml(
        "[general]\n[general.telemetry]\ncore_enabled = false\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.telemetry_core_enabled, default_true());
    assert_eq!(cfg.general.telemetry_user_enabled, default_true());
}

#[test]
fn prefer_ipv6_key_is_stripped_from_general() {
    // strict: the removed prefer_ipv6 alias is rejected; the family
    // preference now lives in network_prefer.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\nprefer_ipv6 = true\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("prefer_ipv6"), "{error}");

    // non-strict: the removed key is silently ignored and no longer
    // affects the family preference.
    let cfg = load_config_from_temp_toml(
        "[general]\nprefer_ipv6 = true\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.network_prefer, default_prefer_4());
}

#[test]
fn network_section_is_stripped_into_general() {
    // strict: the removed [network] section is rejected; its keys now live
    // in [general] as network_ipv4 / network_ipv6 / network_prefer.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n[network]\nipv4 = false\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("network"), "{error}");

    // non-strict: the removed section is silently ignored and [general]
    // keeps its defaults.
    let cfg = load_config_from_temp_toml(
        "[network]\nipv4 = false\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.network_ipv4, default_true());

    // the new [general] location loads explicit values.
    let cfg = load_config_from_temp_toml(
        "[general]\nnetwork_ipv4 = false\nnetwork_ipv6 = true\nnetwork_prefer = 6\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.general.network_ipv4, false);
    assert_eq!(cfg.general.network_ipv6, Some(true));
    assert_eq!(cfg.general.network_prefer, 6);
}

#[test]
fn server_metrics_keys_are_moved_to_metrics() {
    // strict: the stripped [server] section is rejected after the move to the
    // [metrics] section.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n[server]\nmetrics_port = 9090\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("server"), "{error}");

    // non-strict: the old keys are silently ignored and [metrics] keeps
    // its defaults.
    let cfg = load_config_from_temp_toml(
        "[server]\nmetrics_port = 9090\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.metrics.port, None);

    // the new [metrics] section loads explicit values and defaults.
    let cfg = load_config_from_temp_toml(
        "[metrics]\nport = 9090\nlisten = \"127.0.0.1:9090\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert_eq!(cfg.metrics.port, Some(9090));
    assert_eq!(cfg.metrics.listen.as_deref(), Some("127.0.0.1:9090"));
    assert_eq!(cfg.metrics.whitelist, default_metrics_whitelist());
}

#[test]
fn server_port_is_removed_and_listener_endpoints_are_explicit() {
    // strict: the stripped [server] section is rejected after the move to the
    // per-listener [listener] entry.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n[server]\nport = 443\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("server"), "{error}");

    // non-strict: the legacy section is ignored and no listener is set.
    let cfg = load_config_from_temp_toml(
        "[server]\nport = 443\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(cfg.listener.is_none());

    // a listener entry requires both ip and port, or socket_path.
    let error = load_config_error_from_temp_toml(
        "[listener]\ntransport = \"web\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("port"), "{error}");

    // a unix socket listener entry loads with the socket file as its trust
    // boundary; no proxy CIDRs are required.
    let cfg = load_config_from_temp_toml(
        "[listener]\n\
         socket_path = \"/run/telemt/listener.sock\"\ntransport = \"web\"\n\
         [[web.vhosts]]\nhost = \"proxy.example.com\"\n\
         public_addr = \"203.0.113.10:443\"\n\
         [web.vhosts.decoy]\nmode = \"http_upstream\"\n\
         upstream = \"http://127.0.0.1:18090\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    let listener = cfg.listener.as_ref().unwrap();
    assert_eq!(
        listener.socket_path.as_deref(),
        Some("/run/telemt/listener.sock")
    );

    // socket_path cannot be combined with ip or port.
    let error = load_config_error_from_temp_toml(
        "[listener]\n\
         socket_path = \"/run/telemt/listener.sock\"\nip = \"127.0.0.1\"\nport = 443\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("socket_path"), "{error}");

    // socket_path must be an absolute path.
    let error = load_config_error_from_temp_toml(
        "[listener]\nsocket_path = \"run/telemt/listener.sock\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("absolute path"), "{error}");
}

#[test]
fn listener_section_is_known_in_strict_config() {
    // strict: [listener] is a known top-level table.
    let cfg = load_config_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [listener]\nip = \"127.0.0.1\"\nport = 443\ntransport = \"web\"\n\
         web_trusted_proxy_cidrs = [\"127.0.0.1/32\"]\n\
         [[web.vhosts]]\nhost = \"proxy.example.com\"\n\
         public_addr = \"203.0.113.10:443\"\n\
         [web.vhosts.decoy]\nmode = \"http_upstream\"\n\
         upstream = \"http://127.0.0.1:18090\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(cfg.listener.is_some());

    // strict: unknown keys inside [listener] are rejected with the full path.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [listener]\nip = \"127.0.0.1\"\nport = 443\ntransport = \"web\"\n\
         web_trusted_proxy_cidrs = [\"127.0.0.1/32\"]\nbogus = 1\n\
         [[web.vhosts]]\nhost = \"proxy.example.com\"\n\
         public_addr = \"203.0.113.10:443\"\n\
         [web.vhosts.decoy]\nmode = \"http_upstream\"\n\
         upstream = \"http://127.0.0.1:18090\"\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n",
    );
    assert!(error.contains("listener.bogus"), "{error}");
}

#[test]
fn cidr_rate_limits_accept_auto_templates_in_strict_config() {
    let cfg = load_config_from_temp_toml(
        r#"
            [general]
            config_strict = true

            [access.users]
            user = "00000000000000000000000000000000"

            [access.cidr_rate_limits]
            "*/24" = { up_bps = 1024, down_bps = 0 }
            "*4/30" = { up_bps = 0, down_bps = 2048 }
            "*6/64" = { up_bps = 4096, down_bps = 0 }
        "#,
    );

    assert!(
        cfg.access
            .cidr_rate_limits
            .contains_key(&CidrRateLimitKey::AutoDual(24))
    );
    assert!(
        cfg.access
            .cidr_rate_limits
            .contains_key(&CidrRateLimitKey::AutoV4(30))
    );
    assert!(
        cfg.access
            .cidr_rate_limits
            .contains_key(&CidrRateLimitKey::AutoV6(64))
    );
}

#[test]
fn cidr_rate_limits_reject_invalid_auto_template_prefix() {
    let error = load_config_error_from_temp_toml(
        r#"
            [access.users]
            user = "00000000000000000000000000000000"

            [access.cidr_rate_limits]
            "*4/33" = { up_bps = 1024, down_bps = 0 }
        "#,
    );

    assert!(error.contains("prefix must be within 0..=32"));
}

#[test]
fn cidr_rate_limits_reject_duplicate_normalized_auto_templates() {
    let error = load_config_error_from_temp_toml(
        r#"
            [access.users]
            user = "00000000000000000000000000000000"

            [access.cidr_rate_limits]
            "*/32" = { up_bps = 1024, down_bps = 0 }
            "*6/128" = { up_bps = 2048, down_bps = 0 }
        "#,
    );

    assert!(error.contains("duplicates normalized auto-template *6/128"));
}

#[test]
fn rate_limits_accept_the_packed_counter_maximum() {
    let cfg = load_config_from_temp_toml(
        r#"
            [access.users]
            user = "00000000000000000000000000000000"

            [access.user_rate_limits]
            user = { up_bps = 100000000000, down_bps = 0 }

            [access.cidr_rate_limits]
            "203.0.113.0/24" = { up_bps = 0, down_bps = 100000000000 }
        "#,
    );

    assert_eq!(cfg.access.user_rate_limits["user"].up_bps, 100_000_000_000);
    assert_eq!(
        cfg.access.cidr_rate_limits[&CidrRateLimitKey::Network("203.0.113.0/24".parse().unwrap())]
            .down_bps,
        100_000_000_000
    );
}

#[test]
fn user_rate_limits_reject_values_above_the_packed_counter_maximum() {
    let error = load_config_error_from_temp_toml(
        r#"
            [access.users]
            user = "00000000000000000000000000000000"

            [access.user_rate_limits]
            user = { up_bps = 100000000001, down_bps = 0 }
        "#,
    );

    assert!(error.contains("access.user_rate_limits.user.up_bps must be within"));
}

#[test]
fn cidr_rate_limits_reject_values_above_the_packed_counter_maximum() {
    let error = load_config_error_from_temp_toml(
        r#"
            [access.users]
            user = "00000000000000000000000000000000"

            [access.cidr_rate_limits]
            "203.0.113.0/24" = { up_bps = 0, down_bps = 100000000001 }
        "#,
    );

    assert!(error.contains("access.cidr_rate_limits.203.0.113.0/24.down_bps must be within"));
}

#[test]
fn file_logging_requires_path() {
    let error = load_config_error_from_temp_toml(
        r#"
            [logging]
            destination = "file"

            [access.users]
            user = "00000000000000000000000000000000"
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
    assert_eq!(api.listen, default_api_listen());
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

    let access = AccessConfig::default();
    assert_eq!(access.users, default_access_users());
    assert_eq!(
        access.global_user_max_tcp_conns,
        default_global_user_max_tcp_conns()
    );
}

#[test]
fn upstream_type_value_socks_loads_as_socks_upstream() {
    let toml = r#"
        [[upstreams]]
        type = "socks"
        socks_address = "1.2.3.4:1080"
    "#;
    let cfg = load_config_from_temp_toml(toml);
    assert_eq!(cfg.upstreams.len(), 1);
    assert!(matches!(
        cfg.upstreams[0].upstream_type,
        UpstreamType::Socks { .. }
    ));
}

#[test]
fn legacy_upstream_type_value_socks5_is_rejected() {
    let toml = r#"
        [[upstreams]]
        type = "socks5"
        socks_address = "1.2.3.4:1080"
    "#;
    let err = load_config_error_from_temp_toml(toml);
    assert!(err.contains("socks5"), "error should name the rejected value: {err}");
}

#[test]
fn upstream_family_keys_are_stripped_from_upstreams() {
    // strict: the removed per-upstream family keys are rejected; the
    // family policy now lives only in the [general] network_* keys.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [[upstreams]]\ntype = \"direct\"\nipv4 = false\nipv6 = false\nprefer = 4\n",
    );
    assert!(error.contains("upstreams[0].ipv4"), "{error}");
    assert!(error.contains("upstreams[0].ipv6"), "{error}");
    assert!(error.contains("upstreams[0].prefer"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_toml(
        "[[upstreams]]\ntype = \"direct\"\nipv4 = false\nipv6 = false\nprefer = 4\n",
    );
    assert_eq!(cfg.upstreams.len(), 1);
}

#[test]
fn upstream_scopes_key_is_stripped() {
    // strict: the removed scopes key is rejected.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [[upstreams]]\ntype = \"direct\"\nscopes = \"me, fetch\"\n",
    );
    assert!(error.contains("upstreams[0].scopes"), "{error}");

    // non-strict: the removed key is ignored and the config loads.
    let cfg = load_config_from_temp_toml(
        "[[upstreams]]\ntype = \"direct\"\nscopes = \"me, fetch\"\n",
    );
    assert_eq!(cfg.upstreams.len(), 1);
}

#[test]
fn upstream_bind_device_key_replaces_bindtodevice() {
    let cfg = load_config_from_temp_toml(
        "[[upstreams]]\ntype = \"direct\"\nbind_device = \"eth0\"\n",
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
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [[upstreams]]\ntype = \"direct\"\nbindtodevice = \"eth0\"\nforce_bind = \"eth0\"\n",
    );
    assert!(error.contains("upstreams[0].bindtodevice"), "{error}");
    assert!(error.contains("upstreams[0].force_bind"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_toml(
        "[[upstreams]]\ntype = \"direct\"\nbindtodevice = \"eth0\"\nforce_bind = \"eth0\"\n",
    );
    assert_eq!(cfg.upstreams.len(), 1);
}

#[test]
fn upstream_socks_keys_load_into_socks_variant() {
    let cfg = load_config_from_temp_toml(
        "[[upstreams]]\ntype = \"socks\"\nsocks_address = \"127.0.0.1:9050\"\nsocks_username = \"alice\"\nsocks_password = \"secret\"\n",
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
fn access_user_source_deny_key_is_stripped() {
    // strict: the removed key is rejected.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [access.users]\nuser = \"00000000000000000000000000000000\"\n\
         [access.user_source_deny]\nuser = [\"203.0.113.0/24\"]\n",
    );
    assert!(error.contains("access.user_source_deny"), "{error}");

    // non-strict: the removed key is ignored and the config loads.
    let cfg = load_config_from_temp_toml(
        "[access.users]\nuser = \"00000000000000000000000000000000\"\n\
         [access.user_source_deny]\nuser = [\"203.0.113.0/24\"]\n",
    );
    assert_eq!(cfg.access.global_user_max_tcp_conns, 0);
}

#[test]
fn access_global_user_keys_load() {
    let cfg = load_config_from_temp_toml(
        "[access]\nglobal_user_max_tcp_conns = 200\nglobal_user_max_unique_ips = 8\n",
    );
    assert_eq!(cfg.access.global_user_max_tcp_conns, 200);
    assert_eq!(cfg.access.global_user_max_unique_ips, 8);
}

#[test]
fn legacy_access_global_each_keys_are_stripped() {
    // strict: the removed global_each keys are rejected.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [access]\nuser_max_tcp_conns_global_each = 200\nuser_max_unique_ips_global_each = 8\n",
    );
    assert!(error.contains("access.user_max_tcp_conns_global_each"), "{error}");
    assert!(error.contains("access.user_max_unique_ips_global_each"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_toml(
        "[access]\nuser_max_tcp_conns_global_each = 200\nuser_max_unique_ips_global_each = 8\n",
    );
    assert_eq!(cfg.access.global_user_max_tcp_conns, 0);
    assert_eq!(cfg.access.global_user_max_unique_ips, 0);
}

#[test]
fn legacy_upstream_socks_keys_are_stripped() {
    // strict: the removed address/username/password keys are rejected.
    let error = load_config_error_from_temp_toml(
        "[general]\nconfig_strict = true\n\
         [[upstreams]]\ntype = \"socks\"\nsocks_address = \"127.0.0.1:9050\"\naddress = \"127.0.0.1:9050\"\nusername = \"alice\"\npassword = \"secret\"\n",
    );
    assert!(error.contains("upstreams[0].address"), "{error}");
    assert!(error.contains("upstreams[0].username"), "{error}");
    assert!(error.contains("upstreams[0].password"), "{error}");

    // non-strict: the removed keys are ignored and the config loads.
    let cfg = load_config_from_temp_toml(
        "[[upstreams]]\ntype = \"socks\"\nsocks_address = \"127.0.0.1:9050\"\naddress = \"127.0.0.1:9050\"\nusername = \"alice\"\npassword = \"secret\"\n",
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
