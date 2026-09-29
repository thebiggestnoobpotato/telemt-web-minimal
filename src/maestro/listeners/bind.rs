use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use socket2::Socket;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::config::ProxyConfig;
use crate::startup::{COMPONENT_LISTENERS_BIND, StartupTracker};
use crate::transport::find_listener_processes;
use crate::transport::socket::{activate_listener_socket, bind_listener_socket};

use super::plan::{ListenerBindSpec, listener_bind_plan};
use crate::maestro::helpers::print_web_proxy_links;

/// Owns sockets bound before process accept loops start.
pub(crate) struct BoundListeners {
    pub(super) listeners: Vec<BoundTcpListener>,
}

impl BoundListeners {
    pub(crate) fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

/// Active socket and immutable connection policy for one endpoint.
pub(super) struct BoundTcpListener {
    pub(super) listener: Arc<TcpListener>,
    pub(super) spec: ListenerBindSpec,
}

/// Socket bound for a candidate transition but not yet listening.
pub(super) struct PreparedTcpListener {
    socket: Socket,
    spec: ListenerBindSpec,
}

fn log_bind_error(addr: SocketAddr, error_value: &std::io::Error) {
    if error_value.kind() == std::io::ErrorKind::AddrInUse {
        let owners = find_listener_processes(addr);
        if owners.is_empty() {
            error!(%addr, "Failed to bind: address already in use (owner process unresolved)");
        } else {
            for owner in owners {
                error!(
                    %addr,
                    pid = owner.pid,
                    process = %owner.process,
                    "Failed to bind: address already in use"
                );
            }
        }
    } else {
        error!(%addr, error = %error_value, "Failed to bind listener");
    }
}

pub(super) fn prepare_listener(spec: ListenerBindSpec) -> std::io::Result<PreparedTcpListener> {
    match bind_listener_socket(spec.addr, &spec.options) {
        Ok(socket) => Ok(PreparedTcpListener { socket, spec }),
        Err(error_value) => {
            log_bind_error(spec.addr, &error_value);
            Err(error_value)
        }
    }
}

impl PreparedTcpListener {
    pub(super) fn activate(self) -> std::io::Result<BoundTcpListener> {
        activate_listener_socket(&self.socket, self.spec.options.backlog)?;
        let listener = TcpListener::from_std(self.socket.into())?;
        Ok(BoundTcpListener {
            listener: Arc::new(listener),
            spec: self.spec,
        })
    }
}

fn log_listener_profile(spec: &ListenerBindSpec) {
    info!(addr = %spec.addr, transport = ?spec.transport, "Listening on TCP endpoint");
}

/// Binds every eligible configured listener or fails without a partial inventory.
pub(crate) async fn bind_listeners(
    config: &Arc<ProxyConfig>,
    startup_tracker: &Arc<StartupTracker>,
) -> Result<BoundListeners, Box<dyn Error>> {
    startup_tracker
        .start_component(
            COMPONENT_LISTENERS_BIND,
            Some("bind TCP listeners".to_string()),
        )
        .await;
    let plan = listener_bind_plan(config).map_err(std::io::Error::other)?;
    let mut prepared = Vec::with_capacity(plan.len());
    for spec in plan.values().cloned() {
        prepared.push(prepare_listener(spec)?);
    }
    let mut listeners = Vec::with_capacity(prepared.len());
    for candidate in prepared {
        let bound = candidate.activate()?;
        log_listener_profile(&bound.spec);
        listeners.push(bound);
    }
    print_web_proxy_links(config);

    startup_tracker
        .complete_component(
            COMPONENT_LISTENERS_BIND,
            Some(format!("listeners configured tcp={}", listeners.len())),
        )
        .await;

    Ok(BoundListeners { listeners })
}


