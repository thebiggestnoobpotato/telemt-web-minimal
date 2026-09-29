use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::{ListenerTransport, ProxyConfig, ServerConfig, WebClientIpSource};
use crate::transport::ListenOptions;

/// Immutable socket and connection policy for one listener endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ListenerBindSpec {
    pub(super) addr: SocketAddr,
    pub(super) transport: ListenerTransport,
    pub(super) options: ListenOptions,
    pub(super) web_client_ip_source: WebClientIpSource,
    pub(super) web_trusted_proxy_cidrs: Arc<[ipnetwork::IpNetwork]>,
}

fn listener_port_or_legacy(listener: &crate::config::ListenerConfig, server: &ServerConfig) -> u16 {
    listener.port.unwrap_or(server.port)
}

/// Derives inbound listener intent without consulting transient outbound probes.
pub(crate) fn listener_bind_plan(
    config: &ProxyConfig,
) -> Result<BTreeMap<SocketAddr, ListenerBindSpec>, String> {
    let mut plan = BTreeMap::new();

    for listener in &config.server.listeners {
        let addr = SocketAddr::new(
            listener.ip,
            listener_port_or_legacy(listener, &config.server),
        );
        if addr.is_ipv4() && !config.network.ipv4 {
            continue;
        }
        if addr.is_ipv6() && config.network.ipv6 == Some(false) {
            continue;
        }
        let spec = ListenerBindSpec {
            addr,
            transport: listener.transport,
            options: ListenOptions {
                // WEB listeners rely on the external TLS terminator for session
                // affinity, so multi-instance SO_REUSEPORT is never applied here.
                reuse_port: false,
                ipv6_only: listener.ip.is_ipv6(),
                backlog: config.server.listen_backlog,
                ..Default::default()
            },
            web_client_ip_source: listener.web_client_ip_source,
            web_trusted_proxy_cidrs: Arc::from(listener.web_trusted_proxy_cidrs.clone()),
        };
        if plan.insert(addr, spec).is_some() {
            return Err(format!("duplicate effective listener endpoint: {addr}"));
        }
    }

    Ok(plan)
}

/// Returns whether an endpoint-only change can use coordinated process rebind.
pub(crate) fn listener_rebind_supported(old: &ProxyConfig, desired: &ProxyConfig) -> bool {
    let Ok(old_plan) = listener_bind_plan(old) else {
        return false;
    };
    let Ok(desired_plan) = listener_bind_plan(desired) else {
        return false;
    };
    let old_web = old_plan
        .iter()
        .filter(|(_, spec)| spec.transport == ListenerTransport::Web)
        .collect::<BTreeMap<_, _>>();
    let desired_web = desired_plan
        .iter()
        .filter(|(_, spec)| spec.transport == ListenerTransport::Web)
        .collect::<BTreeMap<_, _>>();
    if old_web != desired_web {
        return false;
    }
    let Ok(old_plan) = listener_bind_plan(old) else {
        return false;
    };
    let Ok(desired_plan) = listener_bind_plan(desired) else {
        return false;
    };
    let retained: BTreeSet<_> = old_plan
        .keys()
        .filter(|addr| desired_plan.contains_key(addr))
        .copied()
        .collect();
    retained
        .iter()
        .all(|addr| old_plan.get(addr) == desired_plan.get(addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ListenerConfig;

    fn listener(ip: &str, port: u16) -> ListenerConfig {
        ListenerConfig {
            ip: ip.parse().unwrap(),
            transport: crate::config::ListenerTransport::Web,
            port: Some(port),
            web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
            web_trusted_proxy_cidrs: Vec::new(),
        }
    }

    #[test]
    fn plan_depends_on_inbound_family_policy_only() {
        let mut config = ProxyConfig::default();
        config.server.listeners = vec![listener("0.0.0.0", 443), listener("::", 443)];
        config.network.ipv4 = true;
        config.network.ipv6 = None;

        let plan = listener_bind_plan(&config).unwrap();

        assert_eq!(plan.len(), 2);
        config.network.ipv6 = Some(false);
        let plan = listener_bind_plan(&config).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(plan.keys().all(SocketAddr::is_ipv4));
    }

    #[test]
    fn duplicate_effective_endpoint_is_rejected() {
        let mut config = ProxyConfig::default();
        config.server.listeners = vec![listener("127.0.0.1", 443), listener("127.0.0.1", 443)];

        assert!(listener_bind_plan(&config).is_err());
    }

    #[test]
    fn retained_policy_change_is_not_rebindable() {
        let mut old = ProxyConfig::default();
        old.server.listeners = vec![listener("127.0.0.1", 443)];
        let mut desired = old.clone();
        desired
            .server
            .listeners[0]
            .web_trusted_proxy_cidrs
            .push("127.0.0.1/32".parse().unwrap());

        assert!(!listener_rebind_supported(&old, &desired));
    }

    #[test]
    fn endpoint_move_is_not_rebindable() {
        let mut old = ProxyConfig::default();
        old.server.listeners = vec![listener("127.0.0.1", 443)];
        let mut desired = old.clone();
        desired.server.listeners[0].port = Some(444);

        assert!(!listener_rebind_supported(&old, &desired));
    }
}
