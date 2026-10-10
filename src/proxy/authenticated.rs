use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;

use crate::config::ProxyConfig;
use crate::crypto::SecureRandom;
use crate::error::{ProxyError, Result};
use crate::ip_tracker::UserIpTracker;
use crate::proxy::direct_relay::handle_via_direct_with_shared;
use crate::proxy::handshake::HandshakeSuccess;
use crate::proxy::shared_state::ProxySharedState;
use crate::proxy::user_admission::UserIncarnation;
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
    /// Per-user source-IP admission tracker.
    pub(crate) ip_tracker: Arc<UserIpTracker>,
    /// Process-shared admission and relay coordination state.
    pub(crate) shared: Arc<ProxySharedState>,
}

/// Runs admission and relay after a successful MTProxy handshake.
pub(crate) async fn run_authenticated<R, W>(
    client_reader: CryptoReader<R>,
    client_writer: CryptoWriter<W>,
    success: HandshakeSuccess,
    deps: ClientRuntimeDeps,
    peer_addr: SocketAddr,
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

    let user_reservation = acquire_user_connection_reservation_for_incarnation(
        &user,
        user_incarnation,
        Arc::clone(&deps.stats),
        peer_addr,
        Arc::clone(&deps.ip_tracker),
    )
    .await
    .map_err(|error| {
        warn!(user = %user, error = %error, "User admission check failed");
        error
    })?;
    let session_id = deps.rng.u64();
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
        session_id,
        session_cancel,
    )
    .await;
    user_reservation.release().await;
    relay_result
}

async fn run_direct<R, W>(
    client_reader: CryptoReader<R>,
    client_writer: CryptoWriter<W>,
    success: HandshakeSuccess,
    deps: &ClientRuntimeDeps,
    session_id: u64,
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
        session_id,
        session_cancel,
        Arc::clone(&deps.shared),
    )
    .await
}

#[must_use = "the reservation owns source-IP admission until release or drop"]
/// Owns one authenticated user's source-IP admission slot.
pub(crate) struct UserConnectionReservation {
    stats: Arc<Stats>,
    _stats_observation: Option<UserConnectionObservation>,
    ip_permit: Option<UserIpPermit>,
    released: bool,
}

struct UserIpPermit {
    tracker: Arc<UserIpTracker>,
    owner: Option<UserIpOwner>,
}

struct UserIpOwner {
    user: String,
    incarnation: UserIncarnation,
    ip: IpAddr,
}

impl UserIpPermit {
    fn new(
        tracker: Arc<UserIpTracker>,
        user: String,
        incarnation: UserIncarnation,
        ip: IpAddr,
    ) -> Self {
        Self {
            tracker,
            owner: Some(UserIpOwner {
                user,
                incarnation,
                ip,
            }),
        }
    }

    async fn release(mut self) {
        let Some(owner) = self.owner.as_ref() else {
            return;
        };
        self.tracker
            .remove_ip_for_incarnation(&owner.user, owner.incarnation, owner.ip)
            .await;
        self.owner = None;
    }
}

impl Drop for UserIpPermit {
    fn drop(&mut self) {
        let Some(owner) = self.owner.take() else {
            return;
        };
        self.tracker
            .enqueue_cleanup_for_incarnation(owner.user, owner.incarnation, owner.ip);
    }
}

impl UserConnectionReservation {
    /// Creates a reservation fenced to one authenticated user incarnation.
    pub(crate) fn new_for_incarnation(
        stats: Arc<Stats>,
        ip_tracker: Arc<UserIpTracker>,
        user: String,
        ip: IpAddr,
        incarnation: UserIncarnation,
        stats_observation: Option<UserConnectionObservation>,
        tracks_ip: bool,
    ) -> Self {
        let ip_permit = tracks_ip.then(|| UserIpPermit::new(ip_tracker, user, incarnation, ip));
        Self {
            stats,
            _stats_observation: stats_observation,
            ip_permit,
            released: false,
        }
    }

    /// Releases the source-IP admission slot through the asynchronous cleanup path.
    pub(crate) async fn release(mut self) {
        if let Some(ip_permit) = self.ip_permit.take() {
            ip_permit.release().await;
        }
        self.released = true;
    }

    /// Defers IP cleanup when admission fails after the asynchronous reservation step.
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

/// Applies user source-IP admission atomically.
pub(crate) async fn acquire_user_connection_reservation(
    user: &str,
    stats: Arc<Stats>,
    peer_addr: SocketAddr,
    ip_tracker: Arc<UserIpTracker>,
) -> Result<UserConnectionReservation> {
    acquire_user_connection_reservation_for_incarnation(user, 0, stats, peer_addr, ip_tracker)
        .await
}

async fn acquire_user_connection_reservation_for_incarnation(
    user: &str,
    incarnation: UserIncarnation,
    stats: Arc<Stats>,
    peer_addr: SocketAddr,
    ip_tracker: Arc<UserIpTracker>,
) -> Result<UserConnectionReservation> {
    let stats_observation = stats.observe_user_current_connection(user);

    if let Err(reason) = ip_tracker
        .check_and_add_for_incarnation(user, incarnation, peer_addr.ip())
        .await
    {
        warn!(
            user = %user,
            ip = %peer_addr.ip(),
            reason = %reason,
            "IP limit exceeded"
        );
        return Err(ProxyError::ConnectionLimitExceeded {
            user: user.to_string(),
        });
    }

    Ok(UserConnectionReservation::new_for_incarnation(
        stats,
        ip_tracker,
        user.to_string(),
        peer_addr.ip(),
        incarnation,
        stats_observation,
        true,
    ))
}
