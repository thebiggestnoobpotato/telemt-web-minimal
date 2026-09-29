use base64::Engine as _;

use crate::config::{ProxyConfig, WebSecretMode};

/// Formats one `tg://webproxy` link for a vhost host and secret mode.
pub fn format_web_proxy_link(
    host: &str,
    base_path: &str,
    secret: &str,
    mode: WebSecretMode,
) -> Option<String> {
    if base_path.is_empty() {
        let prefix = match mode {
            WebSecretMode::Plain => "",
            WebSecretMode::Dd => "dd",
        };
        return Some(format!(
            "tg://webproxy?server={host}&secret={prefix}{secret}"
        ));
    }
    let decoded = hex::decode(secret).ok()?;
    let mut marked = Vec::with_capacity(decoded.len() + 2);
    marked.push(0x70);
    if mode == WebSecretMode::Dd {
        marked.push(0xdd);
    }
    marked.extend_from_slice(&decoded);
    let server = url::form_urlencoded::byte_serialize(format!("{host}/{base_path}").as_bytes())
        .collect::<String>();
    let marked = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(marked);
    Some(format!("tg://webproxy?server={server}&secret={marked}"))
}

/// WEB proxy links for one user across all of its runtime profiles.
pub fn web_links_for_user(config: &ProxyConfig, user: &str) -> Vec<String> {
    let Some(runtime) = config.web.runtime.as_ref() else {
        return Vec::new();
    };
    let Some(secret) = config.access.users.get(user) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for profile in &runtime.profiles {
        if profile.user != user {
            continue;
        }
        let Some(vhost) = config
            .web
            .vhosts
            .iter()
            .find(|vhost| vhost.host == profile.host)
        else {
            continue;
        };
        if let Some(link) =
            format_web_proxy_link(&profile.host, &vhost.base_path, secret, profile.secret_mode)
        {
            links.push(link);
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "000102030405060708090a0b0c0d0e0f";

    #[test]
    fn root_web_proxy_links_keep_the_legacy_secret_form() {
        assert_eq!(
            format_web_proxy_link("proxy.example.com", "", SECRET, WebSecretMode::Plain),
            Some(format!(
                "tg://webproxy?server=proxy.example.com&secret={SECRET}"
            ))
        );
        assert_eq!(
            format_web_proxy_link("proxy.example.com", "", SECRET, WebSecretMode::Dd),
            Some(format!(
                "tg://webproxy?server=proxy.example.com&secret=dd{SECRET}"
            ))
        );
    }

    #[test]
    fn path_web_proxy_links_use_the_tdesktop_marker() {
        assert_eq!(
            format_web_proxy_link(
                "proxy.example.com",
                "dobry-cola/super_app",
                SECRET,
                WebSecretMode::Plain,
            ),
            Some("tg://webproxy?server=proxy.example.com%2Fdobry-cola%2Fsuper_app&secret=cAABAgMEBQYHCAkKCwwNDg8".to_string())
        );
        assert_eq!(
            format_web_proxy_link(
                "proxy.example.com",
                "dobry-cola/super_app",
                SECRET,
                WebSecretMode::Dd,
            ),
            Some("tg://webproxy?server=proxy.example.com%2Fdobry-cola%2Fsuper_app&secret=cN0AAQIDBAUGBwgJCgsMDQ4P".to_string())
        );
    }

    #[test]
    fn path_web_proxy_link_round_trips_through_the_tdesktop_grammar() {
        for (mode, expected_secret) in [
            (WebSecretMode::Plain, hex::decode(SECRET).unwrap()),
            (
                WebSecretMode::Dd,
                [vec![0xdd], hex::decode(SECRET).unwrap()].concat(),
            ),
        ] {
            let link = format_web_proxy_link("proxy.example.com", "MixedCase/a_b-9", SECRET, mode)
                .unwrap();
            let parsed = url::Url::parse(&link).unwrap();
            let query = parsed
                .query_pairs()
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(
                query["server"].as_ref(),
                "proxy.example.com/MixedCase/a_b-9"
            );
            let marked = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(query["secret"].as_bytes())
                .unwrap();
            assert_eq!(marked.first(), Some(&0x70));
            assert_eq!(&marked[1..], expected_secret.as_slice());
        }
    }
}
