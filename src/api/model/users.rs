use super::*;

#[derive(Serialize)]
pub(in crate::api) struct UserLinks {
    pub(in crate::api) web: Vec<String>,
}

#[derive(Serialize)]
pub(in crate::api) struct UserInfo {
    pub(in crate::api) username: String,
    pub(in crate::api) in_runtime: bool,
    pub(in crate::api) current_connections: u64,
    pub(in crate::api) total_octets: u64,
    /// Optional per-user WEB hostname; `web.vhost.host` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) host: Option<String>,
    /// Optional per-user live session ceiling; `web.limits.max_sessions_global` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) max_sessions: Option<usize>,
    /// Optional per-user live logical-stream ceiling; `web.limits.max_streams_global` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) max_streams: Option<usize>,
    /// Optional per-user stream ceiling for one session; `web.limits.max_streams_per_session` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) max_streams_per_session: Option<usize>,
    pub(in crate::api) links: UserLinks,
}

#[derive(Serialize)]
pub(in crate::api) struct CreateUserResponse {
    pub(in crate::api) user: UserInfo,
    pub(in crate::api) secret: String,
}

#[derive(Serialize)]
pub(in crate::api) struct DeleteUserResponse {
    pub(in crate::api) username: String,
    pub(in crate::api) in_runtime: bool,
}

#[derive(Deserialize)]
pub(in crate::api) struct CreateUserRequest {
    pub(in crate::api) username: String,
    pub(in crate::api) secret: Option<String>,
    /// Optional per-user WEB hostname; `web.vhost.host` applies when absent.
    pub(in crate::api) host: Option<String>,
    /// Optional per-user live session ceiling.
    pub(in crate::api) max_sessions: Option<usize>,
    /// Optional per-user live logical-stream ceiling.
    pub(in crate::api) max_streams: Option<usize>,
    /// Optional per-user stream ceiling for one session.
    pub(in crate::api) max_streams_per_session: Option<usize>,
}

/// JSON Merge Patch semantics: an absent field is unchanged, an explicit
/// `null` clears an optional restriction, and a value sets it.
#[derive(Deserialize)]
pub(in crate::api) struct PatchUserRequest {
    pub(in crate::api) secret: Option<String>,
    /// Optional per-user WEB hostname; `web.vhost.host` applies when cleared.
    pub(in crate::api) host: Option<Option<String>>,
    /// Optional per-user live session ceiling.
    pub(in crate::api) max_sessions: Option<Option<usize>>,
    /// Optional per-user live logical-stream ceiling.
    pub(in crate::api) max_streams: Option<Option<usize>>,
    /// Optional per-user stream ceiling for one session.
    pub(in crate::api) max_streams_per_session: Option<Option<usize>>,
}

#[derive(Default, Deserialize)]
pub(in crate::api) struct RotateSecretRequest {
    pub(in crate::api) secret: Option<String>,
}

pub(in crate::api) fn is_valid_user_secret(secret: &str) -> bool {
    secret.len() == 32 && secret.chars().all(|c| c.is_ascii_hexdigit())
}

pub(in crate::api) fn is_valid_username(user: &str) -> bool {
    !user.is_empty()
        && user.len() <= MAX_USERNAME_LEN
        && user
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

pub(in crate::api) fn random_user_secret() -> String {
    static API_SECRET_RNG: OnceLock<SecureRandom> = OnceLock::new();
    let rng = API_SECRET_RNG.get_or_init(SecureRandom::new);
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    hex::encode(bytes)
}
