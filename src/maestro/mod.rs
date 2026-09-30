//! telemt — Telegram MTProto Proxy

#![allow(unused_assignments)]

// Runtime orchestration modules.
// - admission: conditional-cast gate and route mode switching.
// - bootstrap: configuration and tracing initialization.
// - connectivity: startup ME/DC connectivity diagnostics.
// - control_plane: process-owned API, metrics, and signal task lifecycle.
// - generation: runtime generation state and task ownership.
// - helpers: CLI and shared startup/runtime helper routines.
// - listeners: TCP/Unix listener planning, binding, and lifecycle control.
// - orchestrator: process startup, listener activation, and shutdown sequencing.
// - reload: reload command coordination.
// - reload_supervisor: generation and listener transition supervision.
// - runtime_build: reload candidate construction.
// - runtime_startup: initial runtime generation preparation.
// - runtime_tasks: hot-reload and background task orchestration.
// - shutdown: graceful shutdown sequence and uptime logging.
mod admission;
mod bootstrap;
mod connectivity;
pub(crate) mod control_plane;
pub(crate) mod generation;
mod helpers;
mod listeners;
mod orchestrator;
pub(crate) mod reload;
mod reload_supervisor;
pub(crate) mod runtime_build;
mod runtime_startup;
mod runtime_tasks;
mod shutdown;

use tracing::error;

#[cfg(unix)]
use crate::daemon::{DaemonOptions, PidFile, drop_privileges};

/// Runs the full telemt runtime startup pipeline and blocks until shutdown.
///
/// On Unix, daemon options should be handled before calling this function
/// because daemonization must happen before the Tokio runtime starts.
#[cfg(unix)]
pub async fn run_with_daemon(
    daemon_opts: DaemonOptions,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    run_inner(daemon_opts).await
}

/// Runs the full telemt runtime startup pipeline and blocks until shutdown.
///
/// This is the main entry point for non-daemon mode or library callers.
#[allow(dead_code)]
pub async fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let daemon_opts = crate::cli::parse_daemon_args(&args);
        run_inner(daemon_opts).await
    }
    #[cfg(not(unix))]
    {
        run_inner().await
    }
}

#[cfg(unix)]
async fn run_inner(
    daemon_opts: DaemonOptions,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Acquire PID file if daemonizing or if explicitly requested.
    // Keep it alive until shutdown for RAII cleanup.
    let _pid_file = if daemon_opts.daemonize || daemon_opts.pid_file.is_some() {
        let mut pf = PidFile::new(daemon_opts.pid_file_path());
        if let Err(e) = pf.acquire() {
            eprintln!("[telemt] {}", e);
            std::process::exit(1);
        }
        Some(pf)
    } else {
        None
    };

    let user = daemon_opts.user.clone();
    let group = daemon_opts.group.clone();

    orchestrator::run_telemt_core(|| {
        if (user.is_some() || group.is_some())
            && let Err(e) = drop_privileges(user.as_deref(), group.as_deref(), _pid_file.as_ref())
        {
            error!(error = %e, "Failed to drop privileges");
            std::process::exit(1);
        }
    })
    .await
}

#[cfg(not(unix))]
async fn run_inner() -> std::result::Result<(), Box<dyn std::error::Error>> {
    orchestrator::run_telemt_core(|| {}).await
}
