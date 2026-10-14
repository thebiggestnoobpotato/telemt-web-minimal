use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;

use crate::config::ProxyConfig;
use crate::crypto::SecureRandom;
use crate::error::{ProxyError, Result};
use crate::proxy::direct_relay::handle_via_direct_with_shared;
use crate::proxy::handshake::HandshakeSuccess;
use crate::proxy::shared_state::ProxySharedState;
use crate::stats::{Stats, UserConnectionObservation};
use crate::stream::{BufferPool, CryptoReader, CryptoWriter};
use crate::transport::UpstreamManager;

/// Immutable dependency snapshot pinned by one authenticated client stream.
#[derive(Clone)]
pub(crate) struct ClientRuntimeDeps {
    /// Immutable effective configuration pinned for this stream.
    pub(crate) config: Arc<ProxyConfig>,
    /// Process statistics registry.
    pub(crate) stats: Arc<Stats>,
    /// Direct Telegram upstream connector.
    pub(crate) upstream_manager: Arc<UpstreamManager>,
    /// Shared relay buffer pool.
    pub(crate) buffer_pool: Arc<BufferPool>,
    /// Process cryptographic random source.
    pub(crate) rng: Arc<SecureRandom>,
    /// Process-shared admission and relay coordination state.
    pub(crate) shared: Arc<ProxySharedState>,
}

/// Runs admission and relay after a successful MTProxy handshake.
pub(crate) async fn run_authenticated<R, W>(
    client_reader: CryptoReader<R>,
    client_writer: CryptoWriter<W>,
    success: HandshakeSuccess,
    deps: ClientRuntimeDeps,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let user = success.user.clone();
    let Some(credential_id) = deps.config.runtime_user_credential_id(&user) else {
        warn!(user = %user, "Authenticated user is absent from the runtime credential snapshot");
        return Err(ProxyError::UserDisabled { user });
    };
    let Some(user_incarnation) = deps
        .shared
        .authenticated_user_incarnation(&user, credential_id)
    else {
        warn!(user = %user, "Disabled user rejected");
        return Err(ProxyError::UserDisabled { user });
    };

    let user_reservation = UserConnectionReservation::new(&user, Arc::clone(&deps.stats));
    let Some(user_session) = deps
        .shared
        .register_authenticated_user_session(&user, credential_id)
    else {
        user_reservation.release_deferred();
        warn!(user = %user, "Disabled user rejected during final admission");
        return Err(ProxyError::UserDisabled { user });
    };
    if user_session.incarnation() != user_incarnation {
        drop(user_session);
        user_reservation.release_deferred();
        warn!(user = %user, "User incarnation changed during admission");
        return Err(ProxyError::UserDisabled { user });
    }
    let session_cancel = user_session.token();

    let relay_result = run_direct(
        client_reader,
        client_writer,
        success,
        &deps,
        session_cancel,
    )
    .await;
    user_reservation.release();
    relay_result
}

async fn run_direct<R, W>(
    client_reader: CryptoReader<R>,
    client_writer: CryptoWriter<W>,
    success: HandshakeSuccess,
    deps: &ClientRuntimeDeps,
    session_cancel: tokio_util::sync::CancellationToken,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    handle_via_direct_with_shared(
        client_reader,
        client_writer,
        success,
        Arc::clone(&deps.upstream_manager),
        Arc::clone(&deps.stats),
        Arc::clone(&deps.config),
        Arc::clone(&deps.buffer_pool),
        Arc::clone(&deps.rng),
        session_cancel,
        Arc::clone(&deps.shared),
    )
    .await
}

#[must_use = "the reservation owns the connection observation until release or drop"]
/// Owns one authenticated user's active-connection observation slot.
pub(crate) struct UserConnectionReservation {
    stats: Arc<Stats>,
    _stats_observation: Option<UserConnectionObservation>,
    released: bool,
}

impl UserConnectionReservation {
    /// Creates a reservation observing one authenticated user's connection.
    pub(crate) fn new(user: &str, stats: Arc<Stats>) -> Self {
        let stats_observation = stats.observe_user_current_connection(user);
        Self {
            stats,
            _stats_observation: stats_observation,
            released: false,
        }
    }

    /// Releases the connection observation.
    pub(crate) fn release(mut self) {
        self.released = true;
    }

    /// Marks the observation released when admission fails after the reservation step.
    pub(crate) fn release_deferred(mut self) {
        self.released = true;
    }
}

impl Drop for UserConnectionReservation {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.stats.increment_session_drop_fallback_total();
    }
}
