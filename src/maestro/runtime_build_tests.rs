use super::*;

fn test_listener(port: u16) -> crate::config::ListenerConfig {
    crate::config::ListenerConfig {
        ip: "127.0.0.1".parse().unwrap(),
        transport: crate::config::ListenerTransport::Web,
        port: Some(port),
        web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
        web_trusted_proxy_cidrs: vec!["127.0.0.1/32".parse().unwrap()],
    }
}

// WEB listeners require a vhost; the decoy target avoids every listener port.
fn test_vhost() -> crate::config::WebVhostConfig {
    serde_json::from_value(serde_json::json!({
        "host": "proxy.example.com",
        "public_addr": "203.0.113.10:443",
        "decoy": {
            "mode": "http_upstream",
            "upstream": "http://127.0.0.1:18090"
        },
        "profiles": []
    }))
    .unwrap()
}

fn web_config_with_fasttrack(mode: &str) -> ProxyConfig {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let config = format!(
        r#"
[access.users]
alice = "000102030405060708090a0b0c0d0e0f"

[[server.listeners]]
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
    std::fs::write(&path, config).unwrap();
    ProxyConfig::load(path).unwrap()
}

#[test]
fn process_socket_and_logging_changes_are_deferred() {
    let old = ProxyConfig::default();
    let mut new = old.clone();
    new.server.listen_backlog = new.server.listen_backlog.saturating_add(1);
    new.logging.destination = crate::config::LoggingDestination::File;

    let fields = deferred_process_fields(&old, &new).unwrap();
    assert!(fields.contains(&"server.listeners".to_string()));
    assert!(fields.contains(&"logging".to_string()));
}

#[test]
fn process_wide_connection_and_direct_buffer_envelopes_are_restart_only() {
    let old = ProxyConfig::default();
    let mut desired = old.clone();
    desired.server.max_connections = old.server.max_connections.saturating_add(1);
    desired.general.direct_relay_buffer_budget_max_bytes = old
        .general
        .direct_relay_buffer_budget_max_bytes
        .saturating_add(4 * 1024);

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec![
            "server.max_connections".to_string(),
            "general.direct_relay_buffer_budget_max_bytes".to_string(),
        ]
    );
    assert_eq!(
        resolved.effective.server.max_connections,
        old.server.max_connections
    );
    assert_eq!(
        resolved
            .effective
            .general
            .direct_relay_buffer_budget_max_bytes,
        old.general.direct_relay_buffer_budget_max_bytes
    );
    assert!(!resolved.runtime_changed);
}

#[test]
fn mixed_reload_retains_process_state_and_applies_runtime_state() {
    let old = ProxyConfig::default();
    let mut desired = old.clone();
    desired.server.listen_backlog = desired.server.listen_backlog.saturating_add(1);
    desired.web.carrier = crate::config::WebCarrier::Websocket;

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(resolved.effective.server.listen_backlog, old.server.listen_backlog);
    assert_eq!(resolved.effective.web.carrier, desired.web.carrier);
    assert!(resolved.runtime_changed);
    assert_eq!(
        resolved.deferred_process_fields,
        vec!["server.listeners".to_string()]
    );
}

#[test]
fn listener_web_policy_change_is_deferred_when_bind_identity_is_stable() {
    let mut old = ProxyConfig::default();
    old.server.listeners.push(crate::config::ListenerConfig {
        ip: "0.0.0.0".parse().unwrap(),
        transport: crate::config::ListenerTransport::Web,
        port: Some(443),
        web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
        web_trusted_proxy_cidrs: vec!["127.0.0.1/32".parse().unwrap()],
    });
    old.web.vhosts = vec![test_vhost()];
    // Active configurations are fully prepared; endpoint equality compares runtime snapshots.
    old.rebuild_runtime_user_auth().unwrap();
    old.rebuild_runtime_web().unwrap();
    let mut desired = old.clone();
    desired
        .server
        .listeners[0]
        .web_trusted_proxy_cidrs
        .push("10.0.0.0/8".parse().unwrap());

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["server.listeners".to_string()]
    );
    assert_eq!(
        resolved.effective.server.listeners[0].web_trusted_proxy_cidrs.len(),
        1
    );
    assert!(!resolved.runtime_changed);
}

