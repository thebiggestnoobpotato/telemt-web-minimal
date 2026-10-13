use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;

use crate::config::ListenerEndpoint;
use crate::maestro::generation::RuntimeGeneration;

use super::accept::ListenerSlot;
use super::bind::BoundListeners;
use crate::web::control::{WebRuntimeControl, WebRuntimeLifecycle};
use crate::web::manager::{WebProcessRuntime, WebShutdownOutcome};
use crate::web::trace::WebTraceStore;

/// Process-owned listener inventory and accept-task lifecycle controller.
pub(crate) struct ListenerManager {
    active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
    slots: BTreeMap<ListenerEndpoint, ListenerSlot>,
    web_runtime: Option<Arc<WebProcessRuntime>>,
    web_control: WebRuntimeControl,
    web_listeners: Arc<[ListenerEndpoint]>,
}

impl ListenerManager {
    /// Starts accept loops for the complete startup-bound inventory.
    pub(crate) fn start(
        bound: BoundListeners,
        active_runtime: Arc<ArcSwap<RuntimeGeneration>>,
        trace: Arc<WebTraceStore>,
        web_control: WebRuntimeControl,
    ) -> Self {
        let web_listeners: Arc<[ListenerEndpoint]> = bound
            .listeners
            .iter()
            .map(|listener| listener.spec.endpoint.clone())
            .collect();
        let has_web = !web_listeners.is_empty();
        let web_runtime = has_web.then(|| {
            WebProcessRuntime::start_with_trace(
                active_runtime.clone(),
                trace,
                web_control.telemetry(),
            )
        });
        let mut slots = BTreeMap::new();
        for listener in bound.listeners {
            let endpoint = listener.spec.endpoint.clone();
            slots.insert(
                endpoint,
                ListenerSlot::start(listener, active_runtime.clone(), web_runtime.clone()),
            );
        }
        web_control.publish(
            if has_web {
                WebRuntimeLifecycle::Running
            } else {
                WebRuntimeLifecycle::NoWebListener
            },
            Arc::clone(&web_listeners),
            web_runtime
                .as_ref()
                .map_or_else(std::sync::Weak::new, Arc::downgrade),
        );
        Self {
            active_runtime,
            slots,
            web_runtime,
            web_control,
            web_listeners,
        }
    }

    /// Stops every accept task and applies one deadline to the complete WEB ingress.
    pub(crate) async fn shutdown(&mut self) -> Result<(), String> {
        self.web_control.publish(
            WebRuntimeLifecycle::Draining,
            Arc::clone(&self.web_listeners),
            self.web_runtime
                .as_ref()
                .map_or_else(std::sync::Weak::new, Arc::downgrade),
        );
        if self.web_runtime.is_none() {
            let mut errors = Vec::new();
            for slot in self.slots.values_mut() {
                if let Err(error_value) = slot.stop().await {
                    errors.push(error_value);
                }
            }
            self.slots.clear();
            self.web_control.publish(
                WebRuntimeLifecycle::Drained,
                Arc::clone(&self.web_listeners),
                std::sync::Weak::new(),
            );
            return if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("; "))
            };
        }
        let timeout_secs = self
            .active_runtime
            .load()
            .config()
            .web
            .timeouts
            .shutdown_secs;
        let now = tokio::time::Instant::now();
        let deadline = now
            .checked_add(Duration::from_secs(timeout_secs))
            .unwrap_or(now);
        for slot in self.slots.values() {
            slot.request_stop();
        }
        let Some(web_runtime) = self.web_runtime.take() else {
            return Err("WEB runtime disappeared during shutdown orchestration".to_string());
        };
        let drain = web_runtime.begin_shutdown();
        let mut errors = Vec::new();
        let slot_waits = futures_util::future::join_all(
            self.slots
                .values_mut()
                .map(|slot| slot.stop_until(deadline)),
        );
        let (slot_results, web_outcome) = tokio::join!(slot_waits, drain.wait_until(deadline));
        for result in slot_results {
            if let Err(error_value) = result {
                errors.push(error_value);
            }
        }
        if web_outcome == WebShutdownOutcome::DeadlineExceeded {
            errors.push("WEB ingress shutdown deadline exceeded".to_string());
        }
        self.slots.clear();
        self.web_control.publish(
            if web_outcome == WebShutdownOutcome::DeadlineExceeded {
                WebRuntimeLifecycle::DeadlineExceeded
            } else {
                WebRuntimeLifecycle::Drained
            },
            Arc::clone(&self.web_listeners),
            std::sync::Weak::new(),
        );
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use crate::config::ProxyConfig;
    use crate::maestro::generation::test_runtime_generation;
    use crate::maestro::listeners::bind::BoundListener;
    use crate::maestro::listeners::plan::ListenerBindSpec;
    use crate::transport::ListenOptions;
    use tokio::net::TcpListener;

    async fn bound_listener() -> (BoundListener, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let spec = ListenerBindSpec {
            endpoint: crate::config::ListenerEndpoint::Tcp(addr),
            options: ListenOptions {
                reuse_port: false,
                ..Default::default()
            },
            web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
            web_trusted_proxy_cidrs: Arc::from([]),
            socket_perm: None,
        };
        (
            BoundListener {
                handle: crate::maestro::listeners::bind::ListenerHandle::Tcp(Arc::new(listener)),
                spec,
            },
            addr,
        )
    }

    #[tokio::test]
    async fn acceptor_liveness_counts_web_listeners() {
        let runtime = test_runtime_generation(1, ProxyConfig::default());
        let active_runtime = Arc::new(ArcSwap::from(runtime.clone()));
        let (first_listener, _first_addr) = bound_listener().await;
        let (second_listener, _second_addr) = bound_listener().await;
        let bound = BoundListeners {
            listeners: vec![first_listener, second_listener],
        };
        let trace = WebTraceStore::new(
            runtime.config().web.debug.clone(),
            &runtime.config().web.limits,
        );
        let control = WebRuntimeControl::new();
        let receiver = control.subscribe();
        let mut manager = ListenerManager::start(bound, active_runtime, trace, control);

        assert_eq!(receiver.borrow().telemetry.live_acceptors(), 2);

        manager.shutdown().await.unwrap();
        runtime.stop_sessions().await;
    }
}
