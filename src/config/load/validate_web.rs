use std::collections::HashSet;
use std::path::Path;

use super::*;

// Debug capture validation is independent from restart-only storage limits.
mod debug;
// Memory-envelope arithmetic remains isolated from protocol validation.
mod memory;
// Carrier ordering, cumulative deadlines, and fallback identity are validated together.
mod negotiation;
// Request and lifecycle timeout relationships are validated together.
mod timeouts;
// WebSocket transport policy is validated independently from HTTP body policy.
mod websocket;

const WEB_FRAME_HEADER_BYTES: usize = 8;
const WEB_QUEUE_ITEM_COST: usize = 256;
const WEB_CONTROL_EXTRA_ITEMS: usize = 16;
const WEB_CONTROL_ITEMS_PER_STREAM: usize = 3;
const WEB_INITIAL_STREAM_WINDOW: usize = 4 * 1024 * 1024;
const MAX_WEB_HEADER_BYTES: usize = 64 * 1024;
const MAX_WEB_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_WEB_FRAME_BYTES: usize = 1024 * 1024;
const MAX_WEB_FRAMES_PER_BODY: usize = 4096;
const MAX_WEB_TOMBSTONES_PER_SESSION: usize = 4096;
const MAX_WEB_MEMORY_ENVELOPE_BYTES: usize = 4 * 1024 * 1024 * 1024;

/// Validates WEB policy and resource bounds before building runtime state.
pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    validate_source(config)?;
    validate_fallback_listener_separation(config)
}

/// Validates source policy without requiring DNS evidence for opted-in origins.
pub(super) fn validate_source(config: &mut ProxyConfig) -> Result<()> {
    let web_listener_count = config
        .listener
        .iter()
        .filter(|listener| listener.transport == ListenerTransport::Web)
        .count();
    let eligible_web_listener_count = config
        .listener
        .iter()
        .filter(|listener| listener.transport == ListenerTransport::Web)
        .filter(|listener| listener_is_network_eligible(listener, config))
        .count();

    if let Some(listener) = &config.listener {
        validate_web_listener(listener)?;
    }

    if config.web.enabled && eligible_web_listener_count == 0 {
        return Err(ProxyError::Config(
            "web.enabled requires a network-eligible [listener] entry with transport=web"
                .to_string(),
        ));
    }
    if web_listener_count > 0 && config.web.vhosts.is_empty() {
        return Err(ProxyError::Config(
            "A WEB listener requires at least one [[web.vhosts]] entry".to_string(),
        ));
    }

    validate_limits(&config.web.limits)?;
    debug::validate(&config.web.debug, &config.web.limits)?;
    let carriers = negotiation::validate(&config.web)?;
    if carriers.contains(&WebCarrier::Https) && config.web.limits.max_http_handlers < 2 {
        return config_error("WEB https candidates require web.limits.max_http_handlers >= 2");
    }
    if carriers.contains(&WebCarrier::HttpsLanes) && config.web.limits.max_http_handlers < 4 {
        return config_error(
            "WEB https-lanes candidates require web.limits.max_http_handlers >= 4",
        );
    }
    timeouts::validate(&config.web.timeouts)?;
    websocket::validate(&carriers, &config.web.limits, &config.web.timeouts)?;
    validate_vhosts(config)?;
    validate_fallback_listener_separation_inner(config, false)?;
    Ok(())
}

/// Rejects a direct fallback recursion into an effective WEB listener.
pub(super) fn validate_fallback_listener_separation(config: &ProxyConfig) -> Result<()> {
    validate_fallback_listener_separation_inner(config, true)
}

fn validate_fallback_listener_separation_inner(
    config: &ProxyConfig,
    require_resolved: bool,
) -> Result<()> {
    let web_endpoints = config
        .listener
        .iter()
        .filter(|listener| listener.transport == ListenerTransport::Web)
        .filter(|listener| listener_is_network_eligible(listener, config))
        .filter_map(ListenerEndpoint::from_listener)
        .collect::<Vec<_>>();
    for (vhost_idx, vhost) in config.web.vhosts.iter().enumerate() {
        let WebFallbackConfig::HttpUpstream { upstream, resolve } = &vhost.fallback;
        let origin = fallback_dns::parse_origin(vhost_idx, upstream, *resolve)?;
        for endpoint in fallback_dns::addresses(config, vhost_idx, &origin, require_resolved)? {
            if web_endpoints
                .iter()
                .any(|listener| fallback_endpoint_overlaps(listener, &endpoint))
            {
                return config_error(&format!(
                    "web.vhosts[{vhost_idx}].fallback upstream overlaps WEB listener {endpoint}"
                ));
            }
        }
    }
    Ok(())
}

