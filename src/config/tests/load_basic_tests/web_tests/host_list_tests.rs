use super::*;

#[test]
fn exact_host_list_builds_distinct_host_bound_capabilities() {
    let source = WEB_CONFIG.replace(
        "host: \"Proxy.Example.COM\"",
        "hosts: [\"Proxy.Example.COM\", \"second.example.com\"]",
    );
    let config = load_config_from_temp_yaml(&source);
    let runtime = config.web.runtime.as_ref().unwrap();
    assert_eq!(runtime.vhosts.len(), 2);
    assert_eq!(runtime.profiles.len(), 2);
    assert_eq!(runtime.capabilities.len(), 2);
    assert_ne!(runtime.capabilities[0], runtime.capabilities[1]);
    for host in ["proxy.example.com", "second.example.com"] {
        let legacy = load_config_from_temp_yaml(&WEB_CONFIG.replace("Proxy.Example.COM", host));
        assert_eq!(
            runtime.vhosts[host].capabilities,
            legacy.web.runtime.as_ref().unwrap().vhosts[host].capabilities,
        );
    }
}

#[test]
fn grouped_profiles_share_policy_but_match_legacy_bindings() {
    for mode in ["plain", "dd"] {
        for base in ["", "MixedCase/path"] {
            let source = WEB_CONFIG
                .replace("secret_mode: \"dd\"", &format!("secret_mode: \"{mode}\""))
                .replace(
                    "  public_addr:",
                    &format!("  base_path: \"{base}\"\n  public_addr:"),
                );
            let grouped = load_config_from_temp_yaml(&source.replace(
                "host: \"Proxy.Example.COM\"",
                "hosts: [\"Proxy.Example.COM\", \"second.example.com\"]",
            ));
            let runtime = grouped.web.runtime.as_ref().unwrap();
            let first = &runtime.vhosts["proxy.example.com"];
            let second = &runtime.vhosts["second.example.com"];
            assert_eq!(first.fallback, second.fallback);
            assert!(Arc::ptr_eq(
                &first.profiles[0].shared,
                &second.profiles[0].shared
            ));
            for host in ["proxy.example.com", "second.example.com"] {
                let legacy = load_config_from_temp_yaml(&source.replace("Proxy.Example.COM", host));
                let expected = &legacy.web.runtime.as_ref().unwrap().vhosts[host];
                let actual = &runtime.vhosts[host];
                assert_eq!(actual.base, expected.base);
                assert_eq!(actual.capabilities, expected.capabilities);
                assert_eq!(actual.profiles[0].host, host);
                let actual = &actual.profiles[0].shared;
                let expected = &expected.profiles[0].shared;
                assert_eq!(actual.user, expected.user);
                assert_eq!(actual.credential_id, expected.credential_id);
                assert_eq!(actual.key_fingerprint, expected.key_fingerprint);
                assert_eq!(actual.public_addr, expected.public_addr);
                assert_eq!(actual.carriers, expected.carriers);
                assert_eq!(actual.max_sessions, expected.max_sessions);
                assert_eq!(actual.max_streams, expected.max_streams);
                assert_eq!(
                    actual.max_streams_per_session,
                    expected.max_streams_per_session
                );
            }
        }
    }
}

#[test]
fn host_selectors_reject_ambiguity_and_canonical_collisions() {
    for (selector, expected) in [
        ("hosts: []", "web.hosts must be a non-empty hostname list"),
        ("hosts: [\"\"]", "web.hosts[0]"),
        (
            "host: \"a.example.com\"\n  hosts: []",
            "web requires exactly one of web.host or web.hosts",
        ),
        (
            "host: \"a.example.com\"\n  hosts: [\"b.example.com\"]",
            "web requires exactly one of web.host or web.hosts",
        ),
        ("hosts: [\"a.example.com\", \"A.EXAMPLE.COM\"]", "duplicate WEB host"),
        (
            "hosts: [\"B\u{dc}CHER.example\", \"xn--bcher-kva.example\"]",
            "duplicate WEB host",
        ),
        ("hosts: [\"*.example.com\"]", "allow_wildcard_hosts"),
        ("hosts: [\"\u{002a}\"]", "allow_wildcard_hosts"),
        ("hosts: [\"127.0.0.1\"]", "web.hosts[0]"),
        ("hosts: [\"a.example.com:443\"]", "web.hosts[0]"),
        ("hosts: [\"a.example.com.\"]", "web.hosts[0]"),
        ("hosts: [\"a.example.com/path\"]", "web.hosts[0]"),
    ] {
        let error = load_config_error_from_temp_yaml(
            &WEB_CONFIG.replace("host: \"Proxy.Example.COM\"", selector),
        );
        assert!(error.contains(expected), "{selector}: {error}");
    }
}

