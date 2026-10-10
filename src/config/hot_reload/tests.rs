use super::*;

fn sample_config() -> ProxyConfig {
    ProxyConfig::default()
}

fn write_reload_config(path: &Path, log_level: Option<&str>, listen_backlog: Option<u32>) {
    let mut config = String::from(
        r#"
                [access.users]
                user = "00000000000000000000000000000000"
            "#,
    );

    if log_level.is_some() {
        config.push_str("\n[logging]\n");
        if let Some(level) = log_level {
            config.push_str(&format!("log_level = \"{level}\"\n"));
        }
    }

    if let Some(backlog) = listen_backlog {
        config.push_str("\n[general]\n");
        config.push_str(&format!("listen_backlog = {backlog}\n"));
    }

    std::fs::write(path, config).unwrap();
}

fn write_web_reload_config(path: &Path, carriers: &str, carrier_learning: bool) {
    let config = format!(
        r#"
                [access.users]
                user = "00000000000000000000000000000000"

                [web]
                carriers = {carriers}
                carrier_learning = {carrier_learning}
            "#,
    );
    std::fs::write(path, config).unwrap();
}

fn write_web_fasttrack_reload_config(path: &Path, mode: &str, log_level: &str) {
    let config = format!(
        r#"
                [logging]
                log_level = "{log_level}"

                [access.users]
                alice = "000102030405060708090a0b0c0d0e0f"

                [listener]
                ip = "127.0.0.1"
                port = 18080
                transport = "web"
                web_client_ip_source = "x_forwarded_for"
                web_trusted_proxy_cidrs = ["127.0.0.1/32"]

                [web]
                enabled = true
                decoy_fasttrack_mode = "{mode}"

                [[web.vhosts]]
                host = "proxy.example.com"
                public_addr = "203.0.113.10:443"

                [web.vhosts.decoy]
                mode = "http_upstream"
                upstream = "http://127.0.0.1:18081"

                [[web.vhosts.profiles]]
                user = "alice"
                secret_mode = "plain"
            "#,
    );
    std::fs::write(path, config).unwrap();
}

fn temp_config_path(prefix: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{nonce}.toml"))
}

#[test]
fn overlay_applies_hot_and_preserves_non_hot() {
    let old = sample_config();
    let mut new = old.clone();
    new.general.direct_relay_copy_buf_c2s_bytes = old.general.direct_relay_copy_buf_c2s_bytes + 1;
    new.general.listen_backlog = old.general.listen_backlog.saturating_add(1);

    let applied = overlay_hot_fields(&old, &new);
    assert_eq!(
        applied.general.direct_relay_copy_buf_c2s_bytes,
        new.general.direct_relay_copy_buf_c2s_bytes
    );
    assert_eq!(applied.general.listen_backlog, old.general.listen_backlog);
}

#[test]
fn non_hot_only_change_does_not_change_hot_snapshot() {
    let old = sample_config();
    let mut new = old.clone();
    new.general.listen_backlog = old.general.listen_backlog.saturating_add(1);

    let applied = overlay_hot_fields(&old, &new);
    assert_eq!(
        HotFields::from_config(&old),
        HotFields::from_config(&applied)
    );
    assert_eq!(applied.general.listen_backlog, old.general.listen_backlog);
}

#[test]
fn web_debug_policy_is_hot_while_debug_capacity_is_process_owned() {
    let old = sample_config();
    let mut new = old.clone();
    new.web.debug.enabled = true;
    new.web.debug.sideband = true;
    new.web.debug.default_window_secs = 60;
    new.web.limits.debug_records_capacity += 1;

    let applied = overlay_hot_fields(&old, &new);
    assert!(applied.web.debug.enabled);
    assert!(applied.web.debug.sideband);
    assert_eq!(applied.web.debug.default_window_secs, 60);
    assert_eq!(
        applied.web.limits.debug_records_capacity,
        old.web.limits.debug_records_capacity
    );
    assert_ne!(
        HotFields::from_config(&old),
        HotFields::from_config(&applied)
    );
}

