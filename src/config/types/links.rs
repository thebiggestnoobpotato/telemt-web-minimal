//! WEB proxy link display policy backing `[logging].show_users`.

use super::*;

/// In TOML, this can be:
/// - `show_users = "*"`          — show links for all users
/// - `show_users = ["a", "b"]`   — show links for specific users
/// - omitted                — defaults to `"*"` (all users)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ShowLink {
    /// Don't show any links (default when omitted).
    #[default]
    None,
    /// Show links for all configured users.
    All,
    /// Show links for specific users.
    Specific(Vec<String>),
}

pub(super) fn default_links_show() -> ShowLink {
    ShowLink::All
}

impl ShowLink {
    /// Returns true if no links should be shown.
    pub fn is_empty(&self) -> bool {
        matches!(self, ShowLink::None) || matches!(self, ShowLink::Specific(v) if v.is_empty())
    }

    /// Resolve the list of user names to display, given all configured users.
    pub fn resolve_users<'a>(&'a self, all_users: &'a HashMap<String, String>) -> Vec<&'a String> {
        match self {
            ShowLink::None => vec![],
            ShowLink::All => {
                let mut names: Vec<&String> = all_users.keys().collect();
                names.sort();
                names
            }
            ShowLink::Specific(names) => names.iter().collect(),
        }
    }
}

impl Serialize for ShowLink {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            ShowLink::None => Vec::<String>::new().serialize(serializer),
            ShowLink::All => serializer.serialize_str("*"),
            ShowLink::Specific(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ShowLink {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de;

        struct ShowLinkVisitor;

        impl<'de> de::Visitor<'de> for ShowLinkVisitor {
            type Value = ShowLink;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(r#""*" or an array of user names"#)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<ShowLink, E> {
                if v == "*" {
                    Ok(ShowLink::All)
                } else {
                    Err(de::Error::invalid_value(de::Unexpected::Str(v), &r#""*""#))
                }
            }

            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<ShowLink, A::Error> {
                let mut names = Vec::new();
                while let Some(name) = seq.next_element::<String>()? {
                    names.push(name);
                }
                if names.is_empty() {
                    Ok(ShowLink::None)
                } else {
                    Ok(ShowLink::Specific(names))
                }
            }
        }

        deserializer.deserialize_any(ShowLinkVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn users(names: &[&str]) -> HashMap<String, String> {
        names
            .iter()
            .map(|name| (name.to_string(), "00000000000000000000000000000000".to_string()))
            .collect()
    }

    #[test]
    fn show_link_defaults_to_all_users() {
        assert_eq!(default_links_show(), ShowLink::All);
        assert!(!ShowLink::All.is_empty());
        assert!(ShowLink::None.is_empty());
        assert!(ShowLink::Specific(Vec::new()).is_empty());
        assert!(!ShowLink::Specific(vec!["alice".to_string()]).is_empty());
    }

    #[test]
    fn show_link_resolves_users() {
        let all = users(&["bob", "alice"]);

        let none = ShowLink::None;
        assert!(none.resolve_users(&all).is_empty());

        let all_users = ShowLink::All;
        let everyone: Vec<&str> = all_users
            .resolve_users(&all)
            .iter()
            .map(|name| name.as_str())
            .collect();
        assert_eq!(everyone, vec!["alice", "bob"]);

        let only_bob = ShowLink::Specific(vec!["bob".to_string()]);
        let bob_only: Vec<&str> = only_bob
            .resolve_users(&all)
            .iter()
            .map(|name| name.as_str())
            .collect();
        assert_eq!(bob_only, vec!["bob"]);
    }

    #[derive(Deserialize)]
    struct Wrapper {
        show_users: ShowLink,
    }

    #[test]
    fn show_link_serde_forms() {
        assert_eq!(
            toml::from_str::<Wrapper>(r#"show_users = "*""#).unwrap().show_users,
            ShowLink::All
        );
        assert_eq!(
            toml::from_str::<Wrapper>(r#"show_users = ["alice", "bob"]"#)
                .unwrap()
                .show_users,
            ShowLink::Specific(vec!["alice".to_string(), "bob".to_string()])
        );
        assert_eq!(
            toml::from_str::<Wrapper>("show_users = []").unwrap().show_users,
            ShowLink::None
        );

        #[derive(Serialize)]
        struct WrapperSer {
            show_users: ShowLink,
        }

        let serialized_all = toml::to_string(&WrapperSer {
            show_users: ShowLink::All,
        })
        .unwrap();
        assert!(
            serialized_all.contains(r#"show_users = "*""#),
            "{serialized_all}"
        );
        let serialized_none = toml::to_string(&WrapperSer {
            show_users: ShowLink::None,
        })
        .unwrap();
        assert!(
            serialized_none.contains("show_users = []"),
            "{serialized_none}"
        );
    }
}
