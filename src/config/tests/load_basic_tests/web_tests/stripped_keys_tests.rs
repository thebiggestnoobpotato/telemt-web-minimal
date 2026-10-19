use super::*;

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
