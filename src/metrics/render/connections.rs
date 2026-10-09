use super::*;
use std::fmt::Write;

pub(super) fn render(
    out: &mut String,
    stats: &Stats,
    shared_state: &ProxySharedState,
    core_enabled: bool,
) {
    let _ = writeln!(
        out,
        "# HELP telemt_connections_total Total accepted connections"
    );
    let _ = writeln!(out, "# TYPE telemt_connections_total counter");
    let _ = writeln!(
        out,
        "telemt_connections_total {}",
        if core_enabled {
            stats.get_connects_all()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_connections_bad_total Bad/rejected connections"
    );
    let _ = writeln!(out, "# TYPE telemt_connections_bad_total counter");
    let _ = writeln!(
        out,
        "telemt_connections_bad_total {}",
        if core_enabled {
            stats.get_connects_bad()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_connections_bad_by_class_total Bad/rejected connections by class"
    );
    let _ = writeln!(out, "# TYPE telemt_connections_bad_by_class_total counter");
    if core_enabled {
        for (class, total) in stats.get_connects_bad_class_counts() {
            let _ = writeln!(
                out,
                "telemt_connections_bad_by_class_total{{class=\"{}\"}} {}",
                class, total
            );
        }
    }

    let _ = writeln!(
        out,
        "# HELP telemt_handshake_timeouts_total Handshake timeouts"
    );
    let _ = writeln!(out, "# TYPE telemt_handshake_timeouts_total counter");
    let _ = writeln!(
        out,
        "telemt_handshake_timeouts_total {}",
        if core_enabled {
            stats.get_handshake_timeouts()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_handshake_failures_by_class_total Handshake failures by class"
    );
    let _ = writeln!(
        out,
        "# TYPE telemt_handshake_failures_by_class_total counter"
    );
    if core_enabled {
        for (class, total) in stats.get_handshake_failure_class_counts() {
            let _ = writeln!(
                out,
                "telemt_handshake_failures_by_class_total{{class=\"{}\"}} {}",
                class, total
            );
        }
    }

    let _ = writeln!(
        out,
        "# HELP telemt_auth_expensive_checks_total Expensive authentication candidate checks executed during handshake validation"
    );
    let _ = writeln!(out, "# TYPE telemt_auth_expensive_checks_total counter");
    let _ = writeln!(
        out,
        "telemt_auth_expensive_checks_total {}",
        if core_enabled {
            shared_state
                .handshake
                .auth_expensive_checks_total
                .load(std::sync::atomic::Ordering::Relaxed)
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_auth_budget_exhausted_total Handshake validations that hit authentication candidate budget limits"
    );
    let _ = writeln!(out, "# TYPE telemt_auth_budget_exhausted_total counter");
    let _ = writeln!(
        out,
        "telemt_auth_budget_exhausted_total {}",
        if core_enabled {
            shared_state
                .handshake
                .auth_budget_exhausted_total
                .load(std::sync::atomic::Ordering::Relaxed)
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_quota_refund_bytes_total Reserved quota bytes returned before commit"
    );
    let _ = writeln!(out, "# TYPE telemt_quota_refund_bytes_total counter");
    let _ = writeln!(
        out,
        "telemt_quota_refund_bytes_total {}",
        if core_enabled {
            stats.get_quota_refund_bytes_total()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "# HELP telemt_quota_contention_total Quota reservation CAS contention events"
    );
    let _ = writeln!(out, "# TYPE telemt_quota_contention_total counter");
    let _ = writeln!(
        out,
        "telemt_quota_contention_total {}",
        if core_enabled {
            stats.get_quota_contention_total()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "# HELP telemt_quota_contention_timeout_total Quota reservations that hit the bounded contention budget"
    );
    let _ = writeln!(out, "# TYPE telemt_quota_contention_timeout_total counter");
    let _ = writeln!(
        out,
        "telemt_quota_contention_timeout_total {}",
        if core_enabled {
            stats.get_quota_contention_timeout_total()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "# HELP telemt_quota_acquire_cancelled_total Quota acquisitions cancelled before reservation completed"
    );
    let _ = writeln!(out, "# TYPE telemt_quota_acquire_cancelled_total counter");
    let _ = writeln!(
        out,
        "telemt_quota_acquire_cancelled_total {}",
        if core_enabled {
            stats.get_quota_acquire_cancelled_total()
        } else {
            0
        }
    );

}
