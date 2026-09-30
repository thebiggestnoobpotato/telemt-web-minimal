use super::*;

#[test]
fn conntrack_inline_explicit_flag_is_false_when_omitted() {
    let cfg = load_config_from_temp_toml(
        r#"
        [general]
        [network]
        [server]
        [server.conntrack_control]
        [access]
        "#,
    );
    assert!(
        !cfg.server
            .conntrack_control
            .inline_conntrack_control_explicit
    );
}

#[test]
fn conntrack_inline_explicit_flag_is_true_when_present() {
    let cfg = load_config_from_temp_toml(
        r#"
        [general]
        [network]
        [server]
        [server.conntrack_control]
        inline_conntrack_control = true
        [access]
        "#,
    );
    assert!(
        cfg.server
            .conntrack_control
            .inline_conntrack_control_explicit
    );
}

#[test]
fn api_gray_action_parses_and_defaults_to_drop() {
    let cfg_default: ProxyConfig = toml::from_str(
        r#"
        [server]
        [general]
        [network]
        [access]
        "#,
    )
    .unwrap();
    assert_eq!(cfg_default.server.api.gray_action, ApiGrayAction::Drop);

    let cfg_api: ProxyConfig = toml::from_str(
        r#"
        [server]
        [general]
        [network]
        [access]
        [server.api]
        gray_action = "api"
        "#,
    )
    .unwrap();
    assert_eq!(cfg_api.server.api.gray_action, ApiGrayAction::Api);

    let cfg_200: ProxyConfig = toml::from_str(
        r#"
        [server]
        [general]
        [network]
        [access]
        [server.api]
        gray_action = "200"
        "#,
    )
    .unwrap();
    assert_eq!(cfg_200.server.api.gray_action, ApiGrayAction::Ok200);

    let cfg_drop: ProxyConfig = toml::from_str(
        r#"
        [server]
        [general]
        [network]
        [access]
        [server.api]
        gray_action = "drop"
        "#,
    )
    .unwrap();
    assert_eq!(cfg_drop.server.api.gray_action, ApiGrayAction::Drop);
}

#[test]
fn dc_overrides_allow_string_and_array() {
    let toml = r#"
        [dc_overrides]
        "201" = "149.154.175.50:443"
        "202" = ["149.154.167.51:443", "149.154.175.100:443"]
    "#;
    let cfg: ProxyConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.dc_overrides["201"], vec!["149.154.175.50:443"]);
    assert_eq!(
        cfg.dc_overrides["202"],
        vec!["149.154.167.51:443", "149.154.175.100:443"]
    );
}

#[test]
fn load_with_metadata_collects_include_files() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("telemt_load_metadata_{nonce}"));
    std::fs::create_dir_all(&dir).unwrap();
    let main_path = dir.join("config.toml");
    let include_path = dir.join("included.toml");

    std::fs::write(
        &include_path,
        r#"
            [access.users]
            user = "00000000000000000000000000000000"
        "#,
    )
    .unwrap();
    std::fs::write(
        &main_path,
        r#"
            include = "included.toml"

            [general]
            prefer_ipv6 = false
        "#,
    )
    .unwrap();

    let loaded = ProxyConfig::load_with_metadata(&main_path).unwrap();
    let main_normalized = normalize_config_path(&main_path);
    let include_normalized = normalize_config_path(&include_path);

    assert!(loaded.source_files.contains(&main_normalized));
    assert!(loaded.source_files.contains(&include_normalized));

    let _ = std::fs::remove_file(main_path);
    let _ = std::fs::remove_file(include_path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn dc_overrides_inject_dc203_default() {
    let toml = r#"
        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_dc_override_test.toml");
    std::fs::write(&path, toml).unwrap();
    let cfg = ProxyConfig::load(&path).unwrap();
    assert!(
        cfg.dc_overrides
            .get("203")
            .map(|v| v.contains(&"91.105.192.100:443".to_string()))
            .unwrap_or(false)
    );
    let _ = std::fs::remove_file(path);
}

