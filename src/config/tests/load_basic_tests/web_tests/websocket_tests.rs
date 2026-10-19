use super::*;

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
