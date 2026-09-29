use super::*;
use http_body_util::BodyExt;
use std::net::IpAddr;

use crate::stats::telemetry::TelemetryPolicy;

const CAS_CONTENTION_SERIES: [(&str, &str, &str, u64); 8] = [
    ("user", "up", "reserve", 1),
    ("user", "down", "reserve", 2),
    ("user", "up", "refund", 3),
    ("user", "down", "refund", 4),
    ("cidr", "up", "reserve", 5),
    ("cidr", "down", "reserve", 6),
    ("cidr", "up", "refund", 7),
    ("cidr", "down", "refund", 8),
];

fn test_web_publication() -> crate::web::control::WebRuntimePublication {
    let control = crate::web::control::WebRuntimeControl::new();
    control.subscribe().borrow().clone()
}

#[tokio::test]
async fn test_render_metrics_format() {
    let stats = Arc::new(Stats::new());
    let shared_state = ProxySharedState::new();
    let tracker = UserIpTracker::new();
    let mut config = ProxyConfig::default();
    shared_state
        .traffic_limiter
        .set_cas_contention_metrics_for_test([1, 2, 3, 4, 5, 6, 7, 8]);
    config
        .access
        .user_max_unique_ips
        .insert("alice".to_string(), 4);

    stats.increment_connects_all();
    stats.increment_connects_all();
    stats.increment_connects_bad_with_class("tls_handshake_bad_client");
    stats.increment_handshake_timeouts();
    stats.increment_handshake_failure_class("timeout");
    stats.increment_conntrack_rule_reconcile_success_total();
    stats.increment_conntrack_rule_reconcile_error_total();
    stats.increment_conntrack_rule_rollback_success_total();
    stats.increment_conntrack_rule_rollback_error_total();
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
    let _connection_permit = stats
        .connection_authority()
        .try_acquire("alice", None)
        .unwrap();
    stats.add_user_octets_from("alice", 1024);
    stats.add_user_octets_to("alice", 2048);
    stats.increment_user_msgs_from("alice");
    stats.increment_user_msgs_to("alice");
    stats.increment_user_msgs_to("alice");
    tracker
        .check_and_add("alice", "203.0.113.10".parse().unwrap())
        .await
        .unwrap();

    let output = render_metrics(
        &stats,
        shared_state.as_ref(),
        &config,
        &tracker,
        &test_web_publication(),
    )
    .await;

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
    assert!(output.contains("telemt_conntrack_rule_reconcile_total{result=\"success\"} 1"));
    assert!(output.contains("telemt_conntrack_rule_reconcile_total{result=\"error\"} 1"));
    assert!(output.contains("telemt_conntrack_rule_rollback_total{result=\"success\"} 1"));
    assert!(output.contains("telemt_conntrack_rule_rollback_total{result=\"error\"} 1"));
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
    assert!(output.contains("telemt_user_unique_ips_current{user=\"alice\"} 1"));
    assert!(output.contains("telemt_user_unique_ips_recent_window{user=\"alice\"} 1"));
    assert!(output.contains("telemt_user_unique_ips_limit{user=\"alice\"} 4"));
    assert!(output.contains("telemt_user_unique_ips_utilization{user=\"alice\"} 0.250000"));
    assert!(output.contains("telemt_ip_tracker_users{scope=\"active\"} 1"));
    assert!(output.contains("telemt_ip_tracker_entries{scope=\"active\"} 1"));
    assert!(output.contains("telemt_ip_tracker_cleanup_queue_len 0"));
    for (scope, direction, operation, value) in CAS_CONTENTION_SERIES {
        assert!(output.contains(&format!(
            "telemt_rate_limiter_cas_retry_exhausted_total{{scope=\"{scope}\",direction=\"{direction}\",operation=\"{operation}\"}} {value}"
        )));
    }
    assert_eq!(
        output
            .matches("telemt_rate_limiter_cas_retry_exhausted_total{")
            .count(),
        8
    );
}

#[tokio::test]
async fn test_render_empty_stats() {
    let stats = Stats::new();
    let shared_state = ProxySharedState::new();
    stats.apply_telemetry_policy(TelemetryPolicy {
        core_enabled: false,
        ..TelemetryPolicy::default()
    });
    shared_state
        .traffic_limiter
        .set_cas_contention_metrics_for_test([1, 2, 3, 4, 5, 6, 7, 8]);
    let tracker = UserIpTracker::new();
    let config = ProxyConfig::default();
    let output = render_metrics(
        &stats,
        &shared_state,
        &config,
        &tracker,
        &test_web_publication(),
    )
    .await;
    assert!(output.contains("telemt_connections_total 0"));
    assert!(output.contains("telemt_connections_bad_total 0"));
    assert!(output.contains("telemt_handshake_timeouts_total 0"));
    assert!(output.contains("telemt_auth_expensive_checks_total 0"));
    assert!(output.contains("telemt_auth_budget_exhausted_total 0"));
    assert!(output.contains("telemt_user_unique_ips_current{user="));
    assert!(output.contains("telemt_user_unique_ips_recent_window{user="));
    for (scope, direction, operation, _) in CAS_CONTENTION_SERIES {
        assert!(output.contains(&format!(
            "telemt_rate_limiter_cas_retry_exhausted_total{{scope=\"{scope}\",direction=\"{direction}\",operation=\"{operation}\"}} 0"
        )));
    }
}

