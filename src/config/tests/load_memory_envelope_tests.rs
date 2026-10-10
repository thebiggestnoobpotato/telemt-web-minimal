use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn write_temp_config(contents: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("telemt-load-memory-envelope-{nonce}.toml"));
    fs::write(&path, contents).expect("temp config write must succeed");
    path
}

fn remove_temp_config(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn load_rejects_max_client_frame_above_upper_bound() {
    let path = write_temp_config(
        r#"
[general]
max_client_frame = 16777217
"#,
    );

    let err = ProxyConfig::load(&path).expect_err("max_client_frame above hard cap must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("general.max_client_frame must be within [4096, 16777216]"),
        "error must explain max_client_frame hard cap, got: {msg}"
    );

    remove_temp_config(&path);
}

#[test]
fn load_rejects_unaligned_dc_buffer_budget() {
    let path = write_temp_config(
        r#"
[general]
dc_buffer_budget_max_bytes = 16777217
"#,
    );

    let err = ProxyConfig::load(&path).expect_err("unaligned direct relay buffer budget must fail");
    assert!(
        err.to_string().contains(
            "general.dc_buffer_budget_max_bytes must be 0 or a multiple of 4096"
        )
    );
    remove_temp_config(&path);
}

#[test]
fn load_rejects_dc_buffer_budget_above_hard_cap() {
    let path = write_temp_config(
        r#"
[general]
dc_buffer_budget_max_bytes = 2147487744
"#,
    );

    let err =
        ProxyConfig::load(&path).expect_err("direct relay buffer budget above hard cap must fail");
    assert!(err.to_string().contains(
        "general.dc_buffer_budget_max_bytes must be 0 or within [16777216, 2147483648]"
    ));
    remove_temp_config(&path);
}

#[test]
fn load_rejects_listen_backlog_above_i32_upper_bound() {
    let path = write_temp_config(
        r#"
[general]
listen_backlog = 2147483648
"#,
    );

    let err = ProxyConfig::load(&path).expect_err("listen_backlog above socket cap must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("general.listen_backlog must be within [1, 2147483647]"),
        "error must explain listen_backlog hard cap, got: {msg}"
    );

    remove_temp_config(&path);
}

#[test]
fn load_rejects_zero_listen_backlog() {
    let path = write_temp_config(
        r#"
[general]
listen_backlog = 0
"#,
    );

    let err = ProxyConfig::load(&path).expect_err("zero listen_backlog must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("general.listen_backlog must be within [1, 2147483647]"),
        "error must explain listen_backlog lower bound, got: {msg}"
    );

    remove_temp_config(&path);
}

#[test]
fn load_accepts_memory_limits_at_hard_upper_bounds() {
    let path = write_temp_config(
        r#"
[general]
dc_buffer_budget_max_bytes = 2147483648
max_client_frame = 16777216
"#,
    );

    let cfg = ProxyConfig::load(&path).expect("hard upper bound values must be accepted");
    assert_eq!(
        cfg.general.dc_buffer_budget_max_bytes,
        2 * 1024 * 1024 * 1024
    );
    assert_eq!(cfg.general.max_client_frame, 16 * 1024 * 1024);

    remove_temp_config(&path);
}
