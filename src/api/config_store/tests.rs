use super::*;

#[tokio::test]
async fn save_sections_preserves_other_tables_and_comments() {
    let dir = std::env::temp_dir().join(format!("cfgtest-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(
        &path,
        "# top comment\ngeneral:\n  listen_backlog: 4096\n\n\
         users:\n  alice: \"000102030405060708090a0b0c0d0e0f\"\n",
    )
    .unwrap();

    let mut cfg = ProxyConfig::default();
    cfg.general.listen_backlog = 4096;

    let rev = save_sections_to_disk(&path, &cfg, &["general"])
        .await
        .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("listen_backlog: 4096"));
    // Untouched comments and sections remain byte content.
    assert!(written.contains("# top comment"));
    assert!(written.contains("users:"));
    assert_eq!(rev, compute_revision(&written));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn find_bounds_matches_sequence_section() {
    let src = "other:\n  port: 1\n\nupstreams:\n  - kind: \"a\"\n  - kind: \"b\"\n";
    let bounds = find_yaml_section_bounds(src, "upstreams");
    assert!(bounds.is_some(), "should locate upstreams block start");
    let (start, end) = bounds.unwrap();
    let slice = &src[start..end];
    assert!(slice.starts_with("upstreams:"));
    // The bound spans through the last sequence item.
    assert!(slice.contains("kind: \"b\""));
}

#[test]
fn find_bounds_matches_header_with_inline_comment() {
    let src = "first: # notes\n  ipv6: false\n\nother:\n  port: 1\n";
    let bounds = find_yaml_section_bounds(src, "first");
    assert!(bounds.is_some(), "commented header must still match");
    let (start, end) = bounds.unwrap();
    let slice = &src[start..end];
    assert!(slice.starts_with("first: # notes"));
    assert!(slice.contains("ipv6"));
    // The bound terminates at the next top-level key.
    assert!(!slice.contains("other:"));
}

#[tokio::test]
async fn save_web_section_keeps_subtables_nested_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    tokio::fs::write(
        &path,
        "web:\n  timeouts:\n    http_overload_timeout_ms: 1\n\n\
         other:\n  port: 443\n",
    )
    .await
    .unwrap();

    let cfg = ProxyConfig::default();

    save_sections_to_disk(&path, &cfg, &["web"])
        .await
        .unwrap();

    let written = tokio::fs::read_to_string(&path).await.unwrap();

    // No bare top-level timeouts key leaked.
    for line in written.lines() {
        assert_ne!(line, "timeouts:", "leaked top-level timeouts:\n{written}");
    }

    // The web timeouts sub-block appears exactly once.
    assert_eq!(
        written.matches("  timeouts:").count(),
        1,
        "web timeouts block must appear exactly once:\n{written}"
    );

    // Result parses (duplicate sections would error here).
    serde_yaml::from_str::<serde_yaml::Value>(&written)
        .unwrap_or_else(|e| panic!("written config must parse: {e}\n{written}"));

    // The unrelated section remains untouched.
    assert!(written.contains("other:\n  port: 443"));
}

#[tokio::test]
async fn save_web_section_is_idempotent_across_repeated_saves() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    tokio::fs::write(
        &path,
        "web:\n  timeouts:\n    http_overload_timeout_ms: 1\n",
    )
    .await
    .unwrap();

    let cfg = ProxyConfig::default();

    save_sections_to_disk(&path, &cfg, &["web"])
        .await
        .unwrap();
    save_sections_to_disk(&path, &cfg, &["web"])
        .await
        .unwrap();

    let written = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(written.matches("  timeouts:").count(), 1, "{written}");
    assert_eq!(
        written.lines().filter(|line| *line == "web:").count(),
        1,
        "{written}"
    );
    serde_yaml::from_str::<serde_yaml::Value>(&written)
        .unwrap_or_else(|e| panic!("written config must parse: {e}\n{written}"));
}

#[test]
fn find_bounds_spans_nested_subtables() {
    let src = "web:\n  timeouts:\n    http_overload_timeout_ms: 1\n\n\
                other:\n  port: 1\n";
    let bounds = find_yaml_section_bounds(src, "web");
    assert!(bounds.is_some(), "should locate web block");
    let (start, end) = bounds.unwrap();
    let slice = &src[start..end];
    assert!(slice.starts_with("web:"));
    // Nested sub-blocks belong to the parent section bound.
    assert!(slice.contains("timeouts:"));
    // The bound terminates before an unrelated top-level key.
    assert!(!slice.contains("other:"));
}

