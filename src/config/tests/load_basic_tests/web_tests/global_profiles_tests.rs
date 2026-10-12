use super::*;

const GLOBAL_WEB_CONFIG: &str = r#"
[users]
alice = "000102030405060708090a0b0c0d0e0f"
bob = "1112131415161718191a1b1c1d1e1f20"

[listener]
ip = "127.0.0.1"
port = 18080
web_client_ip_source = "x_forwarded_for"
web_trusted_proxy_cidrs = ["127.0.0.1/32"]

[web]
enabled = true
carrier = "https-lanes"
secret_mode = "plain"

[web.vhost]
host = "proxy.example.com"
public_addr = "203.0.113.10:443"

upstream = "http://127.0.0.1:18081"
"#;

#[test]
fn global_secret_mode_instantiates_every_user_in_the_vhost() {
    let config = load_config_from_temp_toml(GLOBAL_WEB_CONFIG);
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
fn explicit_profile_wins_over_global_for_same_user() {
    let toml = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.vhost.profiles]]\nuser = \"alice\"\nmax_sessions = 4\n"
    );
    let config = load_config_from_temp_toml(&toml);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
    for profile in &vhost.profiles {
        assert_eq!(profile.secret_mode, WebSecretMode::Plain);
        if profile.user == "alice" {
            // The explicit profile keeps its own ceilings.
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
    let error = load_config_error_from_temp_toml(&format!(
        "{GLOBAL_WEB_CONFIG}\n[web.limits]\nmax_profiles = 1"
    ));
    assert!(
        error.contains("exceed web.limits.max_profiles"),
        "{error}"
    );
    let config = load_config_from_temp_toml(&format!(
        "{GLOBAL_WEB_CONFIG}\n[web.limits]\nmax_profiles = 2"
    ));
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
}

#[test]
fn web_enabled_requires_explicit_or_global_profiles() {
    // No explicit profiles and no web.secret_mode: the requirement fires.
    let none = GLOBAL_WEB_CONFIG.replace("secret_mode = \"plain\"\n", "");
    let error = load_config_error_from_temp_toml(&none);
    assert!(error.contains("has no profiles"), "{error}");
}

#[test]
fn web_profiles_table_is_stripped() {
    // The [[web.profiles]] table was removed: the mode lives on web.secret_mode
    // and instantiated profiles use the web.limits ceilings.
    let with_table = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nmax_sessions = 7\n"
    );
    let strict = format!("{with_table}\n[general]\nconfig_strict = true\n");
    let error = load_config_error_from_temp_toml(&strict);
    assert!(error.contains("profiles"), "{error}");

    // Non-strict: the removed table is ignored and web.secret_mode still
    // instantiates every user with the global ceilings.
    let config = load_config_from_temp_toml(&with_table);
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
