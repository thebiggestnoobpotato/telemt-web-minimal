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

    let config: ProxyConfig = parsed_toml
        .try_into()
        .map_err(|e| ProxyError::Config(e.to_string()))?;

    Ok((config, source_files, source_contents, processed))
}
