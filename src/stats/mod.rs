//! Statistics and replay protection

#![allow(dead_code)]

mod core_counters;
mod core_getters;
mod helpers;
mod quota_store;
mod replay;
pub mod telemetry;
mod users;
mod writer_counters;

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

pub(crate) use self::quota_store::{QuotaReservation, QuotaStore, UserQuotaHandle};
#[allow(unused_imports)]
pub use self::replay::{ReplayChecker, ReplayStats};
use self::telemetry::TelemetryPolicy;
pub(crate) use self::users::UserConnectionObservation;
use crate::proxy::user_connection_authority::UserConnectionAuthority;

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
    ip_reservation_rollback_tcp_limit_total: AtomicU64,
    ip_reservation_rollback_quota_limit_total: AtomicU64,
    quota_refund_bytes_total: AtomicU64,
    quota_contention_total: AtomicU64,
    quota_contention_timeout_total: AtomicU64,
    quota_acquire_cancelled_total: AtomicU64,
    quota_write_fail_bytes_total: AtomicU64,
    quota_write_fail_events_total: AtomicU64,
    session_drop_fallback_total: AtomicU64,
    telemetry_core_enabled: AtomicBool,
    telemetry_user_enabled: AtomicBool,
    cached_epoch_secs: AtomicU64,
    user_stats: DashMap<String, Arc<UserStats>>,
    quota_store: Arc<QuotaStore>,
    connection_authority: Arc<UserConnectionAuthority>,
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
    quota: Arc<quota_store::UserQuotaCounters>,
    pub last_seen_epoch_secs: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct UserQuotaSnapshot {
    pub used_bytes: u64,
    pub last_reset_epoch_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaReserveError {
    LimitExceeded,
    Contended,
}

impl UserStats {
    fn with_quota(quota: Arc<quota_store::UserQuotaCounters>) -> Self {
        Self {
            quota,
            ..Self::default()
        }
    }

    #[inline]
    pub fn quota_used(&self) -> u64 {
        self.quota.used()
    }

    /// Attempts one CAS reservation step against the quota counter.
    ///
    /// Callers control retry/yield policy. This primitive intentionally does
    /// not block or sleep so both sync poll paths and async paths can wrap it
    /// with their own contention strategy.
    #[inline]
    pub fn quota_try_reserve(&self, bytes: u64, limit: u64) -> Result<u64, QuotaReserveError> {
        self.quota
            .try_reserve(bytes, limit)
            .map(QuotaReservation::commit)
    }

    /// Reserves quota until a direct I/O attempt is settled.
    #[inline]
    pub(crate) fn quota_reserve(
        &self,
        bytes: u64,
        limit: u64,
    ) -> Result<QuotaReservation, QuotaReserveError> {
        self.quota.try_reserve(bytes, limit)
    }
}

impl Stats {
    pub fn new() -> Self {
        Self::with_process_authorities(
            Arc::new(QuotaStore::default()),
            Arc::new(UserConnectionAuthority::default()),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_quota_store(quota_store: Arc<QuotaStore>) -> Self {
        Self::with_process_authorities(quota_store, Arc::new(UserConnectionAuthority::default()))
    }

    /// Creates generation telemetry around process-owned enforcement authorities.
    pub(crate) fn with_process_authorities(
        quota_store: Arc<QuotaStore>,
        connection_authority: Arc<UserConnectionAuthority>,
    ) -> Self {
        let stats = Self {
            quota_store,
            connection_authority,
            ..Self::default()
        };
        stats.apply_telemetry_policy(TelemetryPolicy::default());
        stats.refresh_cached_epoch_secs();
        *stats.start_time.write() = Some(Instant::now());
        stats
    }

    /// Returns the process-scoped quota authority for test runtime construction.
    #[cfg(test)]
    pub(crate) fn quota_store(&self) -> Arc<QuotaStore> {
        Arc::clone(&self.quota_store)
    }

    /// Returns process-owned per-user connection admission.
    pub(crate) fn connection_authority(&self) -> Arc<UserConnectionAuthority> {
        Arc::clone(&self.connection_authority)
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
