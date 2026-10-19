use super::*;

#[test]
fn web_base_path_is_canonical_and_precomputed() {
    let maximum = "a".repeat(128);
    for base_path in ["a", "a/b/c9_x-y", "Dobry-Cola/super_app", maximum.as_str()] {
        let configured = WEB_CONFIG.replace(
            "host: \"Proxy.Example.COM\"",
            &format!("host: \"Proxy.Example.COM\"\n    base_path: \"{base_path}\""),
        );
        let config = load_config_from_temp_yaml(&configured);
        assert_eq!(
            config.web.vhost.as_ref().expect("web vhost").base_path,
            base_path
        );
        assert_eq!(
            config.web.runtime.as_ref().unwrap().vhosts["proxy.example.com"].base,
            format!("/{base_path}/")
        );
    }

    let strict = format!(
        "general:\n  config_strict: true\n{}",
        WEB_CONFIG.replace(
            "host: \"Proxy.Example.COM\"",
            "host: \"Proxy.Example.COM\"\n    base_path: \"relay\"",
        )
    );
    assert_eq!(
        load_config_from_temp_yaml(&strict)
            .web
            .vhost
            .as_ref()
            .expect("web vhost")
            .base_path,
        "relay"
    );
}

#[test]
fn web_base_path_rejects_noncanonical_forms() {
    let oversized = "a".repeat(129);
    for base_path in [
        "/relay",
        "relay/",
        "relay//nested",
        "-relay",
        "_relay",
        "a/-lead",
        "a/_lead",
        "dot.ted",
        "..",
        "/",
        "relay/.hidden",
        "relay/%2fhidden",
        "relay path",
        "relay/тест",
        oversized.as_str(),
    ] {
        let invalid = WEB_CONFIG.replace(
            "host: \"Proxy.Example.COM\"",
            &format!("host: \"Proxy.Example.COM\"\n    base_path: \"{base_path}\""),
        );
        assert!(
            load_config_error_from_temp_yaml(&invalid).contains("web.vhost.base_path"),
            "base path {base_path:?} was accepted"
        );
    }
}
