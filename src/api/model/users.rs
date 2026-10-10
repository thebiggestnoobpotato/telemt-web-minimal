use super::*;

#[derive(Serialize)]
pub(in crate::api) struct UserLinks {
    pub(in crate::api) web: Vec<String>,
}

#[derive(Serialize)]
pub(in crate::api) struct UserInfo {
    pub(in crate::api) username: String,
    pub(in crate::api) enabled: bool,
    pub(in crate::api) in_runtime: bool,
    pub(in crate::api) max_unique_ips: Option<usize>,
    pub(in crate::api) current_connections: u64,
    pub(in crate::api) active_unique_ips: usize,
    pub(in crate::api) active_unique_ips_list: Vec<IpAddr>,
    pub(in crate::api) recent_unique_ips: usize,
    pub(in crate::api) recent_unique_ips_list: Vec<IpAddr>,
    pub(in crate::api) total_octets: u64,
    pub(in crate::api) links: UserLinks,
}

#[derive(Serialize)]
pub(in crate::api) struct UserActiveIps {
    pub(in crate::api) username: String,
    pub(in crate::api) active_ips: Vec<IpAddr>,
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
    pub(in crate::api) max_unique_ips: Option<usize>,
    pub(in crate::api) enabled: Option<bool>,
}

#[derive(Deserialize)]
pub(in crate::api) struct PatchUserRequest {
    pub(in crate::api) secret: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    pub(in crate::api) max_unique_ips: Patch<usize>,
    #[serde(default, deserialize_with = "patch_field")]
    pub(in crate::api) enabled: Patch<bool>,
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
