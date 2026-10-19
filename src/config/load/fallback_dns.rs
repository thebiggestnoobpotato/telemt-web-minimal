use std::future::Future;
use std::time::Duration;

use super::*;

/// Parsed fallback origin: an http origin, or a unix socket path without DNS.
pub(super) enum FallbackOrigin {
    /// http origin validated for the configured resolve mode.
    Http { url: url::Url },
    /// `unix:` origin; the socket path is the complete target.
    Unix { path: std::path::PathBuf },
}

/// Parses one origin and preserves the existing literal-address containment policy.
pub(super) fn parse_origin(upstream: &str, resolve: WebFallbackResolve) -> Result<FallbackOrigin> {
    // The unix: prefix is intercepted before URL parsing; the remainder must
    // be an absolute socket path without URL control characters.
    if let Some(rest) = upstream.strip_prefix("unix:") {
        if resolve == WebFallbackResolve::Startup {
            return Err(ProxyError::Config(
                "web.vhost.resolve is only valid for http origins".to_string(),
            ));
        }
        if rest.is_empty() {
            return Err(ProxyError::Config(
                "web.vhost.upstream unix origin requires a socket path".to_string(),
            ));
        }
        if rest.contains('?') || rest.contains('#') || rest.contains(' ') || rest.contains('\\') {
            return Err(ProxyError::Config(
                "web.vhost.upstream unix origin must not contain a query, fragment, or whitespace"
                    .to_string(),
            ));
        }
        let path: std::path::PathBuf =
            std::path::Path::new(rest).components().collect();
        if !path.is_absolute() {
            return Err(ProxyError::Config(
                "web.vhost.upstream unix origin must be an absolute socket path".to_string(),
            ));
        }
        return Ok(FallbackOrigin::Unix { path });
    }
    let parsed = url::Url::parse(upstream).map_err(|error| {
        ProxyError::Config(format!("web.vhost.upstream is invalid: {error}"))
    })?;
    if parsed.scheme() != "http"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
        || parsed.port() == Some(0)
    {
        return Err(ProxyError::Config(
            "web.vhost.upstream must be an http origin without credentials, path, query, or fragment"
                .to_string(),
        ));
    }
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => validate_address(IpAddr::V4(ip))?,
        Some(url::Host::Ipv6(ip)) => validate_address(IpAddr::V6(ip))?,
        Some(url::Host::Domain(_)) if resolve == WebFallbackResolve::Startup => {}
        _ => {
            return Err(ProxyError::Config(
                "web.vhost.upstream host must be a loopback or private IP literal unless resolve = \"startup\""
                    .to_string(),
            ));
        }
    }
    Ok(FallbackOrigin::Http { url: parsed })
}

fn validate_address(ip: IpAddr) -> Result<()> {
    let private = match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    };
    if !private {
        return Err(ProxyError::Config(
            "web.vhost.upstream must remain inside loopback or a private network".to_string(),
        ));
    }
    Ok(())
}

/// Returns all captured answers so non-selected addresses cannot hide a listener loop.
pub(super) fn addresses(
    config: &ProxyConfig,
    origin: &FallbackOrigin,
    require_resolved: bool,
) -> Result<Vec<FallbackEndpoint>> {
    match origin {
        FallbackOrigin::Unix { path } => Ok(vec![FallbackEndpoint::Unix(path.clone())]),
        FallbackOrigin::Http { url } => http_addresses(config, url, require_resolved),
    }
}

