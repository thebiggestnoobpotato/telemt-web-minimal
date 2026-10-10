use crate::stats::{Stats, UserStats};
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Instant;
use tracing::trace;

mod combined;
mod counters;

pub(super) use self::combined::CombinedStream;
pub(super) use self::counters::SharedCounters;

/// Transparent I/O wrapper that tracks per-user statistics and activity.
///
/// Wraps the **client** side of the relay. Direction mapping:
///
/// | poll method  | direction | stats updated                        |
/// |-------------|-----------|--------------------------------------|
/// | `poll_read`  | C→S       | `octets_from`, `msgs_from`, counters |
/// | `poll_write` | S→C       | `octets_to`, `msgs_to`, counters     |
///
/// Both update the shared activity timestamp for the watchdog.
///
/// Note on message counts: the original code counted one `read()`/`write_all()`
/// as one "message". Here we count `poll_read`/`poll_write` completions instead.
/// Byte counts are identical; op counts may differ slightly due to different
/// internal buffering in `copy_bidirectional`. This is fine for monitoring.
pub(super) struct StatsIo<S> {
    inner: S,
    counters: Arc<SharedCounters>,
    stats: Arc<Stats>,
    user: String,
    user_stats: Arc<UserStats>,
    epoch: Instant,
}

impl<S> StatsIo<S> {
    pub(super) fn new(
        inner: S,
        counters: Arc<SharedCounters>,
        stats: Arc<Stats>,
        user: String,
        epoch: Instant,
    ) -> Self {
        // Mark initial activity so the watchdog doesn't fire before data flows
        counters.touch(Instant::now(), epoch);
        let user_stats = stats.get_or_create_user_stats_handle(&user);
        Self {
            inner,
            counters,
            stats,
            user,
            user_stats,
            epoch,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for StatsIo<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let n = buf.filled().len() - before;
                if n > 0 {
                    let n_to_charge = n as u64;

                    // C→S: client sent data
                    this.counters
                        .c2s_bytes
                        .fetch_add(n_to_charge, Ordering::Relaxed);
                    this.counters.c2s_ops.fetch_add(1, Ordering::Relaxed);
                    this.counters.touch(Instant::now(), this.epoch);

                    this.stats
                        .add_user_traffic_from_handle(this.user_stats.as_ref(), n_to_charge);

                    trace!(user = %this.user, bytes = n, "C->S");
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for StatsIo<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                if n > 0 {
                    let n_to_charge = n as u64;

                    // S→C: data written to client
                    this.counters
                        .s2c_bytes
                        .fetch_add(n_to_charge, Ordering::Relaxed);
                    this.counters.s2c_ops.fetch_add(1, Ordering::Relaxed);
                    this.counters.touch(Instant::now(), this.epoch);

                    this.stats
                        .add_user_traffic_to_handle(this.user_stats.as_ref(), n_to_charge);

                    trace!(user = %this.user, bytes = n, "S->C");
                }
                Poll::Ready(Ok(n))
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
