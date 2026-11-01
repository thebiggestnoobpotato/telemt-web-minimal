use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::task::{Wake, Waker};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::sync::CancellationToken;

use super::*;
use crate::crypto::aes::AesCtr;
use crate::stream::CryptoWriter;

// Copy-loop error, backpressure, and wakeup invariants share these transport fixtures.
mod flow;

struct BurstReader {
    bytes: Vec<u8>,
    pos: usize,
    eof: bool,
}

impl AsyncRead for BurstReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.pos == this.bytes.len() && !this.eof {
            return Poll::Pending;
        }
        let n = output.remaining().min(this.bytes.len() - this.pos);
        output.put_slice(&this.bytes[this.pos..this.pos + n]);
        this.pos += n;
        Poll::Ready(Ok(()))
    }
}

struct SinkState {
    bytes: Vec<u8>,
    allowance: usize,
    max_write: usize,
    waker: Option<Waker>,
    flushes: usize,
}

impl SinkState {
    fn new(allowance: usize, max_write: usize) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            bytes: Vec::new(),
            allowance,
            max_write,
            waker: None,
            flushes: 0,
        }))
    }
}

struct Sink(Arc<Mutex<SinkState>>);

impl AsyncWrite for Sink {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.0.lock().unwrap();
        if state.allowance == 0 {
            state.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let n = bytes.len().min(state.allowance).min(state.max_write);
        state.bytes.extend_from_slice(&bytes[..n]);
        state.allowance -= n;
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.lock().unwrap().flushes += 1;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// Reverses the fixed test-key AES-CTR stream applied by the relay writer.
fn decrypt(sink_bytes: &[u8]) -> Vec<u8> {
    let mut plaintext = sink_bytes.to_vec();
    AesCtr::new(&[7; 32], 9).apply(&mut plaintext);
    plaintext
}

#[tokio::test(start_paused = true)]
async fn adaptive_relay_flushes_writer_tail_without_another_source_event() {
    let payload: Vec<_> = (0..65536).map(|i| (i % 251) as u8).collect();
    let sink = SinkState::new(usize::MAX, usize::MAX);
    let writer = CryptoWriter::new(Sink(Arc::clone(&sink)), AesCtr::new(&[7; 32], 9), 65536);
    let stats = Arc::new(Stats::new());
    let user = "adaptive-tail";
    let cancel = CancellationToken::new();
    let mut relay = Box::pin(relay_direct_adaptive(
        tokio::io::empty(),
        writer,
        BurstReader {
            bytes: payload.clone(),
            pos: 0,
            eof: false,
        },
        tokio::io::sink(),
        4096,
        65536,
        user,
        Arc::clone(&stats),
        Duration::from_secs(60),
        cancel.clone(),
        DirectBufferBudget::new(1024 * 1024),
    ));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(relay.as_mut().poll(&mut cx).is_pending());
    for _ in 0..3 {
        assert!(relay.as_mut().poll(&mut cx).is_pending());
    }
    assert_eq!(stats.get_user_total_octets(user), payload.len() as u64);
    assert_eq!(
        decrypt(&sink.lock().unwrap().bytes),
        payload,
        "accepted bytes must not await another read"
    );
    cancel.cancel();
    assert!(matches!(
        relay.as_mut().poll(&mut cx),
        Poll::Ready(Err(ProxyError::UserDisabled { .. }))
    ));
}
