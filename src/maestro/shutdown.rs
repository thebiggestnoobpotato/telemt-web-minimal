//! Shutdown and signal handling for telemt.
//!
//! Handles graceful shutdown on various signals:
//! - SIGINT (Ctrl+C) / SIGTERM: Graceful shutdown
//! - SIGQUIT: Graceful shutdown with stats dump
//! - SIGUSR1: Reserved for log rotation (logs acknowledgment)
//! - SIGUSR2: Dump runtime status to log
//!
//! SIGHUP is handled separately in config/hot_reload.rs for config reload.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
#[cfg(not(unix))]
use tokio::signal;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tracing::{info, warn};

use super::control_plane::ProcessControlPlane;
use super::generation::RuntimeGeneration;
use super::helpers::{format_uptime, unit_label};
use super::reload_supervisor::ReloadSupervisorHandle;
use crate::conntrack_control::FirewallAuthority;
use crate::quota_state::QuotaStateOwner;
use crate::stats::Stats;

/// Signal that triggered shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// SIGINT (Ctrl+C)
    Interrupt,
    /// SIGTERM
    Terminate,
    /// SIGQUIT (with stats dump)
    Quit,
}

impl std::fmt::Display for ShutdownSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShutdownSignal::Interrupt => write!(f, "SIGINT"),
            ShutdownSignal::Terminate => write!(f, "SIGTERM"),
            ShutdownSignal::Quit => write!(f, "SIGQUIT"),
        }
    }
}

/// Waits for a shutdown signal and performs graceful shutdown.
pub(crate) async fn wait_for_shutdown(
    process_started_at: Instant,
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    quota_state: Arc<QuotaStateOwner>,
    reload_supervisor: ReloadSupervisorHandle,
    conntrack_firewall: Option<FirewallAuthority>,
    process_control_plane: ProcessControlPlane,
) {
    let signal = wait_for_shutdown_signal().await;
    perform_shutdown(
        signal,
        process_started_at,
        active_runtime,
        quota_state,
        reload_supervisor,
        conntrack_firewall,
        process_control_plane,
    )
    .await;
}

/// Waits for any shutdown signal (SIGINT, SIGTERM, SIGQUIT).
#[cfg(unix)]
async fn wait_for_shutdown_signal() -> ShutdownSignal {
    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sigquit = signal(SignalKind::quit()).expect("Failed to register SIGQUIT handler");

    tokio::select! {
        _ = sigint.recv() => ShutdownSignal::Interrupt,
        _ = sigterm.recv() => ShutdownSignal::Terminate,
        _ = sigquit.recv() => ShutdownSignal::Quit,
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> ShutdownSignal {
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    ShutdownSignal::Interrupt
}

/// Performs graceful shutdown sequence.
async fn perform_shutdown(
    signal: ShutdownSignal,
    process_started_at: Instant,
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    quota_state: Arc<QuotaStateOwner>,
    reload_supervisor: ReloadSupervisorHandle,
    conntrack_firewall: Option<FirewallAuthority>,
    process_control_plane: ProcessControlPlane,
) {
    let shutdown_started_at = Instant::now();
    info!(signal = %signal, "Received shutdown signal");

    let listener_manager = reload_supervisor.quiesce().await;
    let runtime = active_runtime.load_full();
    let stats = runtime.stats.as_ref();

    // Dump stats if SIGQUIT
    if signal == ShutdownSignal::Quit {
        dump_stats(stats, process_started_at);
    }

    info!("Shutting down...");
    let uptime_secs = process_started_at.elapsed().as_secs();
    info!("Uptime: {}", format_uptime(uptime_secs));

    if let Err(error) = listener_manager.lock().await.shutdown().await {
        warn!(error = %error, "Failed to stop one or more listener tasks cleanly");
    }

    runtime.stop_sessions().await;
    runtime.stop_background_tasks().await;

    if let Some(conntrack_firewall) = conntrack_firewall
        && !conntrack_firewall.shutdown_and_clear().await
    {
        warn!("Conntrack firewall cleanup did not complete successfully");
    }

    if !process_control_plane.shutdown(Duration::from_secs(5)).await {
        warn!("Process control-plane task shutdown deadline expired");
    }

    let configured_quota_users = runtime
        .config()
        .access
        .users
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    match quota_state.save(&configured_quota_users).await {
        Ok(()) => {
            info!(
                path = %quota_state.path().display(),
                "Persisted per-user quota state"
            );
        }
        Err(error) => {
            warn!(
                error = %error,
                path = %quota_state.path().display(),
                "Failed to persist per-user quota state"
            );
        }
    }

    let shutdown_secs = shutdown_started_at.elapsed().as_secs();
    info!(
        "Shutdown completed successfully in {} {}.",
        shutdown_secs,
        unit_label(shutdown_secs, "second", "seconds")
    );
}

/// Dumps runtime statistics to the log.
fn dump_stats(stats: &Stats, process_started_at: Instant) {
    let uptime_secs = process_started_at.elapsed().as_secs();

    info!("=== Runtime Statistics Dump ===");
    info!("Uptime: {}", format_uptime(uptime_secs));

    // Connection stats
    info!(
        "Connections: total={}, current={} (direct={}), bad={}",
        stats.get_connects_all(),
        stats.get_current_connections_total(),
        stats.get_current_connections_direct(),
        stats.get_connects_bad(),
    );


    info!("=== End Statistics Dump ===");
}

/// Spawns a background task to handle operational signals (SIGUSR1, SIGUSR2).
///
/// These signals don't trigger shutdown but perform specific actions:
/// - SIGUSR1: Log rotation acknowledgment (for external log rotation tools)
/// - SIGUSR2: Dump runtime status to log
#[cfg(unix)]
pub(crate) fn spawn_signal_handlers(
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    process_started_at: Instant,
    process_control_plane: ProcessControlPlane,
) {
    let _ = process_control_plane.spawn(async move {
        let mut sigusr1 =
            signal(SignalKind::user_defined1()).expect("Failed to register SIGUSR1 handler");
        let mut sigusr2 =
            signal(SignalKind::user_defined2()).expect("Failed to register SIGUSR2 handler");

        loop {
            tokio::select! {
                _ = sigusr1.recv() => {
                    handle_sigusr1();
                }
                _ = sigusr2.recv() => {
                    let runtime = active_runtime.load_full();
                    handle_sigusr2(runtime.stats.as_ref(), process_started_at);
                }
            }
        }
    });
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
pub(crate) fn spawn_signal_handlers(
    _active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    _process_started_at: Instant,
    _process_control_plane: ProcessControlPlane,
) {
    // No SIGUSR1/SIGUSR2 on non-Unix
}

/// Handles SIGUSR1 - log rotation signal.
///
/// This signal is typically sent by logrotate or similar tools after
/// rotating log files. Since tracing-subscriber doesn't natively support
/// reopening files, we just acknowledge the signal. If file logging is
/// added in the future, this would reopen log file handles.
#[cfg(unix)]
fn handle_sigusr1() {
    info!("SIGUSR1 received - log rotation acknowledged");
    // Future: If using file-based logging, reopen file handles here
}

/// Handles SIGUSR2 - dump runtime status.
#[cfg(unix)]
fn handle_sigusr2(stats: &Stats, process_started_at: Instant) {
    info!("SIGUSR2 received - dumping runtime status");
    dump_stats(stats, process_started_at);
}
