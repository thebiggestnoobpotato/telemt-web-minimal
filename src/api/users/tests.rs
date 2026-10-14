use super::*;
use crate::stats::Stats;

#[tokio::test]
async fn users_from_config_reports_all_configured_users() {
    let mut cfg = ProxyConfig::default();
    cfg.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string(),
    );
    cfg.users.insert(
        "bob".to_string(),
        "fedcba9876543210fedcba9876543210".to_string(),
    );

    let stats = Stats::new();
    let users = users_from_config(&cfg, &stats, None).await;
    let alice = users
        .iter()
        .find(|entry| entry.username == "alice")
        .expect("alice must be present");
    let bob = users
        .iter()
        .find(|entry| entry.username == "bob")
        .expect("bob must be present");

    assert!(!alice.in_runtime);
    assert!(!bob.in_runtime);
}

#[tokio::test]
async fn users_from_config_marks_runtime_membership_when_snapshot_is_provided() {
    let mut disk_cfg = ProxyConfig::default();
    disk_cfg.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string(),
    );
    disk_cfg.users.insert(
        "bob".to_string(),
        "fedcba9876543210fedcba9876543210".to_string(),
    );

    let mut runtime_cfg = ProxyConfig::default();
    runtime_cfg.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string(),
    );

    let stats = Stats::new();
    let users = users_from_config(&disk_cfg, &stats, Some(&runtime_cfg)).await;

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
