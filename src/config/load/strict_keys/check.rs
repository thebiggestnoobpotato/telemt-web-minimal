use super::*;

#[derive(Debug)]
/// One rejected configuration path and its optional nearest known key.
pub(super) struct UnknownConfigKey {
    /// Fully qualified configuration path.
    pub(super) path: String,
    /// Nearest known key when edit distance is sufficiently small.
    pub(super) suggestion: Option<String>,
}

fn table_at<'a>(
    value: &'a serde_yaml::Value,
    path: &[&str],
) -> Option<&'a serde_yaml::Mapping> {
    let mut current = value;
    for segment in path {
        current = current.get(segment)?;
    }
    current.as_mapping()
}

/// Reads strict-key enforcement without deserializing the full configuration.
pub(super) fn is_strict_config(parsed_yaml: &serde_yaml::Value) -> bool {
    table_at(parsed_yaml, &["general"])
        .and_then(|table| table.get("config_strict"))
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(false)
}

fn known_config_keys_for_suggestion() -> Vec<&'static str> {
    let mut keys = Vec::new();
    for group in [
        TOP_LEVEL_CONFIG_KEYS,
        GENERAL_CONFIG_KEYS,
        METRICS_CONFIG_KEYS,
        API_CONFIG_KEYS,
        LISTENER_CONFIG_KEYS,
        WEB_CONFIG_KEYS,
        LIMITS_CONFIG_KEYS,
        WEB_DEBUG_CONFIG_KEYS,
        TIMEOUTS_CONFIG_KEYS,
        USER_ENTRY_CONFIG_KEYS,
        UPSTREAM_CONFIG_KEYS,
        LOGGING_CONFIG_KEYS,
    ] {
        keys.extend_from_slice(group);
    }
    keys
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr = vec![0usize; b_chars.len() + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b_chars.iter().enumerate() {
            let replace = if ca == *cb { prev[j] } else { prev[j] + 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(replace);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_chars.len()]
}

fn unknown_key_suggestion(key: &str, known_keys: &[&'static str]) -> Option<String> {
    let normalized = key.to_ascii_lowercase();
    let mut best: Option<(&str, usize)> = None;
    for known in known_keys {
        let distance = levenshtein_distance(&normalized, known);
        let is_better = match best {
            Some((_, best_distance)) => distance < best_distance,
            None => true,
        };
        if distance <= 4 && is_better {
            best = Some((known, distance));
        }
    }
    best.map(|(known, _)| known.to_string())
}

fn push_unknown_keys(
    unknown: &mut Vec<UnknownConfigKey>,
    known_for_suggestion: &[&'static str],
    path: &str,
    table: &serde_yaml::Mapping,
    allowed: &[&str],
) {
    for key in table.keys() {
        let Some(key_str) = key.as_str() else {
            // Non-string YAML keys can never match a known config key; render
            // the scalar itself for the reported path.
            let key_text = serde_yaml::to_string(key)
                .unwrap_or_default()
                .trim()
                .to_string();
            let full_path = if path.is_empty() {
                key_text
            } else {
                format!("{path}.{key_text}")
            };
            unknown.push(UnknownConfigKey {
                path: full_path,
                suggestion: None,
            });
            continue;
        };
        if !allowed.contains(&key_str) {
            let full_path = if path.is_empty() {
                key_str.to_string()
            } else {
                format!("{path}.{key_str}")
            };
            unknown.push(UnknownConfigKey {
                path: full_path,
                suggestion: unknown_key_suggestion(key_str, known_for_suggestion),
            });
        }
    }
}

fn check_known_table(
    parsed_yaml: &serde_yaml::Value,
    unknown: &mut Vec<UnknownConfigKey>,
    known_for_suggestion: &[&'static str],
    path: &[&str],
    allowed: &[&str],
) {
    if let Some(table) = table_at(parsed_yaml, path) {
        push_unknown_keys(
            unknown,
            known_for_suggestion,
            &path.join("."),
            table,
            allowed,
        );
    }
}

fn check_nested_table_value(
    unknown: &mut Vec<UnknownConfigKey>,
    known_for_suggestion: &[&'static str],
    path: String,
    value: &serde_yaml::Value,
    allowed: &[&str],
) {
    if let Some(table) = value.as_mapping() {
        push_unknown_keys(unknown, known_for_suggestion, &path, table, allowed);
    }
}

/// Collects unknown keys across every supported nested configuration table.
pub(super) fn collect_unknown_config_keys(parsed_yaml: &serde_yaml::Value) -> Vec<UnknownConfigKey> {
    let known_for_suggestion = known_config_keys_for_suggestion();
    let mut unknown = Vec::new();

    if let Some(root) = parsed_yaml.as_mapping() {
        push_unknown_keys(
            &mut unknown,
            &known_for_suggestion,
            "",
            root,
            TOP_LEVEL_CONFIG_KEYS,
        );
    }

    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["general"],
        GENERAL_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["logging"],
        LOGGING_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["metrics"],
        METRICS_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["api"],
        API_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["web"],
        WEB_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["limits"],
        LIMITS_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["web", "debug"],
        WEB_DEBUG_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["timeouts"],
        TIMEOUTS_CONFIG_KEYS,
    );
    check_known_table(
        parsed_yaml,
        &mut unknown,
        &known_for_suggestion,
        &["listener"],
        LISTENER_CONFIG_KEYS,
    );

    if let Some(users) = parsed_yaml.get("users").and_then(serde_yaml::Value::as_mapping) {
        for (key, entry) in users {
            if let Some(name) = key.as_str() {
                check_nested_table_value(
                    &mut unknown,
                    &known_for_suggestion,
                    format!("users.{name}"),
                    entry,
                    USER_ENTRY_CONFIG_KEYS,
                );
            }
        }
    }

    if let Some(upstreams) = parsed_yaml.get("upstreams").and_then(serde_yaml::Value::as_sequence) {
        for (idx, upstream) in upstreams.iter().enumerate() {
            check_nested_table_value(
                &mut unknown,
                &known_for_suggestion,
                format!("upstreams[{idx}]"),
                upstream,
                UPSTREAM_CONFIG_KEYS,
            );
        }
    }

    unknown
}
