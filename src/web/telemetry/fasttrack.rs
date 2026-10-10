use std::sync::atomic::Ordering;

use serde::Serialize;

use super::WebTelemetry;

/// Terminal capability-routing work selected for one WEB root request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum WebFallbackFastTrackDisposition {
    /// Shadow mode identified a request that enforce mode would bypass.
    ShadowWouldFastTrack,
    /// Shadow mode retained a full scan for a plausible capability request.
    ShadowCandidateFullScan,
    /// Enforce mode bypassed a structurally impossible capability request.
    EnforceFastTrack,
    /// Enforce mode retained a full scan for a plausible capability request.
    EnforceCandidateFullScan,
}

impl WebFallbackFastTrackDisposition {
    /// Complete fixed disposition set in stable API and metric order.
    pub(crate) const ALL: [Self; 4] = [
        Self::ShadowWouldFastTrack,
        Self::ShadowCandidateFullScan,
        Self::EnforceFastTrack,
        Self::EnforceCandidateFullScan,
    ];

    /// Returns the stable API and Prometheus label token.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ShadowWouldFastTrack => "shadow_would_fasttrack",
            Self::ShadowCandidateFullScan => "shadow_candidate_full_scan",
            Self::EnforceFastTrack => "enforce_fasttrack",
            Self::EnforceCandidateFullScan => "enforce_candidate_full_scan",
        }
    }
}

/// Fixed storage width for process-owned fallback fast-track counters.
pub(super) const FALLBACK_FASTTRACK_SLOTS: usize = WebFallbackFastTrackDisposition::ALL.len();

/// API-safe fixed fallback fast-track counter.
#[derive(Clone, Serialize)]
pub(crate) struct WebFallbackFastTrackCounter {
    /// Stable capability-routing disposition token.
    pub(crate) disposition: &'static str,
    /// Process-lifetime event count.
    pub(crate) total: u64,
}

impl WebTelemetry {
    /// Records one shadow or enforce capability-routing disposition.
    pub(crate) fn record_fallback_fasttrack(&self, disposition: WebFallbackFastTrackDisposition) {
        self.fallback_fasttrack_requests[disposition as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// Returns one fixed fallback fast-track counter.
    pub(crate) fn fallback_fasttrack_total(&self, disposition: WebFallbackFastTrackDisposition) -> u64 {
        self.fallback_fasttrack_requests[disposition as usize].load(Ordering::Relaxed)
    }

    /// Captures the complete fallback fast-track counter set.
    pub(crate) fn fallback_fasttrack_counters(&self) -> Vec<WebFallbackFastTrackCounter> {
        WebFallbackFastTrackDisposition::ALL
            .into_iter()
            .map(|disposition| WebFallbackFastTrackCounter {
                disposition: disposition.as_str(),
                total: self.fallback_fasttrack_total(disposition),
            })
            .collect()
    }
}
