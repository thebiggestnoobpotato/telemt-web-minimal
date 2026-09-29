use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Semaphore, watch};
use tracing::info;

use crate::config::{LogLevel, ProxyConfig};
use crate::conntrack_control;
use crate::crypto::SecureRandom;
use crate::ip_tracker::UserIpTracker;
use crate::network::probe::{NetworkDecision, NetworkProbe};
use crate::proxy::direct_buffer_budget::{DirectBufferBudget, run_direct_buffer_budget_controller};
use crate::proxy::shared_state::ProxySharedState;
use crate::startup::{
    COMPONENT_ME_CONNECTIVITY_PING, COMPONENT_ME_POOL_CONSTRUCT, COMPONENT_ME_POOL_INIT_STAGE1,
    COMPONENT_ME_PROXY_CONFIG_V4, COMPONENT_ME_PROXY_CONFIG_V6, COMPONENT_ME_SECRET_FETCH,
    StartupMeStatus, StartupTracker,
};
use crate::stats::beobachten::BeobachtenStore;
use crate::stats::{ReplayChecker, Stats};
use crate::stream::BufferPool;
use crate::transport::UpstreamManager;

use super::admission;
use super::generation::RuntimeTaskScope;
use super::{connectivity, runtime_tasks};

pub(super) struct RuntimeStartupState {
    pub(super) config: Arc<ProxyConfig>,
    pub(super) beobachten: Arc<BeobachtenStore>,
    pub(super) rng: Arc<SecureRandom>,
    pub(super) max_connections: Arc<Semaphore>,
    pub(super) replay_checker: Arc<ReplayChecker>,
    pub(super) buffer_pool: Arc<BufferPool>,
    pub(super) config_rx: watch::Receiver<Arc<ProxyConfig>>,
    pub(super) detected_ip_v4: Option<IpAddr>,
    pub(super) detected_ip_v6: Option<IpAddr>,
    pub(super) admission_tx: watch::Sender<bool>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_runtime(
    config: ProxyConfig,
    config_path: &Path,
    probe: &NetworkProbe,
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
    let beobachten = Arc::new(BeobachtenStore::new());
    let rng = Arc::new(SecureRandom::new());

    // This build has no Middle-End pool; keep the startup report shape stable.
    startup_tracker.set_me_status(StartupMeStatus::Skipped, "skipped").await;
    startup_tracker
        .skip_component(
            COMPONENT_ME_SECRET_FETCH,
            Some("not available in this build".to_string()),
        )
        .await;
    startup_tracker
        .skip_component(
            COMPONENT_ME_PROXY_CONFIG_V4,
            Some("not available in this build".to_string()),
        )
        .await;
    startup_tracker
        .skip_component(
            COMPONENT_ME_PROXY_CONFIG_V6,
            Some("not available in this build".to_string()),
        )
        .await;
    startup_tracker
        .skip_component(
            COMPONENT_ME_POOL_CONSTRUCT,
            Some("not available in this build".to_string()),
        )
        .await;
    startup_tracker
        .skip_component(
            COMPONENT_ME_POOL_INIT_STAGE1,
            Some("not available in this build".to_string()),
        )
        .await;
    startup_tracker
        .skip_component(
            COMPONENT_ME_CONNECTIVITY_PING,
            Some("not available in this build".to_string()),
        )
        .await;

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
        probe,
        prefer_ipv6,
        decision.ipv4_dc,
        decision.ipv6_dc,
        startup_tracker,
        stats.clone(),
        upstream_manager.clone(),
        replay_checker.clone(),
        ip_tracker.clone(),
        beobachten.clone(),
        shared_state.clone(),
        runtime_task_scope.clone(),
        None,
    )
    .await;
    let config_rx = runtime_watches.config_rx;
    let log_level_rx = runtime_watches.log_level_rx;
    let detected_ip_v4 = runtime_watches.detected_ip_v4;
    let detected_ip_v6 = runtime_watches.detected_ip_v6;
    runtime_log_filter.start(
        has_rust_log,
        effective_log_level,
        log_level_rx,
        runtime_task_scope.clone(),
    );

    startup_tracker.set_transport_mode("direct").await;
    startup_tracker.set_degraded(false).await;
    info!("Transport: Direct DC - TCP - standard DC-over-TCP");

    admission::configure_admission_gate(&admission_tx).await;
    let conntrack_scope = runtime_task_scope.clone();
    runtime_task_scope.spawn(conntrack_control::run_conntrack_controller(
        config_rx.clone(),
        stats.clone(),
        shared_state.clone(),
        conntrack_scope.cancellation_token(),
    ));
    runtime_task_scope.spawn(run_direct_buffer_budget_controller(
        1,
        direct_buffer_budget,
        buffer_pool.clone(),
        stats,
        shared_state,
        max_connections.clone(),
        config.server.max_connections,
    ));

    RuntimeStartupState {
        config,
        beobachten,
        rng,
        max_connections,
        replay_checker,
        buffer_pool,
        config_rx,
        detected_ip_v4,
        detected_ip_v6,
        admission_tx,
    }
}
