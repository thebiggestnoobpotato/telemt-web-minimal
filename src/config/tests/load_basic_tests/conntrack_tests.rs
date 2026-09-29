use super::*;

#[test]
fn conntrack_pressure_high_watermark_out_of_range_is_rejected() {
    let toml = r#"
        [server.conntrack_control]
        pressure_high_watermark_pct = 0

        [general]
        prefer_ipv6 = false

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_conntrack_high_watermark_invalid_test.toml");
    std::fs::write(&path, toml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(
        err.contains(
            "server.conntrack_control.pressure_high_watermark_pct must be within [1, 100]"
        )
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn conntrack_pressure_low_watermark_must_be_below_high() {
    let toml = r#"
        [server.conntrack_control]
        pressure_high_watermark_pct = 50
        pressure_low_watermark_pct = 50

        [general]
        prefer_ipv6 = false

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_conntrack_low_watermark_invalid_test.toml");
    std::fs::write(&path, toml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains(
        "server.conntrack_control.pressure_low_watermark_pct must be < pressure_high_watermark_pct"
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn conntrack_delete_budget_zero_is_rejected() {
    let toml = r#"
        [server.conntrack_control]
        delete_budget_per_sec = 0

        [general]
        prefer_ipv6 = false

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_conntrack_delete_budget_invalid_test.toml");
    std::fs::write(&path, toml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("server.conntrack_control.delete_budget_per_sec must be > 0"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn conntrack_hybrid_mode_requires_listener_allow_list() {
    let toml = r#"
        [server.conntrack_control]
        mode = "hybrid"

        [general]
        prefer_ipv6 = false

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_conntrack_hybrid_requires_ips_test.toml");
    std::fs::write(&path, toml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(
        err.contains(
            "server.conntrack_control.hybrid_listener_ips must be non-empty in mode=hybrid"
        )
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn conntrack_profile_is_loaded_from_config() {
    let toml = r#"
        [server.conntrack_control]
        profile = "aggressive"

        [general]
        prefer_ipv6 = false

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_conntrack_profile_parse_test.toml");
    std::fs::write(&path, toml).unwrap();
    let cfg = ProxyConfig::load(&path).unwrap();
    assert_eq!(
        cfg.server.conntrack_control.profile,
        ConntrackPressureProfile::Aggressive
    );
    let _ = std::fs::remove_file(path);
}
