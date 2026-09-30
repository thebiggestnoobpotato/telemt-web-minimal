use super::*;

pub(super) fn map_route_kind(value: UpstreamRouteKind) -> &'static str {
    match value {
        UpstreamRouteKind::Direct => "direct",
        UpstreamRouteKind::Socks5 => "socks5",
    }
}

pub(super) fn map_ip_preference(value: IpPreference) -> &'static str {
    match value {
        IpPreference::Unknown => "unknown",
        IpPreference::PreferV6 => "prefer_v6",
        IpPreference::PreferV4 => "prefer_v4",
        IpPreference::BothWork => "both_work",
        IpPreference::Unavailable => "unavailable",
    }
}

pub(super) fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