#[test]
fn find_bounds_does_not_overrun_sibling_prefix() {
    // users must not swallow a sibling key sharing the section-name prefix.
    let src = "users:\n  alice: \"x\"\n\nusers_extra:\n  alice: 16\n";
    let bounds = find_yaml_section_bounds(src, "users").unwrap();
    let slice = &src[bounds.0..bounds.1];
    assert!(slice.starts_with("users:"));
    assert!(!slice.contains("users_extra:"));
}

#[test]
fn nested_include_detection_does_not_reject_similar_access_keys() {
    assert!(has_include_inside_section(
        "users:\n  include: \"users.yaml\"\n"
    ));
    assert!(!has_include_inside_section(
        "users:\n  include_user: \"00000000000000000000000000000000\"\n"
    ));
}

#[tokio::test]
async fn save_web_collapses_scattered_sections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    // Hand-edited layout: a second web block sits after an unrelated section.
    tokio::fs::write(
        &path,
        "web:\n  carrier: \"https-lanes\"\n\n\
         other:\n  port: 443\n\n\
         web:\n  timeouts:\n    http_overload_timeout_ms: 1\n",
    )
    .await
    .unwrap();

    let cfg = ProxyConfig::default();

    save_sections_to_disk(&path, &cfg, &["web"])
        .await
        .unwrap();

    let written = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(
        written.lines().filter(|line| *line == "web:").count(),
        1,
        "scattered web blocks must collapse to one:\n{written}"
    );
    serde_yaml::from_str::<serde_yaml::Value>(&written)
        .unwrap_or_else(|e| panic!("written config must parse: {e}\n{written}"));
    // The unrelated section remains present.
    assert!(written.contains("other:"));
}

#[tokio::test]
async fn manifest_revision_changes_when_an_included_source_changes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    let included = dir.path().join("included.yaml");
    tokio::fs::write(&root, "include: \"included.yaml\"\n")
        .await
        .unwrap();
    tokio::fs::write(&included, "general:\n  network_ipv6: true\n")
        .await
        .unwrap();
    let first = current_revision(&root).await.unwrap();

    tokio::fs::write(&included, "general:\n  network_ipv6: false\n")
        .await
        .unwrap();
    let second = current_revision(&root).await.unwrap();

    assert_ne!(first, second);
}

#[tokio::test]
async fn manifest_revision_does_not_require_typed_config_validation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    tokio::fs::write(&root, "general:\n  listen_backlog: \"invalid\"\n")
        .await
        .unwrap();

    let revision = current_revision(&root).await.unwrap();

    assert_eq!(revision.len(), 64);
    assert!(load_config_snapshot(&root, false).await.is_err());
}

#[tokio::test]
async fn access_mutation_writes_only_the_single_included_owner() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    let included = dir.path().join("users.yaml");
    let root_body = "include: \"users.yaml\"\ngeneral:\n  network_ipv6: true\n";
    let included_body = "users:\n  alice: \"00000000000000000000000000000000\"\n";
    tokio::fs::write(&root, root_body).await.unwrap();
    tokio::fs::write(&included, included_body).await.unwrap();
    let mut cfg = load_config_from_disk(&root).await.unwrap();
    cfg.users.insert(
        "bob".to_string(),
        "11111111111111111111111111111111".to_string(),
    );

    let revision = save_access_sections_to_disk(&root, &cfg, &[AccessSection::Users])
        .await
        .unwrap();

    assert_eq!(tokio::fs::read_to_string(&root).await.unwrap(), root_body);
    let written = tokio::fs::read_to_string(&included).await.unwrap();
    assert!(written.contains("bob: '11111111111111111111111111111111'"));
    assert_eq!(revision, current_revision(&root).await.unwrap());
}

