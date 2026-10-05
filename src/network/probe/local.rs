use super::*;

pub(super) fn detect_local_ip_v4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

pub(super) fn detect_local_ip_v6() -> Option<Ipv6Addr> {
    let socket = UdpSocket::bind("[::]:0").ok()?;
    socket.connect("[2001:4860:4860::8888]:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V6(v6) => Some(v6),
        _ => None,
    }
}

pub fn detect_interface_ipv4() -> Option<Ipv4Addr> {
    detect_local_ip_v4()
}

pub fn detect_interface_ipv6() -> Option<Ipv6Addr> {
    detect_local_ip_v6()
}

pub fn log_probe_result(probe: &NetworkProbe, decision: &NetworkDecision) {
    info!(
        ipv4 = probe
            .detected_ipv4
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
        ipv6 = probe
            .detected_ipv6
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
        ipv4_dc = decision.ipv4_dc,
        ipv6_dc = decision.ipv6_dc,
        prefer = decision.effective_prefer,
        "Network capabilities resolved"
    );
}
