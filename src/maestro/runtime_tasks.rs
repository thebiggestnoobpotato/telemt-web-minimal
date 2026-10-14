use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;
use tracing_subscriber::reload;

use crate::config::{LogLevel, ProxyConfig};
use crate::metrics;
use crate::startup::{
    COMPONENT_METRICS_START, COMPONENT_RUNTIME_READY, StartupTracker,
};
use crate::stats::Stats;
use crate::transport::UpstreamManager;

use super::control_plane::ProcessControlPlane;
use super::generation::RuntimeGeneration;
use super::generation::RuntimeTaskScope;

#[derive(Clone)]
pub(crate) struct RuntimeLogFilter {
    handle: reload::Handle<EnvFilter, Registry>,
}

impl RuntimeLogFilter {
    pub(crate) fn new(handle: reload::Handle<EnvFilter, Registry>) -> Self {
        Self { handle }
    }

    pub(crate) fn start(&self, has_rust_log: bool, effective_log_level: &LogLevel) {
        self.apply(effective_log_level, has_rust_log);
    }

    fn apply(&self, level: &LogLevel, has_rust_log: bool) {
        let runtime_filter = EnvFilter::new(log_filter_spec(has_rust_log, level));
        if let Err(error) = self.handle.reload(runtime_filter) {
            tracing::error!(error = %error, "Failed to update runtime log filter");
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn spawn_runtime_tasks(
    config: &Arc<ProxyConfig>,
    prefer_ipv6: bool,
    decision_ipv4_dc: bool,
    decision_ipv6_dc: bool,
    stats: Arc<Stats>,
    upstream_manager: Arc<UpstreamManager>,
    task_scope: RuntimeTaskScope,
) {
    let um_clone = upstream_manager.clone();
    let dc_overrides_for_health = config.general.dc_overrides.clone();
    task_scope.spawn(async move {
        um_clone
            .run_health_checks(
                prefer_ipv6,
                decision_ipv4_dc,
                decision_ipv6_dc,
                dc_overrides_for_health,
            )
            .await;
    });

    let stats_maintenance = stats.clone();
    task_scope.spawn(async move {
        stats_maintenance
            .run_periodic_user_stats_maintenance()
            .await;
    });

}

pub(crate) fn log_filter_spec(has_rust_log: bool, effective_log_level: &LogLevel) -> String {
    if has_rust_log {
        std::env::var("RUST_LOG")
            .unwrap_or_else(|_| effective_log_level.to_filter_str().to_string())
    } else if matches!(effective_log_level, LogLevel::Silent) {
        "warn,telemt::links=info".to_string()
    } else {
        effective_log_level.to_filter_str().to_string()
    }
}

pub(crate) async fn spawn_metrics_if_configured(
    config: &Arc<ProxyConfig>,
    startup_tracker: &Arc<StartupTracker>,
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    web_runtime_rx: tokio::sync::watch::Receiver<crate::web::control::WebRuntimePublication>,
    control_plane: ProcessControlPlane,
) -> std::io::Result<()> {
    if !config.metrics.enabled {
        startup_tracker
            .skip_component(
                COMPONENT_METRICS_START,
                Some("metrics.enabled is false".to_string()),
            )
            .await;
        return Ok(());
    }

    let listen = match config.metrics.listen_address.clone() {
        Some(address) => address,
        None => {
            let message =
                "metrics.listen_address is required when metrics.enabled is true".to_string();
            startup_tracker
                .fail_component(COMPONENT_METRICS_START, Some(message.clone()))
                .await;
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, message));
        }
    };

    match listen.parse::<std::net::SocketAddr>() {
        Ok(_) => {}
        Err(e) => {
            let message = format!("invalid metrics.listen_address \"{}\": {}", listen, e);
            startup_tracker
                .fail_component(COMPONENT_METRICS_START, Some(message.clone()))
                .await;
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, message));
        }
    }

    startup_tracker
        .start_component(
            COMPONENT_METRICS_START,
            Some(format!("spawn metrics endpoint on {}", listen)),
        )
        .await;
    let listen_backlog = config.general.listen_backlog;
    let bound = match metrics::bind(listen, listen_backlog) {
        Ok(bound) => bound,
        Err(error) => {
            startup_tracker
                .fail_component(
                    COMPONENT_METRICS_START,
                    Some(format!("metrics listener bind failed: {error}")),
                )
                .await;
            return Err(error);
        }
    };
    metrics::serve(
        bound,
        active_runtime,
        web_runtime_rx,
        control_plane,
    );
    startup_tracker
        .complete_component(
            COMPONENT_METRICS_START,
            Some("metrics listener bound and supervised".to_string()),
        )
        .await;
    Ok(())
}

pub(crate) async fn mark_runtime_ready(startup_tracker: &Arc<StartupTracker>) {
    startup_tracker
        .complete_component(
            COMPONENT_RUNTIME_READY,
            Some("startup pipeline is fully initialized".to_string()),
        )
        .await;
    startup_tracker.mark_ready().await;
}

#[cfg(test)]
mod tests {
    use super::log_filter_spec;
    use crate::config::LogLevel;

    #[test]
    fn log_filter_spec_pins_links_target_at_silent() {
        // Silent is the only level whose base filter (`warn`) would drop the
        // INFO-level link lines; the dedicated target directive keeps them
        // visible in the configured destination.
        assert_eq!(
            log_filter_spec(false, &LogLevel::Silent),
            "warn,telemt::links=info"
        );
    }

    #[test]
    fn log_filter_spec_uses_level_filter_for_other_levels() {
        assert_eq!(log_filter_spec(false, &LogLevel::Debug), "trace");
        assert_eq!(log_filter_spec(false, &LogLevel::Verbose), "debug");
        assert_eq!(log_filter_spec(false, &LogLevel::Normal), "info");
    }
}
