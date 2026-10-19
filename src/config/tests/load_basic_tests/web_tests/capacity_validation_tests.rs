use super::*;

#[test]
fn web_http_connection_capacity_policy_is_bounded_and_configurable() {
    let configured = WEB_CONFIG
        .replace(
            "carrier: \"https-lanes\"",
            "carrier: \"https-lanes\"\n  http_connection_capacity_action: \"wait\"",
        )
        .replace(
            "web:\n",
            "limits:\n  max_http_overload_connections: 23\n\ntimeouts:\n  http_overload_timeout_ms: 731\n\nweb:\n",
        );
    let config = load_config_from_temp_yaml(&configured);

    assert_eq!(
        config.web.http_connection_capacity_action,
        WebHttpConnectionCapacityAction::Wait
    );
    assert_eq!(config.limits.max_http_overload_connections, 23);
    assert_eq!(config.timeouts.http_overload_timeout_ms, 731);

    let defaults = ProxyConfig::default();
    assert_eq!(
        defaults.web.http_connection_capacity_action,
        WebHttpConnectionCapacityAction::Drop
    );
    assert_eq!(defaults.limits.max_http_overload_connections, 64);
    assert_eq!(defaults.timeouts.http_overload_timeout_ms, 250);
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
            "web:\n",
            &format!("timeouts:\n  http_overload_timeout_ms: {timeout}\n\nweb:\n"),
        );
        assert!(
            load_config_error_from_temp_yaml(&invalid)
                .contains("timeouts.http_overload_timeout_ms")
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
fn https_lanes_requires_separate_poll_and_control_handler_capacity() {
    let invalid = WEB_CONFIG.replace(
        "web:\n",
        "limits:\n  max_http_handlers: 1\n  max_body_readers: 1\n\nweb:\n",
    );
    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains("WEB https-lanes candidates require"));
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