#[test]
fn host_selectors_round_trip_without_expanding_the_source() {
    for selector in [
        "host: \"Proxy.Example.COM\"",
        "hosts: [\"Proxy.Example.COM\"]",
        "hosts: [\"Proxy.Example.COM\", \"B\u{dc}CHER.example\"]",
    ] {
        let config = load_config_from_temp_yaml(&format!(
            "general:\n  config_strict: true\n{}",
            WEB_CONFIG.replace("host: \"Proxy.Example.COM\"", selector)
        ));
        // The vhost parameters are flattened into the `web` table.
        let value = serde_json::to_value(&config.web).unwrap();
        if selector.starts_with("host:") {
            assert_eq!(value["host"], "proxy.example.com");
            assert!(value.get("hosts").is_none());
        } else {
            assert_eq!(value["host"], "");
            assert_eq!(value["hosts"][0], "proxy.example.com");
            if value["hosts"].as_array().unwrap().len() == 2 {
                assert_eq!(value["hosts"][1], "xn--bcher-kva.example");
            }
        }
        assert!(value.get("runtime").is_none());
        let prefix = format!(
            "general:\n  config_strict: true\n{}",
            WEB_CONFIG.split_once("web:").unwrap().0
        );
        let mut document = serde_yaml::Mapping::new();
        document.insert(
            serde_yaml::Value::String("web".to_string()),
            serde_yaml::to_value(&config.web).unwrap(),
        );
        let round_trip = load_config_from_temp_yaml(&format!(
            "{prefix}{}",
            serde_yaml::to_string(&document).unwrap()
        ));
        assert_eq!(round_trip.web.vhost, config.web.vhost);
    }
}

#[test]
fn expanded_host_profile_limits_apply_before_runtime_preparation() {
    let source = WEB_CONFIG
        .replace(
            "host: \"Proxy.Example.COM\"",
            "hosts: [\"a.example.com\", \"b.example.com\"]"
        )
        .replace(
            "    max_streams_per_session: 16",
            "    max_streams_per_session: 16\n  bob:\n    secret: \"1112131415161718191a1b1c1d1e1f21\""
        );
    let source = format!("{source}\nlimits:\n  max_profiles: 4\n");
    let config = load_config_from_temp_yaml(&source);
    let runtime = config.web.runtime.as_ref().unwrap();
    assert_eq!(runtime.vhosts.len(), 2);
    assert_eq!(runtime.profiles.len(), 4);
    assert_eq!(runtime.capabilities.len(), 4);
    // Profiles materialize user-major: every binding of one user precedes
    // the next user's bindings; the shared policy is one Arc per user.
    assert_eq!(
        runtime
            .profiles
            .iter()
            .map(|profile| profile.host.as_str())
            .collect::<Vec<_>>(),
        ["a.example.com", "b.example.com", "a.example.com", "b.example.com"]
    );
    assert!(Arc::ptr_eq(&runtime.profiles[0].shared, &runtime.profiles[1].shared));
    assert!(!Arc::ptr_eq(
        &runtime.profiles[0].shared,
        &runtime.profiles[2].shared
    ));
    assert!(
        load_config_error_from_temp_yaml(&source.replace("max_profiles: 4", "max_profiles: 3"))
            .contains("max_profiles")
    );
}
