use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::{Semaphore, watch};
use tracing::{error, info};

use crate::api;
use crate::ip_tracker::UserIpTracker;
use crate::network::probe::{decide_network_capabilities, log_probe_result, run_probe};
use crate::proxy::direct_buffer_budget::{DirectBufferBudget, resolve_direct_buffer_hard_limit};
use crate::proxy::shared_state::ProxySharedState;
use crate::proxy::traffic_limiter::TrafficLimiter;
use crate::proxy::user_admission::UserAdmissionAuthority;
use crate::proxy::user_connection_authority::UserConnectionAuthority;
use crate::startup::{COMPONENT_API_BOOTSTRAP, COMPONENT_NETWORK_PROBE};
use crate::stats::telemetry::TelemetryPolicy;
use crate::stats::{QuotaStore, Stats};
use crate::transport::UpstreamManager;
use crate::web::control::WebRuntimeControl;
use crate::web::trace::WebTraceStore;

use super::{
    bootstrap, control_plane, generation, listeners, reload, reload_supervisor, runtime_startup,
    runtime_tasks, shutdown,
};

// Shared maestro startup and main loop. `drop_after_bind` runs on Unix after listeners are
// bound; it is a no-op on other platforms.
pub(super) async fn run_telemt_core(
    strict_runtime_paths: bool,
    drop_after_bind: impl FnOnce(),
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let bootstrap::BootstrapState {
        process_started_at,
        process_started_at_epoch_secs,
        startup_tracker,
        config,
        config_path,
        has_rust_log,
        effective_log_level,
        runtime_log_filter,
        logging_guard: _logging_guard,
    } = bootstrap::bootstrap(strict_runtime_paths).await?;

    let quota_store = Arc::new(QuotaStore::default());
    let connection_authority = Arc::new(UserConnectionAuthority::default());
    let stats = Arc::new(Stats::with_process_authorities(
        quota_store.clone(),
        connection_authority,
    ));
    let process_control_plane = control_plane::ProcessControlPlane::new();
    let runtime_task_scope = generation::RuntimeTaskScope::new();
    let runtime_task_scope_guard =
        generation::RuntimeTaskScopePreparationGuard::new(runtime_task_scope.clone());
    stats.apply_telemetry_policy(TelemetryPolicy::from_config(&config.general));
    let quota_state_path = config.general.quota_state_path.clone();
    let quota_state =
        crate::quota_state::QuotaStateOwner::new(quota_state_path, quota_store.clone());
    let configured_quota_users = config.access.users.keys().cloned().collect::<BTreeSet<_>>();
    quota_state.load(&configured_quota_users).await;

    let upstream_manager = Arc::new(
        UpstreamManager::new(
            config.upstreams.clone(),
            config.general.upstream_connect_retry_attempts,
            config.general.upstream_connect_retry_backoff_ms,
            config.general.upstream_connect_budget_ms,
            config.general.upstream_connect_timeout,
            config.general.upstream_unhealthy_fail_threshold,
            config.general.upstream_connect_failfast_hard_errors,
            stats.clone(),
        ),
    );
    let ip_tracker = Arc::new(UserIpTracker::new());
    let _ = ip_tracker
        .apply_policy_from_source(
            1,
            config.access.user_max_unique_ips_global_each,
            &config.access.user_max_unique_ips,
            config.access.user_max_unique_ips_mode,
            config.access.user_max_unique_ips_window_secs,
        )
        .await;
    if config.access.user_max_unique_ips_global_each > 0
        || !config.access.user_max_unique_ips.is_empty()
    {
        info!(
            global_each_limit = config.access.user_max_unique_ips_global_each,
            explicit_user_limits = config.access.user_max_unique_ips.len(),
            "User unique IP limits configured"
        );
    }
    let direct_buffer_hard_limit =
        resolve_direct_buffer_hard_limit(config.general.direct_relay_buffer_budget_max_bytes).await;
    let direct_buffer_budget = DirectBufferBudget::new(direct_buffer_hard_limit);
    direct_buffer_budget.activate_controller(1);
    info!(
        hard_limit_bytes = direct_buffer_hard_limit,
        configured_override_bytes = config.general.direct_relay_buffer_budget_max_bytes,
        "Direct relay buffer budget initialized"
    );
    let user_admission = UserAdmissionAuthority::new_with_quota_store(quota_store.clone());
    let traffic_limiter = TrafficLimiter::new();
    let _ = traffic_limiter.apply_policy_from_source(
        1,
        config.access.user_rate_limits.clone(),
        config.access.cidr_rate_limits.clone(),
    );
    let shared_state = ProxySharedState::new_with_process_authorities(
        direct_buffer_budget.clone(),
        traffic_limiter,
        user_admission,
    );
    let _ = shared_state.activate_user_config_source(
        1,
        None,
        &config.access.users,
        &config.access.user_enabled,
    );
    let max_connections_limit = if config.server.max_connections == 0 {
        Semaphore::MAX_PERMITS
    } else {
        config.server.max_connections as usize
    };
    let max_connections = Arc::new(Semaphore::new(max_connections_limit));
    let web_trace = WebTraceStore::new(config.web.debug.clone(), &config.web.limits);
    let web_runtime_control = WebRuntimeControl::new();

    let (admission_tx, admission_rx) = watch::channel(true);
    let (reload_control, reload_commands) = reload::ReloadControl::channel(1);
    let (active_runtime_tx, active_runtime_rx) =
        watch::channel(None::<Arc<ArcSwap<generation::RuntimeGeneration>>>);
    let (runtime_watch_tx, runtime_watch_rx) =
        watch::channel(None::<generation::RuntimeWatchState>);
    startup_tracker
        .start_component(
            COMPONENT_API_BOOTSTRAP,
            Some("spawn API listener task".to_string()),
        )
        .await;

    if config.server.api.enabled {
        let listen = match config.server.api.listen.parse::<SocketAddr>() {
            Ok(listen) => listen,
            Err(error) => {
                let message = format!(
                    "invalid server.api.listen \"{}\": {}",
                    config.server.api.listen, error
                );
                startup_tracker
                    .fail_component(COMPONENT_API_BOOTSTRAP, Some(message.clone()))
                    .await;
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into());
            }
        };
        if listen.port() != 0 {
            let api_listener = match tokio::net::TcpListener::bind(listen).await {
                Ok(listener) => listener,
                Err(error) => {
                    startup_tracker
                        .fail_component(
                            COMPONENT_API_BOOTSTRAP,
                            Some(format!("API listener bind failed on {listen}: {error}")),
                        )
                        .await;
                    return Err(error.into());
                }
            };
            let stats_api = stats.clone();
            let ip_tracker_api = ip_tracker.clone();
            let upstream_manager_api = upstream_manager.clone();
            let proxy_shared_api = shared_state.clone();
            let config_path_api = config_path.clone();
            let quota_state_api = quota_state.clone();
            let startup_tracker_api = startup_tracker.clone();
            let reload_control_api = reload_control.clone();
            let active_runtime_rx_api = active_runtime_rx.clone();
            let runtime_watch_rx_api = runtime_watch_rx.clone();
            let web_trace_api = web_trace.clone();
            let web_runtime_rx_api = web_runtime_control.subscribe();
            let api_control_plane = process_control_plane.clone();
            let api_task_control_plane = process_control_plane.clone();
            let api_task = async move {
                api::serve(
                    api_listener,
                    stats_api,
                    ip_tracker_api,
                    proxy_shared_api,
                    upstream_manager_api,
                    config_path_api,
                    quota_state_api,
                    process_started_at_epoch_secs,
                    startup_tracker_api,
                    reload_control_api,
                    active_runtime_rx_api,
                    runtime_watch_rx_api,
                    web_trace_api,
                    web_runtime_rx_api,
                    api_task_control_plane,
                )
                .await;
            };
            if api_control_plane.spawn(api_task).is_err() {
                let message = "process control-plane task admission closed during API startup";
                startup_tracker
                    .fail_component(COMPONENT_API_BOOTSTRAP, Some(message.to_string()))
                    .await;
                return Err(std::io::Error::other(message).into());
            }
            startup_tracker
                .complete_component(
                    COMPONENT_API_BOOTSTRAP,
                    Some(format!("API listener bound and supervised on {}", listen)),
                )
                .await;
        } else {
            startup_tracker
                .skip_component(
                    COMPONENT_API_BOOTSTRAP,
                    Some("server.api.listen has zero port".to_string()),
                )
                .await;
        }
    } else {
        startup_tracker
            .skip_component(
                COMPONENT_API_BOOTSTRAP,
                Some("server.api.enabled is false".to_string()),
            )
            .await;
    }

    startup_tracker
        .start_component(
            COMPONENT_NETWORK_PROBE,
            Some("probe network capabilities".to_string()),
        )
        .await;
    let probe = run_probe(&config.upstreams);
    let decision = decide_network_capabilities(&config.network, &probe);
    log_probe_result(&probe, &decision);
    startup_tracker
        .complete_component(
            COMPONENT_NETWORK_PROBE,
            Some("network capabilities determined".to_string()),
        )
        .await;

    let runtime = runtime_startup::prepare_runtime(
        config,
        &config_path,
        &decision,
        process_started_at,
        &startup_tracker,
        stats.clone(),
        upstream_manager.clone(),
        ip_tracker.clone(),
        shared_state.clone(),
        direct_buffer_budget,
        max_connections,
        runtime_task_scope.clone(),
        admission_tx,
        &runtime_log_filter,
        has_rust_log,
        &effective_log_level,
    )
    .await;
    let _admission_tx_hold = runtime.admission_tx;

    let runtime_generation = generation::RuntimeGeneration::new(
        1,
        runtime.config_rx.clone(),
        admission_rx,
        stats.clone(),
        upstream_manager.clone(),
        runtime.replay_checker,
        runtime.buffer_pool,
        runtime.rng,
        ip_tracker,
        shared_state,
        runtime.max_connections,
        runtime_task_scope,
    );
    runtime_task_scope_guard.disarm();
    let active_runtime = Arc::new(ArcSwap::from(runtime_generation));
    let bound = listeners::bind_listeners(&runtime.config, &startup_tracker).await?;
    if bound.is_empty() {
        error!("No listeners. Exiting.");
        std::process::exit(1);
    }

    drop_after_bind();

    if let Err(error) = runtime_tasks::spawn_metrics_if_configured(
        &runtime.config,
        &startup_tracker,
        active_runtime.clone(),
        web_runtime_control.subscribe(),
        process_control_plane.clone(),
    )
    .await
    {
        return Err(error.into());
    }

    runtime_watch_tx.send_replace(Some(active_runtime.load_full().watch_state()));
    active_runtime_tx.send_replace(Some(active_runtime.clone()));
    runtime_tasks::mark_runtime_ready(&startup_tracker).await;

    let listener_manager = listeners::ListenerManager::start(
        bound,
        active_runtime.clone(),
        web_trace.clone(),
        web_runtime_control,
    );
    let reload_supervisor = reload_supervisor::ReloadSupervisor::spawn(
        active_runtime.clone(),
        reload_control,
        reload_commands,
        config_path,
        quota_store,
        runtime_log_filter,
        runtime_watch_tx,
        listener_manager,
        web_trace,
    );

    shutdown::spawn_signal_handlers(
        active_runtime.clone(),
        process_started_at,
        process_control_plane.clone(),
    );
    shutdown::wait_for_shutdown(
        process_started_at,
        active_runtime,
        quota_state,
        reload_supervisor,
        process_control_plane,
    )
    .await;

    Ok(())
}
