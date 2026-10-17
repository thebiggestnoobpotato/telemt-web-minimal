use super::*;

#[tokio::test]
async fn carrier_method_api_defaults_and_patches_are_persisted() {
    let (path, _directory) = temp_config("web:\n");
    let (value, _) = read_managed_config(&path).await.unwrap();
    assert_eq!(value["web"]["carrier_method"].as_str(), Some("post"));
    for token in ["put", "post"] {
        let patch = serde_json::json!({"web": {"carrier_method": token}});
        let response = apply_patch_to_path(&path, &patch, None).await.unwrap();
        assert!(response.changed.iter().any(|section| section == "web"));
        let (value, revision) = read_managed_config(&path).await.unwrap();
        assert_eq!(value["web"]["carrier_method"].as_str(), Some(token));
        assert_eq!(revision, response.revision);
        let written = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(written.contains(&format!("carrier_method: {token}")));
    }
}

#[tokio::test]
async fn carrier_method_api_rejects_invalid_values_without_writing() {
    let (path, _directory) = temp_config("web:\n  carrier_method: \"put\"\n");
    let original = tokio::fs::read(&path).await.unwrap();
    let revision = crate::api::config_store::current_revision(&path)
        .await
        .unwrap();
    for value in [
        serde_json::json!("PUT"),
        serde_json::json!("patch"),
        serde_json::json!(true),
        serde_json::json!(42),
    ] {
        let patch = serde_json::json!({"web": {"carrier_method": value}});
        let error = apply_patch_to_path(&path, &patch, None).await.unwrap_err();
        assert_eq!(error.status, hyper::StatusCode::BAD_REQUEST);
        assert_eq!(tokio::fs::read(&path).await.unwrap(), original);
        assert_eq!(
            crate::api::config_store::current_revision(&path)
                .await
                .unwrap(),
            revision
        );
    }
}

use std::path::PathBuf;

fn temp_config(body: &str) -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, body).unwrap();
    (path, dir)
}

#[test]
fn json_object_converts_to_yaml_mapping() {
    let j: Json = serde_json::json!({"general": {"network_ipv6": true}, "sample": 2});
    let t = json_to_yaml(&j).expect("convertible");
    let table = t.as_mapping().unwrap();
    assert_eq!(table["general"]["network_ipv6"].as_bool(), Some(true));
    assert_eq!(table["sample"].as_i64(), Some(2));
}

#[test]
fn deep_merge_overlays_tables_and_replaces_scalars() {
    let mut base: Yaml = serde_yaml::from_str(
        "general:\n  network_ipv6: false\n  config_strict: false\n",
    )
    .unwrap();
    let patch: Yaml = serde_yaml::from_str("general:\n  network_ipv6: true\n").unwrap();

    deep_merge(&mut base, &patch);

    let general = base["general"].as_mapping().unwrap();
    assert_eq!(general["network_ipv6"].as_bool(), Some(true));
    assert_eq!(general["config_strict"].as_bool(), Some(false));
}

#[tokio::test]
async fn patch_rejects_access_section() {
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"access": {"users": {"x": "y"}}});
    let err = apply_patch_to_path(&path, &patch, None).await.unwrap_err();
    assert_eq!(err.code, "section_not_editable");
}

#[tokio::test]
async fn patch_revision_conflict() {
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"general": {"network_ipv6": true}});
    let err = apply_patch_to_path(&path, &patch, Some("deadbeef".into()))
        .await
        .unwrap_err();
    assert_eq!(err.code, "revision_conflict");
}

