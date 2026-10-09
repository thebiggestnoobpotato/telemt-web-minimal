use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Semaphore, watch};
use tracing::info;

use crate::config::{LogLevel, ProxyConfig};
use crate::crypto::SecureRandom;
use crate::ip_tracker::UserIpTracker;
use crate::network::probe::NetworkDecision;
use crate::proxy::direct_buffer_budget::{DirectBufferBudget, run_direct_buffer_budget_controller};
use crate::proxy::shared_state::ProxySharedState;
use crate::startup::StartupTracker;
use crate::stats::{ReplayChecker, Stats};
use crate::stream::BufferPool;
use crate::transport::UpstreamManager;

use super::admission;
use super::generation::RuntimeTaskScope;
use super::{connectivity, runtime_tasks};

pub(super) struct RuntimeStartupState {
    pub(super) config: Arc<ProxyConfig>,
    pub(super) rng: Arc<SecureRandom>,
    pub(super) max_connections: Arc<Semaphore>,
    pub(super) replay_checker: Arc<ReplayChecker>,
    pub(super) buffer_pool: Arc<BufferPool>,
    pub(super) config_rx: watch::Receiver<Arc<ProxyConfig>>,
    pub(super) admission_tx: watch::Sender<bool>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_runtime(
    config: ProxyConfig,
    config_path: &Path,
    decision: &NetworkDecision,
    process_started_at: Instant,
    startup_tracker: &Arc<StartupTracker>,
    stats: Arc<Stats>,
    upstream_manager: Arc<UpstreamManager>,
    ip_tracker: Arc<UserIpTracker>,
    shared_state: Arc<ProxySharedState>,
    direct_buffer_budget: Arc<DirectBufferBudget>,
    max_connections: Arc<Semaphore>,
    runtime_task_scope: RuntimeTaskScope,
    admission_tx: watch::Sender<bool>,
    runtime_log_filter: &runtime_tasks::RuntimeLogFilter,
    has_rust_log: bool,
    effective_log_level: &LogLevel,
) -> RuntimeStartupState {
    let prefer_ipv6 = decision.prefer_ipv6();
    let rng = Arc::new(SecureRandom::new());

    let config = Arc::new(config);
    let replay_checker = Arc::new(ReplayChecker::new(
        config.access.replay_check_len,
        Duration::from_secs(config.access.replay_window_secs),
    ));
    let buffer_pool = Arc::new(BufferPool::with_config(64 * 1024, 4096));

    connectivity::run_startup_connectivity(
        &config,
        startup_tracker,
        upstream_manager.clone(),
        prefer_ipv6,
        decision,
        process_started_at,
    )
    .await;

    let runtime_watches = runtime_tasks::spawn_runtime_tasks(
        1,
        &config,
        config_path,
        prefer_ipv6,
        decision.ipv4_dc,
        decision.ipv6_dc,
        startup_tracker,
        stats.clone(),
        upstream_manager.clone(),
        replay_checker.clone(),
        ip_tracker.clone(),
        shared_state.clone(),
        runtime_task_scope.clone(),
        None,
    )
    .await;
    let config_rx = runtime_watches.config_rx;
    let log_level_rx = runtime_watches.log_level_rx;
    runtime_log_filter.start(
        has_rust_log,
        effective_log_level,
        log_level_rx,
        runtime_task_scope.clone(),
    );

    startup_tracker.set_degraded(false).await;
    info!("Transport: Direct DC - TCP - standard DC-over-TCP");

    admission::configure_admission_gate(&admission_tx).await;
    runtime_task_scope.spawn(run_direct_buffer_budget_controller(
        1,
        direct_buffer_budget,
        buffer_pool.clone(),
        stats,
        max_connections.clone(),
        config.general.max_connections,
    ));

    RuntimeStartupState {
        config,
        rng,
        max_connections,
        replay_checker,
        buffer_pool,
        config_rx,
        admission_tx,
    }
}
