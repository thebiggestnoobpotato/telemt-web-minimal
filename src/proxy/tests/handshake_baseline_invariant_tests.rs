
use super::*;
use crate::crypto::sha256_hmac;
use crate::protocol::constants::{TLS_RECORD_HANDSHAKE, TLS_VERSION};
use crate::stats::ReplayChecker;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::time::timeout;

fn test_config_with_secret_hex(secret_hex: &str) -> ProxyConfig {
    let mut cfg = ProxyConfig::default();
    cfg.access.users.clear();
    cfg.access
        .users
        .insert("user".to_string(), secret_hex.to_string());
    cfg.access.ignore_time_skew = true;
    cfg.censorship.mask = true;
    cfg
}

#[test]
fn handshake_baseline_saturation_fires_at_compile_time_threshold() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 33));
    let now = Instant::now();

    for _ in 0..AUTH_PROBE_BACKOFF_START_FAILS.saturating_sub(1) {
        auth_probe_record_failure_in(shared.as_ref(), ip, now);
    }
    assert!(!auth_probe_is_throttled_in(shared.as_ref(), ip, now));

    auth_probe_record_failure_in(shared.as_ref(), ip, now);
    assert!(auth_probe_is_throttled_in(shared.as_ref(), ip, now));
}

#[test]
fn handshake_baseline_repeated_probes_streak_monotonic() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42));
    let now = Instant::now();
    let mut prev = 0u32;

    for _ in 0..100 {
        auth_probe_record_failure_in(shared.as_ref(), ip, now);
        let current =
            auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), ip).unwrap_or(0);
        assert!(current >= prev, "streak must be monotonic");
        prev = current;
    }
}

#[test]
fn handshake_baseline_throttled_ip_incurs_backoff_delay() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 44));
    let now = Instant::now();

    for _ in 0..AUTH_PROBE_BACKOFF_START_FAILS {
        auth_probe_record_failure_in(shared.as_ref(), ip, now);
    }

    let delay = auth_probe_backoff(AUTH_PROBE_BACKOFF_START_FAILS);
    assert!(delay >= Duration::from_millis(AUTH_PROBE_BACKOFF_BASE_MS));

    let before_expiry = now + delay.saturating_sub(Duration::from_millis(1));
    let after_expiry = now + delay + Duration::from_millis(1);

    assert!(auth_probe_is_throttled_in(
        shared.as_ref(),
        ip,
        before_expiry
    ));
    assert!(!auth_probe_is_throttled_in(
        shared.as_ref(),
        ip,
        after_expiry
    ));
}

