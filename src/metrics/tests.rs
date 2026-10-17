use super::*;
use http_body_util::BodyExt;

use crate::stats::telemetry::TelemetryPolicy;

fn test_web_publication() -> crate::web::control::WebRuntimePublication {
    let control = crate::web::control::WebRuntimeControl::new();
    control.subscribe().borrow().clone()
}

#[test]
fn test_render_metrics_format() {
    let stats = Arc::new(Stats::new());
    let shared_state = ProxySharedState::new();
    let config = ProxyConfig::default();

    stats.increment_connects_all();
    stats.increment_connects_all();
    stats.increment_connects_bad_with_class("tls_handshake_bad_client");
    stats.increment_handshake_timeouts();
    stats.increment_handshake_failure_class("timeout");
    shared_state
        .handshake
        .auth_expensive_checks_total
        .fetch_add(9, std::sync::atomic::Ordering::Relaxed);
    shared_state
        .handshake
        .auth_budget_exhausted_total
        .fetch_add(2, std::sync::atomic::Ordering::Relaxed);
    stats.increment_upstream_connect_attempt_total();
    stats.increment_upstream_connect_attempt_total();
    stats.increment_upstream_connect_success_total();
    stats.increment_upstream_connect_fail_total();
    stats.increment_upstream_connect_failfast_hard_error_total();
    stats.observe_upstream_connect_attempts_per_request(2);
    stats.observe_upstream_connect_duration_ms(220, true);
    stats.observe_upstream_connect_duration_ms(1500, false);
    stats.increment_user_connects("alice");
    stats.increment_user_curr_connects("alice");
    stats.add_user_octets_from("alice", 1024);
    stats.add_user_octets_to("alice", 2048);
    stats.increment_user_msgs_from("alice");
    stats.increment_user_msgs_to("alice");
    stats.increment_user_msgs_to("alice");

    let output = render_metrics(
        &stats,
        shared_state.as_ref(),
        &config,
        &test_web_publication(),
    );

    assert!(output.contains(&format!(
        "telemt_build_info{{version=\"{}\"}} 1",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(output.contains("telemt_connections_total 2"));
    assert!(output.contains("telemt_connections_bad_total 1"));
    assert!(
        output.contains(
            "telemt_connections_bad_by_class_total{class=\"tls_handshake_bad_client\"} 1"
        )
    );
    assert!(output.contains("telemt_handshake_timeouts_total 1"));
    assert!(output.contains("telemt_handshake_failures_by_class_total{class=\"timeout\"} 1"));
    assert!(output.contains("telemt_auth_expensive_checks_total 9"));
    assert!(output.contains("telemt_auth_budget_exhausted_total 2"));
    assert!(output.contains("telemt_upstream_connect_attempt_total 2"));
    assert!(output.contains("telemt_upstream_connect_success_total 1"));
    assert!(output.contains("telemt_upstream_connect_fail_total 1"));
    assert!(output.contains("telemt_upstream_connect_failfast_hard_error_total 1"));
    assert!(output.contains("telemt_upstream_connect_attempts_per_request{bucket=\"2\"} 1"));
    assert!(
        output.contains("telemt_upstream_connect_duration_success_total{bucket=\"101_500ms\"} 1")
    );
    assert!(output.contains("telemt_upstream_connect_duration_fail_total{bucket=\"gt_1000ms\"} 1"));
    assert!(output.contains("telemt_user_connections_total{user=\"alice\"} 1"));
    assert!(output.contains("telemt_user_connections_current{user=\"alice\"} 1"));
    assert!(output.contains("telemt_user_octets_from_client_total{user=\"alice\"} 1024"));
    assert!(output.contains("telemt_user_octets_to_client_total{user=\"alice\"} 2048"));
    assert!(output.contains("telemt_user_msgs_from_client_total{user=\"alice\"} 1"));
    assert!(output.contains("telemt_user_msgs_to_client_total{user=\"alice\"} 2"));
}

#[test]
fn test_render_empty_stats() {
    let stats = Stats::new();
    let shared_state = ProxySharedState::new();
    stats.apply_telemetry_policy(TelemetryPolicy {
        core_enabled: false,
        ..TelemetryPolicy::default()
    });
    let mut config = ProxyConfig::default();
    config.users.insert(
        "default".to_string(),
        "00000000000000000000000000000000".to_string().into(),
    );
    let output = render_metrics(&stats, &shared_state, &config, &test_web_publication());
    assert!(output.contains("telemt_connections_total 0"));
    assert!(output.contains("telemt_connections_bad_total 0"));
    assert!(output.contains("telemt_handshake_timeouts_total 0"));
    assert!(output.contains("telemt_auth_expensive_checks_total 0"));
    assert!(output.contains("telemt_auth_budget_exhausted_total 0"));
}

#[test]
fn test_render_has_type_annotations() {
    let stats = Stats::new();
    let shared_state = ProxySharedState::new();
    let config = ProxyConfig::default();
    let output = render_metrics(&stats, &shared_state, &config, &test_web_publication());
    assert!(output.contains("# TYPE telemt_uptime_seconds gauge"));
    assert!(output.contains("# TYPE telemt_connections_total counter"));
    assert!(output.contains("# TYPE telemt_connections_bad_total counter"));
    assert!(output.contains("# TYPE telemt_connections_bad_by_class_total counter"));
    assert!(output.contains("# TYPE telemt_handshake_timeouts_total counter"));
    assert!(output.contains("# TYPE telemt_handshake_failures_by_class_total counter"));
    assert!(output.contains("# TYPE telemt_auth_expensive_checks_total counter"));
    assert!(output.contains("# TYPE telemt_auth_budget_exhausted_total counter"));
    assert!(output.contains("# TYPE telemt_upstream_connect_attempt_total counter"));
    assert!(output.contains("# TYPE telemt_stats_user_entries gauge"));
    assert!(output.contains("# TYPE telemt_telemetry_user_series_users gauge"));
}

#[tokio::test]
async fn test_endpoint_integration() {
    let config = ProxyConfig::default();
    let runtime = crate::maestro::generation::test_runtime_generation(1, config);
    let web_publication = test_web_publication();
    runtime.stats.increment_connects_all();
    runtime.stats.increment_connects_all();
    runtime.stats.increment_connects_all();

    let req = Request::builder().uri("/metrics").body(()).unwrap();
    let resp = handle(req, &runtime, &web_publication).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(
        std::str::from_utf8(body.as_ref())
            .unwrap()
            .contains("telemt_connections_total 3")
    );
    assert!(
        std::str::from_utf8(body.as_ref())
            .unwrap()
            .contains(&format!(
                "telemt_build_info{{version=\"{}\"}} 1",
                env!("CARGO_PKG_VERSION")
            ))
    );

    let req404 = Request::builder().uri("/other").body(()).unwrap();
    let resp404 = handle(req404, &runtime, &web_publication).await.unwrap();
    assert_eq!(resp404.status(), StatusCode::NOT_FOUND);
}
