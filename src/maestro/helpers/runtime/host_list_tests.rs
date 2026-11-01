use super::*;

#[test]
fn grouped_startup_links_include_every_alias_and_selected_profile() {
    const CHILD: &str = "TELEMT_F1_LINK_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        std::fs::write(
            &path,
            r#"
users:
  alice:
    secret: "000102030405060708090a0b0c0d0e0f"
  bob:
    secret: "101112131415161718191a1b1c1d1e1f"

listener:
  ip: "127.0.0.1"
  port: 18080
  web_client_ip_source: "x_forwarded_for"
  web_trusted_proxy_cidrs: ["127.0.0.1/32"]

web:
  secret_mode: "dd"
  hosts: ["a.example.com", "b.example.com"]
  base_path: "MixedCase/path"
  public_addr: "203.0.113.10:443"
  upstream: "http://127.0.0.1:18081"
"#,
        )
        .unwrap();
        let config = ProxyConfig::load(&path).unwrap();
        let options = crate::logging::LoggingOptions {
            destination: crate::logging::LogDestination::Stderr,
            ..Default::default()
        };
        let (_handle, _guard) =
            crate::logging::init_logging(&options, "warn,telemt::links=info");
        print_web_proxy_links(&config);
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "maestro::helpers::runtime::host_list_tests::grouped_startup_links_include_every_alias_and_selected_profile",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    // The fork emits links through tracing; the Stderr destination keeps
    // tracing-subscriber's default writer, which is stdout.
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("WEB: tg://webproxy?").count(), 4, "{stdout}");
    for host in ["a.example.com", "b.example.com"] {
        for (user, secret) in [
            ("alice", "000102030405060708090a0b0c0d0e0f"),
            ("bob", "101112131415161718191a1b1c1d1e1f"),
        ] {
            let link = crate::web::links::format_web_proxy_link(
                host,
                "MixedCase/path",
                secret,
                crate::config::WebSecretMode::Dd,
            )
            .unwrap();
            assert_eq!(stdout.matches(&link).count(), 1, "{user} {host}: {stdout}");
        }
    }
}