#[tokio::test]
async fn patch_logging_show_users_links_is_persisted() {
    // logging.show_users_links is process-owned: links are emitted once at
    // listener bind time, so the change applies at the next process start.
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"logging": {"show_users_links": false}});
    let resp = apply_patch_to_path(&path, &patch, None).await.unwrap();
    assert!(resp.changed.iter().any(|c| c == "logging"));
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("show_users_links: false"));
    assert_eq!(
        resp.revision,
        crate::api::config_store::current_revision(&path)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn read_managed_config_strips_access() {
    let (path, _d) = temp_config(
        "general:\n  network_ipv6: true\n\
         users:\n  bob: \"00000000000000000000000000000000\"\n",
    );
    let (value, revision) = read_managed_config(&path).await.unwrap();
    let table = value.as_mapping().unwrap();
    assert!(table.contains_key("general"));
    // Secrets never leave the box through this endpoint.
    assert!(!table.contains_key("access"));
    assert_eq!(
        revision,
        crate::api::config_store::current_revision(&path)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn read_managed_config_exposes_web_without_runtime_or_access_secrets() {
    let (path, _directory) = temp_config(concat!(
        "web:\n  carrier: \"https\"\n  debug:\n    enabled: true\n    default_window_secs: 180\n",
        "users:\n  bob: \"00000000000000000000000000000000\"\n",
    ));

    let (value, _revision) = read_managed_config(&path).await.unwrap();
    let table = value.as_mapping().unwrap();

    assert!(table.contains_key("web"));
    assert!(table["web"].get("debug").is_some());
    assert!(table["web"].get("runtime").is_none());
    assert!(!table.contains_key("access"));
}

#[tokio::test]
async fn patch_web_debug_and_limits_are_persisted() {
    let (path, _directory) = temp_config("web:\n");
    let debug_patch: Json = serde_json::json!({
        "web": {"debug": {
            "enabled": true,
            "sideband": true,
            "capture_headers": false
        }}
    });
    let debug = apply_patch_to_path(&path, &debug_patch, None).await.unwrap();
    assert!(debug.changed.iter().any(|section| section == "web"));
    let desired = ProxyConfig::load(&path).unwrap();
    assert!(desired.web.debug.sideband);

    let limits_patch: Json = serde_json::json!({
        // Extra HTTP heads must fit alongside the default-on conveyor metadata reservation.
        "web": {"limits": {"max_http_connections": 2049, "memory_envelope_bytes": 1610612736u64}}
    });
    let limits = apply_patch_to_path(&path, &limits_patch, None).await.unwrap();
    assert!(limits.changed.iter().any(|section| section == "web"));
    let desired = ProxyConfig::load(&path).unwrap();
    assert_eq!(desired.web.limits.max_http_connections, 2049);
}

#[tokio::test]
async fn patch_web_fallback_fasttrack_is_persisted() {
    let (path, _directory) = temp_config("web:\n");
    let patch: Json = serde_json::json!({
        "web": {"fallback_fasttrack_mode": "shadow"}
    });
    let response = apply_patch_to_path(&path, &patch, None).await.unwrap();
    assert!(response.changed.iter().any(|c| c == "web"));
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("fallback_fasttrack_mode: shadow"));
}

#[tokio::test]
async fn invalid_web_patch_does_not_modify_the_source() {
    let (path, _directory) = temp_config("web:\n");
    let original = tokio::fs::read_to_string(&path).await.unwrap();
    let patch: Json = serde_json::json!({
        "web": {"debug": {"default_window_secs": 181, "max_window_secs": 180}}
    });

    let error = apply_patch_to_path(&path, &patch, None).await.unwrap_err();

    assert_eq!(error.status, hyper::StatusCode::BAD_REQUEST);
    assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), original);
}

#[tokio::test]
async fn read_managed_config_returns_only_editable_sections() {
    // The top-level api/metrics tables must not leak.
    let (path, _d) = temp_config(concat!(
        "general:\n  network_ipv6: true\n",
        "api:\n  auth_header: \"SECRET\"\n",
        "listener:\n  ip: \"0.0.0.0\"\n  port: 443\n  web_trusted_proxy_cidrs: [\"127.0.0.1/32\"]\n",
        "web:\n  secret_mode: \"plain\"\n  vhost:\n    host: \"proxy.example.com\"\n    public_addr: \"203.0.113.1:443\"\n    upstream: \"http://127.0.0.1:80\"\n    profiles:\n      - user: \"bob\"\n",
        "metrics:\n  enabled: true\n  listen_address: \"127.0.0.1:9090\"\n",
        "users:\n  bob: \"00000000000000000000000000000000\"\n",
    ));
    let (value, _rev) = read_managed_config(&path).await.unwrap();
    let table = value.as_mapping().unwrap();
    assert!(table.contains_key("general"));
    assert!(table.contains_key("listener"));
    assert!(!table.contains_key("api"));
    assert!(!table.contains_key("metrics"));
    assert!(!table.contains_key("access"));
}

