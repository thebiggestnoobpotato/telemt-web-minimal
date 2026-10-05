#![allow(clippy::items_after_test_module)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};

use crate::config::{NetworkConfig, UpstreamConfig, UpstreamType};
use crate::transport::UpstreamManager;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
pub struct NetworkProbe {
    pub detected_ipv4: Option<Ipv4Addr>,
    pub detected_ipv6: Option<Ipv6Addr>,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkDecision {
    pub ipv4_dc: bool,
    pub ipv6_dc: bool,
    pub effective_prefer: u8,
}

impl NetworkDecision {
    pub fn prefer_ipv6(&self) -> bool {
        self.effective_prefer == 6
    }
}

// Resolves local egress addresses for the DC stack decision.
// Explicit Direct upstream bind addresses override interface discovery;
// strict binds discard interface addresses that cannot be confirmed by them.
pub fn run_probe(upstreams: &[UpstreamConfig]) -> NetworkProbe {
    let mut probe = NetworkProbe::default();
    let mut detected_ipv4 = detect_local_ip_v4();
    let mut detected_ipv6 = detect_local_ip_v6();
    let mut explicit_detected_ipv4 = false;
    let mut explicit_detected_ipv6 = false;
    let mut strict_bind_ipv4_requested = false;
    let mut strict_bind_ipv6_requested = false;

    for upstream in upstreams.iter().filter(|upstream| upstream.enabled) {
        let UpstreamType::Direct {
            interface,
            bind_addresses,
            ..
        } = &upstream.upstream_type
        else {
            continue;
        };
        if let Some(addrs) = bind_addresses.as_ref().filter(|v| !v.is_empty()) {
            let mut saw_parsed_ip = false;
            for value in addrs {
                if let Ok(ip) = value.parse::<IpAddr>() {
                    saw_parsed_ip = true;
                    if ip.is_ipv4() {
                        strict_bind_ipv4_requested = true;
                    } else {
                        strict_bind_ipv6_requested = true;
                    }
                }
            }
            if !saw_parsed_ip {
                strict_bind_ipv4_requested = true;
                strict_bind_ipv6_requested = true;
            }
        }

        let bind_v4 = UpstreamManager::resolve_bind_address(
            interface,
            bind_addresses,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)), 443),
            None,
            true,
        );
        let bind_v6 = UpstreamManager::resolve_bind_address(
            interface,
            bind_addresses,
            SocketAddr::new(
                IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                443,
            ),
            None,
            true,
        );

        if let Some(IpAddr::V4(ip)) = bind_v4
            && !explicit_detected_ipv4
        {
            detected_ipv4 = Some(ip);
            explicit_detected_ipv4 = true;
        }
        if let Some(IpAddr::V6(ip)) = bind_v6
            && !explicit_detected_ipv6
        {
            detected_ipv6 = Some(ip);
            explicit_detected_ipv6 = true;
        }
    }

    if strict_bind_ipv4_requested && !explicit_detected_ipv4 {
        detected_ipv4 = None;
    }
    if strict_bind_ipv6_requested && !explicit_detected_ipv6 {
        detected_ipv6 = None;
    }

    probe.detected_ipv4 = detected_ipv4;
    probe.detected_ipv6 = detected_ipv6;
    probe
}

pub fn decide_network_capabilities(config: &NetworkConfig, probe: &NetworkProbe) -> NetworkDecision {
    let ipv4_dc = config.ipv4 && probe.detected_ipv4.is_some();
    let ipv6_dc =
        config.ipv6.unwrap_or(probe.detected_ipv6.is_some()) && probe.detected_ipv6.is_some();

    let effective_prefer = match config.prefer {
        6 if ipv6_dc => 6,
        4 if ipv4_dc => 4,
        6 => {
            warn!("prefer=6 requested but IPv6 unavailable; falling back to IPv4");
            4
        }
        _ => 4,
    };

    NetworkDecision {
        ipv4_dc,
        ipv6_dc,
        effective_prefer,
    }
}

// Local interface discovery.
mod local;
pub use local::log_probe_result;
use local::{detect_local_ip_v4, detect_local_ip_v6};

#[cfg(test)]
mod tests;
