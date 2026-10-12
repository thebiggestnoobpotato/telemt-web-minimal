use super::*;

#[test]
fn fallback_dns_resolution_defaults_to_never() {
    let vhost: WebVhostConfig = serde_json::from_value(serde_json::json!({
        "host": "proxy.example.com",
        "public_addr": "203.0.113.10:443",
        "upstream": "http://127.0.0.1:18081"
    }))
    .unwrap();
    let serialized = serde_json::to_value(vhost).unwrap();
    assert_eq!(serialized["resolve"], "never");
}

#[test]
fn fallback_dns_timeout_defaults_to_five_seconds() {
    let serialized = serde_json::to_value(WebTimeoutsConfig::default()).unwrap();
    assert_eq!(serialized["fallback_resolve_secs"], 5);
}

#[test]
fn fallback_dns_resolution_is_opt_in() {
    let hostname = WEB_CONFIG.replace("127.0.0.1:18081", "example.com:18081");
    assert!(load_config_error_from_temp_toml(&hostname).contains("IP literal"));
    let never = hostname.replace(
        "upstream = \"http://example.com:18081\"",
        "upstream = \"http://example.com:18081\"\nresolve = \"never\"",
    );
    assert!(load_config_error_from_temp_toml(&never).contains("IP literal"));
    let unknown = WEB_CONFIG.replace(
        "upstream = \"http://127.0.0.1:18081\"",
        "upstream = \"http://127.0.0.1:18081\"\nresolve = \"always\"",
    );
    assert!(load_config_error_from_temp_toml(&unknown).contains("unknown variant"));
    // resolve only applies to http origins; unix sockets never need DNS.
    let unix_with_resolve = WEB_CONFIG.replace(
        "upstream = \"http://127.0.0.1:18081\"",
        "upstream = \"unix:/tmp/site.sock\"\nresolve = \"startup\"",
    );
    assert!(
        load_config_error_from_temp_toml(&unix_with_resolve)
            .contains("web.vhost.resolve is only valid for http origins")
    );
}

#[test]
fn fallback_dns_timeout_bounds_are_independent_from_request_deadlines() {
    for seconds in [0, 3601] {
        let text = format!("{WEB_CONFIG}\n[web.timeouts]\nfallback_resolve_secs = {seconds}\n");
        assert!(load_config_error_from_temp_toml(&text).contains("fallback_resolve_secs"));
    }
    let text = format!("{WEB_CONFIG}\n[web.timeouts]\nfallback_resolve_secs = 3600\n");
    assert_eq!(
        load_config_from_temp_toml(&text)
            .web
            .timeouts
            .fallback_resolve_secs,
        3600
    );
}
