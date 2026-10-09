use super::*;

impl Stats {
    pub fn get_connects_all(&self) -> u64 {
        self.connects_all.load(Ordering::Relaxed)
    }
    pub fn get_connects_bad(&self) -> u64 {
        self.connects_bad.load(Ordering::Relaxed)
    }

    pub fn get_connects_bad_class_counts(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self
            .connects_bad_classes
            .iter()
            .map(|entry| {
                (
                    entry.key().to_string(),
                    entry.value().load(Ordering::Relaxed),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn get_handshake_failure_class_counts(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self
            .handshake_failure_classes
            .iter()
            .map(|entry| {
                (
                    entry.key().to_string(),
                    entry.value().load(Ordering::Relaxed),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn get_current_connections_direct(&self) -> u64 {
        self.current_connections_direct.load(Ordering::Relaxed)
    }
    pub fn get_current_connections_total(&self) -> u64 {
        self.get_current_connections_direct()
    }
    pub fn get_buffer_pool_pooled_gauge(&self) -> u64 {
        self.buffer_pool_pooled_gauge.load(Ordering::Relaxed)
    }

    pub fn get_buffer_pool_allocated_gauge(&self) -> u64 {
        self.buffer_pool_allocated_gauge.load(Ordering::Relaxed)
    }

    pub fn get_buffer_pool_in_use_gauge(&self) -> u64 {
        self.buffer_pool_in_use_gauge.load(Ordering::Relaxed)
    }

    /// Returns the count of non-standard buffers replaced before pooling.
    pub fn get_buffer_pool_replaced_nonstandard_total(&self) -> u64 {
        self.buffer_pool_replaced_nonstandard_total
            .load(Ordering::Relaxed)
    }

    pub fn get_ip_reservation_rollback_tcp_limit_total(&self) -> u64 {
        self.ip_reservation_rollback_tcp_limit_total
            .load(Ordering::Relaxed)
    }
    pub fn get_ip_reservation_rollback_quota_limit_total(&self) -> u64 {
        self.ip_reservation_rollback_quota_limit_total
            .load(Ordering::Relaxed)
    }
    pub fn get_quota_refund_bytes_total(&self) -> u64 {
        self.quota_refund_bytes_total.load(Ordering::Relaxed)
    }
    pub fn get_quota_contention_total(&self) -> u64 {
        self.quota_contention_total.load(Ordering::Relaxed)
    }
    pub fn get_quota_contention_timeout_total(&self) -> u64 {
        self.quota_contention_timeout_total.load(Ordering::Relaxed)
    }
    pub fn get_quota_acquire_cancelled_total(&self) -> u64 {
        self.quota_acquire_cancelled_total.load(Ordering::Relaxed)
    }
    pub fn get_quota_write_fail_bytes_total(&self) -> u64 {
        self.quota_write_fail_bytes_total.load(Ordering::Relaxed)
    }
    pub fn get_quota_write_fail_events_total(&self) -> u64 {
        self.quota_write_fail_events_total.load(Ordering::Relaxed)
    }
    pub fn get_session_drop_fallback_total(&self) -> u64 {
        self.session_drop_fallback_total.load(Ordering::Relaxed)
    }
}
