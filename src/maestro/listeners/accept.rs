use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::error;

use crate::config::ListenerTransport;
use crate::web::manager::{HttpConnectionAdmissionError, WebProcessRuntime};
use crate::web::telemetry::{WebAcceptorGuard, WebHttpConnectionOverloadOutcome};

use super::bind::BoundTcpListener;
use super::plan::ListenerBindSpec;
use super::web_overload;
use crate::maestro::generation::RuntimeGeneration;

/// One bound listener and all connection tasks accepted through its lifecycle.
pub(super) struct ListenerSlot {
    pub(super) spec: ListenerBindSpec,
    listener: Arc<TcpListener>,
    cancellation: CancellationToken,
    task: Option<JoinHandle<()>>,
    connections: TaskTracker,
    web_runtime: Option<Arc<WebProcessRuntime>>,
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
}

async fn run_accept_loop(
    listener: Arc<TcpListener>,
    spec: ListenerBindSpec,
    web_runtime: Option<Arc<WebProcessRuntime>>,
    connections: TaskTracker,
    cancellation: CancellationToken,
    _web_acceptor_guard: Option<WebAcceptorGuard>,
) {
    loop {
        let accepted = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return,
            accepted = listener.accept() => accepted,
        };
        match accepted {
            Ok((stream, peer_addr)) => {
                if spec.transport == ListenerTransport::Web {
                    let Some(web_runtime) = web_runtime.as_ref() else {
                        error!(addr = %spec.addr, "WEB listener has no process runtime");
                        return;
                    };
                    web_runtime.telemetry().record_accept();
                    if cancellation.is_cancelled() {
                        drop(stream);
                        continue;
                    }
                    if web_runtime.is_shutdown() {
                        web_runtime.telemetry().record_rejection(
                            crate::web::telemetry::WebRejectionReason::RuntimeClosed,
                        );
                        drop(stream);
                        continue;
                    }
                    let connection_permit = match web_runtime.try_http_connection() {
                        Ok(permit) => permit,
                        Err(HttpConnectionAdmissionError::Closed) => {
                            web_runtime.telemetry().record_rejection(
                                crate::web::telemetry::WebRejectionReason::RuntimeClosed,
                            );
                            drop(stream);
                            continue;
                        }
                        Err(HttpConnectionAdmissionError::AtCapacity) => {
                            let config = web_runtime.active_generation().config();
                            let action = config.web.http_connection_capacity_action;
                            let phase_timeout =
                                Duration::from_millis(config.web.timeouts.http_overload_timeout_ms);
                            drop(config);
                            if action == crate::config::WebHttpConnectionCapacityAction::Drop {
                                web_runtime.telemetry().record_rejection(
                                    crate::web::telemetry::WebRejectionReason::HttpConnectionCapacity,
                                );
                                web_runtime
                                    .telemetry()
                                    .record_overload(WebHttpConnectionOverloadOutcome::Dropped);
                                drop(stream);
                                continue;
                            }
                            let overload_permit = match web_runtime.try_http_overload_connection() {
                                Ok(permit) => permit,
                                Err(HttpConnectionAdmissionError::Closed) => {
                                    web_runtime.telemetry().record_rejection(
                                        crate::web::telemetry::WebRejectionReason::RuntimeClosed,
                                    );
                                    web_runtime.telemetry().record_overload(
                                        WebHttpConnectionOverloadOutcome::ShutdownDrop,
                                    );
                                    drop(stream);
                                    continue;
                                }
                                Err(HttpConnectionAdmissionError::AtCapacity) => {
                                    web_runtime.telemetry().record_rejection(
                                            crate::web::telemetry::WebRejectionReason::HttpConnectionCapacity,
                                        );
                                    web_runtime.telemetry().record_overload(
                                        WebHttpConnectionOverloadOutcome::OverflowCapacityDrop,
                                    );
                                    drop(stream);
                                    continue;
                                }
                            };
                            connections.spawn(web_overload::serve(
                                stream,
                                peer_addr,
                                spec.web_client_ip_source,
                                Arc::clone(&spec.web_trusted_proxy_cidrs),
                                Arc::clone(web_runtime),
                                cancellation.clone(),
                                overload_permit,
                                action,
                                phase_timeout,
                            ));
                            continue;
                        }
                    };
                    connections.spawn(crate::web::http::serve_connection(
                        stream,
                        peer_addr,
                        spec.web_client_ip_source,
                        Arc::clone(&spec.web_trusted_proxy_cidrs),
                        Arc::clone(web_runtime),
                        cancellation.clone(),
                        connection_permit,
                    ));
                    continue;
                }
                // Raw TCP MTProxy transports are not served in this WEB-only build.
                error!(
                    addr = %spec.addr,
                    "Listener transport is not supported in this build; dropping connection"
                );
                drop(stream);
            }
            Err(error_value) => {
                if let Some(web_runtime) = &web_runtime {
                    web_runtime.telemetry().record_accept_error();
                }
                error!(addr = %spec.addr, error = %error_value, "TCP accept error");
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
        }
    }
}

