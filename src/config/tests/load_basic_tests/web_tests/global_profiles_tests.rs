use super::*;

const GLOBAL_WEB_CONFIG: &str = r#"
users:
  alice:
    secret: "000102030405060708090a0b0c0d0e0f"
  bob:
    secret: "1112131415161718191a1b1c1d1e1f20"

listener:
  ip: "127.0.0.1"
  port: 18080
  web_client_ip_source: "x_forwarded_for"
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]

web:
  carrier: "https-lanes"
  secret_mode: "plain"
  vhost:
    host: "proxy.example.com"
    public_addr: "203.0.113.10:443"
    upstream: "http://127.0.0.1:18081"
"#;

#[test]
fn global_secret_mode_instantiates_every_user_in_the_vhost() {
    let config = load_config_from_temp_yaml(GLOBAL_WEB_CONFIG);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
    let users: Vec<&str> = vhost.profiles.iter().map(|p| p.user.as_str()).collect();
    assert!(users.contains(&"alice"), "{users:?}");
    assert!(users.contains(&"bob"), "{users:?}");
    for profile in &vhost.profiles {
        assert_eq!(profile.secret_mode, WebSecretMode::Plain);
        // Instantiated profiles use the global WEB ceilings.
        assert_eq!(profile.max_sessions, config.web.limits.max_sessions_global);
        assert_eq!(profile.max_streams, config.web.limits.max_streams_global);
        assert_eq!(
            profile.max_streams_per_session,
            config.web.limits.max_streams_per_session
        );
    }
    assert_eq!(runtime.profiles.len(), 2);
}

#[test]
fn per_user_ceilings_win_over_global_for_same_user() {
    // alice carries a per-user ceiling; it wins over the global value while
    // bob keeps the global one.
    let yaml = GLOBAL_WEB_CONFIG.replace(
        "  alice:\n    secret: \"000102030405060708090a0b0c0d0e0f\"",
        "  alice:\n    secret: \"000102030405060708090a0b0c0d0e0f\"\n    max_sessions: 4",
    );
    let config = load_config_from_temp_yaml(&yaml);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
    for profile in &vhost.profiles {
        assert_eq!(profile.secret_mode, WebSecretMode::Plain);
        if profile.user == "alice" {
            // The per-user entry keeps its own ceilings.
            assert_eq!(profile.max_sessions, 4);
        } else {
            assert_eq!(profile.user, "bob");
            assert_eq!(
                profile.max_sessions,
                config.web.limits.max_sessions_global
            );
        }
    }
}

#[test]
fn global_profile_expansion_counts_against_max_profiles() {
    // 2 users x 1 vhost = 2 expanded profiles.
    let error = load_config_error_from_temp_yaml(&format!(
        "{GLOBAL_WEB_CONFIG}\n  limits:\n    max_profiles: 1"
    ));
    assert!(
        error.contains("exceed web.limits.max_profiles"),
        "{error}"
    );
    let config = load_config_from_temp_yaml(&format!(
        "{GLOBAL_WEB_CONFIG}\n  limits:\n    max_profiles: 2"
    ));
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
}

#[test]
fn web_listener_requires_secret_mode() {
    // A WEB listener without web.secret_mode: the requirement fires.
    let none = GLOBAL_WEB_CONFIG.replace("  secret_mode: \"plain\"\n", "");
    let error = load_config_error_from_temp_yaml(&none);
    assert!(error.contains("A WEB listener requires web.secret_mode"), "{error}");
}

#[test]
fn web_profiles_table_is_stripped() {
    // The web.profiles table was removed: the mode lives on web.secret_mode
    // and instantiated profiles use the web.limits ceilings.
    let with_table = format!(
        "{GLOBAL_WEB_CONFIG}\n  profiles:\n    max_sessions: 7\n"
    );
    let strict = format!("{with_table}\ngeneral:\n  config_strict: true\n");
    let error = load_config_error_from_temp_yaml(&strict);
    assert!(error.contains("profiles"), "{error}");

    // Non-strict: the removed table is ignored and web.secret_mode still
    // instantiates every user with the global ceilings.
    let config = load_config_from_temp_yaml(&with_table);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
    for profile in &vhost.profiles {
        assert_eq!(profile.max_sessions, config.web.limits.max_sessions_global);
    }
}
