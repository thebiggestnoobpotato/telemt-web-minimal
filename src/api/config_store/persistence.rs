use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::config::ProxyConfig;

#[cfg(test)]
use super::atomic::write_atomic;
use super::atomic::write_atomic_if_unchanged;
#[cfg(test)]
use super::compute_revision;
use super::{
    AccessSection, compute_snapshot_revision, load_candidate_snapshot, load_config_snapshot,
    resolve_single_source_owner, yaml_path_exists,
};
use crate::api::model::ApiFailure;

/// Re-render the given top-level tables from `cfg` and upsert each into the
/// on-disk file, preserving every untouched section (and its comments).
#[cfg(test)]
pub(super) async fn save_sections_to_disk(
    config_path: &Path,
    cfg: &ProxyConfig,
    sections: &[&str],
) -> Result<String, ApiFailure> {
    let mut content = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|e| ApiFailure::internal(format!("failed to read config: {}", e)))?;

    for section in sections {
        let rendered = render_top_level_section(cfg, section)?;
        content = upsert_yaml_section(&content, section, &rendered);
    }

    write_atomic(config_path.to_path_buf(), content.clone()).await?;
    Ok(compute_revision(&content))
}

/// Render one top-level section as `section:\n...\n` (or a sequence for
/// `upstreams`) from the typed `cfg`. Serializes via `serde_yaml` so the
/// output matches the canonical format Telemt parses. The libyaml emitter
/// already quotes any string token that would not round-trip as a string
/// (numeric-looking secrets, hex usernames, `true`-like values), so the
/// rendered text is always safe to splice back into a config source.
pub(in crate::api) fn render_top_level_section(
    cfg: &ProxyConfig,
    section: &str,
) -> Result<String, ApiFailure> {
    let value = serde_yaml::to_value(cfg)
        .map_err(|e| ApiFailure::internal(format!("failed to serialize config: {}", e)))?;
    let table = value
        .get(section)
        .cloned()
        .ok_or_else(|| ApiFailure::internal(format!("unknown section: {}", section)))?;

    let mut out = format!("{section}:\n");
    match table {
        // upstreams is an array; an empty one collapses to the flow form.
        serde_yaml::Value::Sequence(items) if items.is_empty() => {
            out.pop();
            out.push_str(" []\n");
        }
        body => {
            let rendered = serde_yaml::to_string(&body)
                .map_err(|e| ApiFailure::internal(format!("failed to serialize {}: {}", section, e)))?;
            out.push_str(&indent_yaml_lines(&rendered, "  "));
        }
    }
    Ok(out)
}

// Prefixes every non-empty line of a standalone YAML document with the given
// indent so it can be nested under a top-level section key. Empty lines stay
// empty: a blank line is legal inside a block mapping and keeps the diff clean.
fn indent_yaml_lines(rendered: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(rendered.len() + rendered.lines().count() * prefix.len());
    for line in rendered.split_inclusive('\n') {
        if !line.trim_end().is_empty() {
            out.push_str(prefix);
        }
        out.push_str(line);
    }
    out
}

/// Validates and atomically writes access tables to their single source owner.
pub(in crate::api) async fn save_access_sections_to_disk(
    config_path: &Path,
    cfg: &ProxyConfig,
    sections: &[AccessSection],
) -> Result<String, ApiFailure> {
    save_access_sections_to_disk_if_revision(config_path, cfg, sections, None).await
}

/// Persists access tables only while the complete source graph remains unchanged.
pub(in crate::api) async fn save_access_sections_to_disk_if_revision(
    config_path: &Path,
    cfg: &ProxyConfig,
    sections: &[AccessSection],
    expected_revision: Option<&str>,
) -> Result<String, ApiFailure> {
    let loaded = load_config_snapshot(config_path, false).await?;
    let loaded_revision = compute_snapshot_revision(&loaded.source_contents);
    if expected_revision.is_some_and(|expected| expected != loaded_revision) {
        return Err(revision_conflict());
    }
    let mut applied = Vec::new();
    for section in sections {
        if applied.contains(section) {
            continue;
        }
        applied.push(*section);
    }
    applied.retain(|section| {
        !access_section_is_empty(cfg, *section)
            || loaded.source_contents.values().any(|contents| {
                serde_yaml::from_str::<serde_yaml::Value>(contents)
                    .ok()
                    .is_some_and(|value| yaml_path_exists(&value, section.table_name()))
            })
    });
    if applied.is_empty() {
        return Ok(loaded_revision);
    }

    let targets = applied
        .iter()
        .map(|section| section.table_name())
        .collect::<Vec<_>>();
    let owner_path = resolve_single_source_owner(&loaded, config_path, &targets)?;
    let mut owner_contents = loaded
        .source_contents
        .get(&owner_path)
        .cloned()
        .ok_or_else(|| ApiFailure::internal("config source owner is missing from snapshot"))?;
    let expected_owner_contents = owner_contents.clone();
    for section in applied {
        let rendered = render_access_section(cfg, section)?;
        owner_contents = upsert_yaml_section(&owner_contents, section.table_name(), &rendered);
    }

    let candidate = load_candidate_snapshot(
        config_path,
        &loaded.source_contents,
        owner_path.clone(),
        owner_contents.clone(),
    )
    .await?;
    let _candidate_revision = compute_snapshot_revision(&candidate.source_contents);
    let revision = write_atomic_if_unchanged(
        config_path.to_path_buf(),
        loaded_revision,
        owner_path,
        expected_owner_contents,
        owner_contents,
    )
    .await?;
    Ok(revision)
}

