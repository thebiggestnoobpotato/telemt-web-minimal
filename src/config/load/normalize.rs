use crate::error::{ProxyError, Result};

pub(super) fn is_valid_tls_domain_name(domain: &str) -> bool {
    !domain.is_empty()
        && !domain
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '/' | '\\'))
}

pub(super) fn normalize_domain_to_ascii(domain: &str, field: &str) -> Result<String> {
    let domain = domain.trim();
    if !is_valid_tls_domain_name(domain) {
        return Err(ProxyError::Config(format!(
            "Invalid {field}: '{}'. Must be a valid domain name",
            domain
        )));
    }

    let parsed = url::Url::parse(&format!("https://{domain}/")).map_err(|error| {
        ProxyError::Config(format!(
            "Invalid {field}: '{}'. IDNA conversion failed: {error}",
            domain
        ))
    })?;
    let host = parsed.host_str().ok_or_else(|| {
        ProxyError::Config(format!(
            "Invalid {field}: '{}'. Host is empty",
            domain
        ))
    })?;
    Ok(host.to_ascii_lowercase())
}
