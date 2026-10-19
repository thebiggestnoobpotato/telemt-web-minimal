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
    /// Optional per-user WEB hostname; `web.host` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) host: Option<String>,
    /// Optional per-user live session ceiling; `limits.max_sessions_global` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) max_sessions: Option<usize>,
    /// Optional per-user live logical-stream ceiling; `limits.max_streams_global` applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api) max_streams: Option<usize>,
    /// Optional per-user stream ceiling for one session; `limits.max_streams_per_session` applies when absent.
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
    /// Optional per-user WEB hostname; `web.host` applies when absent.
    pub(in crate::api) host: Option<String>,
    /// Optional per-user live session ceiling.
    pub(in crate::api) max_sessions: Option<usize>,
    /// Optional per-user live logical-stream ceiling.
    pub(in crate::api) max_streams: Option<usize>,
    /// Optional per-user stream ceiling for one session.
    pub(in crate::api) max_streams_per_session: Option<usize>,
}

/// One JSON Merge Patch slot. Serde maps both an absent key and an
/// explicit `null` to `Option`'s `None`, so the three states need an
/// explicit enum: `Absent` comes from `#[serde(default)]`, `Clear`
/// from a present `null`, and `Set` from a present value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::api) enum MergeValue<T> {
    /// Absent from the request: leave the restriction unchanged.
    Absent,
    /// Explicit `null`: clear the restriction.
    Clear,
    /// A value: set the restriction.
    Set(T),
}

impl<T> Default for MergeValue<T> {
    fn default() -> Self {
        MergeValue::Absent
    }
}

impl<T> MergeValue<T> {
    /// Whether the key is present in the request at all.
    pub(in crate::api) fn is_present(&self) -> bool {
        !matches!(self, MergeValue::Absent)
    }

    /// The replacement to write to the field: `None` when the key was
    /// absent (no change), `Some(None)` for a clear, `Some(value)` for
    /// a set.
    pub(in crate::api) fn replacement(&self) -> Option<Option<T>>
    where
        T: Clone,
    {
        match self {
            MergeValue::Absent => None,
            MergeValue::Clear => Some(None),
            MergeValue::Set(value) => Some(Some(value.clone())),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MergeValue<T> {
    fn deserialize<D: serde::de::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        struct Slot<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Slot<T> {
            type Value = MergeValue<T>;
            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("a value or null")
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(MergeValue::Clear)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(MergeValue::Clear)
            }
            fn visit_some<D: serde::de::Deserializer<'de>>(
                self,
                d: D,
            ) -> Result<Self::Value, D::Error> {
                T::deserialize(d).map(MergeValue::Set)
            }
        }
        deserializer.deserialize_option(Slot(std::marker::PhantomData))
    }
}

/// JSON Merge Patch semantics: an absent field is unchanged, an explicit
/// `null` clears an optional restriction, and a value sets it.
#[derive(Deserialize)]
pub(in crate::api) struct PatchUserRequest {
    pub(in crate::api) secret: Option<String>,
    /// Optional per-user WEB hostname; `web.host` applies when cleared.
    #[serde(default)]
    pub(in crate::api) host: MergeValue<String>,
    /// Optional per-user live session ceiling.
    #[serde(default)]
    pub(in crate::api) max_sessions: MergeValue<usize>,
    /// Optional per-user live logical-stream ceiling.
    #[serde(default)]
    pub(in crate::api) max_streams: MergeValue<usize>,
    /// Optional per-user stream ceiling for one session.
    #[serde(default)]
    pub(in crate::api) max_streams_per_session: MergeValue<usize>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_patch_keeps_absent_null_and_value_distinct() {
        // Regression: with plain `Option<Option<T>>` fields, an explicit
        // `null` deserialized to `None` exactly like an absent key, so the
        // documented null-clear never fired and the patch was a no-op.
        let req: PatchUserRequest = serde_json::from_str(r#"{"host": null}"#).unwrap();
        assert_eq!(req.host, MergeValue::Clear);
        assert_eq!(req.max_sessions, MergeValue::Absent);

        let req: PatchUserRequest = serde_json::from_str(r#"{"max_streams": 64}"#).unwrap();
        assert_eq!(req.max_streams, MergeValue::Set(64));
        assert_eq!(req.host, MergeValue::Absent);

        let req: PatchUserRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.host, MergeValue::Absent);
        assert_eq!(req.max_sessions, MergeValue::Absent);
        assert_eq!(req.max_streams, MergeValue::Absent);
        assert_eq!(req.max_streams_per_session, MergeValue::Absent);
        assert_eq!(req.secret, None);
    }
}
