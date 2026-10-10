use super::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[test]
fn test_stats_shared_counters() {
    let stats = Arc::new(Stats::new());
    stats.increment_connects_all();
    stats.increment_connects_all();
    stats.increment_connects_all();
    assert_eq!(stats.get_connects_all(), 3);
}

#[test]
fn test_telemetry_policy_disables_core_and_user_counters() {
    let stats = Stats::new();
    stats.apply_telemetry_policy(TelemetryPolicy {
        core_enabled: false,
        user_enabled: false,
    });

    stats.increment_connects_all();
    stats.increment_user_connects("alice");
    stats.add_user_octets_from("alice", 1024);
    assert_eq!(stats.get_connects_all(), 0);
    assert_eq!(stats.get_user_curr_connects("alice"), 0);
    assert_eq!(stats.get_user_total_octets("alice"), 0);
}

#[test]
fn test_replay_checker_basic() {
    let checker = ReplayChecker::new(100, Duration::from_secs(60));
    assert!(!checker.check_handshake(b"test1")); // first time, inserts
    assert!(checker.check_handshake(b"test1")); // duplicate
    assert!(!checker.check_handshake(b"test2")); // new key inserts
}

#[test]
fn test_replay_checker_duplicate_add() {
    let checker = ReplayChecker::new(100, Duration::from_secs(60));
    checker.add_handshake(b"dup");
    checker.add_handshake(b"dup");
    assert!(checker.check_handshake(b"dup"));
}

#[test]
fn test_replay_checker_expiration() {
    let checker = ReplayChecker::new(100, Duration::from_millis(50));
    assert!(!checker.check_handshake(b"expire"));
    assert!(checker.check_handshake(b"expire"));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!checker.check_handshake(b"expire"));
}

#[test]
fn test_replay_checker_zero_window_does_not_retain_entries() {
    let checker = ReplayChecker::new(100, Duration::ZERO);

    for _ in 0..1_000 {
        assert!(!checker.check_handshake(b"no-retain"));
        checker.add_handshake(b"no-retain");
    }

    let stats = checker.stats();
    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.total_queue_len, 0);
}

#[test]
fn test_replay_checker_stats() {
    let checker = ReplayChecker::new(100, Duration::from_secs(60));
    assert!(!checker.check_handshake(b"k1"));
    assert!(!checker.check_handshake(b"k2"));
    assert!(checker.check_handshake(b"k1"));
    assert!(!checker.check_handshake(b"k3"));
    let stats = checker.stats();
    assert_eq!(stats.total_additions, 3);
    assert_eq!(stats.total_checks, 4);
    assert_eq!(stats.total_hits, 1);
}

#[test]
fn test_replay_checker_many_keys() {
    let checker = ReplayChecker::new(10_000, Duration::from_secs(60));
    for i in 0..500u32 {
        checker.add_handshake(&i.to_le_bytes());
    }
    for i in 0..500u32 {
        assert!(checker.check_handshake(&i.to_le_bytes()));
    }
    assert_eq!(checker.stats().total_entries, 500);
}

#[test]
fn test_cached_handle_survives_map_cleanup_until_last_drop() {
    let stats = Stats::new();
    let user = "user-stats-handle-lifetime-user";
    let user_stats = stats.get_or_create_user_stats_handle(user);
    let weak = Arc::downgrade(&user_stats);

    stats.user_stats.remove(user);
    assert!(
        stats.user_stats.get(user).is_none(),
        "map cleanup should remove idle entry"
    );
    assert!(
        weak.upgrade().is_some(),
        "cached handle must keep user stats object alive after map removal"
    );

    stats.add_user_octets_to_handle(user_stats.as_ref(), 3);
    assert_eq!(user_stats.octets_to_client.load(Ordering::Relaxed), 3);

    drop(user_stats);
    assert!(
        weak.upgrade().is_none(),
        "user stats object must be dropped after the last cached handle is released"
    );
}
