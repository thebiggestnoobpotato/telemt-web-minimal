use super::*;

// Method selection must not alter capability identity or accepted config literals.
#[path = "web_tests/carrier_method_tests.rs"]
mod carrier_method_tests;

// Conveyor roundtrip defaults and boolean-only validation.
#[path = "web_tests/conveyor_tests.rs"]
mod conveyor_tests;

// Fallback DNS opt-in resolution mode and timeout bounds.
#[path = "web_tests/fallback_dns_tests.rs"]
mod fallback_dns_tests;

// Global secret-mode profile expansion and per-user ceiling precedence.
#[path = "web_tests/global_profiles_tests.rs"]
mod global_profiles_tests;

// Per-profile host overrides and their runtime authorities.
#[path = "web_tests/profile_host_tests.rs"]
mod profile_host_tests;

// Base path acceptance, rejection, and precomputed runtime form.
#[path = "web_tests/base_path_tests.rs"]
mod base_path_tests;

// Capacity ceilings and cross-field reservation validation.
#[path = "web_tests/capacity_validation_tests.rs"]
mod capacity_validation_tests;

// Carrier negotiation arrays, learning, and bridge deadline configuration.
#[path = "web_tests/carrier_negotiation_tests.rs"]
mod carrier_negotiation_tests;

// Web debug table defaults and fail-closed window validation.
#[path = "web_tests/debug_tests.rs"]
mod debug_tests;

// Removed config keys rejected in strict mode and ignored otherwise.
#[path = "web_tests/stripped_keys_tests.rs"]
mod stripped_keys_tests;

// Vhost fallback fasttrack mode, listener overlap, and authority forms.
#[path = "web_tests/vhost_fallback_tests.rs"]
mod vhost_fallback_tests;

// Websocket carrier runtime defaults and watermark policy validation.
#[path = "web_tests/websocket_tests.rs"]
mod websocket_tests;

const WEB_CONFIG: &str = r#"
users:
  alice:
    secret: "000102030405060708090a0b0c0d0e0f"
    max_sessions: 4
    max_streams: 64
    max_streams_per_session: 16

listener:
  ip: "127.0.0.1"
  port: 18080
  web_client_ip_source: "x_forwarded_for"
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]

web:
  carrier: "https-lanes"
  secret_mode: "dd"
  vhost:
    host: "Proxy.Example.COM"
    public_addr: "203.0.113.10:443"
    upstream: "http://127.0.0.1:18081"
"#;

#[test]
fn web_config_builds_canonical_runtime_snapshot() {
    let config = load_config_from_temp_yaml(WEB_CONFIG);
    let runtime = config.web.runtime.expect("WEB runtime snapshot");
    let vhost = runtime
        .vhosts
        .get("proxy.example.com")
        .expect("canonical WEB vhost");
    assert_eq!(vhost.base, "/");
    assert_eq!(vhost.profiles.len(), 1);
    assert_eq!(vhost.capabilities.len(), vhost.profiles.len());
    assert_eq!(vhost.capabilities[0], vhost.profiles[0].capability);
    assert_eq!(runtime.capabilities.as_ref(), vhost.capabilities.as_ref());
    assert_eq!(vhost.fallback_fasttrack_mode, WebFallbackFastTrackMode::Off);
    assert_eq!(vhost.profiles[0].user, "alice");
    assert_eq!(vhost.profiles[0].secret_mode, WebSecretMode::Dd);
    assert_eq!(vhost.profiles[0].carrier, WebCarrier::HttpsLanes);
    assert_eq!(vhost.profiles[0].max_sessions, 4);
    assert_eq!(vhost.profiles[0].max_streams, 64);
    assert_eq!(vhost.profiles[0].max_streams_per_session, 16);
    assert_eq!(vhost.profiles[0].key_fingerprint.len(), 16);
    assert_ne!(vhost.profiles[0].key_fingerprint, "0001020304050607");
    assert!(!vhost.profiles[0].carrier_negotiation_enabled);
    assert_eq!(
        vhost.profiles[0].carriers.as_ref(),
        [WebCarrier::HttpsLanes]
    );
}

#[test]
fn web_profile_user_labels_are_bounded_for_runtime_status() {
    let user = "a".repeat(65);
    let invalid = WEB_CONFIG.replace("alice", &user);

    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains(&format!("users.{user}")), "{error}");
    assert!(error.contains("must contain 1..64 bytes"), "{error}");
}

#[test]
fn web_listener_requires_an_explicit_trusted_proxy() {
    let invalid = WEB_CONFIG.replace(
        "web_trusted_proxy_cidrs: [\"127.0.0.1/32\"]",
        "web_trusted_proxy_cidrs: []",
    );
    let error = load_config_error_from_temp_yaml(&invalid);
    assert!(error.contains("web_trusted_proxy_cidrs must be non-empty"));
}
