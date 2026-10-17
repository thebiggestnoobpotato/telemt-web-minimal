use super::*;

#[test]
fn api_minimal_runtime_cache_ttl_out_of_range_is_rejected() {
    let yaml = r#"
api:
  enabled: true
  listen_address: "127.0.0.1:9091"
  minimal_runtime_cache_ttl_ms: 70000

general:
  network_ipv6: true

users:
  user:
    secret: "00000000000000000000000000000000"
"#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_api_minimal_runtime_cache_ttl_invalid_test.yaml");
    std::fs::write(&path, yaml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("api.minimal_runtime_cache_ttl_ms must be within [0, 60000]"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn api_runtime_edge_cache_ttl_out_of_range_is_rejected() {
    let yaml = r#"
api:
  enabled: true
  listen_address: "127.0.0.1:9091"
  runtime_edge_cache_ttl_ms: 70000

general:
  network_ipv6: true

users:
  user:
    secret: "00000000000000000000000000000000"
"#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_api_runtime_edge_cache_ttl_invalid_test.yaml");
    std::fs::write(&path, yaml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("api.runtime_edge_cache_ttl_ms must be within [0, 60000]"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn api_runtime_edge_top_n_out_of_range_is_rejected() {
    let yaml = r#"
api:
  enabled: true
  listen_address: "127.0.0.1:9091"
  runtime_edge_top_n: 0

general:
  network_ipv6: true

users:
  user:
    secret: "00000000000000000000000000000000"
"#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_api_runtime_edge_top_n_invalid_test.yaml");
    std::fs::write(&path, yaml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("api.runtime_edge_top_n must be within [1, 1000]"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn api_runtime_edge_events_capacity_out_of_range_is_rejected() {
    let yaml = r#"
api:
  enabled: true
  listen_address: "127.0.0.1:9091"
  runtime_edge_events_capacity: 8

general:
  network_ipv6: true

users:
  user:
    secret: "00000000000000000000000000000000"
"#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_api_runtime_edge_events_capacity_invalid_test.yaml");
    std::fs::write(&path, yaml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("api.runtime_edge_events_capacity must be within [16, 4096]"));
    let _ = std::fs::remove_file(path);
}
