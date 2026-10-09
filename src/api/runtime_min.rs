use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::ProxyConfig;

use super::ApiShared;

const SOURCE_UNAVAILABLE_REASON: &str = "source_unavailable";

#[derive(Serialize)]
pub(super) struct SecurityWhitelistData {
    pub(super) generated_at_epoch_secs: u64,
    pub(super) enabled: bool,
    pub(super) entries_total: usize,
    pub(super) entries: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct RuntimeUpstreamQualityPolicyData {
    pub(super) connect_retry_attempts: u32,
    pub(super) connect_retry_backoff_ms: u64,
    pub(super) connect_budget_ms: u64,
    pub(super) unhealthy_fail_threshold: u32,
    pub(super) connect_failfast_hard_errors: bool,
}

#[derive(Serialize)]
pub(super) struct RuntimeUpstreamQualityCountersData {
    pub(super) connect_attempt_total: u64,
    pub(super) connect_success_total: u64,
    pub(super) connect_fail_total: u64,
    pub(super) connect_failfast_hard_error_total: u64,
}

#[derive(Serialize)]
pub(super) struct RuntimeUpstreamQualitySummaryData {
    pub(super) configured_total: usize,
    pub(super) healthy_total: usize,
    pub(super) unhealthy_total: usize,
    pub(super) direct_total: usize,
    pub(super) socks5_total: usize,
}

#[derive(Serialize)]
pub(super) struct RuntimeUpstreamQualityDcData {
    pub(super) dc: i16,
    pub(super) latency_ema_ms: Option<f64>,
    pub(super) ip_preference: &'static str,
}

#[derive(Serialize)]
pub(super) struct RuntimeUpstreamQualityUpstreamData {
    pub(super) upstream_id: usize,
    pub(super) route_kind: &'static str,
    pub(super) address: String,
    pub(super) weight: u16,
    pub(super) scopes: String,
    pub(super) healthy: bool,
    pub(super) fails: u32,
    pub(super) last_check_age_secs: u64,
    pub(super) effective_latency_ms: Option<f64>,
    pub(super) dc: Vec<RuntimeUpstreamQualityDcData>,
}

#[derive(Serialize)]
pub(super) struct RuntimeUpstreamQualityData {
    pub(super) enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<&'static str>,
    pub(super) generated_at_epoch_secs: u64,
    pub(super) policy: RuntimeUpstreamQualityPolicyData,
    pub(super) counters: RuntimeUpstreamQualityCountersData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) summary: Option<RuntimeUpstreamQualitySummaryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) upstreams: Option<Vec<RuntimeUpstreamQualityUpstreamData>>,
}

pub(super) fn build_security_whitelist_data(cfg: &ProxyConfig) -> SecurityWhitelistData {
    let entries = cfg
        .api
        .whitelist
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    SecurityWhitelistData {
        generated_at_epoch_secs: now_epoch_secs(),
        enabled: !entries.is_empty(),
        entries_total: entries.len(),
        entries,
    }
}

pub(super) async fn build_runtime_upstream_quality_data(
    shared: &ApiShared,
) -> RuntimeUpstreamQualityData {
    let generated_at_epoch_secs = now_epoch_secs();
    let policy = shared.upstream_manager.api_policy_snapshot();
    let counters = RuntimeUpstreamQualityCountersData {
        connect_attempt_total: shared.stats.get_upstream_connect_attempt_total(),
        connect_success_total: shared.stats.get_upstream_connect_success_total(),
        connect_fail_total: shared.stats.get_upstream_connect_fail_total(),
        connect_failfast_hard_error_total: shared
            .stats
            .get_upstream_connect_failfast_hard_error_total(),
    };

    let Some(snapshot) = shared.upstream_manager.try_api_snapshot() else {
        return RuntimeUpstreamQualityData {
            enabled: false,
            reason: Some(SOURCE_UNAVAILABLE_REASON),
            generated_at_epoch_secs,
            policy: RuntimeUpstreamQualityPolicyData {
                connect_retry_attempts: policy.connect_retry_attempts,
                connect_retry_backoff_ms: policy.connect_retry_backoff_ms,
                connect_budget_ms: policy.connect_budget_ms,
                unhealthy_fail_threshold: policy.unhealthy_fail_threshold,
                connect_failfast_hard_errors: policy.connect_failfast_hard_errors,
            },
            counters,
            summary: None,
            upstreams: None,
        };
    };

    RuntimeUpstreamQualityData {
        enabled: true,
        reason: None,
        generated_at_epoch_secs,
        policy: RuntimeUpstreamQualityPolicyData {
            connect_retry_attempts: policy.connect_retry_attempts,
            connect_retry_backoff_ms: policy.connect_retry_backoff_ms,
            connect_budget_ms: policy.connect_budget_ms,
            unhealthy_fail_threshold: policy.unhealthy_fail_threshold,
            connect_failfast_hard_errors: policy.connect_failfast_hard_errors,
        },
        counters,
        summary: Some(RuntimeUpstreamQualitySummaryData {
            configured_total: snapshot.summary.configured_total,
            healthy_total: snapshot.summary.healthy_total,
            unhealthy_total: snapshot.summary.unhealthy_total,
            direct_total: snapshot.summary.direct_total,
            socks5_total: snapshot.summary.socks5_total,

        }),
        upstreams: Some(
            snapshot
                .upstreams
                .into_iter()
                .map(|upstream| RuntimeUpstreamQualityUpstreamData {
                    upstream_id: upstream.upstream_id,
                    route_kind: match upstream.route_kind {
                        crate::transport::UpstreamRouteKind::Direct => "direct",
                        crate::transport::UpstreamRouteKind::Socks5 => "socks5",
                    },
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
                        .map(|dc| RuntimeUpstreamQualityDcData {
                            dc: dc.dc,
                            latency_ema_ms: dc.latency_ema_ms,
                            ip_preference: match dc.ip_preference {
                                crate::transport::upstream::IpPreference::Unknown => "unknown",
                                crate::transport::upstream::IpPreference::PreferV6 => "prefer_v6",
                                crate::transport::upstream::IpPreference::PreferV4 => "prefer_v4",
                                crate::transport::upstream::IpPreference::BothWork => "both_work",
                                crate::transport::upstream::IpPreference::Unavailable => {
                                    "unavailable"
                                }
                            },
                        })
                        .collect(),
                })
                .collect(),
        ),
    }
}

pub(super) fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
