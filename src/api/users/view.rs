use super::*;

pub(in crate::api) async fn users_from_config(
    cfg: &ProxyConfig,
    stats: &Stats,
    ip_tracker: &UserIpTracker,
    runtime_cfg: Option<&ProxyConfig>,
) -> Vec<UserInfo> {
    let mut names = cfg.access.users.keys().cloned().collect::<Vec<_>>();
    names.sort();
    let active_ip_lists = ip_tracker.get_active_ips_for_users(&names).await;
    let recent_ip_lists = ip_tracker.get_recent_ips_for_users(&names).await;

    let mut users = Vec::with_capacity(names.len());
    for username in names {
        let active_ip_list = active_ip_lists
            .get(&username)
            .cloned()
            .unwrap_or_else(Vec::new);
        let recent_ip_list = recent_ip_lists
            .get(&username)
            .cloned()
            .unwrap_or_else(Vec::new);
        let links = build_user_links(cfg, &username);
        users.push(UserInfo {
            enabled: cfg.access.is_user_enabled(&username),
            in_runtime: runtime_cfg
                .map(|runtime| runtime.access.users.contains_key(&username))
                .unwrap_or(false),
            max_unique_ips: cfg
                .access
                .user_max_unique_ips
                .get(&username)
                .copied()
                .filter(|limit| *limit > 0)
                .or((cfg.access.global_user_max_unique_ips > 0)
                    .then_some(cfg.access.global_user_max_unique_ips)),
            current_connections: stats.get_process_user_curr_connects(&username),
            active_unique_ips: active_ip_list.len(),
            active_unique_ips_list: active_ip_list,
            recent_unique_ips: recent_ip_list.len(),
            recent_unique_ips_list: recent_ip_list,
            total_octets: stats.get_user_total_octets(&username),
            links,
            username,
        });
    }
    users
}