/// Renders one access-control table for persistence tests and user mutations.
pub(super) fn render_access_section(
    cfg: &ProxyConfig,
    section: AccessSection,
) -> Result<String, ApiFailure> {
    let body = match section {
        AccessSection::Users => {
            let rows: BTreeMap<String, String> = cfg
                .users
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            serialize_mapping_body(&rows)?
        }
    };

    if body.is_empty() {
        // A bare `users:` line would deserialize as null, not an empty table.
        return Ok(format!("{}: {{}}\n", section.table_name()));
    }
    let mut out = format!("{}:\n", section.table_name());
    out.push_str(&body);
    Ok(out)
}

fn access_section_is_empty(cfg: &ProxyConfig, section: AccessSection) -> bool {
    match section {
        AccessSection::Users => cfg.users.is_empty(),
    }
}

fn serialize_mapping_body<T: Serialize>(value: &T) -> Result<String, ApiFailure> {
    let rendered = serde_yaml::to_string(value)
        .map_err(|e| ApiFailure::internal(format!("failed to serialize access section: {}", e)))?;
    Ok(indent_yaml_lines(&rendered, "  "))
}

/// Replaces all blocks owned by one top-level YAML section with one rendering.
pub(in crate::api) fn upsert_yaml_section(
    source: &str,
    section: &str,
    replacement: &str,
) -> String {
    let blocks = find_all_section_blocks(source, section);
    if let Some(&(first_start, first_end)) = blocks.first() {
        // Replace the first block in place and delete any further blocks that
        // also belong to this section. Telemt writes a section's content
        // contiguously, but a hand-edited config may scatter it; dropping the
        // extras here prevents the duplicate-key corruption that would
        // otherwise break config load.
        let mut out = String::with_capacity(source.len() + replacement.len());
        out.push_str(&source[..first_start]);
        out.push_str(replacement);
        let mut cursor = first_end;
        for &(start, end) in &blocks[1..] {
            out.push_str(&source[cursor..start]);
            cursor = end;
        }
        out.push_str(&source[cursor..]);
        return out;
    }

    let mut out = source.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(replacement);
    out
}

// The top-level section key a line declares, when it is a column-0 mapping
// entry: a bare identifier followed by a colon and whitespace or end of line,
// any trailing comment stripped. Indented lines never own a top-level section.
pub(in crate::api) fn top_level_key_of(line: &str) -> Option<&str> {
    let without_comment = line.split('#').next().unwrap_or("");
    let head = without_comment.trim_end();
    let colon = head.find(':')?;
    let key = &head[..colon];
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    let after_colon = &head[colon + 1..];
    if !after_colon.is_empty() && !after_colon.starts_with(char::is_whitespace) {
        return None;
    }
    Some(key)
}

/// Locate the first contiguous byte range of one top-level section. Used for
/// existence checks; see [`find_all_section_blocks`] for the full set of
/// (possibly scattered) blocks.
#[cfg(test)]
pub(super) fn find_yaml_section_bounds(source: &str, section: &str) -> Option<(usize, usize)> {
    find_all_section_blocks(source, section).into_iter().next()
}

/// Locate every byte range that belongs to `section`: from its column-0 key
/// line to the next top-level key line (or end of file). Returns one range per
/// contiguous run, so a config where a section's content is scattered (e.g.
/// hand-edited) yields several ranges — letting the caller collapse them into
/// a single rendered block.
fn find_all_section_blocks(source: &str, section: &str) -> Vec<(usize, usize)> {
    let mut blocks = Vec::new();
    let mut offset = 0usize;
    let mut start: Option<usize> = None;

    for line in source.split_inclusive('\n') {
        let key = top_level_key_of(line);
        if let Some(start_offset) = start {
            if key.is_some() && key != Some(section) {
                blocks.push((start_offset, offset));
                start = None;
            }
        }
        if start.is_none() && key == Some(section) {
            start = Some(offset);
        }
        offset = offset.saturating_add(line.len());
    }

    if let Some(start_offset) = start {
        blocks.push((start_offset, source.len()));
    }
    blocks
}

fn revision_conflict() -> ApiFailure {
    ApiFailure::new(
        hyper::StatusCode::CONFLICT,
        "revision_conflict",
        "Config revision changed before persistence",
    )
}
