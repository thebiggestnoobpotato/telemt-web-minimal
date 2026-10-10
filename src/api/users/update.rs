use super::*;

pub(in crate::api) async fn patch_user(
    user: &str,
    body: PatchUserRequest,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<(UserInfo, String), ApiFailure> {
    let shared = shared.clone();
    let user = user.to_string();
    shared
        .clone()
        .run_mutation_completion(async move {
            patch_user_to_completion(&user, body, expected_revision, &shared).await
        })
        .await
}

async fn patch_user_to_completion(
    user: &str,
    body: PatchUserRequest,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<(UserInfo, String), ApiFailure> {
    let touches_users = body.secret.is_some();
    let touches_user_max_unique_ips = !matches!(&body.max_unique_ips, Patch::Unchanged);
    let touches_user_enabled = !matches!(&body.enabled, Patch::Unchanged);

    if let Some(secret) = body.secret.as_ref()
        && !is_valid_user_secret(secret)
    {
        return Err(ApiFailure::bad_request(
            "secret must be exactly 32 hex characters",
        ));
    }
    let _guard = shared.mutation_lock.lock().await;
    let (mut cfg, base_revision) =
        load_config_for_mutation(&shared.config_path, expected_revision.as_deref()).await?;

    if !cfg.access.users.contains_key(user) {
        return Err(ApiFailure::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "User not found",
        ));
    }

    if let Some(secret) = body.secret {
        cfg.access.users.insert(user.to_string(), secret);
    }
    // Capture how the per-user IP limit changed, so the in-memory ip_tracker
    // can be synced (set or removed) after the config is persisted.
    let max_unique_ips_change = match body.max_unique_ips {
        Patch::Unchanged => None,
        Patch::Remove => {
            cfg.access.user_max_unique_ips.remove(user);
            Some(None)
        }
        Patch::Set(limit) => {
            cfg.access
                .user_max_unique_ips
                .insert(user.to_string(), limit);
            Some(Some(limit))
        }
    };
    match body.enabled {
        Patch::Unchanged => {}
        Patch::Remove | Patch::Set(true) => {
            cfg.access.user_enabled.remove(user);
        }
        Patch::Set(false) => {
            cfg.access.user_enabled.insert(user.to_string(), false);
        }
    }

    cfg.validate()
        .map_err(|e| ApiFailure::bad_request(format!("config validation failed: {}", e)))?;
    let staged_credential =
        if touches_users || touches_user_enabled {
            let secret = cfg
                .access
                .users
                .get(user)
                .ok_or_else(|| ApiFailure::internal("updated user secret is missing"))?;
            Some(credential_id_from_hex(secret).ok_or_else(|| {
                ApiFailure::internal("validated user secret could not be decoded")
            })?)
        } else {
            None
        };

    let mut touched_sections = Vec::new();
    if touches_users {
        touched_sections.push(AccessSection::Users);
    }
    if touches_user_max_unique_ips {
        touched_sections.push(AccessSection::UserMaxUniqueIps);
    }
    if touches_user_enabled {
        touched_sections.push(AccessSection::UserEnabled);
    }

    let revision = if touched_sections.is_empty() {
        current_revision(&shared.config_path).await?
    } else {
        save_access_sections_to_disk_if_revision(
            &shared.config_path,
            &cfg,
            &touched_sections,
            Some(&base_revision),
        )
        .await?
    };
    if let Some(credential_id) = staged_credential {
        shared.proxy_shared.stage_user_credential(
            user,
            credential_id,
            cfg.access.is_user_enabled(user),
        );
    }
    match max_unique_ips_change {
        Some(Some(limit)) => shared.ip_tracker.set_user_limit(user, limit).await,
        Some(None) => shared.ip_tracker.remove_user_limit(user).await,
        None => {}
    }
    drop(_guard);

    let users = users_from_config(&cfg, &shared.stats, &shared.ip_tracker, None).await;
    let user_info = users
        .into_iter()
        .find(|entry| entry.username == user)
        .ok_or_else(|| ApiFailure::internal("failed to build updated user view"))?;

    Ok((user_info, revision))
}

pub(in crate::api) async fn set_user_enabled(
    user: &str,
    enabled: bool,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<(UserInfo, String), ApiFailure> {
    let shared = shared.clone();
    let user = user.to_string();
    shared
        .clone()
        .run_mutation_completion(async move {
            set_user_enabled_to_completion(&user, enabled, expected_revision, &shared).await
        })
        .await
}

async fn set_user_enabled_to_completion(
    user: &str,
    enabled: bool,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<(UserInfo, String), ApiFailure> {
    let _guard = shared.mutation_lock.lock().await;
    let (mut cfg, base_revision) =
        load_config_for_mutation(&shared.config_path, expected_revision.as_deref()).await?;

    if !cfg.access.users.contains_key(user) {
        return Err(ApiFailure::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "User not found",
        ));
    }

    if enabled {
        cfg.access.user_enabled.remove(user);
    } else {
        cfg.access.user_enabled.insert(user.to_string(), false);
    }

    cfg.validate()
        .map_err(|e| ApiFailure::bad_request(format!("config validation failed: {}", e)))?;
    let credential_id = cfg
        .access
        .users
        .get(user)
        .and_then(|secret| credential_id_from_hex(secret))
        .ok_or_else(|| ApiFailure::internal("validated user secret could not be decoded"))?;
    let revision = save_access_sections_to_disk_if_revision(
        &shared.config_path,
        &cfg,
        &[AccessSection::UserEnabled],
        Some(&base_revision),
    )
    .await?;
    shared
        .proxy_shared
        .stage_user_credential(user, credential_id, enabled);
    drop(_guard);

    let users = users_from_config(&cfg, &shared.stats, &shared.ip_tracker, None).await;
    let user_info = users
        .into_iter()
        .find(|entry| entry.username == user)
        .ok_or_else(|| ApiFailure::internal("failed to build updated user view"))?;

    Ok((user_info, revision))
}