impl ListenerSlot {
    pub(super) fn start(
        bound: BoundTcpListener,
        active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
        web_runtime: Option<Arc<WebProcessRuntime>>,
    ) -> Self {
        let web_runtime = if bound.spec.transport == ListenerTransport::Web {
            web_runtime
        } else {
            None
        };
        let cancellation = CancellationToken::new();
        let connections = TaskTracker::new();
        let web_acceptor_guard = web_runtime
            .as_ref()
            .map(|runtime| runtime.telemetry().acceptor_guard());
        let task = tokio::spawn(run_accept_loop(
            bound.listener.clone(),
            bound.spec.clone(),
            web_runtime.clone(),
            connections.clone(),
            cancellation.clone(),
            web_acceptor_guard,
        ));
        Self {
            spec: bound.spec,
            listener: bound.listener,
            cancellation,
            task: Some(task),
            connections,
            web_runtime,
            active_runtime,
        }
    }

    pub(super) async fn stop(&mut self) -> Result<(), String> {
        self.request_stop();
        if let Some(task) = self.task.take() {
            task.await.map_err(|error_value| {
                format!("listener {} task failed: {error_value}", self.spec.addr)
            })?;
        }
        self.connections.close();
        let connection_stop_timeout = Duration::from_secs(
            self.active_runtime
                .load()
                .config()
                .web
                .timeouts
                .shutdown_secs,
        );
        tokio::time::timeout(connection_stop_timeout, self.connections.wait())
            .await
            .map_err(|_| format!("listener {} connection shutdown timed out", self.spec.addr))?;
        Ok(())
    }

    /// Cancels admission synchronously before the shared shutdown deadline starts draining.
    pub(super) fn request_stop(&self) {
        self.cancellation.cancel();
    }

    /// Joins this acceptor and its WEB connections by one process shutdown deadline.
    pub(super) async fn stop_until(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> Result<(), String> {
        self.request_stop();
        let mut errors = Vec::new();
        if let Some(mut task) = self.task.take() {
            let joined = if task.is_finished() {
                Some(task.await)
            } else {
                match tokio::time::timeout_at(deadline, &mut task).await {
                    Ok(result) => Some(result),
                    Err(_) => {
                        task.abort();
                        let _ = task.await;
                        None
                    }
                }
            };
            match joined {
                Some(Ok(())) => {}
                Some(Err(error_value)) => errors.push(format!(
                    "listener {} task failed: {error_value}",
                    self.spec.addr
                )),
                None => errors.push(format!(
                    "listener {} accept shutdown timed out",
                    self.spec.addr
                )),
            }
        }
        self.connections.close();
        if !self.connections.is_empty()
            && tokio::time::timeout_at(deadline, self.connections.wait())
                .await
                .is_err()
        {
            errors.push(format!(
                "listener {} connection shutdown timed out",
                self.spec.addr
            ));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    pub(super) fn restart(&mut self, active_runtime: Arc<ArcSwap<RuntimeGeneration>>) {
        self.active_runtime = active_runtime;
        self.cancellation = CancellationToken::new();
        self.connections = TaskTracker::new();
        let web_acceptor_guard = self
            .web_runtime
            .as_ref()
            .map(|runtime| runtime.telemetry().acceptor_guard());
        self.task = Some(tokio::spawn(run_accept_loop(
            self.listener.clone(),
            self.spec.clone(),
            self.web_runtime.clone(),
            self.connections.clone(),
            self.cancellation.clone(),
            web_acceptor_guard,
        )));
    }
}
