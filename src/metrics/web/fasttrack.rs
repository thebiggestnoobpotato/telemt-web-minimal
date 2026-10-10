use std::fmt::Write;

use crate::config::{ProxyConfig, WebFallbackFastTrackMode};
use crate::web::control::WebRuntimePublication;
use crate::web::telemetry::WebFallbackFastTrackDisposition;

/// Renders fixed-cardinality fallback capability-routing metrics.
pub(super) fn render(out: &mut String, publication: &WebRuntimePublication, config: &ProxyConfig) {
    let _ = writeln!(
        out,
        "# HELP telemt_web_fallback_fasttrack_mode Effective restart-frozen fallback fast-track mode"
    );
    let _ = writeln!(out, "# TYPE telemt_web_fallback_fasttrack_mode gauge");
    for mode in WebFallbackFastTrackMode::ALL {
        let _ = writeln!(
            out,
            "telemt_web_fallback_fasttrack_mode{{mode=\"{}\"}} {}",
            mode.as_str(),
            u8::from(config.web.fallback_fasttrack_mode == mode)
        );
    }

    let _ = writeln!(
        out,
        "# HELP telemt_web_fallback_fasttrack_requests_total WEB root requests classified by fallback capability-routing work"
    );
    let _ = writeln!(
        out,
        "# TYPE telemt_web_fallback_fasttrack_requests_total counter"
    );
    for disposition in WebFallbackFastTrackDisposition::ALL {
        let _ = writeln!(
            out,
            "telemt_web_fallback_fasttrack_requests_total{{disposition=\"{}\"}} {}",
            disposition.as_str(),
            publication.telemetry.fallback_fasttrack_total(disposition)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::control::WebRuntimeControl;

    #[test]
    fn renderer_emits_one_hot_mode_and_complete_counters() {
        let control = WebRuntimeControl::new();
        control
            .telemetry()
            .record_fallback_fasttrack(WebFallbackFastTrackDisposition::EnforceFastTrack);
        let publication = control.subscribe().borrow().clone();
        let mut config = ProxyConfig::default();
        config.web.fallback_fasttrack_mode = WebFallbackFastTrackMode::Enforce;
        let mut output = String::new();

        render(&mut output, &publication, &config);

        assert!(output.contains("telemt_web_fallback_fasttrack_mode{mode=\"off\"} 0"));
        assert!(output.contains("telemt_web_fallback_fasttrack_mode{mode=\"enforce\"} 1"));
        assert_eq!(
            output
                .matches("telemt_web_fallback_fasttrack_requests_total{")
                .count(),
            WebFallbackFastTrackDisposition::ALL.len()
        );
        assert!(output.contains(
            "telemt_web_fallback_fasttrack_requests_total{disposition=\"enforce_fasttrack\"} 1"
        ));
    }
}
