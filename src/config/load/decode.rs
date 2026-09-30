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
    let server_table = parsed_toml.get("server").and_then(|value| value.as_table());
    let conntrack_control_table = server_table
        .and_then(|table| table.get("conntrack_control"))
        .and_then(|value| value.as_table());
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

    Ok((config, source_files, source_contents, processed))
}