fn http_addresses(
    config: &ProxyConfig,
    url: &url::Url,
    require_resolved: bool,
) -> Result<Vec<FallbackEndpoint>> {
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ProxyError::Config("WEB fallback port cannot be resolved".to_string()))?;
    let answers = match url.host() {
        Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        Some(url::Host::Domain(host)) => {
            let key = (host.to_string(), port);
            match config.web.fallback_dns.origins.get(&key) {
                Some(answers) if !answers.is_empty() => answers.clone(),
                _ if !require_resolved => return Ok(Vec::new()),
                _ => {
                    return Err(ProxyError::Config(
                        "web.vhost.upstream requires async preparation for resolve = \"startup\"".to_string(),
                    ));
                }
            }
        }
        _ => return Err(ProxyError::Config("WEB fallback host is missing".to_string())),
    };
    for addr in &answers {
        validate_address(addr.ip())?;
        if addr.port() != port {
            return Err(ProxyError::Config(
                "WEB fallback DNS snapshot port mismatch".to_string(),
            ));
        }
    }
    Ok(answers
        .into_iter()
        .map(FallbackEndpoint::Tcp)
        .collect())
}

/// Captures fresh resolver evidence without publishing any partially prepared configuration.
pub(super) async fn prepare<F, Fut>(config: &mut ProxyConfig, mut lookup: F) -> Result<()>
where
    F: FnMut(String, u16) -> Fut,
    Fut: Future<Output = std::io::Result<Vec<SocketAddr>>>,
{
    let mut snapshot = WebFallbackDnsSnapshot::default();
    if let Some(vhost) = &config.web.vhost {
        let origin = parse_origin(&vhost.upstream, vhost.resolve)?;
        if let FallbackOrigin::Http { url } = &origin {
            if let Some(url::Host::Domain(host)) = url.host() {
                let port = url.port_or_known_default().ok_or_else(|| {
                    ProxyError::Config("WEB fallback port cannot be resolved".to_string())
                })?;
                let key = (host.to_string(), port);
                if !snapshot.origins.contains_key(&key) {
                    // Dropping this wait does not forcibly cancel a running OS resolver call.
                    let answers = tokio::time::timeout(
                        Duration::from_secs(config.timeouts.fallback_resolve_secs),
                        lookup(key.0.clone(), port),
                    )
                    .await
                    .map_err(|_| {
                        ProxyError::Config(format!(
                            "web.vhost.upstream DNS resolution timed out after {} seconds",
                            config.timeouts.fallback_resolve_secs
                        ))
                    })?
                    .map_err(|error| {
                        ProxyError::Config(format!(
                            "web.vhost.upstream DNS resolution failed: {error}"
                        ))
                    })?;
                    if answers.is_empty() {
                        return Err(ProxyError::Config(
                            "web.vhost.upstream DNS resolution returned no addresses".to_string(),
                        ));
                    }
                    for addr in &answers {
                        validate_address(addr.ip())?;
                    }
                    snapshot.origins.insert(key, answers);
                }
            }
        }
    }
    config.web.fallback_dns = Arc::new(snapshot);
    validate_web::validate_fallback_listener_separation(config)
}

/// Builds the unchanged request-time tuple from immutable configuration evidence.
pub(super) fn build_upstream(
    config: &ProxyConfig,
    upstream: &str,
    resolve: WebFallbackResolve,
) -> Result<WebRuntimeFallback> {
    let origin = parse_origin(upstream, resolve)?;
    match &origin {
        FallbackOrigin::Http { url } => {
            let endpoint = addresses(config, &origin, true)?
                .into_iter()
                .next()
                .ok_or_else(|| ProxyError::Config("WEB fallback DNS snapshot is empty".to_string()))?;
            // URL authority keeps the configured hostname and brackets IPv6 literals.
            let authority = url[url::Position::BeforeHost..url::Position::AfterPort].to_string();
            Ok(WebRuntimeFallback::HttpUpstream { endpoint, authority })
        }
        FallbackOrigin::Unix { path } => {
            // The unix upstream has no origin authority; the spoofed vhost
            // identity selects the fronting site.
            let authority = config
                .web
                .vhost
                .as_ref()
                .map(|vhost| vhost.host.clone())
                .ok_or_else(|| ProxyError::Config("WEB vhost snapshot is missing".to_string()))?;
            Ok(WebRuntimeFallback::HttpUpstream {
                endpoint: FallbackEndpoint::Unix(path.clone()),
                authority,
            })
        }
    }
}


#[cfg(test)]
mod tests;
