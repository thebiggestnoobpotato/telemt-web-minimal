use super::*;

pub(super) fn validate(config: &mut ProxyConfig) -> Result<()> {
    for (user, entry) in &config.users {
        // Usernames are runtime status labels and must stay bounded.
        if user.is_empty() || user.len() > 64 {
            return Err(ProxyError::Config(format!(
                "users.{user} must contain 1..64 bytes"
            )));
        }
        // Validate secrets.
        if !entry.secret.chars().all(|c| c.is_ascii_hexdigit())
            || entry.secret.len() != 32
        {
            return Err(ProxyError::InvalidSecret {
                user: user.clone(),
                reason: "Must be 32 hex characters".to_string(),
            });
        }
    }

    Ok(())
}
