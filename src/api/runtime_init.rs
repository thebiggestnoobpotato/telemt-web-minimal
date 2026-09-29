use serde::Serialize;

use crate::startup::compute_progress_pct;

use super::ApiShared;

#[derive(Serialize)]
pub(super) struct RuntimeInitializationComponentData {
    pub(super) id: &'static str,
    pub(super) title: &'static str,
    pub(super) status: &'static str,
    pub(super) started_at_epoch_ms: Option<u64>,
    pub(super) finished_at_epoch_ms: Option<u64>,
    pub(super) duration_ms: Option<u64>,
    pub(super) attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) details: Option<String>,
}

#[derive(Serialize)]
pub(super) struct RuntimeInitializationData {
    pub(super) status: &'static str,
    pub(super) degraded: bool,
    pub(super) current_stage: String,
    pub(super) progress_pct: f64,
    pub(super) started_at_epoch_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ready_at_epoch_secs: Option<u64>,
    pub(super) total_elapsed_ms: u64,
    pub(super) components: Vec<RuntimeInitializationComponentData>,
}

#[derive(Clone)]
pub(super) struct RuntimeStartupSummaryData {
    pub(super) status: &'static str,
    pub(super) stage: String,
    pub(super) progress_pct: f64,
}

pub(super) async fn build_runtime_startup_summary(shared: &ApiShared) -> RuntimeStartupSummaryData {
    let snapshot = shared.startup_tracker.snapshot().await;
    let progress_pct = compute_progress_pct(&snapshot);
    RuntimeStartupSummaryData {
        status: snapshot.status.as_str(),
        stage: snapshot.current_stage,
        progress_pct,
    }
}

pub(super) async fn build_runtime_initialization_data(
    shared: &ApiShared,
) -> RuntimeInitializationData {
    let snapshot = shared.startup_tracker.snapshot().await;
    let progress_pct = compute_progress_pct(&snapshot);

    RuntimeInitializationData {
        status: snapshot.status.as_str(),
        degraded: snapshot.degraded,
        current_stage: snapshot.current_stage,
        progress_pct,
        started_at_epoch_secs: snapshot.started_at_epoch_secs,
        ready_at_epoch_secs: snapshot.ready_at_epoch_secs,
        total_elapsed_ms: snapshot.total_elapsed_ms,
        components: snapshot
            .components
            .into_iter()
            .map(|component| RuntimeInitializationComponentData {
                id: component.id,
                title: component.title,
                status: component.status.as_str(),
                started_at_epoch_ms: component.started_at_epoch_ms,
                finished_at_epoch_ms: component.finished_at_epoch_ms,
                duration_ms: component.duration_ms,
                attempts: component.attempts,
                details: component.details,
            })
            .collect(),
    }
}
