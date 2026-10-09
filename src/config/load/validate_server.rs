use super::*;

pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    // Validate secrets.
    for (user, secret) in &config.access.users {
        if !secret.chars().all(|c| c.is_ascii_hexdigit()) || secret.len() != 32 {
            return Err(ProxyError::InvalidSecret {
                user: user.clone(),
                reason: "Must be 32 hex characters".to_string(),
            });
        }
    }

    Ok(())
}
