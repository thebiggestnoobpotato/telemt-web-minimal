//! Config-editing API: read managed sections and apply sparse field patches.
//! `users` and `access.*` are intentionally not editable here (owned by the users API).

use serde_json::Value as Json;
use toml::Value as Toml;

use super::ApiShared;
use super::config_store::{
    EDITABLE_SECTIONS, compute_snapshot_revision, is_editable_section, load_candidate_snapshot,
    load_config_snapshot, render_top_level_section, resolve_single_source_owner, upsert_toml_table,
    write_atomic_if_unchanged,
};
use super::model::ApiFailure;
use crate::config::ProxyConfig;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Result of one validated managed-config mutation.
#[derive(Debug, Serialize)]
pub(super) struct PatchConfigResponse {
    /// Revision of the persisted desired configuration.
    pub revision: String,
    /// Top-level config sections changed by the mutation.
    pub changed: Vec<String>,
}

struct PreparedConfigPatch {
    config_path: PathBuf,
    expected_revision: String,
    owner_path: PathBuf,
    expected_owner_contents: String,
    owner_contents: String,
    response: PatchConfigResponse,
}

/// Serializes config mutations behind `mutation_lock`, commits them, and records
/// a runtime event. The route handler calls this shared-state wrapper.
pub(super) async fn patch_config(
    patch_json: Json,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<PatchConfigResponse, ApiFailure> {
    let shared = shared.clone();
    shared
        .clone()
        .run_mutation_completion(async move {
            patch_config_to_completion(patch_json, expected_revision, &shared).await
        })
        .await
}

async fn patch_config_to_completion(
    patch_json: Json,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<PatchConfigResponse, ApiFailure> {
    let _guard = shared.mutation_lock.lock().await;
    let mut prepared =
        prepare_patch_to_path(&shared.config_path, &patch_json, expected_revision).await?;
    prepared.response.revision = write_atomic_if_unchanged(
        prepared.config_path,
        prepared.expected_revision,
        prepared.owner_path,
        prepared.expected_owner_contents,
        prepared.owner_contents,
    )
    .await?;
    let resp = prepared.response;
    drop(_guard);
    shared
        .runtime_events
        .record("api.config.patch.ok", format!("changed={:?}", resp.changed));
    Ok(resp)
}

/// Core patch logic, decoupled from hyper/shared-state so it is unit-testable
/// against a temp file. The route handler holds `mutation_lock` while calling this.
#[cfg(test)]
pub(super) async fn apply_patch_to_path(
    config_path: &Path,
    patch_json: &Json,
    expected_revision: Option<String>,
) -> Result<PatchConfigResponse, ApiFailure> {
    let mut prepared = prepare_patch_to_path(config_path, patch_json, expected_revision).await?;
    let revision = write_atomic_if_unchanged(
        prepared.config_path,
        prepared.expected_revision,
        prepared.owner_path,
        prepared.expected_owner_contents,
        prepared.owner_contents,
    )
    .await?;
    prepared.response.revision = revision;
    Ok(prepared.response)
}

async fn prepare_patch_to_path(
    config_path: &Path,
    patch_json: &Json,
    expected_revision: Option<String>,
) -> Result<PreparedConfigPatch, ApiFailure> {
    // 1. optimistic concurrency
    let loaded = load_config_snapshot(config_path, false).await?;
    let current = compute_snapshot_revision(&loaded.source_contents);
    if expected_revision.is_some_and(|expected| expected != current) {
        return Err(ApiFailure::new(
            hyper::StatusCode::CONFLICT,
            "revision_conflict",
            "Config revision mismatch",
        ));
    }

    // 2. convert + reject access / unknown sections
    let patch_toml = json_to_toml(patch_json)
        .map_err(|e| ApiFailure::bad_request(format!("invalid patch: {}", e)))?;
    let patch_table = patch_toml
        .as_table()
        .ok_or_else(|| ApiFailure::bad_request("patch must be a JSON object"))?;
    if patch_table.contains_key("access") {
        return Err(ApiFailure::new(
            hyper::StatusCode::BAD_REQUEST,
            "access_not_editable",
            "access.* is managed via the users API, not editable here",
        ));
    }
    for (key, _value) in patch_table {
        if !is_editable_section(key.as_str()) {
            return Err(ApiFailure::new(
                hyper::StatusCode::BAD_REQUEST,
                "section_not_editable",
                format!("section not editable: {}", key),
            ));
        }
    }
    let touched: Vec<&str> = patch_table
        .keys()
        .map(|k| k.as_str())
        .filter(|k| is_editable_section(k))
        .collect();
    if touched.is_empty() {
        return Err(ApiFailure::bad_request("empty patch: no editable sections"));
    }

    // 3. Merge against the fully expanded and normalized desired config. The
    // source owner is resolved separately so included sections stay in their
    // original file and unrelated source files remain byte-identical.
    let old_cfg = loaded.config.clone();
    let mut merged = Toml::try_from(&old_cfg)
        .map_err(|e| ApiFailure::internal(format!("failed to serialize config: {}", e)))?;
    deep_merge(&mut merged, &patch_toml);

    let requested_cfg: ProxyConfig = merged
        .clone()
        .try_into()
        .map_err(|e| ApiFailure::bad_request(format!("config does not deserialize: {}", e)))?;
    requested_cfg
        .validate()
        .map_err(|e| ApiFailure::bad_request(format!("config validation failed: {}", e)))?;

    let ownership_targets: Vec<&str> = touched.iter().copied().collect();
    let owner_path = resolve_single_source_owner(&loaded, config_path, &ownership_targets)?;
    let mut owner_contents = loaded
        .source_contents
        .get(&owner_path)
        .cloned()
        .ok_or_else(|| ApiFailure::internal("config source owner is missing from snapshot"))?;
    let expected_owner_contents = owner_contents.clone();
    for section in &touched {
        let rendered = render_top_level_section(&requested_cfg, section)?;
        owner_contents = upsert_toml_table(&owner_contents, section, &rendered);
    }

    let candidate = load_candidate_snapshot(
        config_path,
        &loaded.source_contents,
        owner_path.clone(),
        owner_contents.clone(),
    )
    .await?;
    if touched.contains(&"listener")
        && serde_json::to_value(&candidate.config.listener).ok()
            != serde_json::to_value(&requested_cfg.listener).ok()
    {
        return Err(ApiFailure::new(
            hyper::StatusCode::BAD_REQUEST,
            "ambiguous_listeners",
            "listener normalizes to a different effective listener",
        ));
    }

    // 4. record the validated, normalized candidate with its section-level delta.
    let revision = compute_snapshot_revision(&candidate.source_contents);
    let new_cfg = candidate.config;
    let changed = changed_sections(&old_cfg, &new_cfg);

    Ok(PreparedConfigPatch {
        config_path: config_path.to_path_buf(),
        expected_revision: current,
        owner_path,
        expected_owner_contents,
        owner_contents,
        response: PatchConfigResponse { revision, changed },
    })
}

/// Sorts object keys recursively so section comparison ignores map ordering.
fn canonicalize_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let mut pairs: Vec<(String, serde_json::Value)> =
                std::mem::take(map).into_iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, item) in pairs.iter_mut() {
                canonicalize_json(item);
            }
            for (key, item) in pairs {
                map.insert(key, item);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                canonicalize_json(item);
            }
        }
        _ => {}
    }
}

