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

    // Per-host profile and capability groups backing one runtime vhost entry.
    #[derive(Default)]
    struct HostProfiles {
        profiles: Vec<Arc<WebRuntimeProfile>>,
        capabilities: Vec<[u8; 32]>,
    }

    let carrier_candidates: Arc<[WebCarrier]> = config.web.carrier_candidates().into();
    let vhost = &config.web.vhost;
    let single_host = !vhost.host.is_empty();
    let web_on = single_host || vhost.hosts.as_ref().is_some_and(|hosts| !hosts.is_empty());
    if web_on {
        let fallback = build_fallback(config, vhost)?;
        // Every access user materializes one WEB profile per group host it
        // does not override; absent ceilings inherit the global WEB limits.
        let secret_mode = config.web.secret_mode.ok_or_else(|| {
            ProxyError::Config("web requires web.secret_mode".to_string())
        })?;
        let group_hosts: Vec<String> = if single_host {
            vec![vhost.host.clone()]
        } else {
            vhost.hosts.clone().unwrap_or_default()
        };
        let mut host_groups: BTreeMap<String, HostProfiles> = BTreeMap::new();
        // Capability uniqueness is per exact host: different hosts derive
        // different capabilities from the same secret.
        let mut host_capabilities: BTreeMap<String, HashSet<[u8; 32]>> = BTreeMap::new();
        for auth_entry in auth.entries() {
            let user_entry = config.users.get(&auth_entry.user).ok_or_else(|| {
                ProxyError::Config(format!(
                    "WEB runtime user `{}` is missing from the access entries",
                    auth_entry.user
                ))
            })?;
            let (client_secret, client_secret_len) = client_secret(auth_entry.secret, secret_mode);
            let shared = Arc::new(WebRuntimeProfileShared {
                public_addr: vhost.public_addr,
                user: auth_entry.user.clone(),
                credential_id: auth_entry.credential_id,
                secret_mode,
                carrier: config.web.carrier,
                carrier_negotiation_enabled: config.web.carrier_negotiation_enabled(),
                carrier_learning: config.web.carrier_negotiation_enabled()
                    && config.web.carrier_learning,
                carriers: Arc::clone(&carrier_candidates),
                carrier_negotiation_deadlines_secs: config
                    .timeouts
                    .carrier_negotiation_deadlines_secs,
                key_fingerprint: debug_key_fingerprint(&client_secret[..client_secret_len]),
                max_sessions: user_entry
                    .max_sessions
                    .unwrap_or(config.limits.max_sessions_global),
                max_streams: user_entry
                    .max_streams
                    .unwrap_or(config.limits.max_streams_global),
                max_streams_per_session: user_entry
                    .max_streams_per_session
                    .unwrap_or(config.limits.max_streams_per_session),
            });
            // An explicit user binding pins one host; otherwise every group
            // host receives its own host-bound capability entry.
            let binding_hosts: Vec<String> = match user_entry.host.as_deref() {
                Some(host) => vec![host.to_string()],
                None => group_hosts.clone(),
            };
            for host in binding_hosts {
                let capability = derive_web_capability(
                    &client_secret[..client_secret_len],
                    host.as_bytes(),
                    vhost.base_path.as_bytes(),
                )?;
                if !host_capabilities
                    .entry(host.clone())
                    .or_default()
                    .insert(capability)
                {
                    return Err(ProxyError::Config(format!(
                        "WEB host `{host}` contains profiles with the same client capability"
                    )));
                }
                let runtime_profile = Arc::new(WebRuntimeProfile {
                    host: host.clone(),
                    capability,
                    shared: Arc::clone(&shared),
                });
                runtime_capabilities.push(capability);
                let group = host_groups.entry(host).or_default();
                group.profiles.push(Arc::clone(&runtime_profile));
                group.capabilities.push(capability);
                runtime_profiles.push(runtime_profile);
            }
        }
        let base = if vhost.base_path.is_empty() {
            "/".to_string()
        } else {
            format!("/{}/", vhost.base_path)
        };
        for (host, group) in host_groups {
            let fallback = host_fallback(&fallback, &host);
            runtime_vhosts.insert(
                host.clone(),
                Arc::new(WebRuntimeVhost {
                    host,
                    base: base.clone(),
                    fallback_fasttrack_mode: config.web.fallback_fasttrack_mode,
                    fallback,
                    fallback_header_secs: config.timeouts.fallback_header_secs,
                    profiles: group.profiles,
                    capabilities: group.capabilities.into_boxed_slice(),
                }),
            );
        }
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

fn build_fallback(config: &ProxyConfig, vhost: &WebVhostConfig) -> Result<WebRuntimeFallback> {
    fallback_dns::build_upstream(config, &vhost.upstream, vhost.resolve)
}

// A unix origin has no authority of its own: the request host selects the
// fronting site, so every runtime vhost entry carries its own host as the
// request authority. http origins keep the configured authority everywhere.
fn host_fallback(fallback: &WebRuntimeFallback, host: &str) -> WebRuntimeFallback {
    let WebRuntimeFallback::HttpUpstream { endpoint, authority } = fallback;
    let authority = if matches!(endpoint, FallbackEndpoint::Unix(_)) {
        host.to_string()
    } else {
        authority.clone()
    };
    WebRuntimeFallback::HttpUpstream {
        endpoint: endpoint.clone(),
        authority,
    }
}

// Runtime WEB construction tests remain separate from the production loader.
#[cfg(test)]
#[path = "runtime_web/tests.rs"]
mod tests;
