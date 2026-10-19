use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use hyper::header::IF_MATCH;
use sha2::{Digest, Sha256};

use crate::config::{ConfigSourceGraph, LoadedConfig, ParsedConfigSource, ProxyConfig};

use super::model::ApiFailure;

// Source-preserving YAML rendering and atomic persistence helpers.
mod persistence;
// Compare-and-replace file persistence and metadata preservation.
mod atomic;

pub(in crate::api) use atomic::{write_atomic, write_atomic_if_unchanged};
#[cfg(test)]
use persistence::{find_yaml_section_bounds, render_access_section, save_sections_to_disk};
pub(in crate::api) use persistence::{
    render_top_level_section, save_access_sections_to_disk,
    save_access_sections_to_disk_if_revision, top_level_key_of, upsert_yaml_section,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AccessSection {
    Users,
}

impl AccessSection {
    fn table_name(self) -> &'static str {
        match self {
            Self::Users => "users",
        }
    }
}

pub(super) fn parse_if_match(headers: &hyper::HeaderMap) -> Option<String> {
    headers
        .get(IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_matches('"').to_string())
}

/// Loads one mutation base and validates its revision from the same source snapshot.
pub(super) async fn load_config_for_mutation(
    config_path: &Path,
    expected_revision: Option<&str>,
) -> Result<(ProxyConfig, String), ApiFailure> {
    let loaded = load_config_snapshot(config_path, false).await?;
    let revision = compute_snapshot_revision(&loaded.source_contents);
    if expected_revision.is_some_and(|expected| expected != revision) {
        return Err(ApiFailure::new(
            hyper::StatusCode::CONFLICT,
            "revision_conflict",
            "Config revision mismatch",
        ));
    }
    Ok((loaded.config, revision))
}

pub(super) async fn current_revision(config_path: &Path) -> Result<String, ApiFailure> {
    let config_path = config_path.to_path_buf();
    let graph = tokio::task::spawn_blocking(move || ProxyConfig::read_source_graph(config_path))
        .await
        .map_err(|error| ApiFailure::internal(format!("failed to join config reader: {error}")))?
        .map_err(|error| ApiFailure::internal(format!("failed to read config graph: {error}")))?;
    Ok(compute_source_revision(&graph))
}

