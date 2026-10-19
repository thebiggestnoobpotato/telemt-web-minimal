use super::*;

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
fn web_ipv6_fallback_uses_a_valid_http_authority() {
    let ipv6 = WEB_CONFIG.replace("http://127.0.0.1:18081", "http://[::1]:18081");
    let config = load_config_from_temp_yaml(&ipv6);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime.vhosts.get("proxy.example.com").unwrap();
    let WebRuntimeFallback::HttpUpstream { authority, .. } = &vhost.fallback;
    assert_eq!(authority, "[::1]:18081");
}
