use super::*;

const PROFILE_HOST_CONFIG: &str = r#"
[users]
alice = "000102030405060708090a0b0c0d0e0f"
bob = "1112131415161718191a1b1c1d1e1f20"

[listener]
ip = "127.0.0.1"
port = 18080
web_client_ip_source = "x_forwarded_for"
web_trusted_proxy_cidrs = ["127.0.0.1/32"]

[general]
config_strict = true

[web]
carrier = "https-lanes"
secret_mode = "plain"

[web.vhost]
host = "proxy.example.com"
public_addr = "203.0.113.10:443"
upstream = "unix:/tmp/site.sock"

[[web.vhost.profiles]]
user = "alice"

[[web.vhost.profiles]]
user = "bob"
host = "Alt.Example.COM"
"#;

fn fallback_authority(vhost: &WebRuntimeVhost) -> &str {
    match &vhost.fallback {
        WebRuntimeFallback::HttpUpstream { authority, .. } => authority,
    }
}

#[test]
fn profile_host_override_builds_one_runtime_entry_per_host() {
    let config = load_config_from_temp_toml(PROFILE_HOST_CONFIG);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    assert_eq!(runtime.vhosts.len(), 2, "vhosts: {:?}", runtime.vhosts.keys().collect::<Vec<_>>());

    let default = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("default vhost host entry");
    assert_eq!(default.profiles.len(), 1);
    assert_eq!(default.profiles[0].user, "alice");
    assert_eq!(default.profiles[0].host, "proxy.example.com");
    assert_eq!(default.capabilities.len(), 1);
    assert_eq!(default.capabilities[0], default.profiles[0].capability);

    let overridden = runtime
        .vhosts
        .get("alt.example.com")
        .expect("normalized profile host entry");
    assert_eq!(overridden.profiles.len(), 1);
    assert_eq!(overridden.profiles[0].user, "bob");
    assert_eq!(overridden.profiles[0].host, "alt.example.com");
    assert_eq!(overridden.capabilities.len(), 1);
    assert_eq!(overridden.capabilities[0], overridden.profiles[0].capability);

    // The host is part of the capability derivation.
    assert_ne!(default.capabilities[0], overridden.capabilities[0]);
    assert_eq!(runtime.profiles.len(), 2);
}

#[test]
fn unix_fallback_authority_follows_each_profile_host() {
    let config = load_config_from_temp_toml(PROFILE_HOST_CONFIG);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let default = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("default vhost host entry");
    let overridden = runtime
        .vhosts
        .get("alt.example.com")
        .expect("normalized profile host entry");
    assert_eq!(fallback_authority(default), "proxy.example.com");
    assert_eq!(fallback_authority(overridden), "alt.example.com");
}

#[test]
fn profile_host_link_uses_the_overridden_host() {
    let config = load_config_from_temp_toml(PROFILE_HOST_CONFIG);
    let links = crate::web::links::web_links_for_user(&config, "bob");
    assert_eq!(links.len(), 1, "{links:?}");
    assert!(links[0].starts_with("tg://webproxy?server=alt.example.com"), "{links:?}");
    let alice = crate::web::links::web_links_for_user(&config, "alice");
    assert_eq!(alice.len(), 1, "{alice:?}");
    assert!(alice[0].starts_with("tg://webproxy?server=proxy.example.com"), "{alice:?}");
}

#[test]
fn profile_host_accepts_several_users_on_one_host() {
    // Both users explicitly point at the same profile host: the vhost host
    // then has no runtime entry at all.
    let toml = PROFILE_HOST_CONFIG.replace(
        "user = \"alice\"\n\n[[web.vhost.profiles]]\nuser = \"bob\"\nhost = \"Alt.Example.COM\"",
        "user = \"alice\"\nhost = \"alt.example.com\"\n\n\
         [[web.vhost.profiles]]\nuser = \"bob\"\nhost = \"alt.example.com\"",
    );
    let config = load_config_from_temp_toml(&toml);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    // Both users live under the shared profile host; the vhost host entry is empty.
    let shared = runtime.vhosts.get("alt.example.com").expect("shared profile host entry");
    assert_eq!(shared.profiles.len(), 2);
    assert!(runtime.vhosts.get("proxy.example.com").is_none());
    let users: Vec<&str> = shared.profiles.iter().map(|p| p.user.as_str()).collect();
    assert_eq!(users, vec!["alice", "bob"]);
    assert_eq!(shared.capabilities.len(), 2);
}

#[test]
fn instantiated_profiles_keep_the_vhost_host() {
    // bob has no explicit profile: web.secret_mode instantiates him on the
    // vhost host even when another profile overrides its host.
    let base = PROFILE_HOST_CONFIG
        .replace(
            "[[web.vhost.profiles]]\nuser = \"bob\"\nhost = \"Alt.Example.COM\"\n",
            "",
        )
        .replace(
            "[[web.vhost.profiles]]\nuser = \"alice\"\n",
            "[[web.vhost.profiles]]\nuser = \"alice\"\nhost = \"Alt.Example.COM\"\n",
        );
    let config = load_config_from_temp_toml(&base);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    assert_eq!(runtime.vhosts.len(), 2);
    let default = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("default vhost host entry");
    assert_eq!(default.profiles.len(), 1);
    assert_eq!(default.profiles[0].user, "bob");
    assert_eq!(default.profiles[0].host, "proxy.example.com");
    let overridden = runtime
        .vhosts
        .get("alt.example.com")
        .expect("normalized profile host entry");
    assert_eq!(overridden.profiles.len(), 1);
    assert_eq!(overridden.profiles[0].user, "alice");
}

#[test]
fn profile_host_rejects_ip_literals_with_the_field_path() {
    let toml = PROFILE_HOST_CONFIG.replace(
        "host = \"Alt.Example.COM\"",
        "host = \"203.0.113.10\"",
    );
    let error = load_config_error_from_temp_toml(&toml);
    assert!(error.contains("web.vhost.profiles[1].host"), "{error}");
    assert!(
        error.contains("must be a non-IP fully-qualified hostname"),
        "{error}"
    );
}

#[test]
fn duplicate_secret_on_different_profile_hosts_does_not_collide() {
    // The host is part of the capability derivation, so the same secret on
    // two different profile hosts yields distinct capabilities.
    let toml = PROFILE_HOST_CONFIG.replace(
        "bob = \"1112131415161718191a1b1c1d1e1f20\"",
        "bob = \"000102030405060708090a0b0c0d0e0f\"",
    );
    let config = load_config_from_temp_toml(&toml);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    assert_eq!(runtime.profiles.len(), 2);
    let capabilities: Vec<[u8; 32]> = runtime.profiles.iter().map(|p| p.capability).collect();
    assert_ne!(capabilities[0], capabilities[1]);
}
