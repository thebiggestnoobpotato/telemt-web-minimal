use super::*;

#[test]
fn vhost_mode_key_is_stripped() {
    // The fallback mode tag was removed with the flattened vhost
    // parameters: mode is now an unknown key under web for strict loads.
    let with_mode = WEB_CONFIG.replace(
        "upstream: \"http://127.0.0.1:18081\"",
        "mode: \"static_directory\"\n  upstream: \"http://127.0.0.1:18081\"",
    );
    let strict_error = load_config_error_from_temp_yaml(
        &format!("general:\n  config_strict: true\n{with_mode}"),
    );
    assert!(strict_error.contains("web.mode"), "{strict_error}");

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
        "directory: \"/var/lib/telemt/site\"\n  index: \"index.html\"\n  upstream: \"http://127.0.0.1:18081\"",
    );
    // strict: the removed static keys are unknown under web.
    let error =
        load_config_error_from_temp_yaml(&format!("general:\n  config_strict: true\n{stripped}"));
    assert!(error.contains("web.directory"), "{error}");

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
fn nested_web_timeouts_key_is_stripped() {
    // timeouts is a top-level directive now: a nested web.timeouts table is
    // an unknown key for strict loads.
    let nested = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  timeouts:\n    http_overload_timeout_ms: 1\n",
    );
    let strict_error = load_config_error_from_temp_yaml(
        &format!("general:\n  config_strict: true\n{nested}"),
    );
    assert!(strict_error.contains("web.timeouts"), "{strict_error}");

    // non-strict: the stray table is ignored and the default applies.
    let config = load_config_from_temp_yaml(&nested);
    assert_eq!(config.timeouts.http_overload_timeout_ms, 250);
}

#[test]
fn nested_web_debug_key_is_stripped() {
    // web.debug is a logging directive now (the logging.web_debug_enable
    // switch plus the logging.web_debug table): a nested web.debug table is
    // an unknown key for strict loads.
    let nested = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  debug:\n    enabled: true\n",
    );
    let strict_error = load_config_error_from_temp_yaml(
        &format!("general:\n  config_strict: true\n{nested}"),
    );
    assert!(strict_error.contains("web.debug"), "{strict_error}");

    // non-strict: the stray table is ignored and the recorder stays off.
    let config = load_config_from_temp_yaml(&nested);
    assert!(!config.logging.web_debug_enable);
}

#[test]
fn nested_web_vhost_key_is_stripped() {
    // vhost is a flattened parameter group under web now: a nested
    // web.vhost table is an unknown key for strict loads.
    let nested = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  vhost:\n    host: \"legacy.example.com\"\n",
    );
    let strict_error = load_config_error_from_temp_yaml(
        &format!("general:\n  config_strict: true\n{nested}"),
    );
    assert!(strict_error.contains("web.vhost"), "{strict_error}");

    // non-strict: the stray table is ignored and the direct keys win.
    let config = load_config_from_temp_yaml(&nested);
    assert_eq!(config.web.vhost.host, "proxy.example.com");
}
