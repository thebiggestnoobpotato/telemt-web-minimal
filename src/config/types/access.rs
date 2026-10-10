use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessConfig {
    #[serde(default = "default_access_users")]
    pub users: HashMap<String, String>,

    #[serde(default)]
    pub user_enabled: HashMap<String, bool>,

    #[serde(default)]
    pub user_max_unique_ips: HashMap<String, usize>,

    /// Global per-user unique IP limit applied when a user has no individual override.
    /// `0` disables the inherited limit.
    #[serde(default = "default_global_user_max_unique_ips")]
    pub global_user_max_unique_ips: usize,

    #[serde(default)]
    pub user_max_unique_ips_mode: UserMaxUniqueIpsMode,

    #[serde(default = "default_user_max_unique_ips_window_secs")]
    pub user_max_unique_ips_window_secs: u64,

    #[serde(default = "default_replay_check_len")]
    pub replay_check_len: usize,

    #[serde(default = "default_replay_window_secs")]
    pub replay_window_secs: u64,

    #[serde(default)]
    pub ignore_time_skew: bool,
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self {
            users: default_access_users(),
            user_enabled: HashMap::new(),
            user_max_unique_ips: HashMap::new(),
            global_user_max_unique_ips: default_global_user_max_unique_ips(),
            user_max_unique_ips_mode: UserMaxUniqueIpsMode::default(),
            user_max_unique_ips_window_secs: default_user_max_unique_ips_window_secs(),
            replay_check_len: default_replay_check_len(),
            replay_window_secs: default_replay_window_secs(),
            ignore_time_skew: false,
        }
    }
}

impl AccessConfig {
    pub fn is_user_enabled(&self, username: &str) -> bool {
        self.user_enabled.get(username).copied().unwrap_or(true)
    }

}
