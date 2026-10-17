use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::watch;
use tracing::{error, info};

use crate::api;
use crate::network::probe::{decide_network_capabilities, log_probe_result, run_probe};
use crate::proxy::direct_buffer_budget::{DirectBufferBudget, resolve_direct_buffer_hard_limit};
use crate::proxy::shared_state::ProxySharedState;
use crate::proxy::user_admission::UserAdmissionAuthority;
use crate::startup::{COMPONENT_API_BOOTSTRAP, COMPONENT_NETWORK_PROBE};
use crate::stats::telemetry::TelemetryPolicy;
use crate::stats::Stats;
use crate::transport::UpstreamManager;
use crate::web::control::WebRuntimeControl;
use crate::web::trace::WebTraceStore;

use super::{bootstrap, control_plane, generation, listeners, runtime_startup, runtime_tasks, shutdown};

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

    let stats = Arc::new(Stats::new());
    let process_control_plane = control_plane::ProcessControlPlane::new();
    let runtime_task_scope = generation::RuntimeTaskScope::new();
    let runtime_task_scope_guard =
        generation::RuntimeTaskScopePreparationGuard::new(runtime_task_scope.clone());
    stats.apply_telemetry_policy(TelemetryPolicy::from_config(&config.general));

    let upstream_manager = Arc::new(
        UpstreamManager::new(
            config.upstreams.clone(),
            config.general.upstream_connect_retry_attempts,
            config.general.upstream_connect_retry_backoff_ms,
            config.general.upstream_connect_budget_ms,
            config.general.upstream_connect_timeout,
            config.general.upstream_unhealthy_fail_threshold,
            config.general.upstream_connect_failfast_hard_errors,
            config.general.network_prefer == 6,
            stats.clone(),
        ),
    );
    let direct_buffer_hard_limit =
        resolve_direct_buffer_hard_limit(config.general.dc_buffer_budget_max_bytes).await;
    let direct_buffer_budget = DirectBufferBudget::new(direct_buffer_hard_limit);
    direct_buffer_budget.activate_controller(1);
    info!(
        hard_limit_bytes = direct_buffer_hard_limit,
        configured_override_bytes = config.general.dc_buffer_budget_max_bytes,
        "Direct relay buffer budget initialized"
    );
    let user_admission = UserAdmissionAuthority::new();
    let shared_state = ProxySharedState::new_with_direct_buffer_budget_and_user_admission(
        direct_buffer_budget.clone(),
        user_admission,
    );
    let _ = shared_state
        .activate_user_config_source(1, None, &config.user_secret_map());
    let web_trace = WebTraceStore::new(config.web.debug.clone(), &config.web.limits);
    let web_runtime_control = WebRuntimeControl::new();

    let (admission_tx, admission_rx) = watch::channel(true);
    let (active_runtime_tx, active_runtime_rx) =
        watch::channel(None::<Arc<ArcSwap<generation::RuntimeGeneration>>>);
    startup_tracker
        .start_component(
            COMPONENT_API_BOOTSTRAP,
            Some("spawn API listener task".to_string()),
        )
        .await;

    if config.api.enabled {
        let listen = match config.api.listen_address.parse::<SocketAddr>() {
            Ok(listen) => listen,
            Err(error) => {
                let message = format!(
                    "invalid api.listen_address \"{}\": {}",
                    config.api.listen_address, error
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
            let upstream_manager_api = upstream_manager.clone();
            let proxy_shared_api = shared_state.clone();
            let config_path_api = config_path.clone();
            let startup_tracker_api = startup_tracker.clone();
            let active_runtime_rx_api = active_runtime_rx.clone();
            let admission_rx_api = admission_rx.clone();
            let web_trace_api = web_trace.clone();
            let web_runtime_rx_api = web_runtime_control.subscribe();
            let api_control_plane = process_control_plane.clone();
            let api_task_control_plane = process_control_plane.clone();
            let api_task = async move {
                api::serve(
                    api_listener,
                    stats_api,
                    proxy_shared_api,
                    upstream_manager_api,
                    config_path_api,
                    process_started_at_epoch_secs,
                    startup_tracker_api,
                    active_runtime_rx_api,
                    admission_rx_api,
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
                    Some("api.listen_address has zero port".to_string()),
                )
                .await;
        }
    } else {
        startup_tracker
            .skip_component(
                COMPONENT_API_BOOTSTRAP,
                Some("api.enabled is false".to_string()),
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
    let decision = decide_network_capabilities(&config.general, &probe);
    log_probe_result(&probe, &decision);
    startup_tracker
        .complete_component(
            COMPONENT_NETWORK_PROBE,
            Some("network capabilities determined".to_string()),
        )
        .await;

    let runtime = runtime_startup::prepare_runtime(
        config,
        &decision,
        process_started_at,
        &startup_tracker,
        stats.clone(),
        upstream_manager.clone(),
        direct_buffer_budget,
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
        runtime.config.clone(),
        admission_rx,
        stats.clone(),
        upstream_manager.clone(),
        runtime.buffer_pool,
        runtime.rng,
        shared_state,
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

    active_runtime_tx.send_replace(Some(active_runtime.clone()));
    runtime_tasks::mark_runtime_ready(&startup_tracker).await;

    let listener_manager = listeners::ListenerManager::start(
        bound,
        active_runtime.clone(),
        web_trace.clone(),
        web_runtime_control,
    );
    shutdown::spawn_signal_handlers(
        active_runtime.clone(),
        process_started_at,
        process_control_plane.clone(),
    );
    shutdown::wait_for_shutdown(
        process_started_at,
        active_runtime,
        listener_manager,
        process_control_plane,
    )
    .await;

    Ok(())
}