pub(super) fn compute_revision(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hashes source documents only; prepared DNS answers never change a revision.
pub(super) fn compute_snapshot_revision(source_contents: &BTreeMap<PathBuf, String>) -> String {
    compute_source_revision(&ConfigSourceGraph {
        source_contents: source_contents.clone(),
        rendered: String::new(),
    })
}

pub(super) fn compute_source_revision(graph: &ConfigSourceGraph) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"telemt-config-manifest-v1\0");
    for (path, content) in &graph.source_contents {
        let path = path.as_os_str().as_encoded_bytes();
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path);
        hasher.update((content.len() as u64).to_le_bytes());
        hasher.update(content.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Reads a normalized source snapshot without resolving fallbacks or loading static assets.
pub(super) async fn load_config_snapshot(
    config_path: &Path,
    invalid_is_bad_request: bool,
) -> Result<ParsedConfigSource, ApiFailure> {
    let config_path = config_path.to_path_buf();
    tokio::task::spawn_blocking(move || ProxyConfig::parse_source(config_path))
        .await
        .map_err(|error| ApiFailure::internal(format!("failed to join config loader: {error}")))?
        .map_err(|error| {
            if invalid_is_bad_request {
                ApiFailure::bad_request(format!("invalid runtime config: {error}"))
            } else {
                ApiFailure::internal(format!("failed to load config: {error}"))
            }
        })
}

/// Selects the single writable source document owning all requested sections.
pub(super) fn resolve_single_source_owner(
    loaded: &ParsedConfigSource,
    config_path: &Path,
    targets: &[&str],
) -> Result<PathBuf, ApiFailure> {
    let root = normalize_source_path(config_path);
    let mut mutation_owners = BTreeSet::new();

    if loaded
        .source_contents
        .values()
        .any(|content| has_include_inside_section(content))
    {
        return Err(ApiFailure::new(
            hyper::StatusCode::CONFLICT,
            "config_patch_not_atomic",
            "config includes nested inside a YAML section cannot be mutated atomically",
        ));
    }

    for target in targets {
        let mut owners = BTreeSet::new();
        for (path, content) in &loaded.source_contents {
            let parsed: serde_yaml::Value = serde_yaml::from_str(content).map_err(|error| {
                ApiFailure::new(
                    hyper::StatusCode::CONFLICT,
                    "config_patch_not_atomic",
                    format!(
                        "config source {} is not independently writable: {error}",
                        path.display()
                    ),
                )
            })?;
            if yaml_path_exists(&parsed, target) {
                owners.insert(path.clone());
            }
        }
        match owners.len() {
            0 => {
                mutation_owners.insert(root.clone());
            }
            1 => {
                mutation_owners.extend(owners);
            }
            _ => {
                return Err(ApiFailure::new(
                    hyper::StatusCode::CONFLICT,
                    "config_patch_not_atomic",
                    format!("config section {target} is owned by multiple source files"),
                ));
            }
        }
    }

    if mutation_owners.len() != 1 {
        return Err(ApiFailure::new(
            hyper::StatusCode::CONFLICT,
            "config_patch_not_atomic",
            "one mutation may update only one config source file",
        ));
    }
    mutation_owners
        .into_iter()
        .next()
        .ok_or_else(|| ApiFailure::bad_request("empty mutation: no owned config sections"))
}

fn yaml_path_exists(value: &serde_yaml::Value, target: &str) -> bool {
    target
        .split('.')
        .try_fold(value, |current, part| current.get(part))
        .is_some()
}

// An include directive inside a section defeats atomic section replacement:
// its spliced content lands at column 0 in the middle of the section's block.
// A top-level `include:` line is legal and ends the section it follows.
fn has_include_inside_section(content: &str) -> bool {
    let mut inside_section = false;
    for line in content.lines() {
        if let Some(key) = top_level_key_of(line) {
            inside_section = key != "include";
            continue;
        }
        if inside_section
            && line.starts_with(' ')
            && line.trim_start().starts_with("include:")
        {
            return true;
        }
    }
    false
}

/// Prepares the entire mutation candidate before any source file may be replaced.
pub(super) async fn load_candidate_snapshot(
    config_path: &Path,
    base_sources: &BTreeMap<PathBuf, String>,
    owner_path: PathBuf,
    owner_contents: String,
) -> Result<LoadedConfig, ApiFailure> {
    let config_path = config_path.to_path_buf();
    let mut overrides = base_sources.clone();
    overrides.insert(owner_path, owner_contents);
    let parsed = tokio::task::spawn_blocking(move || {
        ProxyConfig::parse_source_with_overrides(config_path, &overrides)
    })
    .await
    .map_err(|error| ApiFailure::internal(format!("failed to join config loader: {error}")))?
    .map_err(|error| ApiFailure::bad_request(format!("invalid patched config: {error}")))?;
    parsed
        .prepare()
        .await
        .map_err(|error| ApiFailure::bad_request(format!("invalid patched config: {error}")))
}

fn normalize_source_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Reads desired user policy without acquiring runtime fallback state.
pub(super) async fn load_config_from_disk(config_path: &Path) -> Result<ProxyConfig, ApiFailure> {
    load_config_snapshot(config_path, false)
        .await
        .map(|source| source.config)
}

#[allow(dead_code)]
pub(super) async fn save_config_to_disk(
    config_path: &Path,
    cfg: &ProxyConfig,
) -> Result<String, ApiFailure> {
    let serialized = serde_yaml::to_string(cfg)
        .map_err(|e| ApiFailure::internal(format!("failed to serialize config: {}", e)))?;
    write_atomic(config_path.to_path_buf(), serialized.clone()).await?;
    Ok(compute_revision(&serialized))
}

/// Top-level config tables that may be edited via the config API.
///
/// Intentionally excluded (defense-in-depth, enforces the spec's per-node
/// identity invariant at the Telemt layer too):
///
///   - `users`     : owned by the users API.
///   - `access`    : per-user policy maps owned by the users API.
///   - `metrics`   : process-owned endpoint identity (bind address, scraper
///                   whitelist).
///   - `api`       : process-owned endpoint identity (bind address, auth).
pub(super) const EDITABLE_SECTIONS: &[&str] = &[
    "general",
    "logging",
    "listener",
    "limits",
    "upstreams",
    "web",
];

/// Whether `key` is an allowed top-level PATCH/GET section name.
pub(super) fn is_editable_section(key: &str) -> bool {
    EDITABLE_SECTIONS.contains(&key)
}

#[cfg(test)]
#[path = "config_store/tests.rs"]
mod tests;
