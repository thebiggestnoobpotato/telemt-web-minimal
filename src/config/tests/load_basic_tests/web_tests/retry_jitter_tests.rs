use super::*;

#[test]
fn web_retry_jitter_table_parses_with_bounded_defaults_for_omitted_keys() {
    let configured = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  retry_jitter:\n    enabled: true\n    distribution: \"triangular\"\n    peak_ms: 125\n",
    );
    let config = load_config_from_temp_yaml(&configured);
    assert!(config.web.retry_jitter.enabled);
    assert_eq!(config.web.retry_jitter.min_ms, 0);
    assert_eq!(config.web.retry_jitter.max_ms, 250);
    assert_eq!(
        config.web.retry_jitter.distribution,
        WebRetryJitterDistribution::Triangular
    );
    assert_eq!(config.web.retry_jitter.peak_ms, Some(125));

    // Strict loading knows the web.retry_jitter table and its keys.
    let strict = format!("general:\n  config_strict: true\n{configured}");
    let config = load_config_from_temp_yaml(&strict);
    assert!(config.web.retry_jitter.enabled);
    assert_eq!(config.web.retry_jitter.peak_ms, Some(125));
}

#[test]
fn web_retry_jitter_strict_mode_rejects_unknown_keys_and_validation_fails_closed() {
    let unknown = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  retry_jitter:\n    enabled: true\n    spread_ms: 10\n",
    );
    let strict_error = load_config_error_from_temp_yaml(&format!(
        "general:\n  config_strict: true\n{unknown}"
    ));
    assert!(strict_error.contains("web.retry_jitter.spread_ms"), "{strict_error}");

    // deny_unknown_fields keeps the sub-key fail-closed outside strict mode.
    let loose_error = load_config_error_from_temp_yaml(&unknown);
    assert!(loose_error.contains("spread_ms"), "{loose_error}");

    let uniform_peak = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  retry_jitter:\n    enabled: true\n    peak_ms: 100\n",
    );
    let error = load_config_error_from_temp_yaml(&uniform_peak);
    assert!(error.contains("web.retry_jitter.peak_ms"), "{error}");

    let reversed = WEB_CONFIG.replace(
        "web:\n",
        "web:\n  retry_jitter:\n    enabled: true\n    min_ms: 250\n    max_ms: 100\n",
    );
    let error = load_config_error_from_temp_yaml(&reversed);
    assert!(error.contains("web.retry_jitter requires"), "{error}");
}
