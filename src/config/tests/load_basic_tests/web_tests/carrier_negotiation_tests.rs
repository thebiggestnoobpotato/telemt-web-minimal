use super::*;

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
        "web:\n",
        "timeouts:\n  carrier_negotiation_deadlines_secs: [1, 2, 4, 9]\n  carrier_learning_secs: 30\n  bridge_request_secs: 7\n  bridge_retry_secs: 41\n  bridge_recovery_secs: 13\n  carrier_probe_coalesce_ms: 4\n\nweb:\n",
    );
    let config = load_config_from_temp_yaml(&configured);
    assert_eq!(
        config.timeouts.carrier_negotiation_deadlines_secs,
        [1, 2, 4, 9]
    );
    assert_eq!(config.timeouts.carrier_learning_secs, 30);
    assert_eq!(config.timeouts.bridge_request_secs, 7);
    assert_eq!(config.timeouts.bridge_retry_secs, 41);
    assert_eq!(config.timeouts.bridge_recovery_secs, 13);
    assert_eq!(config.timeouts.carrier_probe_coalesce_ms, 4);
}

#[test]
fn web_bridge_deadlines_are_known_in_strict_mode() {
    let configured = WEB_CONFIG.replace(
        "web:\n",
        "timeouts:\n  bridge_request_secs: 7\n  bridge_retry_secs: 41\n  bridge_recovery_secs: 13\n  carrier_probe_coalesce_ms: 4\n\nweb:\n",
    );
    let configured = format!("general:\n  config_strict: true\n{configured}");
    let config = load_config_from_temp_yaml(&configured);

    assert_eq!(config.timeouts.bridge_request_secs, 7);
    assert_eq!(config.timeouts.bridge_retry_secs, 41);
    assert_eq!(config.timeouts.bridge_recovery_secs, 13);
    assert_eq!(config.timeouts.carrier_probe_coalesce_ms, 4);
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
            "web:\n",
            &format!("timeouts:\n  {field}: {value}\n\nweb:\n"),
        );
        assert!(
            load_config_error_from_temp_yaml(&invalid).contains(&format!("timeouts.{field}"))
        );
    }

    let reversed = WEB_CONFIG.replace(
        "web:\n",
        "timeouts:\n  bridge_request_secs: 20\n  bridge_retry_secs: 10\n\nweb:\n",
    );
    assert!(
        load_config_error_from_temp_yaml(&reversed)
            .contains("bridge_request_secs must not exceed bridge_retry_secs")
    );
}
