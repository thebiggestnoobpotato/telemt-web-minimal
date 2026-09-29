use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

pub const COMPONENT_CONFIG_LOAD: &str = "config_load";
pub const COMPONENT_TRACING_INIT: &str = "tracing_init";
pub const COMPONENT_API_BOOTSTRAP: &str = "api_bootstrap";
pub const COMPONENT_NETWORK_PROBE: &str = "network_probe";
pub const COMPONENT_DC_CONNECTIVITY_PING: &str = "dc_connectivity_ping";
pub const COMPONENT_LISTENERS_BIND: &str = "listeners_bind";
pub const COMPONENT_CONFIG_WATCHER_START: &str = "config_watcher_start";
pub const COMPONENT_METRICS_START: &str = "metrics_start";
pub const COMPONENT_RUNTIME_READY: &str = "runtime_ready";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupStatus {
    Initializing,
    Ready,
}

impl StartupStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::Ready => "ready",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupComponentStatus {
    Pending,
    Running,
    Ready,
    Failed,
    Skipped,
}

impl StartupComponentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StartupComponentSnapshot {
    pub id: &'static str,
    pub title: &'static str,
    pub weight: f64,
    pub status: StartupComponentStatus,
    pub started_at_epoch_ms: Option<u64>,
    pub finished_at_epoch_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub attempts: u32,
    pub details: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StartupSnapshot {
    pub status: StartupStatus,
    pub degraded: bool,
    pub current_stage: String,
    pub started_at_epoch_secs: u64,
    pub ready_at_epoch_secs: Option<u64>,
    pub total_elapsed_ms: u64,
    pub components: Vec<StartupComponentSnapshot>,
}

#[derive(Clone, Debug)]
struct StartupComponent {
    id: &'static str,
    title: &'static str,
    weight: f64,
    status: StartupComponentStatus,
    started_at_epoch_ms: Option<u64>,
    finished_at_epoch_ms: Option<u64>,
    duration_ms: Option<u64>,
    attempts: u32,
    details: Option<String>,
}

#[derive(Clone, Debug)]
struct StartupState {
    status: StartupStatus,
    degraded: bool,
    current_stage: String,
    started_at_epoch_secs: u64,
    ready_at_epoch_secs: Option<u64>,
    components: Vec<StartupComponent>,
}

pub struct StartupTracker {
    started_at_instant: Instant,
    state: RwLock<StartupState>,
}

impl StartupTracker {
    pub fn new(started_at_epoch_secs: u64) -> Self {
        Self {
            started_at_instant: Instant::now(),
            state: RwLock::new(StartupState {
                status: StartupStatus::Initializing,
                degraded: false,
                current_stage: COMPONENT_CONFIG_LOAD.to_string(),
                started_at_epoch_secs,
                ready_at_epoch_secs: None,
                components: component_blueprint(),
            }),
        }
    }

    pub async fn set_degraded(&self, degraded: bool) {
        self.state.write().await.degraded = degraded;
    }

    pub async fn start_component(&self, id: &'static str, details: Option<String>) {
        let mut guard = self.state.write().await;
        guard.current_stage = id.to_string();
        if let Some(component) = guard
            .components
            .iter_mut()
            .find(|component| component.id == id)
        {
            if component.started_at_epoch_ms.is_none() {
                component.started_at_epoch_ms = Some(now_epoch_ms());
            }
            component.attempts = component.attempts.saturating_add(1);
            component.status = StartupComponentStatus::Running;
            component.details = normalize_details(details);
        }
    }

    pub async fn complete_component(&self, id: &'static str, details: Option<String>) {
        self.finish_component(id, StartupComponentStatus::Ready, details)
            .await;
    }

    pub async fn fail_component(&self, id: &'static str, details: Option<String>) {
        self.finish_component(id, StartupComponentStatus::Failed, details)
            .await;
    }

    pub async fn skip_component(&self, id: &'static str, details: Option<String>) {
        self.finish_component(id, StartupComponentStatus::Skipped, details)
            .await;
    }