#[tokio::test]
async fn read_managed_config_returns_full_listener_table() {
    let (path, _d) = temp_config(concat!(
        "general:\n  network_ipv6: true\n",
        "api:\n  auth_header: \"SECRET\"\n",
        "listener:\n  ip: \"0.0.0.0\"\n  port: 443\n  web_trusted_proxy_cidrs: [\"127.0.0.1/32\"]\n",
        "web:\n  secret_mode: \"plain\"\n  vhost:\n    host: \"proxy.example.com\"\n    public_addr: \"203.0.113.1:443\"\n    upstream: \"http://127.0.0.1:80\"\n    profiles:\n      - user: \"bob\"\n",
        "users:\n  bob: \"00000000000000000000000000000000\"\n",
    ));
    let (value, _rev) = read_managed_config(&path).await.unwrap();
    let table = value.as_mapping().unwrap();
    let listener = table
        .get("listener")
        .expect("listener present")
        .as_mapping()
        .unwrap();
    assert!(listener.contains_key("ip"));
    assert!(listener.contains_key("port"));
    assert!(!table.contains_key("api"));
    assert_eq!(listener["port"].as_i64(), Some(443));
}

#[tokio::test]
async fn patch_rejects_stripped_server_section() {
    // `server` no longer exists; any patch under it is rejected wholesale.
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"server": {"listeners": []}});
    let err = apply_patch_to_path(&path, &patch, None).await.unwrap_err();
    assert_eq!(err.code, "section_not_editable");
}

#[tokio::test]
async fn patch_rejects_api_section() {
    // `api` is process-owned endpoint identity and stays outside the
    // editable section allowlist.
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"api": {"enabled": false}});
    let err = apply_patch_to_path(&path, &patch, None).await.unwrap_err();
    assert_eq!(err.code, "section_not_editable");
}

#[tokio::test]
async fn patch_listener_preserves_api() {
    let (path, _d) = temp_config(concat!(
        "general:\n  network_ipv6: true\n",
        "api:\n  enabled: true\n  auth_header: \"SECRET\"\n",
        "listener:\n  ip: \"0.0.0.0\"\n  port: 443\n  web_trusted_proxy_cidrs: [\"127.0.0.1/32\"]\n",
        "web:\n  secret_mode: \"plain\"\n  vhost:\n    host: \"proxy.example.com\"\n    public_addr: \"203.0.113.1:443\"\n    upstream: \"http://127.0.0.1:80\"\n    profiles:\n      - user: \"bob\"\n",
        "users:\n  bob: \"00000000000000000000000000000000\"\n",
    ));
    let patch: Json = serde_json::json!({
        "listener": {
            "ip": "0.0.0.0",
            "port": 8443,
            "web_trusted_proxy_cidrs": ["10.0.0.0/8"]
        }
    });
    let resp = apply_patch_to_path(&path, &patch, None).await.unwrap();
    assert!(resp.changed.iter().any(|c| c == "listener"));
    let written = tokio::fs::read_to_string(&path).await.unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&written).unwrap();
    assert_eq!(
        parsed["api"]["auth_header"].as_str(),
        Some("SECRET"),
        "{written}"
    );
    let listener = parsed["listener"].as_mapping().unwrap();
    assert_eq!(listener["port"].as_i64(), Some(8443), "{written}");
    assert_eq!(
        listener["web_trusted_proxy_cidrs"][0].as_str(),
        Some("10.0.0.0/8"),
        "{written}"
    );
}

#[tokio::test]
async fn patch_rejects_non_editable_top_level_section() {
    // Top-level sections outside the editable allowlist are rejected wholesale;
    // the removed `network` section is one such key.
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"network": {"ipv4": false}});
    let err = apply_patch_to_path(&path, &patch, None).await.unwrap_err();
    assert_eq!(err.code, "section_not_editable");
}

