use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::config::{ListenerEndpoint, ListenerTransport, ProxyConfig, WebClientIpSource};
use crate::transport::ListenOptions;

/// Immutable socket and connection policy for one listener endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ListenerBindSpec {
    pub(super) endpoint: ListenerEndpoint,
    pub(super) transport: ListenerTransport,
    pub(super) options: ListenOptions,
    pub(super) web_client_ip_source: WebClientIpSource,
    pub(super) web_trusted_proxy_cidrs: Arc<[ipnetwork::IpNetwork]>,
    pub(super) socket_perm: Option<String>,
}

/// Derives inbound listener intent without consulting transient outbound probes.
pub(crate) fn listener_bind_plan(
    config: &ProxyConfig,
) -> Result<BTreeMap<ListenerEndpoint, ListenerBindSpec>, String> {
    let mut plan = BTreeMap::new();

    for listener in &config.server.listeners {
        let endpoint = ListenerEndpoint::from_listener(listener).ok_or(
            "listener endpoint is incomplete; each entry requires ip and port, or socket_path",
        )?;
        if let ListenerEndpoint::Tcp(addr) = &endpoint {
            if addr.is_ipv4() && !config.general.network_ipv4 {
                continue;
            }
            if addr.is_ipv6() && config.general.network_ipv6 == Some(false) {
                continue;
            }
        }
        let spec = ListenerBindSpec {
            endpoint: endpoint.clone(),
            transport: listener.transport,
            options: ListenOptions {
                // WEB listeners rely on the external TLS terminator for session
                // affinity, so multi-instance SO_REUSEPORT is never applied here.
                reuse_port: false,
                ipv6_only: matches!(endpoint, ListenerEndpoint::Tcp(addr) if addr.is_ipv6()),
                backlog: config.general.listen_backlog,
                ..Default::default()
            },
            web_client_ip_source: listener.web_client_ip_source,
            web_trusted_proxy_cidrs: Arc::from(listener.web_trusted_proxy_cidrs.clone()),
            socket_perm: listener.socket_perm.clone(),
        };
        if plan.contains_key(&endpoint) {
            return Err(format!("duplicate effective listener endpoint: {endpoint}"));
        }
        plan.insert(endpoint, spec);
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
        .filter(|endpoint| desired_plan.contains_key(endpoint))
        .cloned()
        .collect();
    retained
        .iter()
        .all(|endpoint| old_plan.get(endpoint) == desired_plan.get(endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ListenerConfig;

    fn listener(ip: &str, port: u16) -> ListenerConfig {
        ListenerConfig {
            ip: Some(ip.parse().unwrap()),
            transport: crate::config::ListenerTransport::Web,
            port: Some(port),
            socket_path: None,
            socket_perm: None,
            web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
            web_trusted_proxy_cidrs: Vec::new(),
        }
    }

    fn unix_listener(path: &str) -> ListenerConfig {
        ListenerConfig {
            ip: None,
            transport: crate::config::ListenerTransport::Web,
            port: None,
            socket_path: Some(path.to_string()),
            socket_perm: None,
            web_client_ip_source: crate::config::WebClientIpSource::XForwardedFor,
            web_trusted_proxy_cidrs: Vec::new(),
        }
    }

    #[test]
    fn plan_depends_on_inbound_family_policy_only() {
        let mut config = ProxyConfig::default();
        config.server.listeners = vec![listener("0.0.0.0", 443), listener("::", 443)];
        config.general.network_ipv4 = true;
        config.general.network_ipv6 = None;

        let plan = listener_bind_plan(&config).unwrap();

        assert_eq!(plan.len(), 2);
        config.general.network_ipv6 = Some(false);
        let plan = listener_bind_plan(&config).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(plan
            .keys()
            .all(|endpoint| matches!(endpoint, ListenerEndpoint::Tcp(addr) if addr.is_ipv4())));
    }

    #[test]
    fn unix_listeners_are_eligible_regardless_of_family_policy() {
        let mut config = ProxyConfig::default();
        config.server.listeners = vec![unix_listener("/tmp/telemt-plan-test.sock")];
        config.general.network_ipv4 = false;
        config.general.network_ipv6 = Some(false);

        let plan = listener_bind_plan(&config).unwrap();

        assert_eq!(plan.len(), 1);
        assert!(plan
            .keys()
            .all(|endpoint| matches!(endpoint, ListenerEndpoint::Unix(_))));
    }

    #[test]
    fn duplicate_effective_endpoint_is_rejected() {
        let mut config = ProxyConfig::default();
        config.server.listeners = vec![listener("127.0.0.1", 443), listener("127.0.0.1", 443)];

        assert!(listener_bind_plan(&config).is_err());
    }

    #[test]
    fn duplicate_effective_unix_endpoint_is_rejected() {
        let mut config = ProxyConfig::default();
        config.server.listeners = vec![
            unix_listener("/tmp/telemt-plan-dup.sock"),
            unix_listener("/tmp/telemt-plan-dup.sock"),
        ];

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
