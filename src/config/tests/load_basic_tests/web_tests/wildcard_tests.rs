use super::*;

#[test]
fn wildcard_opt_in_accepts_shared_pattern_without_exact_expansion() {
    let source = WEB_CONFIG
        .replace("web:\n", "web:\n  allow_wildcard_hosts: true\n")
        .replace("host: \"Proxy.Example.COM\"", "host: \"*.Example.COM\"")
        + "\nlimits:\n  memory_envelope_bytes: 1610612736\n";
    let config = load_config_from_temp_yaml(&source);
    assert_eq!(config.web.vhost.host.as_str(), "*.example.com");
    let runtime = config.web.runtime.as_ref().unwrap();
    assert!(runtime.vhosts.is_empty());
    assert!(runtime.profiles.is_empty());
    assert!(runtime.capabilities.is_empty());
}

#[test]
fn wildcard_group_never_emits_startup_links() {
    let source = wildcard_source("host: \"*.Example.COM\"");
    let config = load_config_from_temp_yaml(&source);
    let runtime = config.web.runtime.as_ref().unwrap();
    assert!(runtime.vhosts.is_empty());
    assert!(runtime.profiles.is_empty());
    assert!(crate::web::links::web_links_for_user(&config, "alice").is_empty());
}

fn wildcard_source(selector: &str) -> String {
    WEB_CONFIG
        .replace("web:\n", "web:\n  allow_wildcard_hosts: true\n")
        .replace("host: \"Proxy.Example.COM\"", selector)
        + "\nlimits:\n  memory_envelope_bytes: 1610612736\n"
}

#[test]
fn wildcard_selector_bounds_precedence_and_shared_inventory() {
    let source = wildcard_source(
        "hosts: [\"*.Example.COM\", \"*.deep.example.com\", \"exact.example.com\", \"*\"]",
    );
    let config = load_config_from_temp_yaml(&source);
    let runtime = config.web.runtime.as_ref().unwrap();
    assert_eq!(runtime.vhosts.len(), 1);
    assert_eq!(runtime.profiles.len(), 1);
    assert_eq!(runtime.wildcards.len(), 2);
    assert!(std::sync::Arc::ptr_eq(
        &runtime.wildcards["example.com"],
        &runtime.wildcards["deep.example.com"]
    ));
    assert!(runtime
        .route("exact.example.com")
        .unwrap()
        .wildcard
        .is_none());
    assert!(std::sync::Arc::ptr_eq(
        runtime.route("a.deep.example.com").unwrap(),
        &runtime.wildcards["deep.example.com"]
    ));
    assert!(std::sync::Arc::ptr_eq(
        runtime.route("example.com").unwrap(),
        runtime.catch_all.as_ref().unwrap()
    ));
    let repeated = load_config_from_temp_yaml(&source);
    assert_ne!(
        runtime.snapshot_id,
        repeated.web.runtime.as_ref().unwrap().snapshot_id
    );
    // Exact bindings plus one common wildcard profile set consume two slots.
    let bounded = source.replace(
        "memory_envelope_bytes: 1610612736",
        "memory_envelope_bytes: 1610612736\n  max_profiles: 2",
    );
    load_config_from_temp_yaml(&bounded);
    assert!(
        load_config_error_from_temp_yaml(&bounded.replace("max_profiles: 2", "max_profiles: 1"))
            .contains("max_profiles")
    );
}

#[test]
fn wildcard_configuration_rejects_ambiguous_selectors_and_resource_exhaustion() {
    for selector in [
        "foo*.example.com",
        "**.example.com",
        "*.127.0.0.1",
        "*.example.com.",
        "*.example.com:443",
        "*.*.example.com",
    ] {
        assert!(
            !load_config_error_from_temp_yaml(&wildcard_source(&format!(
                "host: \"{selector}\""
            )))
            .is_empty(),
            "{selector}"
        );
    }
    assert!(
        load_config_error_from_temp_yaml(&wildcard_source(
            "hosts: [\"*.EXAMPLE.com\", \"*.example.com\"]"
        ))
        .contains("duplicate")
    );
    assert!(
        load_config_error_from_temp_yaml(&wildcard_source("host: \"*.example.com\"").replace(
            "allow_wildcard_hosts: true",
            "allow_wildcard_hosts: false"
        ))
        .contains("allow_wildcard_hosts")
    );
    for limits in [
        "max_rules: 0",
        "max_rules: 9223372036854775807",
        "max_profiles_per_request: 0",
        "cache_bytes: 0",
        "cold_hmac_per_minute: 0",
        "cold_hmac_burst: 255",
        "max_profiles_per_request: 9223372036854775807",
    ] {
        let source = wildcard_source("host: \"*.example.com\"").replace(
            "limits:\n  memory_envelope_bytes: 1610612736",
            &format!(
                "limits:\n  memory_envelope_bytes: 1610612736\n  wildcard:\n    {limits}"
            ),
        );
        assert!(
            load_config_error_from_temp_yaml(&source).contains("wildcard"),
            "{limits}"
        );
    }
    let source = format!(
        "general:\n  config_strict: true\n{}",
        wildcard_source("host: \"*.example.com\"").replace(
            "limits:\n  memory_envelope_bytes: 1610612736",
            "limits:\n  memory_envelope_bytes: 1610612736\n  wildcard:\n    cache_hosts: 0",
        )
    );
    load_config_from_temp_yaml(&source);
    assert!(
        load_config_error_from_temp_yaml(&source.replace("cache_hosts", "cache_hostz"))
            .contains("cache_hostz")
    );
}

#[test]
fn wildcard_context_matches_exact_protocol_and_fences_reload_changes() {
    let source = wildcard_source("hosts: [\"proxy.example.com\", \"*.example.com\"]");
    let config = load_config_from_temp_yaml(&source);
    let runtime = config.web.runtime.as_ref().unwrap();
    let template = &runtime.wildcards["example.com"];
    assert_eq!(
        config
            .wildcard_capabilities(template, "proxy.example.com")
            .unwrap()[0],
        runtime.profiles[0].capability
    );
    let context = template
        .wildcard
        .as_ref()
        .unwrap()
        .profiles[0]
        .derivation_context;
    for changed in [
        source.replace("secret_mode: \"dd\"", "secret_mode: \"plain\""),
        source.replace(
            "public_addr: \"203.0.113.10:443\"",
            "base_path: \"relay\"\n  public_addr: \"203.0.113.10:443\"",
        ),
        source.replace(
            "000102030405060708090a0b0c0d0e0f",
            "100102030405060708090a0b0c0d0e0f",
        ),
    ] {
        let changed = load_config_from_temp_yaml(&changed);
        let changed_context = changed
            .web
            .runtime
            .as_ref()
            .unwrap()
            .wildcards["example.com"]
            .wildcard
            .as_ref()
            .unwrap()
            .profiles[0]
            .derivation_context;
        assert_ne!(context, changed_context);
    }
    let changed =
        load_config_from_temp_yaml(&source.replace("max_sessions: 4", "max_sessions: 5"));
    assert_eq!(
        context,
        changed
            .web
            .runtime
            .as_ref()
            .unwrap()
            .profiles[0]
            .shared
            .derivation_context
    );
}
