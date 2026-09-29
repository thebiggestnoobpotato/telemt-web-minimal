use super::*;

pub(in crate::api::users) fn build_user_links(cfg: &ProxyConfig, user: &str) -> UserLinks {
    UserLinks {
        web: crate::web::links::web_links_for_user(cfg, user),
    }
}
