//! Statistics and replay protection

#![allow(dead_code)]

mod core_counters;
mod core_getters;
mod helpers;
mod replay;
pub mod telemetry;
mod users;
mod writer_counters;

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

#[allow(unused_imports)]
pub use self::replay::{ReplayChecker, ReplayStats};
use self::telemetry::TelemetryPolicy;
pub(crate) use self::users::UserConnectionObservation;

#[derive(Clone, Copy)]
enum RouteConnectionGauge {
    Direct,
}

#[must_use = "RouteConnectionLease must be kept alive to hold the connection gauge increment"]
pub struct RouteConnectionLease {
    stats: Arc<Stats>,
    gauge: RouteConnectionGauge,
    active: bool,
}

impl RouteConnectionLease {
    fn new(stats: Arc<Stats>, gauge: RouteConnectionGauge) -> Self {
        Self {
            stats,
            gauge,
            active: true,
        }
    }

    #[cfg(test)]
    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for RouteConnectionLease {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match self.gauge {
            RouteConnectionGauge::Direct => self.stats.decrement_current_connections_direct(),
        }
    }
}

// ============= Stats =============

#[derive(Default)]
pub struct Stats {
    connects_all: AtomicU64,
    connects_bad: AtomicU64,
    connects_bad_classes: DashMap<&'static str, AtomicU64>,
    handshake_failure_classes: DashMap<&'static str, AtomicU64>,
    current_connections_direct: AtomicU64,
    handshake_timeouts: AtomicU64,
    upstream_connect_attempt_total: AtomicU64,
    upstream_connect_success_total: AtomicU64,
    upstream_connect_fail_total: AtomicU64,
    upstream_connect_failfast_hard_error_total: AtomicU64,
    upstream_connect_attempts_bucket_1: AtomicU64,
    upstream_connect_attempts_bucket_2: AtomicU64,
    upstream_connect_attempts_bucket_3_4: AtomicU64,
    upstream_connect_attempts_bucket_gt_4: AtomicU64,
    upstream_connect_duration_success_bucket_le_100ms: AtomicU64,
    upstream_connect_duration_success_bucket_101_500ms: AtomicU64,
    upstream_connect_duration_success_bucket_501_1000ms: AtomicU64,
    upstream_connect_duration_success_bucket_gt_1000ms: AtomicU64,
    upstream_connect_duration_fail_bucket_le_100ms: AtomicU64,
    upstream_connect_duration_fail_bucket_101_500ms: AtomicU64,
    upstream_connect_duration_fail_bucket_501_1000ms: AtomicU64,
    upstream_connect_duration_fail_bucket_gt_1000ms: AtomicU64,
    // Buffer pool gauges
    buffer_pool_pooled_gauge: AtomicU64,
    buffer_pool_allocated_gauge: AtomicU64,
    buffer_pool_in_use_gauge: AtomicU64,
    buffer_pool_replaced_nonstandard_total: AtomicU64,
    session_drop_fallback_total: AtomicU64,
    telemetry_core_enabled: AtomicBool,
    telemetry_user_enabled: AtomicBool,
    cached_epoch_secs: AtomicU64,
    user_stats: DashMap<String, Arc<UserStats>>,
    user_stats_last_cleanup_epoch_secs: AtomicU64,
    start_time: parking_lot::RwLock<Option<Instant>>,
}

#[derive(Default)]
pub struct UserStats {
    pub connects: AtomicU64,
    pub curr_connects: AtomicU64,
    pub octets_from_client: AtomicU64,
    pub octets_to_client: AtomicU64,
    pub msgs_from_client: AtomicU64,
    pub msgs_to_client: AtomicU64,
    pub last_seen_epoch_secs: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        let stats = Self::default();
        stats.apply_telemetry_policy(TelemetryPolicy::default());
        stats.refresh_cached_epoch_secs();
        *stats.start_time.write() = Some(Instant::now());
        stats
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests/connection_lease_security_tests.rs"]
mod connection_lease_security_tests;

#[cfg(test)]
#[path = "tests/replay_checker_security_tests.rs"]
mod replay_checker_security_tests;
