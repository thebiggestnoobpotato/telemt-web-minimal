use super::*;

const SOURCE: &str = r#"
[users]
alice = "000102030405060708090a0b0c0d0e0f"
[listener]
ip = "127.0.0.1"
port = 18080
web_trusted_proxy_cidrs = ["127.0.0.1/32"]
[web]
enabled = true
secret_mode = "dd"
[web.vhost]
host = "proxy.example.com"
public_addr = "203.0.113.10:443"
upstream = "http://localhost:18081"
resolve = "startup"
[[web.vhost.profiles]]
user = "alice"
"#;

#[tokio::test]
async fn fallback_dns_api_get_and_mutation_base_need_no_prepared_runtime() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::write(
        &path,
        SOURCE.replace("localhost", "unresolvable.example.invalid"),
    )
    .unwrap();
    let (managed, revision) = read_managed_config(&path).await.unwrap();
    assert_eq!(
        managed["web"]["vhost"]["resolve"].as_str(),
        Some("startup")
    );
    assert!(managed["web"].get("fallback_dns").is_none());
    let (base, base_revision) = crate::api::config_store::load_config_for_mutation(&path, None)
        .await
        .unwrap();
    assert_eq!(revision, base_revision);
    assert!(base.web.runtime.is_none());
    assert!(base.web.fallback_dns.origins.is_empty());
}

#[tokio::test]
async fn fallback_dns_api_patch_prepares_before_disk_write() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::write(&path, SOURCE).unwrap();
    // This current-thread runtime exercises the real Tokio resolver without nested block_on.
    let active = ProxyConfig::load_prepared(path.clone())
        .await
        .unwrap()
        .config;
    let before = std::fs::read(&path).unwrap();
    let mut vhost = serde_json::to_value(&active.web.vhost).unwrap();
    vhost["upstream"] = Json::String("http://localhost:18080".to_string());
    let error = apply_patch_to_path(&path, &serde_json::json!({"web": {"vhost": vhost}}), None)
        .await
        .unwrap_err();
    assert!(error.message.contains("overlaps WEB listener"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(active.web.runtime.is_some());

    prepare_patch_to_path(
        &path,
        &serde_json::json!({"web": {"conveyor": false}}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    apply_patch_to_path(
        &path,
        &serde_json::json!({"web": {"conveyor": false}}),
        None,
    )
    .await
    .unwrap();
    let reloaded = ProxyConfig::load_prepared(path).await.unwrap();
    assert!(!reloaded.config.web.conveyor);
    assert_eq!(reloaded.config.web.fallback_dns.origins.len(), 1);
    assert!(reloaded.config.web.runtime.is_some());
}

#[tokio::test]
async fn fallback_dns_api_patch_drops_removed_fallback_table() {
    // The [web.vhosts.fallback] table is gone: a patch still carrying it is
    // dropped by serde and the flattened upstream wins.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::write(&path, SOURCE).unwrap();
    let base = ProxyConfig::parse_source(&path).unwrap();
    let mut vhost = serde_json::to_value(&base.config.web.vhost).unwrap();
    vhost["fallback"] = serde_json::json!({
        "mode": "http_upstream",
        "upstream": "http://localhost:18082"
    });
    apply_patch_to_path(&path, &serde_json::json!({"web": {"vhost": vhost}}), None)
        .await
        .unwrap();
    let reloaded = ProxyConfig::parse_source(&path).unwrap();
    assert_eq!(
        reloaded.config.web.vhost.as_ref().expect("web vhost").upstream,
        "http://localhost:18081"
    );
}
