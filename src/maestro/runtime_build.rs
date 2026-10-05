use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{Semaphore, watch};

use crate::config::{
    ProxyConfig, ServerConfig, WEB_CARRIER_LEARNING_MIN_ENTRIES, web_debug_fits_limits,
};
use crate::crypto::SecureRandom;
use crate::ip_tracker::UserIpTracker;
use crate::network::probe::{decide_network_capabilities, run_probe};
use crate::proxy::direct_buffer_budget::{DirectBufferBudget, run_direct_buffer_budget_controller};
use crate::proxy::shared_state::ProxySharedState;
use crate::proxy::traffic_limiter::TrafficLimiter;
use crate::proxy::user_admission::UserAdmissionAuthority;
use crate::proxy::user_connection_authority::UserConnectionAuthority;
use crate::startup::StartupTracker;
use crate::stats::telemetry::TelemetryPolicy;
use crate::stats::{QuotaStore, ReplayChecker, Stats};
use crate::stream::BufferPool;
use crate::transport::UpstreamManager;

use super::admission;
use super::generation::{RuntimeGeneration, RuntimeTaskScope, RuntimeTaskScopePreparationGuard};
use super::listeners::listener_rebind_supported;
use super::runtime_tasks::RuntimeLogFilter;
use super::runtime_tasks;

/// Fully prepared candidate runtime and its activation-gated config watcher.
pub(crate) struct PreparedRuntime {
    /// Candidate generation ready for publication.
    pub(crate) generation: Arc<RuntimeGeneration>,
    /// Gate opened only after the candidate becomes the active generation.
    pub(crate) config_watcher_activation: watch::Sender<bool>,
    /// User-authority epoch captured before candidate construction.
    pub(crate) user_admission_epoch: u64,
}

pub(crate) async fn prepare_runtime(
    generation_id: u64,
    config: ProxyConfig,
    config_path: &Path,
    quota_store: Arc<QuotaStore>,
    connection_authority: Arc<UserConnectionAuthority>,
    runtime_log_filter: RuntimeLogFilter,
    user_admission: Arc<UserAdmissionAuthority>,
    ip_tracker: Arc<UserIpTracker>,
    traffic_limiter: Arc<TrafficLimiter>,
    direct_buffer_budget: Arc<DirectBufferBudget>,
    max_connections: Arc<Semaphore>,
) -> Result<PreparedRuntime, String> {
    let user_admission_epoch = user_admission.epoch();
    config
        .validate_web_decoy_listener_separation()
        .map_err(|error| error.to_string())?;
    let started_at_epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let startup_tracker = Arc::new(StartupTracker::new(started_at_epoch_secs));
    let task_scope = RuntimeTaskScope::new();
    let task_scope_guard = RuntimeTaskScopePreparationGuard::new(task_scope.clone());
    let stats = Arc::new(Stats::with_process_authorities(
        quota_store,
        connection_authority,
    ));
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
            stats.clone(),
        )
        .with_dns_overrides(&config.network.dns_overrides)
        .map_err(|error| format!("DNS override preparation failed: {}", error))?,
    );
    let proxy_shared = ProxySharedState::new_with_process_authorities(
        direct_buffer_budget.clone(),
        traffic_limiter,
        user_admission,
    );

    let probe = run_probe(&config.upstreams);
    let decision = decide_network_capabilities(&config.network, &probe);
    let prefer_ipv6 = decision.prefer_ipv6();

    let rng = Arc::new(SecureRandom::new());

    let config = Arc::new(config);
    let replay_checker = Arc::new(ReplayChecker::new(
        config.access.replay_check_len,
        Duration::from_secs(config.access.replay_window_secs),
    ));
    let buffer_pool = Arc::new(BufferPool::with_config(64 * 1024, 4096));
    let (config_watcher_activation, config_watcher_activation_rx) = watch::channel(false);
    let watches = runtime_tasks::spawn_runtime_tasks(
        generation_id,
        &config,
        config_path,
        prefer_ipv6,
        decision.ipv4_dc,
        decision.ipv6_dc,
        &startup_tracker,
        stats.clone(),
        upstream_manager.clone(),
        replay_checker.clone(),
        ip_tracker.clone(),
        proxy_shared.clone(),
        task_scope.clone(),
        Some(config_watcher_activation_rx),
    )
    .await;
    let config_rx = watches.config_rx;
    runtime_log_filter.spawn_watcher(watches.log_level_rx, task_scope.clone());
    // This build always relays directly to DCs, so admission opens immediately.
    let (admission_tx, admission_rx) = watch::channel(true);
    admission::configure_admission_gate(&admission_tx).await;

    task_scope.spawn(run_direct_buffer_budget_controller(
        generation_id,
        direct_buffer_budget,
        buffer_pool.clone(),
        stats.clone(),
        max_connections.clone(),
        config.server.max_connections,
    ));
    let generation = RuntimeGeneration::new(
        generation_id,
        config_rx,
        admission_rx,
        stats,
        upstream_manager,
        replay_checker,
        buffer_pool,
        rng,
        ip_tracker,
        proxy_shared,
        max_connections,
        task_scope,
    );
    task_scope_guard.disarm();
    drop(admission_tx);

    Ok(PreparedRuntime {
        generation,
        config_watcher_activation,
        user_admission_epoch,
    })
}