#[tokio::test]
async fn patch_logging_show_users_links_is_editable() {
    // The supported path: edit show_users_links via the logging section.
    let (path, _d) = temp_config(
        "general:\n  network_ipv6: true\n\
         logging:\n  show_users_links: true\n",
    );
    let patch: Json = serde_json::json!({"logging": {"show_users_links": false}});
    let resp = apply_patch_to_path(&path, &patch, None).await.unwrap();
    assert!(resp.changed.iter().any(|c| c == "logging"));
    let written = tokio::fs::read_to_string(&path).await.unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&written).unwrap();
    assert_eq!(
        parsed["logging"]["show_users_links"].as_bool(),
        Some(false),
        "{written}"
    );
    // No duplicate logging sections.
    assert_eq!(
        written.lines().filter(|line| *line == "logging:").count(),
        1,
        "{written}"
    );
}

#[tokio::test]
async fn patch_writes_the_included_section_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    let included = dir.path().join("general.yaml");
    let root_body = "include: \"general.yaml\"\nother:\n  value: 443\n";
    let included_body = "general:\n  network_ipv6: true\n";
    tokio::fs::write(&root, root_body).await.unwrap();
    tokio::fs::write(&included, included_body).await.unwrap();
    let patch: Json = serde_json::json!({
        "general": {"network_ipv6": true}
    });

    let response = apply_patch_to_path(&root, &patch, None).await.unwrap();

    assert_eq!(tokio::fs::read_to_string(&root).await.unwrap(), root_body);
    let written = tokio::fs::read_to_string(&included).await.unwrap();
    assert!(written.contains("network_ipv6: true"));
    assert_eq!(
        response.revision,
        crate::api::config_store::current_revision(&root)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn prepared_patch_rejects_external_edit_before_commit() {
    let (path, _directory) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({
        "general": {"network_ipv6": true}
    });
    let prepared = prepare_patch_to_path(&path, &patch, None).await.unwrap();
    let external = "general:\n  network_ipv6: false\n";
    tokio::fs::write(&path, external).await.unwrap();

    let error = write_atomic_if_unchanged(
        prepared.config_path,
        prepared.expected_revision,
        prepared.owner_path,
        prepared.expected_owner_contents,
        prepared.owner_contents,
    )
    .await
    .unwrap_err();

    assert_eq!(error.code, "revision_conflict");
    assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), external);
}

#[tokio::test]
async fn patch_rejects_multiple_source_owners_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    let included = dir.path().join("web.yaml");
    let root_body = concat!(
        "include: \"web.yaml\"\n",
        "general:\n  network_ipv6: true\n"
    );
    let included_body = "web:\n";
    tokio::fs::write(&root, root_body).await.unwrap();
    tokio::fs::write(&included, included_body).await.unwrap();
    let patch: Json = serde_json::json!({
        "general": {"network_ipv6": true},
        "web": {"carrier_method": "post"}
    });

    let error = apply_patch_to_path(&root, &patch, None).await.unwrap_err();

    assert_eq!(error.code, "config_patch_not_atomic");
    assert_eq!(tokio::fs::read_to_string(&root).await.unwrap(), root_body);
    assert_eq!(
        tokio::fs::read_to_string(&included).await.unwrap(),
        included_body
    );
}

#[tokio::test]
async fn patch_empty_is_rejected() {
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({});
    assert!(apply_patch_to_path(&path, &patch, None).await.is_err());
}

#[tokio::test]
async fn patch_log_level_is_persisted() {
    // Default LogLevel is Normal; patch to "debug" and verify the persisted
    // source reflects the change.
    let (path, _d) = temp_config("general:\n  network_ipv6: true\n");
    let patch: Json = serde_json::json!({"logging": {"log_level": "debug"}});
    let resp = apply_patch_to_path(&path, &patch, None).await.unwrap();
    assert!(resp.changed.iter().any(|c| c == "logging"));
    let written = tokio::fs::read_to_string(&path).await.unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&written).unwrap();
    assert_eq!(parsed["logging"]["log_level"].as_str(), Some("debug"));
}
