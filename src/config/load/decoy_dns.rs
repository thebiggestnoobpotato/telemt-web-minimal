use std::future::Future;
use std::time::Duration;

use super::*;

/// Rejects mode-specific keys even when unknown-key handling is permissive.
pub(super) fn validate_mode_keys(document: &toml::Value) -> Result<()> {
    if let Some(vhosts) = document
        .get("web")
        .and_then(|web| web.get("vhosts"))
        .and_then(toml::Value::as_array)
    {
        for (idx, vhost) in vhosts.iter().enumerate() {
            if let Some(decoy) = vhost.get("decoy")
                && decoy.get("resolve").is_some()
                && decoy.get("mode").and_then(toml::Value::as_str) != Some("http_upstream")
            {
                return Err(ProxyError::Config(format!(
                    "web.vhosts[{idx}].decoy.resolve is only valid for mode=http_upstream"
                )));
            }
        }
    }
    Ok(())
}

/// Parsed decoy origin: an http origin, or a unix socket path without DNS.
pub(super) enum DecoyOrigin {
    /// http origin validated for the configured resolve mode.
    Http { url: url::Url },
    /// `unix:` origin; the socket path is the complete target.
    Unix { path: std::path::PathBuf },
}

/// Parses one origin and preserves the existing literal-address containment policy.
pub(super) fn parse_origin(
    idx: usize,
    upstream: &str,
    resolve: WebDecoyResolve,
) -> Result<DecoyOrigin> {
    // The unix: prefix is intercepted before URL parsing; the remainder must
    // be an absolute socket path without URL control characters.
    if let Some(rest) = upstream.strip_prefix("unix:") {
        if resolve == WebDecoyResolve::Startup {
            return Err(ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.resolve is only valid for http origins"
            )));
        }
        if rest.is_empty() {
            return Err(ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream unix origin requires a socket path"
            )));
        }
        if rest.contains('?') || rest.contains('#') || rest.contains(' ') || rest.contains('\\') {
            return Err(ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream unix origin must not contain a query, fragment, or whitespace"
            )));
        }
        let path: std::path::PathBuf =
            std::path::Path::new(rest).components().collect();
        if !path.is_absolute() {
            return Err(ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream unix origin must be an absolute socket path"
            )));
        }
        return Ok(DecoyOrigin::Unix { path });
    }
    let parsed = url::Url::parse(upstream).map_err(|error| {
        ProxyError::Config(format!(
            "web.vhosts[{idx}].decoy.upstream is invalid: {error}"
        ))
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
        return Err(ProxyError::Config(format!(
            "web.vhosts[{idx}].decoy.upstream must be an http origin without credentials, path, query, or fragment"
        )));
    }
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => validate_address(idx, IpAddr::V4(ip))?,
        Some(url::Host::Ipv6(ip)) => validate_address(idx, IpAddr::V6(ip))?,
        Some(url::Host::Domain(_)) if resolve == WebDecoyResolve::Startup => {}
        _ => {
            return Err(ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream host must be a loopback or private IP literal unless resolve = \"startup\""
            )));
        }
    }
    Ok(DecoyOrigin::Http { url: parsed })
}

fn validate_address(idx: usize, ip: IpAddr) -> Result<()> {
    let private = match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    };
    if !private {
        return Err(ProxyError::Config(format!(
            "web.vhosts[{idx}].decoy.upstream must remain inside loopback or a private network"
        )));
    }
    Ok(())
}

/// Returns all captured answers so non-selected addresses cannot hide a listener loop.
pub(super) fn addresses(
    config: &ProxyConfig,
    idx: usize,
    origin: &DecoyOrigin,
    require_resolved: bool,
) -> Result<Vec<DecoyEndpoint>> {
    match origin {
        DecoyOrigin::Unix { path } => Ok(vec![DecoyEndpoint::Unix(path.clone())]),
        DecoyOrigin::Http { url } => http_addresses(config, idx, url, require_resolved),
    }
}

fn http_addresses(
    config: &ProxyConfig,
    idx: usize,
    url: &url::Url,
    require_resolved: bool,
) -> Result<Vec<DecoyEndpoint>> {
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ProxyError::Config("WEB decoy port cannot be resolved".to_string()))?;
    let answers = match url.host() {
        Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        Some(url::Host::Domain(host)) => {
            let key = (host.to_string(), port);
            match config.web.decoy_dns.origins.get(&key) {
                Some(answers) if !answers.is_empty() => answers.clone(),
                _ if !require_resolved => return Ok(Vec::new()),
                _ => {
                    return Err(ProxyError::Config(format!(
                        "web.vhosts[{idx}].decoy.upstream requires async preparation for resolve = \"startup\""
                    )));
                }
            }
        }
        _ => return Err(ProxyError::Config("WEB decoy host is missing".to_string())),
    };
    for addr in &answers {
        validate_address(idx, addr.ip())?;
        if addr.port() != port {
            return Err(ProxyError::Config(
                "WEB decoy DNS snapshot port mismatch".to_string(),
            ));
        }
    }
    Ok(answers
        .into_iter()
        .map(DecoyEndpoint::Tcp)
        .collect())
}

