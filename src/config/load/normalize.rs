use crate::error::{ProxyError, Result};
use tracing::warn;

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

pub(super) fn is_valid_ad_tag(tag: &str) -> bool {
    tag.len() == 32 && tag.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn sanitize_ad_tag(ad_tag: &mut Option<String>) {
    let Some(tag) = ad_tag.as_ref() else {
        return;
    };

    if !is_valid_ad_tag(tag) {
        warn!("Invalid general.ad_tag value, expected exactly 32 hex chars; ad_tag is disabled");
        *ad_tag = None;
    }
}