pub(crate) struct ResolvedReloadConfig {
    /// Runtime-safe candidate with process-owned values retained from active state.
    pub(crate) effective: ProxyConfig,
    /// Stable public labels for desired fields deferred until process restart.
    pub(crate) deferred_process_fields: Vec<String>,
    /// Whether activating the effective candidate changes runtime-owned state.
    pub(crate) runtime_changed: bool,
}

/// Resolves desired configuration into effective runtime and deferred process state.
pub(crate) fn resolve_reload_config(
    old: &ProxyConfig,
    desired: &ProxyConfig,
) -> Result<ResolvedReloadConfig, String> {
    let mut effective = desired.clone();
    let mut fields = Vec::new();
    let listener_identity_matches = listeners_have_same_bind_identity(&old.server, &desired.server);
    let global_listener_policy_changed = old.server.port != desired.server.port
        || old.server.listen_backlog != desired.server.listen_backlog;
    let listener_policy_changed =
        listener_identity_matches && !listener_process_fields_equal(&old.server, &desired.server);
    let unsupported_identity_change =
        !listener_identity_matches && !listener_rebind_supported(old, desired);
    if global_listener_policy_changed || listener_policy_changed || unsupported_identity_change {
        fields.push("server.listeners".to_string());
        effective.server.port = old.server.port;
        effective.server.listen_backlog = old.server.listen_backlog;
        effective.server.listeners = old.server.listeners.clone();
    }
    if old.server.api.listen != desired.server.api.listen
        || old.server.api.enabled != desired.server.api.enabled
    {
        fields.push("server.api.listen".to_string());
        effective.server.api.listen = old.server.api.listen.clone();
        effective.server.api.enabled = old.server.api.enabled;
    }
    if old.server.api.runtime_edge_events_capacity
        != desired.server.api.runtime_edge_events_capacity
    {
        fields.push("server.api.runtime_edge_events_capacity".to_string());
        effective.server.api.runtime_edge_events_capacity =
            old.server.api.runtime_edge_events_capacity;
    }
    if old.server.metrics_listen != desired.server.metrics_listen
        || old.server.metrics_port != desired.server.metrics_port
    {
        fields.push("server.metrics_listen".to_string());
        effective.server.metrics_listen = old.server.metrics_listen.clone();
        effective.server.metrics_port = old.server.metrics_port;
    }
    if old.server.max_connections != desired.server.max_connections {
        fields.push("server.max_connections".to_string());
        effective.server.max_connections = old.server.max_connections;
    }
    if old.general.direct_relay_buffer_budget_max_bytes
        != desired.general.direct_relay_buffer_budget_max_bytes
    {
        fields.push("general.direct_relay_buffer_budget_max_bytes".to_string());
        effective.general.direct_relay_buffer_budget_max_bytes =
            old.general.direct_relay_buffer_budget_max_bytes;
    }
    if old.general.quota_state_path != desired.general.quota_state_path {
        fields.push("general.quota_state_path".to_string());
        effective.general.quota_state_path = old.general.quota_state_path.clone();
    }
    if old.general.data_path != desired.general.data_path {
        fields.push("general.data_path".to_string());
        effective.general.data_path = old.general.data_path.clone();
    }
    // `logging.log_level` is hot-reloadable; the remaining logging fields are
    // process-owned and deferred until restart. `logging.show_users` is
    // process-owned too: links are emitted once at listener bind time.
    let logging_process_fields_changed =
        old.logging.destination != desired.logging.destination
            || old.logging.path != desired.logging.path
            || old.logging.show_users != desired.logging.show_users
            || old.logging.unknown_dc_log_enabled
                != desired.logging.unknown_dc_log_enabled;
    if logging_process_fields_changed {
        fields.push("logging".to_string());
        effective.logging = old.logging.clone();
        effective.logging.log_level = desired.logging.log_level;
    }
    if serde_json::to_value(&old.web.limits).ok() != serde_json::to_value(&desired.web.limits).ok()
    {
        fields.push("web.limits".to_string());
        effective.web.limits = old.web.limits.clone();
    }
    if old.web.decoy_fasttrack_mode != desired.web.decoy_fasttrack_mode {
        fields.push("web.decoy_fasttrack_mode".to_string());
        effective.web.decoy_fasttrack_mode = old.web.decoy_fasttrack_mode;
    }
    if effective.web.carrier_negotiation_enabled()
        && effective.web.carrier_learning
        && effective.web.limits.max_carrier_learning_entries < WEB_CARRIER_LEARNING_MIN_ENTRIES
    {
        if old.web.carrier_learning != desired.web.carrier_learning {
            fields.push("web.carrier_learning".to_string());
            effective.web.carrier_learning = old.web.carrier_learning;
        } else {
            fields.push("web.carriers".to_string());
            effective.web.carriers = old.web.carriers.clone();
        }
    }
    if !web_debug_fits_limits(&effective.web.debug, &effective.web.limits) {
        fields.push("web.debug".to_string());
        effective.web.debug = old.web.debug.clone();
    }
    effective
        .validate_effective_web()
        .map_err(|error| format!("effective WEB configuration is invalid: {error}"))?;
    effective
        .rebuild_runtime_user_auth()
        .map_err(|error| format!("effective user runtime preparation failed: {error}"))?;
    effective
        .rebuild_runtime_web()
        .map_err(|error| format!("effective WEB runtime preparation failed: {error}"))?;
    let runtime_changed = !configs_equal(old, &effective);
    Ok(ResolvedReloadConfig {
        effective,
        deferred_process_fields: fields,
        runtime_changed,
    })
}