#[tokio::test]
async fn test_render_uses_global_each_unique_ip_limit() {
    let stats = Stats::new();
    let shared_state = ProxySharedState::new();
    stats.increment_user_connects("alice");
    stats.increment_user_curr_connects("alice");
    let tracker = UserIpTracker::new();
    tracker
        .check_and_add("alice", "203.0.113.10".parse().unwrap())
        .await
        .unwrap();
    let mut config = ProxyConfig::default();
    config.access.user_max_unique_ips_global_each = 2;

    let output = render_metrics(
        &stats,
        &shared_state,
        &config,
        &tracker,
        &test_web_publication(),
    )
    .await;

    assert!(output.contains("telemt_user_unique_ips_limit{user=\"alice\"} 2"));
    assert!(output.contains("telemt_user_unique_ips_utilization{user=\"alice\"} 0.500000"));
}

#[tokio::test]
async fn test_render_has_type_annotations() {
    let stats = Stats::new();
    let shared_state = ProxySharedState::new();
    let tracker = UserIpTracker::new();
    let config = ProxyConfig::default();
    let output = render_metrics(
        &stats,
        &shared_state,
        &config,
        &tracker,
        &test_web_publication(),
    )
    .await;
    assert!(output.contains("# TYPE telemt_uptime_seconds gauge"));
    assert!(output.contains("# TYPE telemt_connections_total counter"));
    assert!(output.contains("# TYPE telemt_connections_bad_total counter"));
    assert!(output.contains("# TYPE telemt_connections_bad_by_class_total counter"));
    assert!(output.contains("# TYPE telemt_handshake_timeouts_total counter"));
    assert!(output.contains("# TYPE telemt_handshake_failures_by_class_total counter"));
    assert!(output.contains("# TYPE telemt_auth_expensive_checks_total counter"));
    assert!(output.contains("# TYPE telemt_auth_budget_exhausted_total counter"));
    assert!(output.contains("# TYPE telemt_upstream_connect_attempt_total counter"));
    assert!(output.contains("# TYPE telemt_user_unique_ips_current gauge"));
    assert!(output.contains("# TYPE telemt_user_unique_ips_recent_window gauge"));
    assert!(output.contains("# TYPE telemt_user_unique_ips_limit gauge"));
    assert!(output.contains("# TYPE telemt_user_unique_ips_utilization gauge"));
    assert!(output.contains("# TYPE telemt_stats_user_entries gauge"));
    assert!(output.contains("# TYPE telemt_telemetry_user_series_users gauge"));
    assert!(output.contains("# TYPE telemt_ip_tracker_users gauge"));
    assert!(output.contains("# TYPE telemt_ip_tracker_entries gauge"));
    assert!(output.contains("# TYPE telemt_ip_tracker_cleanup_queue_len gauge"));
    assert!(output.contains("# TYPE telemt_ip_tracker_cleanup_total counter"));
    assert!(output.contains("# TYPE telemt_ip_tracker_cap_rejects_total counter"));
}

#[tokio::test]
async fn test_endpoint_integration() {
    let mut config = ProxyConfig::default();
    config.general.beobachten = true;
    config.general.beobachten_minutes = 10;
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

    runtime.beobachten.record(
        "TLS-scanner",
        "203.0.113.10".parse::<IpAddr>().unwrap(),
        Duration::from_secs(600),
    );
    let req_beob = Request::builder().uri("/beobachten").body(()).unwrap();
    let resp_beob = handle(req_beob, &runtime, &web_publication).await.unwrap();
    assert_eq!(resp_beob.status(), StatusCode::OK);
    let body_beob = resp_beob.into_body().collect().await.unwrap().to_bytes();
    let beob_text = std::str::from_utf8(body_beob.as_ref()).unwrap();
    assert!(beob_text.contains("[TLS-scanner]"));
    assert!(beob_text.contains("203.0.113.10-1"));

    let req404 = Request::builder().uri("/other").body(()).unwrap();
    let resp404 = handle(req404, &runtime, &web_publication).await.unwrap();
    assert_eq!(resp404.status(), StatusCode::NOT_FOUND);
}
