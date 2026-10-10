use super::*;
use std::fmt::Write;

pub(super) fn render(out: &mut String, stats: &Stats, core_enabled: bool) {
    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_attempt_total Upstream connect attempts across all requests"
    );
    let _ = writeln!(out, "# TYPE telemt_upstream_connect_attempt_total counter");
    let _ = writeln!(
        out,
        "telemt_upstream_connect_attempt_total {}",
        if core_enabled {
            stats.get_upstream_connect_attempt_total()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_success_total Successful upstream connect request cycles"
    );
    let _ = writeln!(out, "# TYPE telemt_upstream_connect_success_total counter");
    let _ = writeln!(
        out,
        "telemt_upstream_connect_success_total {}",
        if core_enabled {
            stats.get_upstream_connect_success_total()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_fail_total Failed upstream connect request cycles"
    );
    let _ = writeln!(out, "# TYPE telemt_upstream_connect_fail_total counter");
    let _ = writeln!(
        out,
        "telemt_upstream_connect_fail_total {}",
        if core_enabled {
            stats.get_upstream_connect_fail_total()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_failfast_hard_error_total Hard errors that triggered upstream connect failfast"
    );
    let _ = writeln!(
        out,
        "# TYPE telemt_upstream_connect_failfast_hard_error_total counter"
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_failfast_hard_error_total {}",
        if core_enabled {
            stats.get_upstream_connect_failfast_hard_error_total()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_attempts_per_request Histogram-like buckets for attempts per upstream connect request cycle"
    );
    let _ = writeln!(
        out,
        "# TYPE telemt_upstream_connect_attempts_per_request counter"
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_attempts_per_request{{bucket=\"1\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_attempts_bucket_1()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_attempts_per_request{{bucket=\"2\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_attempts_bucket_2()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_attempts_per_request{{bucket=\"3_4\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_attempts_bucket_3_4()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_attempts_per_request{{bucket=\"gt_4\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_attempts_bucket_gt_4()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_duration_success_total Histogram-like buckets of successful upstream connect cycle duration"
    );
    let _ = writeln!(
        out,
        "# TYPE telemt_upstream_connect_duration_success_total counter"
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_success_total{{bucket=\"le_100ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_success_bucket_le_100ms()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_success_total{{bucket=\"101_500ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_success_bucket_101_500ms()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_success_total{{bucket=\"501_1000ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_success_bucket_501_1000ms()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_success_total{{bucket=\"gt_1000ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_success_bucket_gt_1000ms()
        } else {
            0
        }
    );

    let _ = writeln!(
        out,
        "# HELP telemt_upstream_connect_duration_fail_total Histogram-like buckets of failed upstream connect cycle duration"
    );
    let _ = writeln!(
        out,
        "# TYPE telemt_upstream_connect_duration_fail_total counter"
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_fail_total{{bucket=\"le_100ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_fail_bucket_le_100ms()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_fail_total{{bucket=\"101_500ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_fail_bucket_101_500ms()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_fail_total{{bucket=\"501_1000ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_fail_bucket_501_1000ms()
        } else {
            0
        }
    );
    let _ = writeln!(
        out,
        "telemt_upstream_connect_duration_fail_total{{bucket=\"gt_1000ms\"}} {}",
        if core_enabled {
            stats.get_upstream_connect_duration_fail_bucket_gt_1000ms()
        } else {
            0
        }
    );
}
