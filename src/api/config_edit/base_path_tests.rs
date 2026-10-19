use super::*;

fn web_config() -> &'static str {
    r#"
users:
  alice:
    secret: "000102030405060708090a0b0c0d0e0f"

listener:
  ip: "127.0.0.1"
  port: 18080
  web_client_ip_source: "x_forwarded_for"
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]

web:
  enabled: true
  secret_mode: "plain"
  host: "proxy.example.com"
  public_addr: "203.0.113.10:443"
  upstream: "http://127.0.0.1:18081"
"#
}

fn vhost_patch(base_path: &str) -> Json {
    serde_json::json!({
        "web": {
            "host": "proxy.example.com",
            "base_path": base_path,
            "public_addr": "203.0.113.10:443",
            "upstream": "http://127.0.0.1:18081"
        }
    })
}

#[tokio::test]
async fn config_api_applies_valid_base_path_and_preserves_source_on_invalid_patch() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.yaml");
    std::fs::write(&path, web_config()).unwrap();

    let response = apply_patch_to_path(&path, &vhost_patch("MixedCase/path"), None)
        .await
        .unwrap();
    assert!(response.changed.iter().any(|section| section == "web"));
    let desired = ProxyConfig::load(&path).unwrap();
    assert_eq!(desired.web.vhost.base_path, "MixedCase/path");
    assert_eq!(
        desired.web.runtime.as_ref().unwrap().vhosts["proxy.example.com"].base,
        "/MixedCase/path/"
    );

    let (managed, _revision) = read_managed_config(&path).await.unwrap();
    assert_eq!(managed["web"]["base_path"].as_str(), Some("MixedCase/path"));
    assert!(managed["web"].get("runtime").is_none());
    assert!(!managed.as_mapping().unwrap().contains_key("access"));

    let before_invalid = std::fs::read(&path).unwrap();
    let error = apply_patch_to_path(&path, &vhost_patch("/invalid"), None)
        .await
        .unwrap_err();
    assert_eq!(error.status, hyper::StatusCode::BAD_REQUEST);
    assert_eq!(std::fs::read(&path).unwrap(), before_invalid);
}
