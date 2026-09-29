use super::*;

pub(super) type DecodedSourceGraph = (
    ProxyConfig,
    BTreeSet<PathBuf>,
    BTreeMap<PathBuf, String>,
    String,
);

pub(super) fn decode_source_graph(graph: ConfigSourceGraph) -> Result<DecodedSourceGraph> {
    let ConfigSourceGraph {
        source_contents,
        rendered: processed,
    } = graph;
    let source_files: BTreeSet<PathBuf> = source_contents.keys().cloned().collect();

    let parsed_toml: toml::Value =
        toml::from_str(&processed).map_err(|e| ProxyError::Config(e.to_string()))?;
    handle_unknown_config_keys(&parsed_toml)?;
    let general_table = parsed_toml
        .get("general")
        .and_then(|value| value.as_table());
    let server_table = parsed_toml.get("server").and_then(|value| value.as_table());
    let conntrack_control_table = server_table
        .and_then(|table| table.get("conntrack_control"))
        .and_then(|value| value.as_table());
    let beobachten_is_explicit = general_table
        .map(|table| table.contains_key("beobachten"))
        .unwrap_or(false);
    let beobachten_minutes_is_explicit = general_table
        .map(|table| table.contains_key("beobachten_minutes"))
        .unwrap_or(false);
    let beobachten_flush_secs_is_explicit = general_table
        .map(|table| table.contains_key("beobachten_flush_secs"))
        .unwrap_or(false);
    let beobachten_file_is_explicit = general_table
        .map(|table| table.contains_key("beobachten_file"))
        .unwrap_or(false);
    let legacy_top_level_beobachten = parsed_toml.get("beobachten").cloned();
    let legacy_top_level_beobachten_minutes = parsed_toml.get("beobachten_minutes").cloned();
    let legacy_top_level_beobachten_flush_secs = parsed_toml.get("beobachten_flush_secs").cloned();
    let legacy_top_level_beobachten_file = parsed_toml.get("beobachten_file").cloned();
    let inline_conntrack_control_is_explicit = conntrack_control_table
        .map(|table| table.contains_key("inline_conntrack_control"))
        .unwrap_or(false);

    let mut config: ProxyConfig = parsed_toml
        .try_into()
        .map_err(|e| ProxyError::Config(e.to_string()))?;
    config
        .server
        .conntrack_control
        .inline_conntrack_control_explicit = inline_conntrack_control_is_explicit;

    // Backward compatibility: legacy top-level beobachten* keys.
    // Prefer `[general].*` when both are present.
    let mut legacy_beobachten_applied = false;
    if !beobachten_is_explicit && let Some(value) = legacy_top_level_beobachten.as_ref() {
        let parsed = value.as_bool().ok_or_else(|| {
            ProxyError::Config("beobachten (top-level) must be a boolean".to_string())
        })?;
        config.general.beobachten = parsed;
        legacy_beobachten_applied = true;
    }
    if !beobachten_minutes_is_explicit
        && let Some(value) = legacy_top_level_beobachten_minutes.as_ref()
    {
        let raw = value.as_integer().ok_or_else(|| {
            ProxyError::Config("beobachten_minutes (top-level) must be an integer".to_string())
        })?;
        let parsed = u64::try_from(raw).map_err(|_| {
            ProxyError::Config(
                "beobachten_minutes (top-level) must be within u64 range".to_string(),
            )
        })?;
        config.general.beobachten_minutes = parsed;
        legacy_beobachten_applied = true;
    }
    if !beobachten_flush_secs_is_explicit
        && let Some(value) = legacy_top_level_beobachten_flush_secs.as_ref()
    {
        let raw = value.as_integer().ok_or_else(|| {
            ProxyError::Config("beobachten_flush_secs (top-level) must be an integer".to_string())
        })?;
        let parsed = u64::try_from(raw).map_err(|_| {
            ProxyError::Config(
                "beobachten_flush_secs (top-level) must be within u64 range".to_string(),
            )
        })?;
        config.general.beobachten_flush_secs = parsed;
        legacy_beobachten_applied = true;
    }
    if !beobachten_file_is_explicit && let Some(value) = legacy_top_level_beobachten_file.as_ref() {
        let parsed = value.as_str().ok_or_else(|| {
            ProxyError::Config("beobachten_file (top-level) must be a string".to_string())
        })?;
        config.general.beobachten_file = parsed.to_string();
        legacy_beobachten_applied = true;
    }
    if legacy_beobachten_applied {
        warn!("top-level beobachten* keys are deprecated; use general.beobachten* instead");
    }

    sanitize_ad_tag(&mut config.general.ad_tag);
    Ok((config, source_files, source_contents, processed))
}
