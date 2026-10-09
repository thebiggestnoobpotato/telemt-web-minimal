use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::ApiConfig;
use crate::stats::Stats;
use crate::transport::UpstreamRouteKind;
use crate::transport::upstream::IpPreference;

use super::ApiShared;
use super::model::{
    ClassCount, UpstreamDcStatus, UpstreamStatus, UpstreamSummaryData, UpstreamsData, ZeroAllData,
    ZeroCoreData, ZeroUpstreamData,
};

const FEATURE_DISABLED_REASON: &str = "feature_disabled";
const SOURCE_UNAVAILABLE_REASON: &str = "source_unavailable";

pub(super) fn build_zero_all_data(stats: &Stats, configured_users: usize) -> ZeroAllData {
    let telemetry = stats.telemetry_policy();
    let bad_connection_classes = stats
        .get_connects_bad_class_counts()
        .into_iter()
        .map(|(class, total)| ClassCount { class, total })
        .collect();
    let handshake_failure_classes = stats
        .get_handshake_failure_class_counts()
        .into_iter()
        .map(|(class, total)| ClassCount { class, total })
        .collect();

    ZeroAllData {
        generated_at_epoch_secs: now_epoch_secs(),
        core: ZeroCoreData {
            uptime_seconds: stats.uptime_secs(),
            connections_total: stats.get_connects_all(),
            connections_bad_total: stats.get_connects_bad(),
            connections_bad_by_class: bad_connection_classes,
            handshake_failures_by_class: handshake_failure_classes,
            handshake_timeouts_total: stats.get_handshake_timeouts(),
            configured_users,
            telemetry_core_enabled: telemetry.core_enabled,
            telemetry_user_enabled: telemetry.user_enabled,
        },
        upstream: build_zero_upstream_data(stats),
    }
}

fn build_zero_upstream_data(stats: &Stats) -> ZeroUpstreamData {
    ZeroUpstreamData {
        connect_attempt_total: stats.get_upstream_connect_attempt_total(),
        connect_success_total: stats.get_upstream_connect_success_total(),
        connect_fail_total: stats.get_upstream_connect_fail_total(),
        connect_failfast_hard_error_total: stats.get_upstream_connect_failfast_hard_error_total(),
        connect_attempts_bucket_1: stats.get_upstream_connect_attempts_bucket_1(),
        connect_attempts_bucket_2: stats.get_upstream_connect_attempts_bucket_2(),
        connect_attempts_bucket_3_4: stats.get_upstream_connect_attempts_bucket_3_4(),
        connect_attempts_bucket_gt_4: stats.get_upstream_connect_attempts_bucket_gt_4(),
        connect_duration_success_bucket_le_100ms: stats
            .get_upstream_connect_duration_success_bucket_le_100ms(),
        connect_duration_success_bucket_101_500ms: stats
            .get_upstream_connect_duration_success_bucket_101_500ms(),
        connect_duration_success_bucket_501_1000ms: stats
            .get_upstream_connect_duration_success_bucket_501_1000ms(),
        connect_duration_success_bucket_gt_1000ms: stats
            .get_upstream_connect_duration_success_bucket_gt_1000ms(),
        connect_duration_fail_bucket_le_100ms: stats
            .get_upstream_connect_duration_fail_bucket_le_100ms(),
        connect_duration_fail_bucket_101_500ms: stats
            .get_upstream_connect_duration_fail_bucket_101_500ms(),
        connect_duration_fail_bucket_501_1000ms: stats
            .get_upstream_connect_duration_fail_bucket_501_1000ms(),
        connect_duration_fail_bucket_gt_1000ms: stats
            .get_upstream_connect_duration_fail_bucket_gt_1000ms(),
    }
}

pub(super) fn build_upstreams_data(shared: &ApiShared, api_cfg: &ApiConfig) -> UpstreamsData {
    let generated_at_epoch_secs = now_epoch_secs();
    let zero = build_zero_upstream_data(&shared.stats);
    if !api_cfg.minimal_runtime_enabled {
        return UpstreamsData {
            enabled: false,
            reason: Some(FEATURE_DISABLED_REASON),
            generated_at_epoch_secs,
            zero,
            summary: None,
            upstreams: None,
        };
    }

    let Some(snapshot) = shared.upstream_manager.try_api_snapshot() else {
        return UpstreamsData {
            enabled: true,
            reason: Some(SOURCE_UNAVAILABLE_REASON),
            generated_at_epoch_secs,
            zero,
            summary: None,
            upstreams: None,
        };
    };

    let summary = UpstreamSummaryData {
        configured_total: snapshot.summary.configured_total,
        healthy_total: snapshot.summary.healthy_total,
        unhealthy_total: snapshot.summary.unhealthy_total,
        direct_total: snapshot.summary.direct_total,
        socks5_total: snapshot.summary.socks5_total,
    };
    let upstreams = snapshot
        .upstreams
        .into_iter()
        .map(|upstream| UpstreamStatus {
            upstream_id: upstream.upstream_id,
            route_kind: map_route_kind(upstream.route_kind),
            address: upstream.address,
            weight: upstream.weight,
            scopes: upstream.scopes,
            healthy: upstream.healthy,
            fails: upstream.fails,
            last_check_age_secs: upstream.last_check_age_secs,
            effective_latency_ms: upstream.effective_latency_ms,
            dc: upstream
                .dc
                .into_iter()
                .map(|dc| UpstreamDcStatus {
                    dc: dc.dc,
                    latency_ema_ms: dc.latency_ema_ms,
                    ip_preference: map_ip_preference(dc.ip_preference),
                })
                .collect(),
        })
        .collect();

    UpstreamsData {
        enabled: true,
        reason: None,
        generated_at_epoch_secs,
        zero,
        summary: Some(summary),
        upstreams: Some(upstreams),
    }
}

// Disabled-state builders and stable upstream enum mappings.
mod helpers;
use helpers::*;
