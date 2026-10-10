use super::*;

pub(in crate::api) async fn create_user(
    body: CreateUserRequest,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<(CreateUserResponse, String), ApiFailure> {
    let shared = shared.clone();
    shared
        .clone()
        .run_mutation_completion(async move {
            create_user_to_completion(body, expected_revision, &shared).await
        })
        .await
}

async fn create_user_to_completion(
    body: CreateUserRequest,
    expected_revision: Option<String>,
    shared: &ApiShared,
) -> Result<(CreateUserResponse, String), ApiFailure> {
    let touches_user_max_unique_ips = body.max_unique_ips.is_some();
    let touches_user_enabled = matches!(body.enabled, Some(false));

    if !is_valid_username(&body.username) {
        return Err(ApiFailure::bad_request(
            "username must match [A-Za-z0-9_.-] and be 1..64 chars",
        ));
    }

    let secret = match body.secret {
        Some(secret) => {
            if !is_valid_user_secret(&secret) {
                return Err(ApiFailure::bad_request(
                    "secret must be exactly 32 hex characters",
                ));
            }
            secret
        }
        None => random_user_secret(),
    };

    let credential_id = credential_id_from_hex(&secret)
        .ok_or_else(|| ApiFailure::internal("validated user secret could not be decoded"))?;
    let _guard = shared.mutation_lock.lock().await;
    let (mut cfg, base_revision) =
        load_config_for_mutation(&shared.config_path, expected_revision.as_deref()).await?;

    if cfg.access.users.contains_key(&body.username) {
        return Err(ApiFailure::new(
            StatusCode::CONFLICT,
            "user_exists",
            "User already exists",
        ));
    }

    cfg.access
        .users
        .insert(body.username.clone(), secret.clone());

    let updated_limit = body.max_unique_ips;
    if let Some(limit) = updated_limit {
        cfg.access
            .user_max_unique_ips
            .insert(body.username.clone(), limit);
    }
    if matches!(body.enabled, Some(false)) {
        cfg.access.user_enabled.insert(body.username.clone(), false);
    }

    cfg.validate()
        .map_err(|e| ApiFailure::bad_request(format!("config validation failed: {}", e)))?;

    let mut touched_sections = vec![AccessSection::Users];
    if touches_user_max_unique_ips {
        touched_sections.push(AccessSection::UserMaxUniqueIps);
    }
    if touches_user_enabled {
        touched_sections.push(AccessSection::UserEnabled);
    }

    let revision = save_access_sections_to_disk_if_revision(
        &shared.config_path,
        &cfg,
        &touched_sections,
        Some(&base_revision),
    )
    .await?;
    shared.proxy_shared.stage_user_credential(
        &body.username,
        credential_id,
        cfg.access.is_user_enabled(&body.username),
    );

    if let Some(limit) = updated_limit {
        shared
            .ip_tracker
            .set_user_limit(&body.username, limit)
            .await;
    }
    drop(_guard);

    let users = users_from_config(&cfg, &shared.stats, &shared.ip_tracker, None).await;
    let user = users
        .into_iter()
        .find(|entry| entry.username == body.username)
        .unwrap_or(UserInfo {
            username: body.username.clone(),
            enabled: cfg.access.is_user_enabled(&body.username),
            in_runtime: false,
            max_unique_ips: updated_limit,
            current_connections: 0,
            active_unique_ips: 0,
            active_unique_ips_list: Vec::new(),
            recent_unique_ips: 0,
            recent_unique_ips_list: Vec::new(),
            total_octets: 0,
            links: build_user_links(&cfg, &body.username),
        });

    Ok((CreateUserResponse { user, secret }, revision))
}