#[test]
fn process_field_labels_are_stable_ordered_and_unique() {
    let old = ProxyConfig::default();
    let mut desired = old.clone();
    desired.server.listen_backlog = desired.server.listen_backlog.saturating_add(1);
    desired.server.api.enabled = !desired.server.api.enabled;
    desired.server.api.runtime_edge_events_capacity = desired
        .server
        .api
        .runtime_edge_events_capacity
        .saturating_add(1);
    desired.logging.destination = crate::config::LoggingDestination::File;

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec![
            "server.listeners".to_string(),
            "server.api.listen".to_string(),
            "server.api.runtime_edge_events_capacity".to_string(),
            "logging".to_string(),
        ]
    );
}

#[test]
fn runtime_only_change_does_not_require_process_rebind() {
    let old = ProxyConfig::default();
    let mut new = old.clone();
    new.web.carrier = crate::config::WebCarrier::Websocket;
    assert!(deferred_process_fields(&old, &new).unwrap().is_empty());
}

#[test]
fn web_allocation_limits_are_deferred_until_restart() {
    let mut old = ProxyConfig::default();
    old.rebuild_runtime_user_auth().unwrap();
    old.rebuild_runtime_web().unwrap();
    let mut desired = old.clone();
    desired.web.limits.max_sessions_global += 1;

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["web.limits".to_string()]
    );
    assert_eq!(
        resolved.effective.web.limits.max_sessions_global,
        old.web.limits.max_sessions_global
    );
    assert!(!resolved.runtime_changed);
}

#[test]
fn web_decoy_fasttrack_mode_is_deferred_without_runtime_publication() {
    let old = web_config_with_fasttrack("off");
    let desired = web_config_with_fasttrack("enforce");

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["web.decoy_fasttrack_mode".to_string()]
    );
    assert_eq!(
        resolved.effective.web.decoy_fasttrack_mode,
        old.web.decoy_fasttrack_mode
    );
    let effective_runtime = resolved.effective.web.runtime.as_ref().unwrap();
    let effective_vhost = &effective_runtime.vhosts["proxy.example.com"];
    assert_eq!(
        effective_vhost.decoy_fasttrack_mode,
        old.web.decoy_fasttrack_mode
    );
    assert!(!resolved.runtime_changed);
}

#[test]
fn base_path_change_is_runtime_owned_and_rebuilds_route_identity() {
    let old = web_config_with_fasttrack("off");
    let old_runtime = old.web.runtime.as_ref().unwrap();
    let old_capability = old_runtime.vhosts["proxy.example.com"].capabilities[0];
    let mut desired = old.clone();
    desired.web.vhosts[0].base_path = "MixedCase/path".to_string();

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert!(resolved.deferred_process_fields.is_empty());
    assert!(resolved.runtime_changed);
    assert_eq!(resolved.effective.web.vhosts[0].base_path, "MixedCase/path");
    let runtime = resolved.effective.web.runtime.as_ref().unwrap();
    let vhost = &runtime.vhosts["proxy.example.com"];
    assert_eq!(vhost.base, "/MixedCase/path/");
    assert_ne!(vhost.capabilities[0], old_capability);
    assert_eq!(vhost.capabilities[0], vhost.profiles[0].capability);
    assert_eq!(runtime.capabilities.as_ref(), vhost.capabilities.as_ref());
    assert!(!runtime.capabilities.contains(&old_capability));
}

