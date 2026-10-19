use super::*;

pub(super) fn validate_vhosts(config: &mut ProxyConfig) -> Result<()> {
    let limits = &config.limits;
    // The secret-mode requirement follows the WEB-on condition: a configured
    // listener. An earlier check guarantees any listener present here is
    // network-eligible.
    let web_listener_count = usize::from(config.listener.is_some());
    let Some(vhost) = config.web.vhost.as_mut() else {
        return Ok(());
    };
    vhost.host = normalize_web_host(&vhost.host, "web.vhost.host")?;
    validate_web_base_path(&vhost.base_path, "web.vhost.base_path")?;
    if vhost.public_addr.port() != 443 || vhost.public_addr.ip().is_unspecified() {
        return config_error(
            "web.vhost.public_addr must be a concrete socket address on port 443",
        );
    }
    if web_listener_count > 0 && config.web.secret_mode.is_none() {
        return config_error("A WEB listener requires web.secret_mode");
    }
    validate_fallback(&vhost.upstream, vhost.resolve)?;
    // Every access user materializes exactly one WEB profile; the vhost
    // ceiling bounds the user count.
    if config.users.len() > limits.max_profiles {
        return config_error("WEB users exceed limits.max_profiles");
    }
    Ok(())
}

/// Normalizes optional per-user WEB hosts and validates their ceilings.
pub(crate) fn validate_user_web_fields(config: &mut ProxyConfig) -> Result<()> {
    let limits = &config.limits;
    for (user, entry) in config.users.iter_mut() {
        let field = format!("users.{user}");
        if let Some(user_host) = entry.host.as_deref() {
            entry.host = Some(normalize_web_host(
                user_host,
                &format!("{field}.host"),
            )?);
        }
        validate_user_limits(
            &entry.max_sessions,
            &entry.max_streams,
            &entry.max_streams_per_session,
            limits,
            &field,
        )?;
    }
    Ok(())
}

/// Per-user ceilings must be non-zero, within the global WEB ceilings, and
/// nested: the per-session stream ceiling cannot exceed the stream ceiling.
fn validate_user_limits(
    max_sessions: &Option<usize>,
    max_streams: &Option<usize>,
    max_streams_per_session: &Option<usize>,
    limits: &WebLimitsConfig,
    field: &str,
) -> Result<()> {
    let effective_streams = max_streams.unwrap_or(limits.max_streams_global);
    let effective_streams_per_session =
        max_streams_per_session.unwrap_or(limits.max_streams_per_session);
    if max_sessions == &Some(0)
        || max_sessions.is_some_and(|value| value > limits.max_sessions_global)
        || max_streams == &Some(0)
        || max_streams.is_some_and(|value| value > limits.max_streams_global)
        || max_streams_per_session == &Some(0)
        || max_streams_per_session
            .is_some_and(|value| value > limits.max_streams_per_session)
        || effective_streams_per_session > effective_streams
    {
        return config_error(&format!(
            "{field} limits must be non-zero and within global WEB limits"
        ));
    }
    Ok(())
}

fn validate_web_base_path(value: &str, field: &str) -> Result<()> {
    let valid = value.len() <= 128
        && !value.starts_with('/')
        && !value.ends_with('/')
        && value.split('/').all(|segment| {
            let mut bytes = segment.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        });
    if value.is_empty() || valid {
        return Ok(());
    }
    config_error(&format!(
        "{field} must be empty or contain at most 128 ASCII bytes in slash-separated [A-Za-z0-9][A-Za-z0-9_-]* segments"
    ))
}

pub(super) fn normalize_web_host(value: &str, field: &str) -> Result<String> {
    let input = value.trim();
    if input.is_empty()
        || input.ends_with('.')
        || input
            .chars()
            .any(|character| matches!(character, ':' | '/' | '?' | '#' | '@'))
    {
        return config_error(&format!(
            "{field} must be a hostname without a port, path, credentials, or trailing dot"
        ));
    }
    let host = normalize_domain_to_ascii(input, field)?;
    if host.len() > 253
        || !host.contains('.')
        || host.parse::<IpAddr>().is_ok()
        || web_host_last_label_is_numeric(&host)
    {
        return config_error(&format!(
            "{field} must be a non-IP fully-qualified hostname accepted by Telegram Desktop"
        ));
    }
    for label in host.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return config_error(&format!(
                "{field} contains a hostname label rejected by Telegram Desktop"
            ));
        }
    }
    Ok(host)
}

pub(super) fn web_host_last_label_is_numeric(host: &str) -> bool {
    let label = host.rsplit('.').next().unwrap_or_default();
    let digits = label
        .strip_prefix("0x")
        .or_else(|| label.strip_prefix("0X"));
    if let Some(digits) = digits {
        return digits.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    label.bytes().all(|byte| byte.is_ascii_digit())
}

pub(super) fn validate_fallback(upstream: &str, resolve: WebFallbackResolve) -> Result<()> {
    fallback_dns::parse_origin(upstream, resolve)?;
    Ok(())
}

pub(super) fn config_error<T>(message: &str) -> Result<T> {
    Err(ProxyError::Config(message.to_string()))
}
