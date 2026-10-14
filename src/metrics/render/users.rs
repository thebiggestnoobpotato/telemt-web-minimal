use super::*;
use std::fmt::Write;

pub(super) fn render(out: &mut String, stats: &Stats, user_enabled: bool) {
    let _ = writeln!(
        out,
        "# HELP telemt_user_connections_total Per-user total connections"
    );
    let _ = writeln!(out, "# TYPE telemt_user_connections_total counter");
    let _ = writeln!(
        out,
        "# HELP telemt_user_connections_current Per-user active connections"
    );
    let _ = writeln!(out, "# TYPE telemt_user_connections_current gauge");
    let _ = writeln!(
        out,
        "# HELP telemt_user_octets_from_client_total Per-user total bytes received"
    );
    let _ = writeln!(out, "# TYPE telemt_user_octets_from_client_total counter");
    let _ = writeln!(
        out,
        "# HELP telemt_user_octets_to_client_total Per-user total bytes sent"
    );
    let _ = writeln!(out, "# TYPE telemt_user_octets_to_client_total counter");
    let _ = writeln!(
        out,
        "# HELP telemt_user_msgs_from_client_total Per-user total messages received"
    );
    let _ = writeln!(out, "# TYPE telemt_user_msgs_from_client_total counter");
    let _ = writeln!(
        out,
        "# HELP telemt_user_msgs_to_client_total Per-user total messages sent"
    );
    let _ = writeln!(out, "# TYPE telemt_user_msgs_to_client_total counter");

    let mut user_stats_emitted = 0usize;
    let mut user_stats_suppressed = 0usize;

    if user_enabled {
        for entry in stats.iter_user_stats() {
            if user_stats_emitted >= USER_LABELED_METRICS_MAX_USERS {
                user_stats_suppressed = user_stats_suppressed.saturating_add(1);
                continue;
            }
            let user = entry.key();
            let s = entry.value();
            user_stats_emitted = user_stats_emitted.saturating_add(1);
            let _ = writeln!(
                out,
                "telemt_user_connections_total{{user=\"{}\"}} {}",
                user,
                s.connects.load(std::sync::atomic::Ordering::Relaxed)
            );
            let _ = writeln!(
                out,
                "telemt_user_connections_current{{user=\"{}\"}} {}",
                user,
                stats.get_process_user_curr_connects(user)
            );
            let _ = writeln!(
                out,
                "telemt_user_octets_from_client_total{{user=\"{}\"}} {}",
                user,
                s.octets_from_client
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
            let _ = writeln!(
                out,
                "telemt_user_octets_to_client_total{{user=\"{}\"}} {}",
                user,
                s.octets_to_client
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
            let _ = writeln!(
                out,
                "telemt_user_msgs_from_client_total{{user=\"{}\"}} {}",
                user,
                s.msgs_from_client
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
            let _ = writeln!(
                out,
                "telemt_user_msgs_to_client_total{{user=\"{}\"}} {}",
                user,
                s.msgs_to_client.load(std::sync::atomic::Ordering::Relaxed)
            );
        }
    }

    let _ = writeln!(
        out,
        "# HELP telemt_telemetry_user_series_suppressed User-labeled metric series suppression flag"
    );
    let _ = writeln!(out, "# TYPE telemt_telemetry_user_series_suppressed gauge");
    let _ = writeln!(
        out,
        "telemt_telemetry_user_series_suppressed {}",
        if user_enabled && user_stats_suppressed == 0 {
            0
        } else {
            1
        }
    );
    let _ = writeln!(
        out,
        "# HELP telemt_telemetry_user_series_users User-labeled metric users by export status"
    );
    let _ = writeln!(out, "# TYPE telemt_telemetry_user_series_users gauge");
    let _ = writeln!(
        out,
        "telemt_telemetry_user_series_users{{family=\"stats\",status=\"emitted\"}} {}",
        user_stats_emitted
    );
    let _ = writeln!(
        out,
        "telemt_telemetry_user_series_users{{family=\"stats\",status=\"suppressed\"}} {}",
        user_stats_suppressed
    );
}
