use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;

use crate::config::ListenerEndpoint;
use crate::config::ListenerTransport;
use crate::config::ProxyConfig;
use crate::maestro::generation::RuntimeGeneration;

use super::accept::ListenerSlot;
use super::bind::{BoundListener, BoundListeners, PreparedListener, prepare_listener};
use super::plan::{ListenerBindSpec, listener_bind_plan};
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

/// Socket changes prepared without activating or stopping accept loops.
pub(crate) struct PreparedListenerTransition {
    target_specs: BTreeMap<ListenerEndpoint, ListenerBindSpec>,
    additions: Vec<PreparedListener>,
    removals: Vec<ListenerEndpoint>,
}

/// Activated additions and stopped removals awaiting runtime publication.
pub(crate) struct PendingListenerTransition {
    target_specs: BTreeMap<ListenerEndpoint, ListenerBindSpec>,
    additions: Vec<BoundListener>,
    removals: Vec<ListenerEndpoint>,
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
            .filter(|listener| listener.spec.transport == ListenerTransport::Web)
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

    #[cfg(test)]
    pub(crate) fn empty(active_runtime: Arc<ArcSwap<RuntimeGeneration>>) -> Self {
        let web_control = WebRuntimeControl::new();
        web_control.publish(
            WebRuntimeLifecycle::NoWebListener,
            Arc::from([]),
            std::sync::Weak::new(),
        );
        Self {
            active_runtime,
            slots: BTreeMap::new(),
            web_runtime: None,
            web_control,
            web_listeners: Arc::from([]),
        }
    }

    /// Binds added endpoints without calling `listen(2)` or changing active tasks.
    pub(crate) fn prepare_transition(
        &self,
        desired: &ProxyConfig,
    ) -> Result<Option<PreparedListenerTransition>, String> {
        let target_specs = listener_bind_plan(desired)?;
        let web_inventory_changed = self
            .slots
            .iter()
            .filter(|(_, slot)| slot.spec.transport == ListenerTransport::Web)
            .map(|(addr, slot)| (addr.clone(), slot.spec.clone()))
            .collect::<BTreeMap<_, _>>()
            != target_specs
                .iter()
                .filter(|(_, spec)| spec.transport == ListenerTransport::Web)
                .map(|(addr, spec)| (addr.clone(), spec.clone()))
                .collect::<BTreeMap<_, _>>();
        if web_inventory_changed {
            return Err(
                "WEB listener inventory is process-owned; process restart required".to_string(),
            );
        }
        let current_addresses: BTreeSet<_> = self.slots.keys().cloned().collect();
        let target_addresses: BTreeSet<_> = target_specs.keys().cloned().collect();
        if current_addresses == target_addresses
            && self
                .slots
                .iter()
                .all(|(addr, slot)| target_specs.get(addr) == Some(&slot.spec))
        {
            return Ok(None);
        }
        for addr in current_addresses.intersection(&target_addresses) {
            let current = &self.slots[addr].spec;
            let desired_spec = &target_specs[addr];
            if current != desired_spec {
                return Err(format!(
                    "listener {addr} bind policy changed at the same endpoint; process restart required"
                ));
            }
        }

        let mut additions = Vec::new();
        for addr in target_addresses.difference(&current_addresses) {
            let spec = target_specs
                .get(addr)
                .expect("address originated from target listener plan")
                .clone();
            additions.push(prepare_listener(spec).map_err(|error_value| {
                format!("failed to prepare listener {addr}: {error_value}")
            })?);
        }
        let removals = current_addresses
            .difference(&target_addresses)
            .cloned()
            .collect();
        Ok(Some(PreparedListenerTransition {
            target_specs,
            additions,
            removals,
        }))
    }