/// Captures fresh resolver evidence without publishing any partially prepared configuration.
pub(super) async fn prepare<F, Fut>(config: &mut ProxyConfig, mut lookup: F) -> Result<()>
where
    F: FnMut(String, u16) -> Fut,
    Fut: Future<Output = std::io::Result<Vec<SocketAddr>>>,
{
    let mut snapshot = WebDecoyDnsSnapshot::default();
    for (idx, vhost) in config.web.vhosts.iter().enumerate() {
        let WebDecoyConfig::HttpUpstream { upstream, resolve } = &vhost.decoy else {
            continue;
        };
        let origin = parse_origin(idx, upstream, *resolve)?;
        let DecoyOrigin::Http { url } = &origin else {
            continue;
        };
        let Some(url::Host::Domain(host)) = url.host() else {
            continue;
        };
        let port = url
            .port_or_known_default()
            .ok_or_else(|| ProxyError::Config("WEB decoy port cannot be resolved".to_string()))?;
        let key = (host.to_string(), port);
        if snapshot.origins.contains_key(&key) {
            continue;
        }
        // Dropping this wait does not forcibly cancel a running OS resolver call.
        let answers = tokio::time::timeout(
            Duration::from_secs(config.web.timeouts.decoy_resolve_secs),
            lookup(key.0.clone(), port),
        )
        .await
        .map_err(|_| {
            ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream DNS resolution timed out after {} seconds",
                config.web.timeouts.decoy_resolve_secs
            ))
        })?
        .map_err(|error| {
            ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream DNS resolution failed: {error}"
            ))
        })?;
        if answers.is_empty() {
            return Err(ProxyError::Config(format!(
                "web.vhosts[{idx}].decoy.upstream DNS resolution returned no addresses"
            )));
        }
        for addr in &answers {
            validate_address(idx, addr.ip())?;
        }
        snapshot.origins.insert(key, answers);
    }
    config.web.decoy_dns = Arc::new(snapshot);
    validate_web::validate_decoy_listener_separation(config)
}

/// Builds the unchanged request-time tuple from immutable configuration evidence.
pub(super) fn build_upstream(
    config: &ProxyConfig,
    idx: usize,
    upstream: &str,
    resolve: WebDecoyResolve,
) -> Result<WebRuntimeDecoy> {
    let origin = parse_origin(idx, upstream, resolve)?;
    match &origin {
        DecoyOrigin::Http { url } => {
            let endpoint = addresses(config, idx, &origin, true)?
                .into_iter()
                .next()
                .ok_or_else(|| ProxyError::Config("WEB decoy DNS snapshot is empty".to_string()))?;
            // URL authority keeps the configured hostname and brackets IPv6 literals.
            let authority = url[url::Position::BeforeHost..url::Position::AfterPort].to_string();
            Ok(WebRuntimeDecoy::HttpUpstream { endpoint, authority })
        }
        DecoyOrigin::Unix { path } => {
            // The unix upstream has no origin authority; the spoofed vhost
            // identity selects the fronting site.
            let authority = config.web.vhosts[idx].host.clone();
            Ok(WebRuntimeDecoy::HttpUpstream {
                endpoint: DecoyEndpoint::Unix(path.clone()),
                authority,
            })
        }
    }
}

impl ProxyConfig {
    /// Compares selected endpoints separately from source-only configuration equality.
    pub(crate) fn web_decoy_endpoints_equal(&self, other: &Self) -> bool {
        fn selected(config: &ProxyConfig) -> impl Iterator<Item = (&String, DecoyEndpoint)> {
            config
                .web
                .runtime
                .iter()
                .flat_map(|runtime| runtime.vhosts.iter())
                .filter_map(|(host, vhost)| match &vhost.decoy {
                    WebRuntimeDecoy::HttpUpstream { endpoint, .. } => {
                        Some((host, endpoint.clone()))
                    }
                    WebRuntimeDecoy::StaticDirectory(_) => None,
                })
        }
        selected(self).eq(selected(other))
    }
}

#[cfg(test)]
mod tests;
