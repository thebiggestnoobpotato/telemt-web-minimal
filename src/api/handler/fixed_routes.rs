use super::*;

pub(super) async fn create_user_route(
    req: Request<Incoming>,
    shared: &Arc<ApiShared>,
    cfg: &ProxyConfig,
    config: &Arc<ProxyConfig>,
    request_id: u64,
    body_limit: usize,
) -> Result<Response<Full<Bytes>>, ApiFailure> {
    let api_cfg = &cfg.api;
    if api_cfg.read_only {
        return Ok(error_response(
            request_id,
            ApiFailure::new(
                StatusCode::FORBIDDEN,
                "read_only",
                "API runs in read-only mode",
            ),
        ));
    }
    let expected_revision = parse_if_match(req.headers());
    let body = read_json::<CreateUserRequest>(req.into_body(), body_limit).await?;
    let result = create_user(body, expected_revision, shared).await;
    let (mut data, revision) = match result {
        Ok(ok) => ok,
        Err(error) => {
            shared
                .runtime_events
                .record("api.user.create.failed", error.code);
            return Err(error);
        }
    };
    let runtime_cfg = config.clone();
    data.user.in_runtime = runtime_cfg.users.contains_key(&data.user.username);
    shared.runtime_events.record(
        "api.user.create.ok",
        format!("username={}", data.user.username),
    );
    let status = if data.user.in_runtime {
        StatusCode::CREATED
    } else {
        StatusCode::ACCEPTED
    };
    Ok(success_response(status, data, revision))
}

pub(super) async fn get_config_route(
    shared: &Arc<ApiShared>,
) -> Result<Response<Full<Bytes>>, ApiFailure> {
    let (value, revision) = config_edit::read_managed_config(&shared.config_path).await?;
    Ok(success_response(StatusCode::OK, value, revision))
}

pub(super) async fn patch_config_route(
    req: Request<Incoming>,
    shared: &Arc<ApiShared>,
    cfg: &ProxyConfig,
    request_id: u64,
    body_limit: usize,
) -> Result<Response<Full<Bytes>>, ApiFailure> {
    let api_cfg = &cfg.api;
    if api_cfg.read_only {
        return Ok(error_response(
            request_id,
            ApiFailure::new(
                StatusCode::FORBIDDEN,
                "read_only",
                "API runs in read-only mode",
            ),
        ));
    }
    let expected_revision = parse_if_match(req.headers());
    let body = read_json::<serde_json::Value>(req.into_body(), body_limit).await?;
    match config_edit::patch_config(body, expected_revision, shared).await {
        Ok(resp) => {
            let revision = resp.revision.clone();
            Ok(success_response(StatusCode::OK, resp, revision))
        }
        Err(error) => {
            shared
                .runtime_events
                .record("api.config.patch.failed", error.code);
            Err(error)
        }
    }
}
