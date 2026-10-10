use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf, split};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::config::ProxyConfig;
use crate::crypto::SecureRandom;
use crate::error::{ProxyError, Result};
use crate::protocol::constants::*;
use crate::proxy::handshake::{HandshakeSuccess, encrypt_tg_nonce_with_ciphers, generate_tg_nonce};
use crate::proxy::shared_state::ProxySharedState;
use crate::stats::Stats;
use crate::stream::{BufferPool, CryptoReader, CryptoWriter};
use crate::transport::UpstreamManager;

// Direct relay lifecycle.
mod relay;
// Telegram DC resolution and upstream handshake.
mod routing;

pub(crate) use relay::handle_via_direct_with_shared;
#[cfg(test)]
pub(crate) use relay::handle_via_direct;
use routing::*;
const UNKNOWN_DC_LOG_DISTINCT_LIMIT: usize = 1024;
static LOGGED_UNKNOWN_DCS: OnceLock<Mutex<HashSet<i16>>> = OnceLock::new();

// In tests, this function shares global mutable state. Callers that also use
// cache-reset helpers must hold `unknown_dc_test_lock()` to keep assertions
// deterministic under parallel execution.
fn should_log_unknown_dc(dc_idx: i16) -> bool {
    let set = LOGGED_UNKNOWN_DCS.get_or_init(|| Mutex::new(HashSet::new()));
    should_log_unknown_dc_with_set(set, dc_idx)
}

fn should_log_unknown_dc_with_set(set: &Mutex<HashSet<i16>>, dc_idx: i16) -> bool {
    match set.lock() {
        Ok(mut guard) => {
            if guard.contains(&dc_idx) {
                return false;
            }
            if guard.len() >= UNKNOWN_DC_LOG_DISTINCT_LIMIT {
                return false;
            }
            guard.insert(dc_idx)
        }
        // Fail closed on poisoned state to avoid unbounded blocking log writes.
        Err(_) => false,
    }
}

#[cfg(test)]
fn clear_unknown_dc_log_cache_for_testing() {
    if let Some(set) = LOGGED_UNKNOWN_DCS.get()
        && let Ok(mut guard) = set.lock()
    {
        guard.clear();
    }
}

#[cfg(test)]
fn unknown_dc_test_lock() -> &'static tokio::sync::Mutex<()> {
    static TEST_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(test)]
#[path = "tests/direct_relay_security_tests.rs"]
mod security_tests;

#[cfg(test)]
#[path = "tests/direct_relay_business_logic_tests.rs"]
mod business_logic_tests;

#[cfg(test)]
#[path = "tests/direct_relay_common_mistakes_tests.rs"]
mod common_mistakes_tests;

#[cfg(test)]
#[path = "tests/direct_relay_subtle_adversarial_tests.rs"]
mod subtle_adversarial_tests;
