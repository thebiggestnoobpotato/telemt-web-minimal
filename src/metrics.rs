use std::collections::{BTreeSet, HashMap};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::config::ProxyConfig;
use crate::ip_tracker::UserIpTracker;
use crate::maestro::control_plane::ProcessControlPlane;
use crate::maestro::generation::RuntimeGeneration;
use crate::proxy::shared_state::ProxySharedState;
use crate::stats::Stats;
use crate::transport::{ListenOptions, create_listener};

// Process-owned WEB metrics stay isolated from the legacy renderer body.
mod web;

// Keeps `/metrics` response size bounded when per-user telemetry is enabled.
const USER_LABELED_METRICS_MAX_USERS: usize = 4096;
const METRICS_MAX_CONTROL_CONNECTIONS: usize = 512;
const METRICS_HTTP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

/// Bound process-owned metrics listeners ready for supervised serving.
pub(crate) struct BoundMetricsListeners {
    listeners: Vec<(TcpListener, SocketAddr)>,
}

/// Binds every configured metrics socket before process readiness is published.
pub(crate) fn bind(
    port: u16,
    listen: Option<String>,
    listen_backlog: u32,
) -> std::io::Result<BoundMetricsListeners> {
    if let Some(ref listen_addr) = listen {
        let addr: SocketAddr = listen_addr.parse().map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid metrics.listen address {listen_addr}: {error}"),
            )
        })?;
        // Match `api.listen`: `[::]:port` is a dual-stack wildcard
        // on Linux when `net.ipv6.bindv6only=0`.
        let ipv6_only = addr.is_ipv6() && !addr.ip().is_unspecified();
        let listener = bind_metrics_listener(addr, ipv6_only, listen_backlog)?;
        return Ok(BoundMetricsListeners {
            listeners: vec![(listener, addr)],
        });
    }

    let mut listeners = Vec::with_capacity(2);
    let mut last_error = None;

    let addr_v4 = SocketAddr::from(([127, 0, 0, 1], port));
    match bind_metrics_listener(addr_v4, false, listen_backlog) {
        Ok(listener) => listeners.push((listener, addr_v4)),
        Err(e) => {
            warn!(error = %e, "Failed to bind metrics on {}", addr_v4);
            last_error = Some(e);
        }
    }

    let addr_v6 = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], port));
    match bind_metrics_listener(addr_v6, true, listen_backlog) {
        Ok(listener) => listeners.push((listener, addr_v6)),
        Err(e) => {
            warn!(error = %e, "Failed to bind metrics on {}", addr_v6);
            last_error = Some(e);
        }
    }

    if listeners.is_empty() {
        return Err(last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "metrics listener is unavailable on both IPv4 and IPv6",
            )
        }));
    }
    Ok(BoundMetricsListeners { listeners })
}

/// Starts supervised accept loops for previously bound metrics sockets.
pub(crate) fn serve(
    bound: BoundMetricsListeners,
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    web_runtime_rx: tokio::sync::watch::Receiver<crate::web::control::WebRuntimePublication>,
    control_plane: ProcessControlPlane,
) {
    for (listener, addr) in bound.listeners {
        info!("Metrics endpoint: http://{}/metrics", addr);
        let active_runtime = active_runtime.clone();
        let web_runtime_rx = web_runtime_rx.clone();
        let listener_scope = control_plane.clone();
        let _ = control_plane.spawn(async move {
            serve_listener(listener, active_runtime, web_runtime_rx, listener_scope).await;
        });
    }
}

fn bind_metrics_listener(
    addr: SocketAddr,
    ipv6_only: bool,
    listen_backlog: u32,
) -> std::io::Result<TcpListener> {
    let options = ListenOptions {
        reuse_port: false,
        ipv6_only,
        backlog: listen_backlog,
        ..Default::default()
    };
    let socket = create_listener(addr, &options)?;
    TcpListener::from_std(socket.into())
}

async fn serve_listener(
    listener: TcpListener,
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    web_runtime_rx: tokio::sync::watch::Receiver<crate::web::control::WebRuntimePublication>,
    control_plane: ProcessControlPlane,
) {
    let connection_permits = Arc::new(Semaphore::new(METRICS_MAX_CONTROL_CONNECTIONS));

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Metrics accept error");
                continue;
            }
        };

        let runtime = active_runtime.load_full();
        let config = runtime.config();
        if !config.metrics.whitelist.is_empty()
            && !config
                .metrics
                .whitelist
                .iter()
                .any(|net| net.contains(peer.ip()))
        {
            debug!(peer = %peer, "Metrics request denied by whitelist");
            continue;
        }

        let connection_permit = match connection_permits.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                debug!(
                    peer = %peer,
                    max_connections = METRICS_MAX_CONTROL_CONNECTIONS,
                    "Dropping metrics connection: control-plane connection budget exhausted"
                );
                continue;
            }
        };

        let active_runtime = active_runtime.clone();
        let web_runtime_rx = web_runtime_rx.clone();
        let _ = control_plane.spawn(async move {
            let _connection_permit = connection_permit;
            let svc = service_fn(move |req| {
                let runtime = active_runtime.load_full();
                let web_publication = web_runtime_rx.borrow().clone();
                async move { handle(req, &runtime, &web_publication).await }
            });
            match timeout(
                METRICS_HTTP_CONNECTION_TIMEOUT,
                http1::Builder::new().serve_connection(hyper_util::rt::TokioIo::new(stream), svc),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    debug!(error = %e, "Metrics connection error");
                }
                Err(_) => {
                    debug!(
                        peer = %peer,
                        timeout_ms = METRICS_HTTP_CONNECTION_TIMEOUT.as_millis() as u64,
                        "Metrics connection timed out"
                    );
                }
            }
        });
    }
}

async fn handle<B>(
    req: Request<B>,
    runtime: &RuntimeGeneration,
    web_publication: &crate::web::control::WebRuntimePublication,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let stats = &runtime.stats;
    let shared_state = &runtime.proxy_shared;
    let ip_tracker = &runtime.ip_tracker;
    let config = runtime.config();

    if req.uri().path() == "/metrics" {
        let body = render_metrics(stats, shared_state, &config, ip_tracker, web_publication).await;
        let resp = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
        return Ok(resp);
    }

    let resp = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from("Not Found\n")))
        .unwrap();
    Ok(resp)
}

// Ordered Prometheus text rendering split by bounded metric families.
mod render;
use render::render_metrics;

#[cfg(test)]
mod tests;