#[test]
fn decoy_fasttrack_mode_is_deferred_until_restart() {
    let old = sample_config();
    let mut new = old.clone();
    new.web.decoy_fasttrack_mode = crate::config::WebDecoyFastTrackMode::Enforce;

    let applied = overlay_hot_fields(&old, &new);

    assert_eq!(
        applied.web.decoy_fasttrack_mode,
        old.web.decoy_fasttrack_mode
    );
    assert_eq!(
        HotFields::from_config(&old),
        HotFields::from_config(&applied)
    );
}

#[test]
fn hot_overlay_defers_learning_that_requires_new_process_capacity() {
    let mut old = sample_config();
    old.web.limits.max_carrier_learning_entries = 1;
    old.web.carriers = crate::config::WebCarriers::Disabled;
    old.web.carrier_learning = false;
    let mut new = old.clone();
    new.web.limits.max_carrier_learning_entries = 3;
    new.web.carriers = crate::config::WebCarriers::Enabled(vec![
        crate::config::WebCarrier::Websocket,
        crate::config::WebCarrier::Https,
    ]);
    new.web.carrier_learning = true;

    let applied = overlay_hot_fields(&old, &new);

    assert_eq!(applied.web.limits.max_carrier_learning_entries, 1);
    assert!(applied.web.carrier_negotiation_enabled());
    assert!(!applied.web.carrier_learning);
}

#[test]
fn hot_overlay_defers_carriers_for_dormant_learning_with_small_capacity() {
    let mut old = sample_config();
    old.web.limits.max_carrier_learning_entries = 1;
    old.web.carriers = crate::config::WebCarriers::Disabled;
    old.web.carrier_learning = true;
    let mut new = old.clone();
    new.web.limits.max_carrier_learning_entries = 3;
    new.web.carriers = crate::config::WebCarriers::Enabled(vec![
        crate::config::WebCarrier::Websocket,
        crate::config::WebCarrier::Https,
    ]);

    let applied = overlay_hot_fields(&old, &new);

    assert_eq!(applied.web.limits.max_carrier_learning_entries, 1);
    assert!(!applied.web.carrier_negotiation_enabled());
    assert!(applied.web.carrier_learning);
}

#[test]
fn web_debug_prefix_requiring_deferred_capacity_is_not_hot_applied() {
    let old = sample_config();
    let mut new = old.clone();
    new.web.limits.max_body_bytes = 4 * 1024 * 1024;
    new.web.debug.body_prefix_bytes = 3 * 1024 * 1024;

    let applied = overlay_hot_fields(&old, &new);
    assert_eq!(
        applied.web.limits.max_body_bytes,
        old.web.limits.max_body_bytes
    );
    assert_eq!(
        applied.web.debug.body_prefix_bytes,
        old.web.debug.body_prefix_bytes
    );
}

#[test]
fn mixed_hot_and_non_hot_change_applies_only_hot_subset() {
    let old = sample_config();
    let mut new = old.clone();
    new.general.direct_relay_copy_buf_s2c_bytes = old.general.direct_relay_copy_buf_s2c_bytes + 1;
    new.general.listen_backlog = old.general.listen_backlog.saturating_add(1);

    let applied = overlay_hot_fields(&old, &new);
    assert_eq!(
        applied.general.direct_relay_copy_buf_s2c_bytes,
        new.general.direct_relay_copy_buf_s2c_bytes
    );
    assert_eq!(applied.general.listen_backlog, old.general.listen_backlog);
    assert!(!config_equal(&applied, &new));
}