/// Unix socket paths and TCP endpoints never collide across namespaces.
fn fallback_endpoint_overlaps(listener: &ListenerEndpoint, target: &FallbackEndpoint) -> bool {
    match (listener, target) {
        (ListenerEndpoint::Tcp(listener), FallbackEndpoint::Tcp(target)) => {
            listener_covers(*listener, *target)
        }
        (ListenerEndpoint::Unix(listener), FallbackEndpoint::Unix(target)) => listener == target,
        _ => false,
    }
}

/// Unix socket listeners are eligible on every address family; TCP listeners
/// follow the configured family policy.
fn listener_is_network_eligible(listener: &ListenerConfig, config: &ProxyConfig) -> bool {
    matches!(
        ListenerEndpoint::from_listener(listener),
        Some(ListenerEndpoint::Unix(_))
    ) || matches!(
        ListenerEndpoint::from_listener(listener),
        Some(ListenerEndpoint::Tcp(addr))
            if (addr.is_ipv4() && config.general.network_ipv4)
                || (addr.is_ipv6() && config.general.network_ipv6 != Some(false))
    )
}

fn listener_covers(listener: SocketAddr, target: SocketAddr) -> bool {
    listener.port() == target.port()
        && (listener.ip() == target.ip()
            || (listener.ip().is_unspecified() && listener.is_ipv4() == target.is_ipv4()))
}

fn validate_web_listener(
    listener: &ListenerConfig,
) -> Result<()> {
    if let Some(path) = &listener.socket_path {
        // The socket file is the trust boundary: no remote host can reach
        // this listener, so the TCP-only non-empty and /0 CIDR checks are
        // skipped. The synthetic loopback peer is still matched against the
        // configured CIDRs at request time.
        if !Path::new(path).is_absolute() {
            return Err(ProxyError::Config(
                "listener.socket_path must be an absolute path".to_string(),
            ));
        }
        if listener.ip.is_some() || listener.port.is_some() {
            return Err(ProxyError::Config(
                "listener.socket_path cannot be combined with ip or port".to_string(),
            ));
        }
        return Ok(());
    }
    if listener.ip.is_none() || listener.port.is_none() {
        return Err(ProxyError::Config(
            "listener requires both ip and port, or socket_path".to_string(),
        ));
    }
    if listener.web_trusted_proxy_cidrs.is_empty() {
        return Err(ProxyError::Config(
            "listener.web_trusted_proxy_cidrs must be non-empty for transport=web".to_string(),
        ));
    }
    if listener
        .web_trusted_proxy_cidrs
        .iter()
        .any(|network| network.prefix() == 0)
    {
        return Err(ProxyError::Config(
            "listener.web_trusted_proxy_cidrs must not contain a /0 network".to_string(),
        ));
    }
    Ok(())
}

