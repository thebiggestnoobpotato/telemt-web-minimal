use super::*;

#[allow(dead_code)]
/// Runs Direct relay with standalone cancellation and shared-state defaults.
pub(crate) async fn handle_via_direct<R, W>(
    client_reader: CryptoReader<R>,
    client_writer: CryptoWriter<W>,
    success: HandshakeSuccess,
    upstream_manager: Arc<UpstreamManager>,
    stats: Arc<Stats>,
    config: Arc<ProxyConfig>,
    buffer_pool: Arc<BufferPool>,
    rng: Arc<SecureRandom>,
    session_id: u64,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let quota_handle = stats.current_user_quota_handle(&success.user);
    handle_via_direct_with_shared(
        client_reader,
        client_writer,
        success,
        upstream_manager,
        stats,
        config.clone(),
        buffer_pool,
        rng,
        session_id,
        CancellationToken::new(),
        ProxySharedState::new(),
        quota_handle,
    )
    .await
}

/// Runs Direct relay with explicit shared runtime state and session cancellation.
pub(crate) async fn handle_via_direct_with_shared<R, W>(
    client_reader: CryptoReader<R>,
    client_writer: CryptoWriter<W>,
    success: HandshakeSuccess,
    upstream_manager: Arc<UpstreamManager>,
    stats: Arc<Stats>,
    config: Arc<ProxyConfig>,
    buffer_pool: Arc<BufferPool>,
    rng: Arc<SecureRandom>,
    session_id: u64,
    session_cancel: CancellationToken,
    shared: Arc<ProxySharedState>,
    quota_handle: UserQuotaHandle,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let user = &success.user;
    let dc_addr = get_dc_addr_static(success.dc_idx, &config)?;

    debug!(
        user = %user,
        peer = %success.peer,
        dc = success.dc_idx,
        dc_addr = %dc_addr,
        proto = ?success.proto_tag,
        mode = "direct",
        "Connecting to Telegram DC"
    );

    let scope_hint = validated_scope_hint(user);
    if user.starts_with("scope_") && scope_hint.is_none() {
        warn!(
            user = %user,
            "Ignoring invalid scope hint and falling back to default upstream selection"
        );
    }
    let tg_stream = tokio::select! {
        result = upstream_manager.connect(dc_addr, Some(success.dc_idx), scope_hint) => result?,
        _ = session_cancel.cancelled() => {
            return Err(ProxyError::UserDisabled {
                user: user.to_string(),
            });
        }
    };

    debug!(peer = %success.peer, dc_addr = %dc_addr, "Connected, performing TG handshake");

    let (tg_reader, tg_writer) = tokio::select! {
        result = do_tg_handshake_static(tg_stream, &success, &config, rng.as_ref()) => result?,
        _ = session_cancel.cancelled() => {
            return Err(ProxyError::UserDisabled {
                user: user.to_string(),
            });
        }
    };

    debug!(peer = %success.peer, "TG handshake complete, starting relay");

    stats.increment_user_connects(user);
    let _direct_connection_lease = stats.acquire_direct_connection_lease();
    let traffic_lease = shared
        .traffic_limiter
        .acquire_lease(user, success.peer.ip());

    let buffer_pool_trim = Arc::clone(&buffer_pool);
    let relay_activity_timeout = Duration::from_secs(1800);
    let relay_result = crate::proxy::relay::relay_direct_adaptive(
        client_reader,
        client_writer,
        tg_reader,
        tg_writer,
        config.general.direct_relay_copy_buf_c2s_bytes,
        config.general.direct_relay_copy_buf_s2c_bytes,
        config.general.max_connections,
        user,
        Arc::clone(&stats),
        quota_handle,
        config.access.user_data_quota.get(user).copied(),
        traffic_lease,
        relay_activity_timeout,
        session_cancel.clone(),
        Arc::clone(&shared.direct_buffer_budget),
    );
    tokio::pin!(relay_result);
    let relay_result = tokio::select! {
        result = &mut relay_result => result,
        _ = session_cancel.cancelled() => {
            Err(ProxyError::UserDisabled {
                user: user.to_string(),
            })
        }
    };

    match &relay_result {
        Ok(()) => debug!(user = %user, "Direct relay completed"),
        Err(e) => debug!(user = %user, error = %e, "Direct relay ended with error"),
    }

    let pool_snapshot = buffer_pool_trim.stats();
    stats.set_buffer_pool_gauges(
        pool_snapshot.pooled,
        pool_snapshot.allocated,
        pool_snapshot.allocated.saturating_sub(pool_snapshot.pooled),
    );

    relay_result
}
