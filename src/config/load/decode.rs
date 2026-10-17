use serde::de::IntoDeserializer;
use serde::Deserialize;

use super::*;

/// Typed source together with its exact include graph and rendered input.
pub(super) type DecodedSourceGraph = (
    ProxyConfig,
    BTreeSet<PathBuf>,
    BTreeMap<PathBuf, String>,
    String,
);

/// Decodes source while retaining the metadata needed for atomic mutations.
pub(super) fn decode_source_graph(graph: ConfigSourceGraph) -> Result<DecodedSourceGraph> {
    let ConfigSourceGraph {
        source_contents,
        rendered: processed,
    } = graph;
    let source_files: BTreeSet<PathBuf> = source_contents.keys().cloned().collect();

    let parsed_yaml: serde_yaml::Value =
        serde_yaml::from_str(&processed).map_err(|e| ProxyError::Config(e.to_string()))?;
    handle_unknown_config_keys(&parsed_yaml)?;

    let config: ProxyConfig = ProxyConfig::deserialize(parsed_yaml.into_deserializer())
        .map_err(|e| ProxyError::Config(e.to_string()))?;

    Ok((config, source_files, source_contents, processed))
}