    /// Activates additions and stops removed acceptors before the runtime swap.
    pub(crate) async fn begin_transition(
        &mut self,
        prepared: PreparedListenerTransition,
    ) -> Result<PendingListenerTransition, String> {
        let mut additions = Vec::with_capacity(prepared.additions.len());
        for candidate in prepared.additions {
            additions.push(candidate.activate().map_err(|error_value| {
                format!("failed to activate prepared listener: {error_value}")
            })?);
        }

        let mut stopped = Vec::new();
        for addr in &prepared.removals {
            let stop_result = self
                .slots
                .get_mut(addr)
                .expect("removal originated from active listener inventory")
                .stop()
                .await;
            if let Err(error_value) = stop_result {
                for stopped_addr in stopped {
                    if let Some(stopped_slot) = self.slots.get_mut(&stopped_addr) {
                        stopped_slot.restart(self.active_runtime.clone());
                    }
                }
                self.slots
                    .get_mut(addr)
                    .expect("failed slot remains in active listener inventory")
                    .restart(self.active_runtime.clone());
                return Err(error_value);
            }
            stopped.push(addr.clone());
        }

        Ok(PendingListenerTransition {
            target_specs: prepared.target_specs,
            additions,
            removals: prepared.removals,
        })
    }

    /// Publishes new acceptors after the runtime generation has been swapped.
    pub(crate) fn finish_transition(&mut self, pending: PendingListenerTransition) {
        for endpoint in pending.removals {
            self.slots.remove(&endpoint);
        }
        for listener in pending.additions {
            let endpoint = listener.spec.endpoint.clone();
            self.slots.insert(
                endpoint,
                ListenerSlot::start(
                    listener,
                    self.active_runtime.clone(),
                    self.web_runtime.clone(),
                ),
            );
        }
        debug_assert_eq!(
            self.slots
                .iter()
                .map(|(endpoint, slot)| (endpoint.clone(), slot.spec.clone()))
                .collect::<BTreeMap<_, _>>(),
            pending.target_specs
        );
    }

    /// Publishes one generation through the process-owned WEB policy fence.
    pub(crate) fn activate_runtime_generation(
        &self,
        generation: Arc<RuntimeGeneration>,
    ) -> Arc<RuntimeGeneration> {
        if let Some(runtime) = &self.web_runtime {
            runtime.activate_generation(generation)
        } else {
            self.active_runtime.swap(generation)
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
    use crate::config::ListenerConfig;
    use crate::maestro::generation::test_runtime_generation;
    use crate::transport::ListenOptions;
    use tokio::net::{TcpListener, TcpStream};

    fn listener_config(addr: SocketAddr) -> ListenerConfig {
        ListenerConfig {
            ip: Some(addr.ip()),
            transport: crate::config::ListenerTransport::Web,
            port: Some(addr.port()),
            socket_path: None,
            socket_perm: None,
            web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
            web_trusted_proxy_cidrs: Vec::new(),
        }
    }

    async fn bound_listener() -> (BoundListener, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let spec = ListenerBindSpec {
            endpoint: crate::config::ListenerEndpoint::Tcp(addr),
            transport: crate::config::ListenerTransport::Web,
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
    async fn candidate_bind_failure_keeps_old_listener_accepting() {
        let runtime = test_runtime_generation(1, ProxyConfig::default());
        let active_runtime = Arc::new(ArcSwap::from(runtime.clone()));
        let (old_listener, old_addr) = bound_listener().await;
        let bound = BoundListeners {
            listeners: vec![old_listener],
        };
        let trace = WebTraceStore::new(
            runtime.config().web.debug.clone(),
            &runtime.config().web.limits,
        );
        let mut manager =
            ListenerManager::start(bound, active_runtime, trace, WebRuntimeControl::new());
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let blocked_addr = blocker.local_addr().unwrap();
        let mut desired = ProxyConfig::default();
        desired.listener = Some(listener_config(blocked_addr));

        assert!(manager.prepare_transition(&desired).is_err());
        TcpStream::connect(old_addr).await.unwrap();

        manager.shutdown().await.unwrap();
        runtime.stop_sessions().await;
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