    async fn finish_component(
        &self,
        id: &'static str,
        status: StartupComponentStatus,
        details: Option<String>,
    ) {
        let mut guard = self.state.write().await;
        let finished_at = now_epoch_ms();
        if let Some(component) = guard
            .components
            .iter_mut()
            .find(|component| component.id == id)
        {
            if component.started_at_epoch_ms.is_none() {
                component.started_at_epoch_ms = Some(finished_at);
                component.attempts = component.attempts.saturating_add(1);
            }
            component.finished_at_epoch_ms = Some(finished_at);
            component.duration_ms = component
                .started_at_epoch_ms
                .map(|started_at| finished_at.saturating_sub(started_at));
            component.status = status;
            component.details = normalize_details(details);
        }
    }

    pub async fn mark_ready(&self) {
        let mut guard = self.state.write().await;
        if guard.status == StartupStatus::Ready {
            return;
        }
        guard.status = StartupStatus::Ready;
        guard.current_stage = "ready".to_string();
        guard.ready_at_epoch_secs = Some(now_epoch_secs());
    }

    pub async fn snapshot(&self) -> StartupSnapshot {
        let guard = self.state.read().await;
        StartupSnapshot {
            status: guard.status,
            degraded: guard.degraded,
            current_stage: guard.current_stage.clone(),
            started_at_epoch_secs: guard.started_at_epoch_secs,
            ready_at_epoch_secs: guard.ready_at_epoch_secs,
            total_elapsed_ms: self.started_at_instant.elapsed().as_millis() as u64,
            components: guard
                .components
                .iter()
                .map(|component| StartupComponentSnapshot {
                    id: component.id,
                    title: component.title,
                    weight: component.weight,
                    status: component.status,
                    started_at_epoch_ms: component.started_at_epoch_ms,
                    finished_at_epoch_ms: component.finished_at_epoch_ms,
                    duration_ms: component.duration_ms,
                    attempts: component.attempts,
                    details: component.details.clone(),
                })
                .collect(),
        }
    }
}

pub fn compute_progress_pct(snapshot: &StartupSnapshot) -> f64 {
    if snapshot.status == StartupStatus::Ready {
        return 100.0;
    }

    let mut total_weight = 0.0f64;
    let mut completed_weight = 0.0f64;

    for component in &snapshot.components {
        total_weight += component.weight;
        let unit_progress = match component.status {
            StartupComponentStatus::Pending => 0.0,
            StartupComponentStatus::Running => 0.0,
            StartupComponentStatus::Ready
            | StartupComponentStatus::Failed
            | StartupComponentStatus::Skipped => 1.0,
        };
        completed_weight += component.weight * unit_progress;
    }

    if total_weight <= f64::EPSILON {
        0.0
    } else {
        ((completed_weight / total_weight) * 100.0).clamp(0.0, 100.0)
    }
}

fn component_blueprint() -> Vec<StartupComponent> {
    vec![
        component(COMPONENT_CONFIG_LOAD, "Config load", 5.0),
        component(COMPONENT_TRACING_INIT, "Tracing init", 3.0),
        component(COMPONENT_API_BOOTSTRAP, "API bootstrap", 5.0),
        component(COMPONENT_NETWORK_PROBE, "Network probe", 10.0),
        component(COMPONENT_DC_CONNECTIVITY_PING, "DC connectivity ping", 8.0),
        component(COMPONENT_LISTENERS_BIND, "Listener bind", 8.0),
        component(COMPONENT_CONFIG_WATCHER_START, "Config watcher start", 2.0),
        component(COMPONENT_METRICS_START, "Metrics start", 1.0),
        component(COMPONENT_RUNTIME_READY, "Runtime ready", 1.0),
    ]
}

fn component(id: &'static str, title: &'static str, weight: f64) -> StartupComponent {
    StartupComponent {
        id,
        title,
        weight,
        status: StartupComponentStatus::Pending,
        started_at_epoch_ms: None,
        finished_at_epoch_ms: None,
        duration_ms: None,
        attempts: 0,
        details: None,
    }
}

fn normalize_details(details: Option<String>) -> Option<String> {
    details.map(|detail| {
        if detail.len() <= 256 {
            detail
        } else {
            detail[..256].to_string()
        }
    })
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
