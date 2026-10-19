use super::*;

#[test]
fn logging_web_debug_table_uses_web_debug_name_and_bounded_defaults() {
    assert!(!crate::config::WebDebugConfig::default().sideband);
    let mut ineffective = crate::config::WebDebugConfig {
        sideband: true,
        ..Default::default()
    };
    ineffective.capture_lifecycle = false;
    assert!(!ineffective.bridge_diagnostics_enabled(true));

    let configured = WEB_CONFIG.replace(
        "web:\n",
        "logging:\n  web_debug_enable: true\n  web_debug:\n    sideband: true\n    body_capture: \"prefix\"\n    body_prefix_bytes: 2048\n    default_window_secs: 180\n    max_window_secs: 900\n\nweb:\n",
    );
    let config = load_config_from_temp_yaml(&configured);
    assert!(config.logging.web_debug_enable);
    assert!(config.logging.web_debug.sideband);
    assert!(config
        .logging
        .web_debug
        .bridge_diagnostics_enabled(config.logging.web_debug_enable));
    assert_eq!(
        config.logging.web_debug.body_capture,
        WebDebugBodyCapture::Prefix
    );
    assert_eq!(config.logging.web_debug.body_prefix_bytes, 2048);
    assert_eq!(config.logging.web_debug.default_window_secs, 180);
    assert_eq!(config.logging.web_debug.max_window_secs, 900);

    let strict = format!("general:\n  config_strict: true\n{configured}");
    assert!(load_config_from_temp_yaml(&strict).logging.web_debug.sideband);

    let old_name = format!(
        "general:\n  config_strict: true\n{}",
        WEB_CONFIG.replace(
            "web:\n",
            "web:\n  trace:\n    enabled: true\n",
        )
    );
    let error = load_config_error_from_temp_yaml(&old_name);
    assert!(error.contains("web.trace"));

    let old_parameter = format!(
        "general:\n  config_strict: true\n{}",
        WEB_CONFIG.replace(
            "web:\n",
            "web:\n  debug:\n    bridge_diagnostics: true\n",
        )
    );
    let error = load_config_error_from_temp_yaml(&old_parameter);
    assert!(error.contains("web.debug"));
}

#[test]
fn logging_web_debug_prefix_and_window_validation_fail_closed() {
    let oversized_prefix = WEB_CONFIG.replace(
        "web:\n",
        "logging:\n  web_debug:\n    body_prefix_bytes: 2097153\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&oversized_prefix);
    assert!(error.contains("logging.web_debug.body_prefix_bytes"));

    let reversed_window = WEB_CONFIG.replace(
        "web:\n",
        "logging:\n  web_debug:\n    default_window_secs: 181\n    max_window_secs: 180\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&reversed_window);
    assert!(error.contains("logging.web_debug windows"));

    let undersized_store = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  debug_bytes_global: 4095\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&undersized_store);
    assert!(error.contains("debug_bytes_global must be at least 4096"));
}