fn listeners_have_same_bind_identity(old: &ServerConfig, desired: &ServerConfig) -> bool {
    old.listeners.len() == desired.listeners.len()
        && old
            .listeners
            .iter()
            .zip(&desired.listeners)
            .all(|(old_listener, desired_listener)| {
                old_listener.ip == desired_listener.ip
                    && old_listener.port.unwrap_or(old.port)
                        == desired_listener.port.unwrap_or(desired.port)
            })
}

// Every remaining listener field is process-bound (the accept loop reads its
// policy from the bind-time spec), so the whole listener list is compared.
fn listener_process_fields_equal(old: &ServerConfig, desired: &ServerConfig) -> bool {
    serde_json::to_value(&old.listeners).ok() == serde_json::to_value(&desired.listeners).ok()
}

/// Returns process-owned fields that cannot change in the current generation.
pub(crate) fn deferred_process_fields(
    old: &ProxyConfig,
    new: &ProxyConfig,
) -> Result<Vec<String>, String> {
    resolve_reload_config(old, new).map(|resolved| resolved.deferred_process_fields)
}

fn configs_equal(old: &ProxyConfig, new: &ProxyConfig) -> bool {
    serde_json::to_value(old).ok() == serde_json::to_value(new).ok()
}

#[cfg(test)]
#[path = "runtime_build_tests.rs"]
mod tests;