/// Top-level config sections whose canonical serialized form differs between
/// the active and persisted snapshots.
fn changed_sections(old_cfg: &ProxyConfig, new_cfg: &ProxyConfig) -> Vec<String> {
    let mut lhs = serde_json::to_value(old_cfg).unwrap_or(serde_json::Value::Null);
    let mut rhs = serde_json::to_value(new_cfg).unwrap_or(serde_json::Value::Null);
    canonicalize_json(&mut lhs);
    canonicalize_json(&mut rhs);

    let mut out = Vec::new();
    if let (Some(lo), Some(ro)) = (lhs.as_object(), rhs.as_object()) {
        let mut keys: std::collections::BTreeSet<&String> = lo.keys().collect();
        keys.extend(ro.keys());
        for key in keys {
            if lo.get(key) != ro.get(key) {
                out.push(key.clone());
            }
        }
    }
    out
}

/// Returns only the editable config sections and current revision.
pub(super) async fn read_managed_config(config_path: &Path) -> Result<(Toml, String), ApiFailure> {
    let loaded = load_config_snapshot(config_path, false).await?;
    let revision = compute_snapshot_revision(&loaded.source_contents);
    let parsed = Toml::try_from(&loaded.config)
        .map_err(|error| ApiFailure::internal(format!("failed to serialize config: {error}")))?;

    let parsed_table = parsed
        .as_table()
        .cloned()
        .unwrap_or_else(toml::value::Table::new);
    // Whitelist: return ONLY the editable sections. A blacklist (just removing
    // `access`) would leak `api` (auth_header) and per-node network
    // addresses. Mirror the PATCH section allowlist.
    let mut table = toml::value::Table::new();
    for section in EDITABLE_SECTIONS {
        if let Some(value) = parsed_table.get(*section) {
            table.insert((*section).to_string(), value.clone());
        }
    }

    Ok((Toml::Table(table), revision))
}

/// Convert a serde_json value to a toml value. `null` is dropped from objects
/// (a patch never sets a key to TOML-null). Numbers become integers when exact,
/// otherwise floats.
fn json_to_toml(j: &Json) -> Result<Toml, String> {
    Ok(match j {
        Json::Null => return Err("null is not representable in TOML".into()),
        Json::Bool(b) => Toml::Boolean(*b),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                Toml::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Toml::Float(f)
            } else {
                return Err(format!("unrepresentable number: {}", n));
            }
        }
        Json::String(s) => Toml::String(s.clone()),
        Json::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_toml(item)?);
            }
            Toml::Array(out)
        }
        Json::Object(map) => {
            let mut table = toml::value::Table::new();
            for (k, v) in map {
                if v.is_null() {
                    // TOML has no null value, so sparse object nulls are omitted.
                    continue;
                }
                table.insert(k.clone(), json_to_toml(v)?);
            }
            Toml::Table(table)
        }
    })
}

/// Recursively overlay `patch` onto `base`. Tables merge key-by-key; every
/// other value type (scalars, arrays) replaces wholesale.
fn deep_merge(base: &mut Toml, patch: &Toml) {
    match (base, patch) {
        (Toml::Table(b), Toml::Table(p)) => {
            for (k, pv) in p {
                match b.get_mut(k) {
                    Some(bv) => deep_merge(bv, pv),
                    None => {
                        b.insert(k.clone(), pv.clone());
                    }
                }
            }
        }
        (b, p) => *b = p.clone(),
    }
}

#[cfg(test)]
#[path = "config_edit/base_path_tests.rs"]
mod base_path_tests;
#[cfg(test)]
#[path = "config_edit/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "config_edit/conveyor_tests.rs"]
mod conveyor_tests;

#[cfg(test)]
#[path = "config_edit/fallback_dns_tests.rs"]
mod fallback_dns_tests;
