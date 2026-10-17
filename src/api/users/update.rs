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
    let secret_changed = body.secret.is_some();
    let touches_users = secret_changed
        || body.host.is_some()
        || body.max_sessions.is_some()
        || body.max_streams.is_some()
        || body.max_streams_per_session.is_some();

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

    if !cfg.users.contains_key(user) {
        return Err(ApiFailure::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "User not found",
        ));
    }

    if let Some(entry) = cfg.users.get_mut(user) {
        if let Some(secret) = body.secret {
            entry.secret = secret;
        }
        if let Some(host) = body.host {
            entry.host = host;
        }
        if let Some(max_sessions) = body.max_sessions {
            entry.max_sessions = max_sessions;
        }
        if let Some(max_streams) = body.max_streams {
            entry.max_streams = max_streams;
        }
        if let Some(max_streams_per_session) = body.max_streams_per_session {
            entry.max_streams_per_session = max_streams_per_session;
        }
    }
    cfg.validate_user_web_fields()
        .map_err(|e| ApiFailure::bad_request(format!("config validation failed: {}", e)))?;
    cfg.validate()
        .map_err(|e| ApiFailure::bad_request(format!("config validation failed: {}", e)))?;
    let staged_credential = if secret_changed {
            let entry = cfg
                .users
                .get(user)
                .ok_or_else(|| ApiFailure::internal("updated user secret is missing"))?;
            Some(credential_id_from_hex(&entry.secret).ok_or_else(|| {
                ApiFailure::internal("validated user secret could not be decoded")
            })?)
        } else {
            None
        };

    let mut touched_sections = Vec::new();
    if touches_users {
        touched_sections.push(AccessSection::Users);
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
        shared.proxy_shared.stage_user_credential(user, credential_id);
    }
    drop(_guard);

    let users = users_from_config(&cfg, &shared.stats, None).await;
    let user_info = users
        .into_iter()
        .find(|entry| entry.username == user)
        .ok_or_else(|| ApiFailure::internal("failed to build updated user view"))?;

    Ok((user_info, revision))
}
