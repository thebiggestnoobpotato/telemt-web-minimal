use tokio::sync::watch;

use crate::config::ProxyConfig;

// Dedicated log target for startup WEB proxy link lines. `log_filter_spec`
// pins this target to INFO under every `[logging].log_level`, so links reach
// the configured destination (stderr, syslog, or file) even at Silent level.
const WEB_LINKS_LOG_TARGET: &str = "telemt::links";

/// Emits WEB proxy links for all users when `[logging].show_users_links` is
/// set, through the tracing subscriber so they follow the configured log
/// destination.
pub(crate) fn print_web_proxy_links(config: &ProxyConfig) {
    if !config.web.enabled || !config.logging.show_users_links {
        return;
    }
    let Some(runtime) = config.web.runtime.as_ref() else {
        return;
    };
    let mut heading_printed = false;
    for profile in &runtime.profiles {
        if !heading_printed {
            tracing::info!(target: WEB_LINKS_LOG_TARGET, "WEB proxy links");
            heading_printed = true;
        }
        let Some(secret) = config.access.users.get(&profile.user) else {
            continue;
        };
        let Some(vhost) = config
            .web
            .vhosts
            .iter()
            .find(|vhost| vhost.host == profile.host)
        else {
            continue;
        };
        tracing::info!(
            target: WEB_LINKS_LOG_TARGET,
            "User: {} ({:?})",
            profile.user,
            profile.secret_mode
        );
        if let Some(link) = crate::web::links::format_web_proxy_link(
            &profile.host,
            &vhost.base_path,
            secret,
            profile.secret_mode,
        ) {
            tracing::info!(target: WEB_LINKS_LOG_TARGET, "WEB: {link}");
        }
    }
}

/// Selects a singular or plural display label for one integer value.
pub(crate) fn unit_label(value: u64, singular: &'static str, plural: &'static str) -> &'static str {
    if value == 1 { singular } else { plural }
}

/// Formats process uptime into bounded human-readable units and exact seconds.
pub(crate) fn format_uptime(total_secs: u64) -> String {
    const SECS_PER_MINUTE: u64 = 60;
    const SECS_PER_HOUR: u64 = 60 * SECS_PER_MINUTE;
    const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
    const SECS_PER_MONTH: u64 = 30 * SECS_PER_DAY;
    const SECS_PER_YEAR: u64 = 12 * SECS_PER_MONTH;

    let mut remaining = total_secs;
    let years = remaining / SECS_PER_YEAR;
    remaining %= SECS_PER_YEAR;
    let months = remaining / SECS_PER_MONTH;
    remaining %= SECS_PER_MONTH;
    let days = remaining / SECS_PER_DAY;
    remaining %= SECS_PER_DAY;
    let hours = remaining / SECS_PER_HOUR;
    remaining %= SECS_PER_HOUR;
    let minutes = remaining / SECS_PER_MINUTE;
    let seconds = remaining % SECS_PER_MINUTE;

    let mut parts = Vec::new();
    if total_secs > SECS_PER_YEAR {
        parts.push(format!("{} {}", years, unit_label(years, "year", "years")));
    }
    if total_secs > SECS_PER_MONTH {
        parts.push(format!(
            "{} {}",
            months,
            unit_label(months, "month", "months")
        ));
    }
    if total_secs > SECS_PER_DAY {
        parts.push(format!("{} {}", days, unit_label(days, "day", "days")));
    }
    if total_secs > SECS_PER_HOUR {
        parts.push(format!("{} {}", hours, unit_label(hours, "hour", "hours")));
    }
    if total_secs > SECS_PER_MINUTE {
        parts.push(format!(
            "{} {}",
            minutes,
            unit_label(minutes, "minute", "minutes")
        ));
    }
    parts.push(format!(
        "{} {}",
        seconds,
        unit_label(seconds, "second", "seconds")
    ));

    format!("{} / {} seconds", parts.join(", "), total_secs)
}

#[allow(dead_code)]
/// Waits until admission opens or its watch channel closes.
pub(crate) async fn wait_until_admission_open(admission_rx: &mut watch::Receiver<bool>) -> bool {
    loop {
        if *admission_rx.borrow() {
            return true;
        }
        if admission_rx.changed().await.is_err() {
            return *admission_rx.borrow();
        }
    }
}

/// Classifies peer closure that is expected during an incomplete handshake.
pub(crate) fn is_expected_handshake_eof(err: &crate::error::ProxyError) -> bool {
    expected_handshake_close_description(err).is_some()
}

/// Returns a stable diagnostic description for transport-level peer closure.
pub(crate) fn peer_close_description(err: &crate::error::ProxyError) -> Option<&'static str> {
    fn from_kind(kind: std::io::ErrorKind) -> Option<&'static str> {
        match kind {
            std::io::ErrorKind::ConnectionReset => Some("Peer reset TCP connection (RST)"),
            std::io::ErrorKind::ConnectionAborted => {
                Some("Peer aborted TCP connection during transport")
            }
            std::io::ErrorKind::BrokenPipe => Some("Peer closed write side (broken pipe)"),
            std::io::ErrorKind::NotConnected => Some("Socket was already closed by peer"),
            _ => None,
        }
    }

    match err {
        crate::error::ProxyError::Io(ioe) => from_kind(ioe.kind()),
        crate::error::ProxyError::Stream(crate::error::StreamError::Io(ioe)) => {
            from_kind(ioe.kind())
        }
        _ => None,
    }
}

/// Returns a stable diagnostic description for expected handshake closure.
pub(crate) fn expected_handshake_close_description(
    err: &crate::error::ProxyError,
) -> Option<&'static str> {
    fn from_kind(kind: std::io::ErrorKind) -> Option<&'static str> {
        match kind {
            std::io::ErrorKind::UnexpectedEof => {
                Some("Peer closed before sending full 64-byte MTProto handshake")
            }
            std::io::ErrorKind::ConnectionReset => {
                Some("Peer reset TCP connection during initial MTProto handshake")
            }
            std::io::ErrorKind::ConnectionAborted => {
                Some("Peer aborted TCP connection during initial MTProto handshake")
            }
            std::io::ErrorKind::BrokenPipe => {
                Some("Peer closed write side before MTProto handshake completed")
            }
            std::io::ErrorKind::NotConnected => Some("Handshake socket was already closed by peer"),
            _ => None,
        }
    }

    match err {
        crate::error::ProxyError::Io(ioe) => from_kind(ioe.kind()),
        crate::error::ProxyError::Stream(crate::error::StreamError::UnexpectedEof) => {
            Some("Peer closed before sending full 64-byte MTProto handshake")
        }
        crate::error::ProxyError::Stream(crate::error::StreamError::Io(ioe)) => {
            from_kind(ioe.kind())
        }
        _ => None,
    }
}