#[tokio::test]
async fn access_mutation_rejects_source_graph_change_after_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    let included = dir.path().join("users.yaml");
    let root_body = "include: \"users.yaml\"\ngeneral:\n  network_ipv6: true\n";
    let external_root = "include: \"users.yaml\"\ngeneral:\n  network_ipv6: false\n";
    let included_body = "users:\n  alice: \"00000000000000000000000000000000\"\n";
    tokio::fs::write(&root, root_body).await.unwrap();
    tokio::fs::write(&included, included_body).await.unwrap();
    let (mut cfg, revision) = load_config_for_mutation(&root, None).await.unwrap();
    cfg.users.insert(
        "bob".to_string(),
        "11111111111111111111111111111111".to_string(),
    );
    tokio::fs::write(&root, external_root).await.unwrap();

    let error = save_access_sections_to_disk_if_revision(
        &root,
        &cfg,
        &[AccessSection::Users],
        Some(&revision),
    )
    .await
    .unwrap_err();

    assert_eq!(error.code, "revision_conflict");
    assert_eq!(
        tokio::fs::read_to_string(&root).await.unwrap(),
        external_root
    );
    assert_eq!(
        tokio::fs::read_to_string(&included).await.unwrap(),
        included_body
    );
}

#[test]
fn rendered_users_section_round_trips_numeric_looking_values() {
    // The rendered section is spliced back into the source, so every scalar
    // must survive re-parsing as a string. The libyaml emitter quotes any
    // token that would otherwise re-resolve as a non-string.
    let mut cfg = ProxyConfig::default();
    cfg.users.insert(
        "00000000000000000000000000000000".to_string(),
        "00000000000000000000000000000000".to_string(),
    );
    cfg.users.insert(
        "12345678901234567890123456789012".to_string(),
        "12345678901234567890123456789012".to_string(),
    );
    cfg.users.insert("0x1f".to_string(), "0x1f".to_string());
    cfg.users.insert("true".to_string(), "plain-secret".to_string());

    let rendered = render_access_section(&cfg, AccessSection::Users).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&rendered).unwrap();
    let users = parsed["users"].as_mapping().unwrap();
    assert_eq!(users.len(), cfg.users.len(), "{rendered}");
    for (key, value) in users.iter() {
        let secret = cfg.users.get(key.as_str().unwrap()).unwrap();
        assert_eq!(
            value.as_str(),
            Some(secret.as_str()),
            "scalar must round-trip as a string: {rendered}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn atomic_write_preserves_existing_file_mode() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    tokio::fs::write(&path, "old").await.unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let before = std::fs::metadata(&path).unwrap();

    write_atomic(path.clone(), "new".to_string()).await.unwrap();

    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(after.mode() & 0o7777, 0o640);
    assert_eq!(after.uid(), before.uid());
    assert_eq!(after.gid(), before.gid());
}

#[tokio::test]
async fn config_sidecar_lock_serializes_competing_revision_writers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    let original = concat!(
        "general:\n",
        "  network_ipv6: true\n",
        "users:\n",
        "  alice: \"00000000000000000000000000000000\"\n"
    );
    tokio::fs::write(&path, original).await.unwrap();
    let graph = ProxyConfig::read_source_graph(&path).unwrap();
    let revision = compute_source_revision(&graph);

    let first = tokio::spawn(write_atomic_if_unchanged(
        path.clone(),
        revision.clone(),
        path.clone(),
        original.to_string(),
        original.replace("general:", "general:\n# one"),
    ));
    let second = tokio::spawn(write_atomic_if_unchanged(
        path.clone(),
        revision,
        path.clone(),
        original.to_string(),
        original.replace("general:", "general:\n# two"),
    ));
    let first = first.await.unwrap();
    let second = second.await.unwrap();

    assert_ne!(first.is_ok(), second.is_ok());
    let (winner_revision, conflict) = match (first, second) {
        (Ok(revision), Err(error)) | (Err(error), Ok(revision)) => (revision, error),
        _ => unreachable!("exactly one cooperative writer must commit"),
    };
    assert_eq!(conflict.code, "revision_conflict");
    assert_eq!(winner_revision, current_revision(&path).await.unwrap());
    let persisted = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(persisted.contains("# one") || persisted.contains("# two"));
}

#[cfg(unix)]
#[test]
fn source_owner_normalization_preserves_symlinks() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real.yaml");
    let linked = dir.path().join("linked.yaml");
    std::fs::write(&real, "").unwrap();
    symlink(&real, &linked).unwrap();

    assert_eq!(normalize_source_path(&linked), linked);
}
