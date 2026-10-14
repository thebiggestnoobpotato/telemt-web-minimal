use super::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;

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
