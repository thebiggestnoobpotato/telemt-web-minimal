use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::watch;

use crate::maestro::control_plane::ProcessControlPlane;

use super::ApiRuntimeState;
use super::events::ApiEventStore;

pub(super) fn spawn_runtime_watchers(
    generation_id: u64,
    admission_rx: watch::Receiver<bool>,
    runtime_state: Arc<ApiRuntimeState>,
    runtime_events: Arc<ApiEventStore>,
    control_plane: &ProcessControlPlane,
) {
    spawn_admission_watcher(
        generation_id,
        admission_rx,
        runtime_state,
        runtime_events,
        control_plane,
    );
}

fn spawn_admission_watcher(
    generation_id: u64,
    mut admission_rx: watch::Receiver<bool>,
    runtime_state: Arc<ApiRuntimeState>,
    runtime_events: Arc<ApiEventStore>,
    control_plane: &ProcessControlPlane,
) {
    let _ = control_plane.spawn(async move {
        record_admission_state(&runtime_state, &runtime_events, generation_id, &admission_rx);
        while admission_rx.changed().await.is_ok() {
            record_admission_state(&runtime_state, &runtime_events, generation_id, &admission_rx);
        }
    });
}

fn record_admission_state(
    runtime_state: &ApiRuntimeState,
    runtime_events: &ApiEventStore,
    generation_id: u64,
    admission_rx: &watch::Receiver<bool>,
) {
    let admission_open = *admission_rx.borrow();
    runtime_state
        .admission_open
        .store(admission_open, Ordering::Relaxed);
    runtime_events.record(
        "admission.state",
        format!("generation={generation_id} accepting_new_connections={admission_open}"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    fn runtime_state() -> Arc<ApiRuntimeState> {
        Arc::new(ApiRuntimeState {
            process_started_at_epoch_secs: 1,
            admission_open: AtomicBool::new(false),
        })
    }

    #[tokio::test]
    async fn admission_watcher_records_state_changes() {
        let (admission_tx, admission_rx) = watch::channel(true);
        let runtime_state = runtime_state();
        let events = Arc::new(ApiEventStore::new(16));
        let control_plane = ProcessControlPlane::new();
        spawn_runtime_watchers(
            1,
            admission_rx,
            runtime_state.clone(),
            events.clone(),
            &control_plane,
        );
        tokio::task::yield_now().await;
        assert!(runtime_state.admission_open.load(Ordering::Relaxed));

        admission_tx.send_replace(false);
        tokio::time::timeout(Duration::from_secs(1), async {
            while runtime_state.admission_open.load(Ordering::Relaxed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let snapshot = events.snapshot(16);
        assert_eq!(
            snapshot
                .events
                .iter()
                .filter(|event| event.event_type == "admission.state")
                .count(),
            2
        );
        assert!(control_plane.shutdown(Duration::from_secs(1)).await);
    }
}
