use super::*;
use crate::stats::Stats;

#[tokio::test]
async fn users_from_config_reports_all_configured_users() {
    let mut cfg = ProxyConfig::default();
    cfg.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string().into(),
    );
    cfg.users.insert(
        "bob".to_string(),
        "fedcba9876543210fedcba9876543210".to_string().into(),
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
        "0123456789abcdef0123456789abcdef".to_string().into(),
    );
    disk_cfg.users.insert(
        "bob".to_string(),
        "fedcba9876543210fedcba9876543210".to_string().into(),
    );

    let mut runtime_cfg = ProxyConfig::default();
    runtime_cfg.users.insert(
        "alice".to_string(),
        "0123456789abcdef0123456789abcdef".to_string().into(),
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

#[tokio::test]
async fn users_from_config_builds_links_from_the_runtime_snapshot() {
    // The disk snapshot is parse-only (no runtime rebuild), so the link
    // view must come from the runtime snapshot per the API.md contract.
    let yaml = r#"
users:
  alice:
    secret: "000102030405060708090a0b0c0d0e0f"

listener:
  ip: "127.0.0.1"
  port: 18080
  web_client_ip_source: "x_forwarded_for"
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]

general:
  config_strict: true

web:
  carrier: "https-lanes"
  secret_mode: "plain"
  host: "proxy.example.com"
  public_addr: "203.0.113.10:443"
  upstream: "unix:/tmp/site.sock"
"#;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("telemt_users_links_{nonce}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(&path, yaml).unwrap();
    let runtime_cfg = ProxyConfig::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);

    let mut disk_cfg = ProxyConfig::default();
    disk_cfg.users.insert(
        "alice".to_string(),
        "000102030405060708090a0b0c0d0e0f".to_string().into(),
    );

    let stats = Stats::new();
    let users = users_from_config(&disk_cfg, &stats, Some(&runtime_cfg)).await;
    let alice = users
        .iter()
        .find(|entry| entry.username == "alice")
        .expect("alice must be present");

    assert_eq!(
        alice.links.web,
        vec![
            "tg://webproxy?server=proxy.example.com&secret=000102030405060708090a0b0c0d0e0f"
                .to_string()
        ]
    );
}