fn validate_limits(limits: &WebLimitsConfig) -> Result<()> {
    if !(8192..=MAX_WEB_HEADER_BYTES).contains(&limits.max_header_bytes) {
        return config_error("web.limits.max_header_bytes must be within [8192, 65536]");
    }
    if !(WEB_FRAME_HEADER_BYTES..=MAX_WEB_BODY_BYTES).contains(&limits.max_body_bytes) {
        return config_error("web.limits.max_body_bytes must be within [8, 16777216]");
    }
    if !(1..=MAX_WEB_FRAME_BYTES).contains(&limits.max_frame_payload_bytes) {
        return config_error("web.limits.max_frame_payload_bytes must be within [1, 1048576]");
    }
    if !(1..=MAX_WEB_FRAMES_PER_BODY).contains(&limits.max_frames_per_body) {
        return config_error("web.limits.max_frames_per_body must be within [1, 4096]");
    }
    if !(1..=MAX_WEB_TOMBSTONES_PER_SESSION).contains(&limits.max_tombstones_per_session) {
        return config_error("web.limits.max_tombstones_per_session must be within [1, 4096]");
    }
    if limits.pending_bytes_per_lane <= WEB_FRAME_HEADER_BYTES + WEB_QUEUE_ITEM_COST {
        return config_error(
            "web.limits.pending_bytes_per_lane must preserve one non-empty DATA frame",
        );
    }
    if limits.carrier_batch_bytes > limits.max_body_bytes
        || limits.carrier_batch_bytes
            < limits
                .max_frame_payload_bytes
                .saturating_add(WEB_FRAME_HEADER_BYTES)
    {
        return config_error(
            "web.limits.carrier_batch_bytes must fit max_body_bytes and one maximum frame",
        );
    }
    if limits.max_frame_payload_bytes > WEB_INITIAL_STREAM_WINDOW {
        return config_error(
            "web.limits.max_frame_payload_bytes must not exceed the initial stream window",
        );
    }

    let positive = [
        ("max_http_connections", limits.max_http_connections),
        (
            "max_http_overload_connections",
            limits.max_http_overload_connections,
        ),
        ("max_http_handlers", limits.max_http_handlers),
        (
            "max_lane_open_waits_per_session",
            limits.max_lane_open_waits_per_session,
        ),
        ("pending_bytes_per_lane", limits.pending_bytes_per_lane),
        ("pending_items_per_lane", limits.pending_items_per_lane),
        (
            "max_websocket_evictions_in_flight",
            limits.max_websocket_evictions_in_flight,
        ),
        (
            "max_carrier_learning_entries",
            limits.max_carrier_learning_entries,
        ),
        ("max_body_readers", limits.max_body_readers),
        ("max_body_bytes_global", limits.max_body_bytes_global),
        ("max_sessions_global", limits.max_sessions_global),
        ("max_sessions_per_ip", limits.max_sessions_per_ip),
        ("max_streams_per_session", limits.max_streams_per_session),
        ("max_streams_global", limits.max_streams_global),
        ("max_stream_handshakes", limits.max_stream_handshakes),
        (
            "pending_bytes_per_session",
            limits.pending_bytes_per_session,
        ),
        ("pending_bytes_global", limits.pending_bytes_global),
        (
            "pending_items_per_session",
            limits.pending_items_per_session,
        ),
        ("pending_items_global", limits.pending_items_global),
        (
            "control_bytes_per_session",
            limits.control_bytes_per_session,
        ),
        ("control_bytes_global", limits.control_bytes_global),
        ("max_bootstraps_global", limits.max_bootstraps_global),
        ("max_bootstraps_per_ip", limits.max_bootstraps_per_ip),
        ("max_vhosts", limits.max_vhosts),
        ("max_profiles", limits.max_profiles),
        ("debug_records_capacity", limits.debug_records_capacity),
        ("debug_bytes_global", limits.debug_bytes_global),
        ("memory_envelope_bytes", limits.memory_envelope_bytes),
    ];
    if let Some((field, _)) = positive.into_iter().find(|(_, value)| *value == 0) {
        return config_error(&format!("web.limits.{field} must be > 0"));
    }
    for (field, value) in [
        ("max_http_connections", limits.max_http_connections),
        (
            "max_http_overload_connections",
            limits.max_http_overload_connections,
        ),
        ("max_http_handlers", limits.max_http_handlers),
        ("max_body_readers", limits.max_body_readers),
        ("max_body_bytes_global", limits.max_body_bytes_global),
        ("max_stream_handshakes", limits.max_stream_handshakes),
    ] {
        if value > tokio::sync::Semaphore::MAX_PERMITS {
            return config_error(&format!(
                "web.limits.{field} exceeds Tokio semaphore capacity"
            ));
        }
    }
    let rates = [
        (
            "new_bootstraps_per_minute",
            limits.new_bootstraps_per_minute,
        ),
        ("new_bootstraps_burst", limits.new_bootstraps_burst),
        ("new_sessions_per_minute", limits.new_sessions_per_minute),
        ("new_sessions_burst", limits.new_sessions_burst),
        ("new_streams_per_minute", limits.new_streams_per_minute),
        ("new_streams_burst", limits.new_streams_burst),
    ];
    if let Some((field, _)) = rates.into_iter().find(|(_, value)| *value == 0) {
        return config_error(&format!("web.limits.{field} must be > 0"));
    }
    if limits.max_streams_per_session > u16::MAX as usize {
        return config_error("web.limits.max_streams_per_session must fit synthetic source ports");
    }
    if limits.max_sessions_per_ip > limits.max_sessions_global
        || limits.max_streams_per_session > limits.max_streams_global
        || limits.max_stream_handshakes > limits.max_streams_global
        || limits.max_bootstraps_per_ip > limits.max_bootstraps_global
        || limits.max_http_handlers > limits.max_http_connections
        || limits.max_body_readers > limits.max_http_handlers
        || limits.max_lane_open_waits_per_session > limits.max_streams_per_session
        || limits.pending_bytes_per_session > limits.pending_bytes_global
        || limits.pending_items_per_session > limits.pending_items_global
        || limits.pending_bytes_per_lane > limits.pending_bytes_per_session
        || limits.pending_items_per_lane > limits.pending_items_per_session
        || limits.control_bytes_per_session > limits.control_bytes_global
        || limits.control_bytes_per_session > limits.pending_bytes_per_session
        || limits.control_bytes_global > limits.pending_bytes_global
    {
        return config_error("web.limits per-owner ceilings must not exceed global ceilings");
    }
    let control_items_per_session = WEB_CONTROL_EXTRA_ITEMS
        .checked_add(
            limits
                .max_streams_per_session
                .checked_mul(WEB_CONTROL_ITEMS_PER_STREAM)
                .ok_or_else(|| {
                    ProxyError::Config(
                        "web.limits control item reservation overflowed usize".to_string(),
                    )
                })?,
        )
        .ok_or_else(|| {
            ProxyError::Config("web.limits control item reservation overflowed usize".to_string())
        })?;
    let control_items_global = control_items_per_session
        .checked_mul(limits.max_sessions_global)
        .ok_or_else(|| {
            ProxyError::Config("web.limits global control reservation overflowed usize".to_string())
        })?;
    let control_frame_cost = WEB_FRAME_HEADER_BYTES + 4 + WEB_QUEUE_ITEM_COST;
    let required_control_bytes_per_session = control_items_per_session
        .checked_mul(control_frame_cost)
        .ok_or_else(|| {
            ProxyError::Config("web.limits control byte reservation overflowed usize".to_string())
        })?;
    let required_control_bytes_global = control_items_global
        .checked_mul(control_frame_cost)
        .ok_or_else(|| {
            ProxyError::Config(
                "web.limits global control byte reservation overflowed usize".to_string(),
            )
        })?;
    if control_items_per_session >= limits.pending_items_per_session
        || control_items_global >= limits.pending_items_global
        || required_control_bytes_per_session > limits.control_bytes_per_session
        || required_control_bytes_global > limits.control_bytes_global
    {
        return config_error(
            "web.limits control reserves must cover bounded control frames and leave data capacity",
        );
    }
    let uplink_bytes = limits
        .max_frames_per_body
        .checked_mul(WEB_QUEUE_ITEM_COST)
        .and_then(|value| value.checked_add(limits.max_body_bytes))
        .ok_or_else(|| {
            ProxyError::Config("web.limits uplink reservation overflowed usize".to_string())
        })?;
    let minimum_downlink_frame_bytes = WEB_FRAME_HEADER_BYTES + 1 + WEB_QUEUE_ITEM_COST;
    let session_required_bytes = limits
        .control_bytes_per_session
        .checked_add(uplink_bytes)
        .and_then(|value| value.checked_add(minimum_downlink_frame_bytes))
        .ok_or_else(|| {
            ProxyError::Config("web.limits session reservation overflowed usize".to_string())
        })?;
    let session_required_items = control_items_per_session
        .checked_add(limits.max_frames_per_body)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            ProxyError::Config("web.limits session item reservation overflowed usize".to_string())
        })?;
    let global_required_bytes = limits
        .control_bytes_global
        .checked_add(uplink_bytes)
        .and_then(|value| value.checked_add(minimum_downlink_frame_bytes))
        .ok_or_else(|| {
            ProxyError::Config("web.limits global reservation overflowed usize".to_string())
        })?;
    let global_required_items = control_items_global
        .checked_add(limits.max_frames_per_body)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            ProxyError::Config("web.limits global item reservation overflowed usize".to_string())
        })?;
    if session_required_bytes > limits.pending_bytes_per_session
        || session_required_items > limits.pending_items_per_session
        || global_required_bytes > limits.pending_bytes_global
        || global_required_items > limits.pending_items_global
    {
        return config_error(
            "web.limits pending ceilings must preserve one uplink batch and downlink progress",
        );
    }
    memory::validate(limits)?;
    Ok(())
}

// Virtual-host, hostname, and fallback validation.
mod vhosts;
use vhosts::*;

#[cfg(test)]
mod tests;
