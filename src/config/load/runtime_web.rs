use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use super::*;

const WEB_CAPABILITY_CONTEXT_V1: &[u8] = b"tdesktop-web-proxy-bridge-v1\n";
const WEB_CAPABILITY_CONTEXT_V2: &[u8] = b"tdesktop-web-proxy-bridge-v2\n";
const WEB_DEBUG_FINGERPRINT_CONTEXT: &[u8] = b"telemt-web-debug-key-fingerprint-v1\0";

/// Builds the immutable WEB routing and fallback snapshot for one generation.
pub(super) fn rebuild(config: &mut ProxyConfig) -> Result<()> {
    let auth = config.runtime_user_auth().ok_or_else(|| {
        ProxyError::Config("WEB runtime requires the user authentication snapshot".to_string())
    })?;
    let mut runtime_vhosts = BTreeMap::new();
    let mut runtime_profiles = Vec::new();
    let mut runtime_capabilities = Vec::new();

    let carrier_candidates: Arc<[WebCarrier]> = config.web.carrier_candidates().into();
    for (vhost_idx, vhost) in config.web.vhosts.iter().enumerate() {
        let fallback = build_fallback(config, vhost_idx, vhost)?;
        let mut profiles = Vec::with_capacity(vhost.profiles.len());
        let mut capability_table = Vec::with_capacity(vhost.profiles.len());
        let mut capabilities = HashSet::with_capacity(vhost.profiles.len());
        for profile in &vhost.profiles {
            let user_id = auth.user_id_by_name(&profile.user).ok_or_else(|| {
                ProxyError::Config(format!(
                    "WEB profile references unknown access user `{}`",
                    profile.user
                ))
            })?;
            let auth_entry = auth.entry_by_id(user_id).ok_or_else(|| {
                ProxyError::Config("WEB profile user snapshot is inconsistent".to_string())
            })?;
            let (client_secret, client_secret_len) =
                client_secret(auth_entry.secret, profile.secret_mode);
            let capability = derive_web_capability(
                &client_secret[..client_secret_len],
                vhost.host.as_bytes(),
                vhost.base_path.as_bytes(),
            )?;
            let key_fingerprint = debug_key_fingerprint(&client_secret[..client_secret_len]);
            if !capabilities.insert(capability) {
                return Err(ProxyError::Config(format!(
                    "WEB vhost `{}` contains profiles with the same client capability",
                    vhost.host
                )));
            }
            let runtime_profile = Arc::new(WebRuntimeProfile {
                host: vhost.host.clone(),
                public_addr: vhost.public_addr,
                user: profile.user.clone(),
                credential_id: auth_entry.credential_id,
                secret_mode: profile.secret_mode,
                carrier: config.web.carrier,
                carrier_negotiation_enabled: config.web.carrier_negotiation_enabled(),
                carrier_learning: config.web.carrier_negotiation_enabled()
                    && config.web.carrier_learning,
                carriers: Arc::clone(&carrier_candidates),
                carrier_negotiation_deadlines_secs: config
                    .web
                    .timeouts
                    .carrier_negotiation_deadlines_secs,
                capability,
                key_fingerprint,
                max_sessions: profile
                    .max_sessions
                    .unwrap_or(config.web.limits.max_sessions_global),
                max_streams: profile
                    .max_streams
                    .unwrap_or(config.web.limits.max_streams_global),
                max_streams_per_session: profile
                    .max_streams_per_session
                    .unwrap_or(config.web.limits.max_streams_per_session),
            });
            capability_table.push(capability);
            runtime_capabilities.push(capability);
            profiles.push(Arc::clone(&runtime_profile));
            runtime_profiles.push(runtime_profile);
        }
        runtime_vhosts.insert(
            vhost.host.clone(),
            Arc::new(WebRuntimeVhost {
                host: vhost.host.clone(),
                base: if vhost.base_path.is_empty() {
                    "/".to_string()
                } else {
                    format!("/{}/", vhost.base_path)
                },
                fallback_fasttrack_mode: config.web.fallback_fasttrack_mode,
                fallback,
                fallback_header_secs: config.web.timeouts.fallback_header_secs,
                profiles,
                capabilities: capability_table.into_boxed_slice(),
            }),
        );
    }

    config.web.runtime = Some(Arc::new(WebRuntimeConfig {
        vhosts: runtime_vhosts,
        profiles: runtime_profiles,
        capabilities: runtime_capabilities.into_boxed_slice(),
    }));
    Ok(())
}

fn debug_key_fingerprint(secret: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(WEB_DEBUG_FINGERPRINT_CONTEXT);
    digest.update(secret);
    hex::encode(&digest.finalize()[..8])
}

/// Derives the Telegram Desktop WEB capability for one exact secret, host, and base path.
pub(crate) fn derive_web_capability(
    secret: &[u8],
    host: &[u8],
    base_path: &[u8],
) -> Result<[u8; 32]> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .map_err(|_| ProxyError::Config("WEB capability secret must not be empty".to_string()))?;
    if base_path.is_empty() {
        mac.update(WEB_CAPABILITY_CONTEXT_V1);
        mac.update(host);
    } else {
        mac.update(WEB_CAPABILITY_CONTEXT_V2);
        mac.update(host);
        mac.update(b"\n");
        mac.update(base_path);
    }
    Ok(mac.finalize().into_bytes().into())
}

fn client_secret(secret: [u8; 16], mode: WebSecretMode) -> ([u8; 17], usize) {
    let mut client_secret = [0u8; 17];
    match mode {
        WebSecretMode::Plain => {
            client_secret[..16].copy_from_slice(&secret);
            (client_secret, 16)
        }
        WebSecretMode::Dd => {
            client_secret[0] = 0xdd;
            client_secret[1..].copy_from_slice(&secret);
            (client_secret, 17)
        }
    }
}

fn build_fallback(
    config: &ProxyConfig,
    vhost_idx: usize,
    vhost: &WebVhostConfig,
) -> Result<WebRuntimeFallback> {
    let WebFallbackConfig::HttpUpstream { upstream, resolve } = &vhost.fallback;
    fallback_dns::build_upstream(config, vhost_idx, upstream, *resolve)
}

// Runtime WEB construction tests remain separate from the production loader.
#[cfg(test)]
#[path = "runtime_web/tests.rs"]
mod tests;
