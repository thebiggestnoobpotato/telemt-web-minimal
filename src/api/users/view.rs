use super::*;

pub(in crate::api) async fn users_from_config(
    cfg: &ProxyConfig,
    stats: &Stats,
    runtime_cfg: Option<&ProxyConfig>,
) -> Vec<UserInfo> {
    let mut names = cfg.users.keys().cloned().collect::<Vec<_>>();
    names.sort();

    let mut users = Vec::with_capacity(names.len());
    for username in &names {
        let links = build_user_links(cfg, username);
        users.push(UserInfo {
            in_runtime: runtime_cfg
                .map(|runtime| runtime.users.contains_key(username))
                .unwrap_or(false),
            current_connections: stats.get_process_user_curr_connects(username),
            total_octets: stats.get_user_total_octets(username),
            links,
            username: username.clone(),
        });
    }
    users
}
