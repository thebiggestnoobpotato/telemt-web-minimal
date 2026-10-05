
use super::*;
use crate::config::{UpstreamConfig, UpstreamType};
use crate::crypto::{AesCtr, SecureRandom};
use crate::protocol::constants::ProtoTag;
use crate::stats::Stats;
use crate::stream::{BufferPool, CryptoReader, CryptoWriter};
use crate::transport::UpstreamManager;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::duplex;
use tokio::net::TcpListener;
use tokio::time::{Duration as TokioDuration, timeout};

fn make_crypto_reader<R>(reader: R) -> CryptoReader<R>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let key = [0u8; 32];
    let iv = 0u128;
    CryptoReader::new(reader, AesCtr::new(&key, iv))
}

fn make_crypto_writer<W>(writer: W) -> CryptoWriter<W>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let key = [0u8; 32];
    let iv = 0u128;
    CryptoWriter::new(writer, AesCtr::new(&key, iv), 8 * 1024)
}

// Unknown-DC deduplication and logging switch.
#[path = "direct_relay_security_tests/unknown_paths.rs"]
mod unknown_paths;
// Direct relay cancellation and cutover lifecycle.
#[path = "direct_relay_security_tests/relay_lifecycle.rs"]
mod relay_lifecycle;
// DC override routing and negative connection paths.
#[path = "direct_relay_security_tests/routing.rs"]
mod routing;
