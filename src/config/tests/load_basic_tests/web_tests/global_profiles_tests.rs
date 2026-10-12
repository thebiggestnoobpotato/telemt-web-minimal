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
secret_mode = "plain"

[[web.vhosts]]
host = "proxy.example.com"
public_addr = "203.0.113.10:443"

[web.vhosts.fallback]
mode = "http_upstream"
upstream = "http://127.0.0.1:18081"
"#;

#[test]
fn global_secret_mode_instantiates_every_user_in_every_vhost() {
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
    assert_eq!(alice, vec![WebSecretMode::Dd, WebSecretMode::Plain]);
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
    let toml = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nmax_sessions = 7\nmax_streams = 8\nmax_streams_per_session = 2\n"
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
fn global_profile_surface_requires_secret_mode_and_single_entry() {
    // Ceilings without a global mode have nothing to apply to.
    let no_mode = GLOBAL_WEB_CONFIG
        .replace("secret_mode = \"plain\"\n", "")
        + "\n[[web.profiles]]\nmax_sessions = 7\n";
    let error = load_config_error_from_temp_toml(&no_mode);
    assert!(
        error.contains("web.profiles requires web.secret_mode"),
        "{error}"
    );

    // At most one ceilings entry: the mode lives on [web].
    let two_entries = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nmax_sessions = 7\n\
         [[web.profiles]]\nmax_streams = 8\n"
    );
    let error = load_config_error_from_temp_toml(&two_entries);
    assert!(
        error.contains("web.profiles supports a single entry"),
        "{error}"
    );
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
fn global_profile_rejects_secret_mode_key() {
    // secret_mode moved to [web]; inside [[web.profiles]] it is unknown.
    let with_mode = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nsecret_mode = \"dd\"\nmax_sessions = 5\n"
    );
    let strict = format!("{with_mode}\n[general]\nconfig_strict = true\n");
    let error = load_config_error_from_temp_toml(&strict);
    assert!(error.contains("secret_mode"), "{error}");

    // Non-strict: the stray key is ignored; the entry ceilings still apply
    // to the [web] mode.
    let config = load_config_from_temp_toml(&with_mode);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.profiles.len(), 2);
    for profile in &vhost.profiles {
        assert_eq!(profile.secret_mode, WebSecretMode::Plain);
        assert_eq!(profile.max_sessions, 5);
    }
}

#[test]
fn global_profile_limits_must_be_non_zero_and_within_globals() {
    let zero = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nmax_sessions = 0\n"
    );
    let error = load_config_error_from_temp_toml(&zero);
    assert!(
        error.contains("web.profiles[0] limits must be non-zero"),
        "{error}"
    );

    let above = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nmax_sessions = 3\n"
    );
    let error = load_config_error_from_temp_toml(&format!(
        "{above}\n[web.limits]\nmax_sessions_global = 2\nmax_sessions_per_ip = 1"
    ));
    assert!(
        error.contains("web.profiles[0] limits must be non-zero"),
        "{error}"
    );

    let nested = format!(
        "{GLOBAL_WEB_CONFIG}\n\
         [[web.profiles]]\nmax_streams = 4\nmax_streams_per_session = 8\n"
    );
    let error = load_config_error_from_temp_toml(&nested);
    assert!(
        error.contains("web.profiles[0] limits must be non-zero"),
        "{error}"
    );
}
