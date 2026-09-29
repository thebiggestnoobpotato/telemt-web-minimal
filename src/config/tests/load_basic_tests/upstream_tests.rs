use super::*;

#[test]
fn invalid_user_ad_tag_reports_access_user_ad_tags_key() {
    let toml = r#"
        [censorship]
        tls_domain = "example.com"

        [access.users]
        alice = "00000000000000000000000000000000"

        [access.user_ad_tags]
        alice = "not_hex"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_invalid_user_ad_tag_message_test.toml");
    std::fs::write(&path, toml).unwrap();
    let cfg = ProxyConfig::load(&path).unwrap();
    let err = cfg.validate().unwrap_err().to_string();
    assert!(err.contains("access.user_ad_tags['alice'] must be exactly 32 hex characters"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn invalid_dns_override_is_rejected() {
    let toml = r#"
        [network]
        dns_overrides = ["example.com:443:2001:db8::10"]

        [censorship]
        tls_domain = "example.com"

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_invalid_dns_override_test.toml");
    std::fs::write(&path, toml).unwrap();
    let err = ProxyConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("must be bracketed"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn valid_dns_override_is_accepted() {
    let toml = r#"
        [network]
        dns_overrides = ["example.com:443:127.0.0.1", "example.net:443:[2001:db8::10]"]

        [censorship]
        tls_domain = "example.com"

        [access.users]
        user = "00000000000000000000000000000000"
    "#;
    let dir = std::env::temp_dir();
    let path = dir.join("telemt_valid_dns_override_test.toml");
    std::fs::write(&path, toml).unwrap();
    let cfg = ProxyConfig::load(&path).unwrap();
    assert_eq!(cfg.network.dns_overrides.len(), 2);
    let _ = std::fs::remove_file(path);
}
