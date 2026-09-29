use super::*;

impl Stats {
    pub fn increment_ip_reservation_rollback_tcp_limit_total(&self) {
        if self.telemetry_core_enabled() {
            self.ip_reservation_rollback_tcp_limit_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn increment_ip_reservation_rollback_quota_limit_total(&self) {
        if self.telemetry_core_enabled() {
            self.ip_reservation_rollback_quota_limit_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn add_quota_refund_bytes_total(&self, bytes: u64) {
        if self.telemetry_core_enabled() {
            self.quota_refund_bytes_total
                .fetch_add(bytes, Ordering::Relaxed);
        }
    }
    pub fn increment_quota_contention_total(&self) {
        if self.telemetry_core_enabled() {
            self.quota_contention_total.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn increment_quota_contention_timeout_total(&self) {
        if self.telemetry_core_enabled() {
            self.quota_contention_timeout_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn increment_quota_acquire_cancelled_total(&self) {
        if self.telemetry_core_enabled() {
            self.quota_acquire_cancelled_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn add_quota_write_fail_bytes_total(&self, bytes: u64) {
        if self.telemetry_core_enabled() {
            self.quota_write_fail_bytes_total
                .fetch_add(bytes, Ordering::Relaxed);
        }
    }
    pub fn increment_quota_write_fail_events_total(&self) {
        if self.telemetry_core_enabled() {
            self.quota_write_fail_events_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn increment_session_drop_fallback_total(&self) {
        if self.telemetry_core_enabled() {
            self.session_drop_fallback_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn set_buffer_pool_gauges(&self, pooled: usize, allocated: usize, in_use: usize) {
        if self.telemetry_core_enabled() {
            self.buffer_pool_pooled_gauge
                .store(pooled as u64, Ordering::Relaxed);
            self.buffer_pool_allocated_gauge
                .store(allocated as u64, Ordering::Relaxed);
            self.buffer_pool_in_use_gauge
                .store(in_use as u64, Ordering::Relaxed);
        }
    }

    /// Publishes the cumulative count of non-standard pool buffer replacements.
    pub fn set_buffer_pool_replaced_nonstandard_total(&self, value: usize) {
        if self.telemetry_core_enabled() {
            self.buffer_pool_replaced_nonstandard_total
                .store(value as u64, Ordering::Relaxed);
        }
    }
}
