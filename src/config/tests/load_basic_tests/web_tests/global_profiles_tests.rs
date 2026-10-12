use super::*;

const GLOBAL_WEB_CONFIG: &str = r#"
[users]
alice = "000102030405060708090a0b0c0d0e0f"
bob = "1112131415161718191a1b1c1d1e1f20"

[listener]
ip = "127.0.0.1"
port = 18080
transport = "web"
web_client_ip_source = "x_forwarded_for"
web_trusted_proxy_cidrs = ["127.0.0.1/32"]

[web]
enabled = true
carrier = "https-lanes"

[[web.profiles]]
secret_mode = "plain"

[[web.vhosts]]
host = "proxy.example.com"
public_addr = "203.0.113.10:443"

[web.vhosts.fallback]
mode = "http_upstream"
upstream = "http://127.0.0.1:18081"
"#;

#[test]
fn global_profile_instantiates_every_user_in_every_vhost() {
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
        // Unset ceilings fall back to the global WEB ceilings.
        assert_eq!(profile.max_sessions, config.web.limits.max_sessions_global);
        assert_eq!(profile.max_streams, config.web.limits.max_streams_global);
        assert_eq!(
            profile.max_streams_per_session,
            config.web.limits.max_streams_per_session
        );
    }
    assert_eq!(runtime.profiles.len(), 2);

    let two_vhosts = GLOBAL_WEB_CONFIG.replace(
        "[[web.vhosts]]\nhost = \"proxy.example.com\"",
        "[[web.vhosts]]\nhost = \"second.example.com\"\npublic_addr = \"203.0.113.11:443\"\n\n\
         [web.vhosts.fallback]\nmode = \"http_upstream\"\nupstream = \"http://127.0.0.1:18082\"\n\n\
         [[web.vhosts]]\nhost = \"proxy.example.com\"",
    );
    let config = load_config_from_temp_toml(&two_vhosts);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    for host in ["second.example.com", "proxy.example.com"] {
        let vhost = runtime.vhosts.get(host).expect("canonical WEB vhost");
        assert_eq!(vhost.profiles.len(), 2, "{host}");
    }
}

#[test]
fn explicit_profile_wins_over_global_for_same_mode() {
    let toml = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.vhosts.profiles]]\nuser = \"alice\"\nsecret_mode = \"plain\"\nmax_sessions = 4\n"
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
fn explicit_profile_keeps_both_modes_alongside_global() {
    let toml = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.vhosts.profiles]]\nuser = \"alice\"\nsecret_mode = \"dd\"\n"
    );
    let config = load_config_from_temp_toml(&toml);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 3);
    let alice: Vec<_> = vhost
        .profiles
        .iter()
        .filter(|p| p.user == "alice")
        .map(|p| p.secret_mode)
        .collect();
    assert_eq!(
        alice,
        vec![WebSecretMode::Dd, WebSecretMode::Plain]
    );
    let bob: Vec<_> = vhost
        .profiles
        .iter()
        .filter(|p| p.user == "bob")
        .map(|p| p.secret_mode)
        .collect();
    assert_eq!(bob, vec![WebSecretMode::Plain]);
}

#[test]
fn global_profile_limits_apply_to_instantiated_profiles() {
    let toml = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\nmax_sessions = 7\nmax_streams = 8\n\
         max_streams_per_session = 2",
    );
    let config = load_config_from_temp_toml(&toml);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    for profile in &vhost.profiles {
        assert_eq!(profile.max_sessions, 7);
        assert_eq!(profile.max_streams, 8);
        assert_eq!(profile.max_streams_per_session, 2);
    }
}

#[test]
fn duplicate_global_secret_mode_is_rejected() {
    let toml = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\n\n[[web.profiles]]\nsecret_mode = \"plain\"",
    );
    let error = load_config_error_from_temp_toml(&toml);
    assert!(
        error.contains("duplicate WEB global profile"),
        "{error}"
    );
}

#[test]
fn global_profile_expansion_counts_against_max_profiles() {
    // 2 users x 2 global modes x 1 vhost = 4 expanded profiles.
    let with_limits = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\n\n[[web.profiles]]\nsecret_mode = \"dd\"",
    );
    let error = load_config_error_from_temp_toml(&format!(
        "{with_limits}\n[web.limits]\nmax_profiles = 3"
    ));
    assert!(
        error.contains("exceed web.limits.max_profiles"),
        "{error}"
    );
    let config = load_config_from_temp_toml(&format!(
        "{with_limits}\n[web.limits]\nmax_profiles = 4"
    ));
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 4);
}

#[test]
fn web_enabled_requires_explicit_or_global_profiles() {
    // Neither explicit nor global profiles: the requirement still fires.
    let none = GLOBAL_WEB_CONFIG
        .replace("[[web.profiles]]\nsecret_mode = \"plain\"\n\n", "");
    let error = load_config_error_from_temp_toml(&none);
    assert!(error.contains("has no profiles"), "{error}");
}

#[test]
fn global_profile_rejects_user_key() {
    let with_user = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\nuser = \"alice\"",
    );
    let strict = format!("{with_user}\n[general]\nconfig_strict = true\n");
    let error = load_config_error_from_temp_toml(&strict);
    assert!(error.contains("user"), "{error}");

    // Non-strict: the stray key is ignored and the profile still applies
    // to every user.
    let config = load_config_from_temp_toml(&with_user);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
}

#[test]
fn global_profile_limits_must_be_non_zero_and_within_globals() {
    let zero = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\nmax_sessions = 0",
    );
    let error = load_config_error_from_temp_toml(&zero);
    assert!(
        error.contains("web.profiles[0] limits must be non-zero"),
        "{error}"
    );

    let above = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\nmax_sessions = 3",
    );
    let error = load_config_error_from_temp_toml(&format!(
        "{above}\n[web.limits]\nmax_sessions_global = 2\nmax_sessions_per_ip = 1"
    ));
    assert!(
        error.contains("web.profiles[0] limits must be non-zero"),
        "{error}"
    );

    let nested = GLOBAL_WEB_CONFIG.replace(
        "[[web.profiles]]\nsecret_mode = \"plain\"",
        "[[web.profiles]]\nsecret_mode = \"plain\"\nmax_streams = 4\nmax_streams_per_session = 8",
    );
    let error = load_config_error_from_temp_toml(&nested);
    assert!(
        error.contains("web.profiles[0] limits must be non-zero"),
        "{error}"
    );
}
