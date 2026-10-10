use super::*;
use crate::ip_tracker::UserIpTracker;
use crate::stats::Stats;

#[tokio::test]
async fn users_from_config_reports_user_enabled_default_and_override() {
    let mut cfg = ProxyConfig::default();
    cfg.access.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string(),
    );
    cfg.access.users.insert(
        "bob".to_string(),
        "fedcba9876543210fedcba9876543210".to_string(),
    );
    cfg.access.user_enabled.insert("bob".to_string(), false);

    let stats = Stats::new();
    let tracker = UserIpTracker::new();
    let users = users_from_config(&cfg, &stats, &tracker, None).await;
    let alice = users
        .iter()
        .find(|entry| entry.username == "alice")
        .expect("alice must be present");
    let bob = users
        .iter()
        .find(|entry| entry.username == "bob")
        .expect("bob must be present");

    assert!(alice.enabled);
    assert!(!bob.enabled);

    cfg.access.user_enabled.insert("bob".to_string(), true);
    let users = users_from_config(&cfg, &stats, &tracker, None).await;
    let bob = users
        .iter()
        .find(|entry| entry.username == "bob")
        .expect("bob must be present");
    assert!(bob.enabled);
}

#[tokio::test]
async fn users_from_config_marks_runtime_membership_when_snapshot_is_provided() {
    let mut disk_cfg = ProxyConfig::default();
    disk_cfg.access.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string(),
    );
    disk_cfg.access.users.insert(
        "bob".to_string(),
        "fedcba9876543210fedcba9876543210".to_string(),
    );

    let mut runtime_cfg = ProxyConfig::default();
    runtime_cfg.access.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string(),
    );

    let stats = Stats::new();
    let tracker = UserIpTracker::new();
    let users =
        users_from_config(&disk_cfg, &stats, &tracker, Some(&runtime_cfg)).await;

    let alice = users
        .iter()
        .find(|entry| entry.username == "alice")
        .expect("alice must be present");
    let bob = users
        .iter()
        .find(|entry| entry.username == "bob")
        .expect("bob must be present");

    assert!(alice.in_runtime);
    assert!(!bob.in_runtime);
}