#[test]
fn listener_web_policy_fields_are_process_owned() {
    let mut old = sample_config();
    old.listener = Some(ListenerConfig {
        ip: Some("0.0.0.0".parse().unwrap()),
        transport: crate::config::ListenerTransport::Web,
        port: Some(443),
        socket_path: None,
        socket_perm: None,
        web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
        web_trusted_proxy_cidrs: Vec::new(),
    });
    let mut new = old.clone();
    new.listener.as_mut().unwrap().port = Some(8443);
    new.listener
        .as_mut()
        .unwrap()
        .web_trusted_proxy_cidrs
        .push("127.0.0.1/32".parse().unwrap());

    let applied = overlay_hot_fields(&old, &new);
    let listener = applied.listener.as_ref().unwrap();
    assert_eq!(listener.port, old.listener.as_ref().unwrap().port);
    assert!(listener.web_trusted_proxy_cidrs.is_empty());
    assert!(classify_config_changes(&old, &new).restart_required);
}

#[test]
fn reload_applies_hot_change_on_first_observed_snapshot() {
    let path = temp_config_path("telemt_hot_reload_stable");

    write_reload_config(&path, Some("normal"), None);
    let initial_cfg = Arc::new(ProxyConfig::load(&path).unwrap());
    let initial_hash = ProxyConfig::load_with_metadata(&path)
        .unwrap()
        .rendered_hash;
    let (config_tx, _config_rx) = watch::channel(initial_cfg.clone());
    let (log_tx, _log_rx) = watch::channel(initial_cfg.logging.log_level.clone());
    let mut reload_state = ReloadState::new(Some(initial_hash));

    write_reload_config(&path, Some("silent"), None);
    reload_config(&path, &config_tx, &log_tx, &mut reload_state).unwrap();
    assert_eq!(
        config_tx.borrow().logging.log_level,
        LogLevel::Silent
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn candidate_watcher_waits_for_activation_and_reconciles_disk() {
    let path = temp_config_path("telemt_hot_reload_activation_gate");
    write_reload_config(&path, Some("normal"), None);
    let initial = Arc::new(ProxyConfig::load(&path).unwrap());
    write_reload_config(&path, Some("debug"), None);
    let cancellation = tokio_util::sync::CancellationToken::new();
    let (activation_tx, activation_rx) = watch::channel(false);
    let (mut config_rx, _log_rx, watcher) = spawn_config_watcher(
        path.clone(),
        initial,
        cancellation.clone(),
        Some(activation_rx),
    );
    let watcher = tokio::spawn(watcher);

    tokio::task::yield_now().await;
    assert_eq!(
        config_rx.borrow().logging.log_level,
        LogLevel::Normal
    );
    activation_tx.send_replace(true);
    tokio::time::timeout(Duration::from_secs(2), config_rx.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        config_rx.borrow_and_update().logging.log_level,
        LogLevel::Debug
    );

    cancellation.cancel();
    watcher.await.unwrap();
    let _ = std::fs::remove_file(path);
}

#[test]
fn reload_keeps_hot_apply_when_non_hot_fields_change() {
    let path = temp_config_path("telemt_hot_reload_mixed");

    write_reload_config(&path, Some("normal"), None);
    let initial_cfg = Arc::new(ProxyConfig::load(&path).unwrap());
    let initial_hash = ProxyConfig::load_with_metadata(&path)
        .unwrap()
        .rendered_hash;
    let (config_tx, _config_rx) = watch::channel(initial_cfg.clone());
    let (log_tx, _log_rx) = watch::channel(initial_cfg.logging.log_level.clone());
    let mut reload_state = ReloadState::new(Some(initial_hash));

    write_reload_config(&path, Some("verbose"), Some(initial_cfg.general.listen_backlog + 1));
    reload_config(&path, &config_tx, &log_tx, &mut reload_state).unwrap();

    let applied = config_tx.borrow().clone();
    assert_eq!(applied.logging.log_level, LogLevel::Verbose);
    assert_eq!(applied.general.listen_backlog, initial_cfg.general.listen_backlog);

    let _ = std::fs::remove_file(path);
}

#[test]
fn reload_rebuilds_vhosts_with_the_effective_fasttrack_mode() {
    let path = temp_config_path("telemt_web_fasttrack_reload");

    write_web_fasttrack_reload_config(&path, "off", "normal");
    let initial_cfg = Arc::new(ProxyConfig::load(&path).unwrap());
    let initial_hash = ProxyConfig::load_with_metadata(&path)
        .unwrap()
        .rendered_hash;
    let (config_tx, _config_rx) = watch::channel(Arc::clone(&initial_cfg));
    let (log_tx, _log_rx) = watch::channel(initial_cfg.logging.log_level.clone());
    let mut reload_state = ReloadState::new(Some(initial_hash));

    write_web_fasttrack_reload_config(&path, "enforce", "silent");
    reload_config(&path, &config_tx, &log_tx, &mut reload_state).unwrap();

    let applied = config_tx.borrow().clone();
    assert_eq!(applied.logging.log_level, LogLevel::Silent);
    assert_eq!(
        applied.web.decoy_fasttrack_mode,
        crate::config::WebDecoyFastTrackMode::Off
    );
    let runtime = applied.web.runtime.as_ref().unwrap();
    assert_eq!(
        runtime.vhosts["proxy.example.com"].decoy_fasttrack_mode,
        crate::config::WebDecoyFastTrackMode::Off
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn reload_publishes_web_negotiation_policy_outside_hot_field_reporting() {
    let path = temp_config_path("telemt_web_negotiation_reload");

    write_web_reload_config(&path, "false", true);
    let initial_cfg = Arc::new(ProxyConfig::load(&path).unwrap());
    let initial_hash = ProxyConfig::load_with_metadata(&path)
        .unwrap()
        .rendered_hash;
    let (config_tx, _config_rx) = watch::channel(initial_cfg.clone());
    let (log_tx, _log_rx) = watch::channel(initial_cfg.logging.log_level.clone());
    let mut reload_state = ReloadState::new(Some(initial_hash));

    write_web_reload_config(&path, "[\"websocket\", \"https\"]", false);
    reload_config(&path, &config_tx, &log_tx, &mut reload_state).unwrap();

    let applied = config_tx.borrow().clone();
    assert!(applied.web.carrier_negotiation_enabled());
    assert!(!applied.web.carrier_learning);

    let _ = std::fs::remove_file(path);
}

#[test]
fn classify_timeouts_change_requires_restart() {
    // timeouts.* is NOT in overlay_hot_fields -> restart.
    let old = ProxyConfig::default();
    let mut new = ProxyConfig::default();
    new.timeouts.client_handshake = old.timeouts.client_handshake + 1;

    let class = classify_config_changes(&old, &new);
    assert!(class.restart_required);
}

#[test]
fn reload_recovers_after_parse_error_on_next_attempt() {
    let path = temp_config_path("telemt_hot_reload_parse_recovery");

    write_reload_config(&path, Some("normal"), None);
    let initial_cfg = Arc::new(ProxyConfig::load(&path).unwrap());
    let initial_hash = ProxyConfig::load_with_metadata(&path)
        .unwrap()
        .rendered_hash;
    let (config_tx, _config_rx) = watch::channel(initial_cfg.clone());
    let (log_tx, _log_rx) = watch::channel(initial_cfg.logging.log_level.clone());
    let mut reload_state = ReloadState::new(Some(initial_hash));

    std::fs::write(&path, "[access.users\nuser = \"broken\"\n").unwrap();
    assert!(reload_config(&path, &config_tx, &log_tx, &mut reload_state).is_none());
    assert_eq!(
        config_tx.borrow().logging.log_level,
        LogLevel::Normal
    );

    write_reload_config(&path, Some("debug"), None);
    reload_config(&path, &config_tx, &log_tx, &mut reload_state).unwrap();
    assert_eq!(
        config_tx.borrow().logging.log_level,
        LogLevel::Debug
    );

    let _ = std::fs::remove_file(path);
}
