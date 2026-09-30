use std::net::IpAddr;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};

use super::patch::{Patch, patch_field};
use crate::crypto::SecureRandom;

const MAX_USERNAME_LEN: usize = 64;

#[derive(Debug)]
pub(super) struct ApiFailure {
    pub(super) status: StatusCode,
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) allow: Option<&'static str>,
}

impl ApiFailure {
    pub(super) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            allow: None,
        }
    }

    pub(super) fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }

    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub(super) fn method_not_allowed(allow: &'static str) -> Self {
        Self {
            status: StatusCode::METHOD_NOT_ALLOWED,
            code: "method_not_allowed",
            message: "Unsupported HTTP method for this route".to_string(),
            allow: Some(allow),
        }
    }
}

#[derive(Serialize)]
pub(super) struct ErrorBody {
    pub(super) code: &'static str,
    pub(super) message: String,
}

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    pub(super) ok: bool,
    pub(super) error: ErrorBody,
    pub(super) request_id: u64,
}

#[derive(Serialize)]
pub(super) struct SuccessResponse<T> {
    pub(super) ok: bool,
    pub(super) data: T,
    pub(super) revision: String,
}

#[derive(Serialize)]
pub(super) struct HealthData {
    pub(super) status: &'static str,
    pub(super) read_only: bool,
}

#[derive(Serialize)]
pub(super) struct HealthReadyData {
    pub(super) ready: bool,
    pub(super) status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<&'static str>,
    pub(super) admission_open: bool,
    pub(super) healthy_upstreams: usize,
    pub(super) total_upstreams: usize,
}

#[derive(Serialize, Clone)]
pub(super) struct ClassCount {
    pub(super) class: String,
    pub(super) total: u64,
}

#[derive(Serialize)]
pub(super) struct SummaryData {
    pub(super) uptime_seconds: f64,
    pub(super) connections_total: u64,
    pub(super) connections_bad_total: u64,
    pub(super) connections_bad_by_class: Vec<ClassCount>,
    pub(super) handshake_failures_by_class: Vec<ClassCount>,
    pub(super) handshake_timeouts_total: u64,
    pub(super) configured_users: usize,
}

#[derive(Serialize, Clone)]
pub(super) struct ZeroCoreData {
    pub(super) uptime_seconds: f64,
    pub(super) connections_total: u64,
    pub(super) connections_bad_total: u64,
    pub(super) connections_bad_by_class: Vec<ClassCount>,
    pub(super) handshake_failures_by_class: Vec<ClassCount>,
    pub(super) handshake_timeouts_total: u64,
    pub(super) accept_permit_timeout_total: u64,
    pub(super) configured_users: usize,
    pub(super) telemetry_core_enabled: bool,
    pub(super) telemetry_user_enabled: bool,
}

#[derive(Serialize, Clone)]
pub(super) struct ZeroUpstreamData {
    pub(super) connect_attempt_total: u64,
    pub(super) connect_success_total: u64,
    pub(super) connect_fail_total: u64,
    pub(super) connect_failfast_hard_error_total: u64,
    pub(super) connect_attempts_bucket_1: u64,
    pub(super) connect_attempts_bucket_2: u64,
    pub(super) connect_attempts_bucket_3_4: u64,
    pub(super) connect_attempts_bucket_gt_4: u64,
    pub(super) connect_duration_success_bucket_le_100ms: u64,
    pub(super) connect_duration_success_bucket_101_500ms: u64,
    pub(super) connect_duration_success_bucket_501_1000ms: u64,
    pub(super) connect_duration_success_bucket_gt_1000ms: u64,
    pub(super) connect_duration_fail_bucket_le_100ms: u64,
    pub(super) connect_duration_fail_bucket_101_500ms: u64,
    pub(super) connect_duration_fail_bucket_501_1000ms: u64,
    pub(super) connect_duration_fail_bucket_gt_1000ms: u64,
}

#[derive(Serialize, Clone)]
pub(super) struct UpstreamDcStatus {
    pub(super) dc: i16,
    pub(super) latency_ema_ms: Option<f64>,
    pub(super) ip_preference: &'static str,
}

#[derive(Serialize, Clone)]
pub(super) struct UpstreamStatus {
    pub(super) upstream_id: usize,
    pub(super) route_kind: &'static str,
    pub(super) address: String,
    pub(super) weight: u16,
    pub(super) scopes: String,
    pub(super) healthy: bool,
    pub(super) fails: u32,
    pub(super) last_check_age_secs: u64,
    pub(super) effective_latency_ms: Option<f64>,
    pub(super) dc: Vec<UpstreamDcStatus>,
}

#[derive(Serialize, Clone)]
pub(super) struct UpstreamSummaryData {
    pub(super) configured_total: usize,
    pub(super) healthy_total: usize,
    pub(super) unhealthy_total: usize,
    pub(super) direct_total: usize,
    pub(super) socks4_total: usize,
    pub(super) socks5_total: usize,
}

#[derive(Serialize, Clone)]
pub(super) struct UpstreamsData {
    pub(super) enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<&'static str>,
    pub(super) generated_at_epoch_secs: u64,
    pub(super) zero: ZeroUpstreamData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) summary: Option<UpstreamSummaryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) upstreams: Option<Vec<UpstreamStatus>>,
}

#[derive(Serialize, Clone)]
pub(super) struct ZeroAllData {
    pub(super) generated_at_epoch_secs: u64,
    pub(super) core: ZeroCoreData,
    pub(super) upstream: ZeroUpstreamData,
}

// User-management request, response, and validation models.
mod users;
pub(super) use users::*;