#[test]
fn enabling_learning_is_deferred_when_retained_capacity_is_too_small() {
    let mut old = ProxyConfig::default();
    old.web.limits.max_carrier_learning_entries = 1;
    old.web.carriers = crate::config::WebCarriers::Disabled;
    old.web.carrier_learning = false;
    old.rebuild_runtime_user_auth().unwrap();
    old.rebuild_runtime_web().unwrap();
    let mut desired = old.clone();
    desired.web.limits.max_carrier_learning_entries = 3;
    desired.web.carriers = crate::config::WebCarriers::Enabled(vec![
        crate::config::WebCarrier::Websocket,
        crate::config::WebCarrier::Https,
    ]);
    desired.web.carrier_learning = true;

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["web.limits".to_string(), "web.carrier_learning".to_string()]
    );
    assert_eq!(
        resolved.effective.web.limits.max_carrier_learning_entries,
        1
    );
    assert!(resolved.effective.web.carrier_negotiation_enabled());
    assert!(!resolved.effective.web.carrier_learning);
}

#[test]
fn enabling_carriers_is_deferred_for_dormant_learning_with_small_capacity() {
    let mut old = ProxyConfig::default();
    old.web.limits.max_carrier_learning_entries = 1;
    old.web.carriers = crate::config::WebCarriers::Disabled;
    old.web.carrier_learning = true;
    old.rebuild_runtime_user_auth().unwrap();
    old.rebuild_runtime_web().unwrap();
    let mut desired = old.clone();
    desired.web.limits.max_carrier_learning_entries = 3;
    desired.web.carriers = crate::config::WebCarriers::Enabled(vec![
        crate::config::WebCarrier::Websocket,
        crate::config::WebCarrier::Https,
    ]);

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["web.limits".to_string(), "web.carriers".to_string()]
    );
    assert_eq!(
        resolved.effective.web.limits.max_carrier_learning_entries,
        1
    );
    assert!(!resolved.effective.web.carrier_negotiation_enabled());
    assert!(resolved.effective.web.carrier_learning);
}

#[test]
fn web_debug_prefix_dependent_on_new_capacity_is_deferred_with_limits() {
    let mut old = ProxyConfig::default();
    old.rebuild_runtime_user_auth().unwrap();
    old.rebuild_runtime_web().unwrap();
    let mut desired = old.clone();
    desired.web.limits.max_body_bytes = 4 * 1024 * 1024;
    desired.web.debug.body_prefix_bytes = 3 * 1024 * 1024;

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["web.limits".to_string(), "web.debug".to_string()]
    );
    assert_eq!(
        resolved.effective.web.debug.body_prefix_bytes,
        old.web.debug.body_prefix_bytes
    );
}

#[test]
fn endpoint_only_listener_move_is_deferred_to_process_restart() {
    let mut old = ProxyConfig::default();
    old.server.listeners = vec![test_listener(443)];
    old.web.vhosts = vec![test_vhost()];
    // Active configurations are fully prepared; endpoint equality compares runtime snapshots.
    old.rebuild_runtime_user_auth().unwrap();
    old.rebuild_runtime_web().unwrap();
    let mut desired = old.clone();
    desired.server.listeners[0].port = Some(8443);

    let resolved = resolve_reload_config(&old, &desired).unwrap();

    assert_eq!(
        resolved.deferred_process_fields,
        vec!["server.listeners".to_string()]
    );
    assert_eq!(resolved.effective.server.listeners[0].port, Some(443));
    assert!(!resolved.runtime_changed);
}


#[test]
fn deferred_listener_identity_cannot_create_an_effective_decoy_loop() {
    let mut old = ProxyConfig::default();
    old.server.listeners = vec![test_listener(18080)];
    old.server.listeners[0].transport = crate::config::ListenerTransport::Web;
    let mut desired = old.clone();
    desired.server.listeners[0].port = Some(18081);
    desired.server.listen_backlog = desired.server.listen_backlog.saturating_add(1);
    desired.web.vhosts = vec![
        serde_json::from_value(serde_json::json!({
            "host": "proxy.example",
            "public_addr": "203.0.113.10:443",
            "decoy": {
                "mode": "http_upstream",
                "upstream": "http://127.0.0.1:18080"
            },
            "profiles": []
        }))
        .unwrap(),
    ];

    assert!(desired.validate_web_decoy_listener_separation().is_ok());
    assert!(resolve_reload_config(&old, &desired).is_err());
}
