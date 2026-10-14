use super::*;

// Process, buffer, and TLS cache metrics.
mod process;
// Connection metrics.
mod connections;
// Upstream metrics.
mod traffic;
// Bounded per-user metrics.
mod users;

pub(super) fn render_metrics(
    stats: &Stats,
    shared_state: &ProxySharedState,
    config: &ProxyConfig,
    web_publication: &crate::web::control::WebRuntimePublication,
) -> String {
    let mut out = String::with_capacity(4096);
    let telemetry = stats.telemetry_policy();
    let core_enabled = telemetry.core_enabled;
    let user_enabled = telemetry.user_enabled;

    process::render(&mut out, stats, shared_state, telemetry);
    connections::render(&mut out, stats, shared_state, core_enabled);
    traffic::render(&mut out, stats, core_enabled);
    users::render(&mut out, stats, user_enabled);
    super::web::render(&mut out, web_publication, config);
    out
}
