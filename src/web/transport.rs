//! Transport streams shared by process-owned listener ingress and fallback
//! upstream egress.
//!
//! The WEB HTTP front is transport-agnostic: one enum covers the TCP streams
//! accepted from external terminators and the unix socket streams accepted
//! from local fronting processes or connected to unix fallback upstreams.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpStream, UnixStream};

use crate::config::FallbackEndpoint;
use crate::web::telemetry::WebFallbackUpstreamOutcome;

/// Stream accepted from a process-owned listener or connected to a fallback
/// upstream.
pub(crate) enum WebListenerStream {
    /// TCP stream behind an external TLS terminator.
    Tcp(TcpStream),
    /// Unix socket stream from a local fronting process.
    Unix(UnixStream),
}

impl WebListenerStream {
    /// Waits for readability, mirroring the inherent `TcpStream`/`UnixStream`
    /// method used by the websocket read boundary.
    pub(crate) async fn readable(&self) -> io::Result<()> {
        match self {
            WebListenerStream::Tcp(stream) => stream.readable().await,
            WebListenerStream::Unix(stream) => stream.readable().await,
        }
    }
}

impl AsyncRead for WebListenerStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WebListenerStream::Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
            WebListenerStream::Unix(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for WebListenerStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            WebListenerStream::Tcp(stream) => Pin::new(stream).poll_write(cx, buf),
            WebListenerStream::Unix(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WebListenerStream::Tcp(stream) => Pin::new(stream).poll_flush(cx),
            WebListenerStream::Unix(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WebListenerStream::Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
            WebListenerStream::Unix(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

/// Connects to one frozen fallback endpoint within the header deadline.
///
/// A missing or unlistened unix socket maps to the same connect-refused
/// outcome class as a refused TCP connection.
pub(crate) async fn connect_fallback(
    endpoint: &FallbackEndpoint,
    timeout: Duration,
) -> Result<WebListenerStream, WebFallbackUpstreamOutcome> {
    match endpoint {
        FallbackEndpoint::Tcp(addr) => match tokio::time::timeout(timeout, TcpStream::connect(*addr))
            .await
        {
            Ok(Ok(stream)) => Ok(WebListenerStream::Tcp(stream)),
            Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => {
                Err(WebFallbackUpstreamOutcome::ConnectRefused)
            }
            Ok(Err(_)) => Err(WebFallbackUpstreamOutcome::ConnectError),
            Err(_) => Err(WebFallbackUpstreamOutcome::ConnectTimeout),
        },
        FallbackEndpoint::Unix(path) => {
            match tokio::time::timeout(timeout, UnixStream::connect(path)).await {
                Ok(Ok(stream)) => Ok(WebListenerStream::Unix(stream)),
                Ok(Err(error))
                    if error.kind() == io::ErrorKind::NotFound
                        || error.kind() == io::ErrorKind::ConnectionRefused =>
                {
                    Err(WebFallbackUpstreamOutcome::ConnectRefused)
                }
                Ok(Err(_)) => Err(WebFallbackUpstreamOutcome::ConnectError),
                Err(_) => Err(WebFallbackUpstreamOutcome::ConnectTimeout),
            }
        }
    }
}
