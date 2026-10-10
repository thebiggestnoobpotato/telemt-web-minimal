use super::*;

/// Precomputed WEB configuration consumed by listener hot paths.
#[derive(Debug)]
pub(crate) struct WebRuntimeConfig {
    /// Canonical host lookup used by HTTP request routing.
    pub(crate) vhosts: BTreeMap<String, Arc<WebRuntimeVhost>>,
    /// Flat profile inventory used by startup link emission.
    pub(crate) profiles: Vec<Arc<WebRuntimeProfile>>,
    /// Complete active capability table used to contain misplaced credentials.
    pub(crate) capabilities: Box<[[u8; 32]]>,
}

/// Precomputed immutable virtual-host data.
#[derive(Debug)]
pub(crate) struct WebRuntimeVhost {
    /// Canonical lowercase ACE hostname.
    pub(crate) host: String,
    /// Exact slash-delimited endpoint base, including the trailing slash.
    pub(crate) base: String,
    /// Restart-frozen fallback capability-scan policy.
    pub(crate) fallback_fasttrack_mode: WebFallbackFastTrackMode,
    /// Immutable ordinary-site fallback snapshot.
    pub(crate) fallback: WebRuntimeFallback,
    /// Upstream connect and response-head deadline.
    pub(crate) fallback_header_secs: u64,
    /// Exact capability profiles accepted by this host.
    pub(crate) profiles: Vec<Arc<WebRuntimeProfile>>,
    /// Contiguous capability table aligned one-to-one with `profiles`.
    pub(crate) capabilities: Box<[[u8; 32]]>,
}

/// Precomputed exact-user capability entry.
#[derive(Debug)]
pub(crate) struct WebRuntimeProfile {
    /// Canonical host that owns this profile.
    pub(crate) host: String,
    /// Stable public destination tuple supplied to relay routing.
    pub(crate) public_addr: SocketAddr,
    /// Exact access user authenticated by logical streams.
    pub(crate) user: String,
    /// Stable credential identity used by process-wide admission fencing.
    pub(crate) credential_id: [u8; 16],
    /// Client secret representation and inner protocol policy.
    pub(crate) secret_mode: WebSecretMode,
    /// Sole carrier or final fallback frozen into the issued bridge policy.
    pub(crate) carrier: WebCarrier,
    /// Whether an explicit carrier list enabled automatic negotiation.
    pub(crate) carrier_negotiation_enabled: bool,
    /// Whether automatic outcomes consult and update process-local evidence.
    pub(crate) carrier_learning: bool,
    /// Ordered negotiation candidates including the fallback carrier exactly once.
    pub(crate) carriers: Arc<[WebCarrier]>,
    /// Cumulative carrier-attempt deadlines frozen when the bridge is issued.
    pub(crate) carrier_negotiation_deadlines_secs: [u64; 4],
    /// HMAC-derived bridge capability.
    pub(crate) capability: [u8; 32],
    /// Non-secret domain-separated client-secret fingerprint for debugging.
    pub(crate) key_fingerprint: String,
    /// Per-profile live session ceiling.
    pub(crate) max_sessions: usize,
    /// Per-profile live logical-stream ceiling.
    pub(crate) max_streams: usize,
    /// Per-session live relay-task ceiling.
    pub(crate) max_streams_per_session: usize,
}

/// Fallback upstream endpoint frozen into the runtime snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FallbackEndpoint {
    /// TCP origin address captured from the configured http origin.
    Tcp(SocketAddr),
    /// Unix domain socket path from the configured `unix:` origin.
    Unix(PathBuf),
}

impl fmt::Display for FallbackEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FallbackEndpoint::Tcp(addr) => write!(f, "{addr}"),
            FallbackEndpoint::Unix(path) => write!(f, "unix:{}", path.display()),
        }
    }
}

/// Runtime-ready ordinary-site fallback.
#[derive(Debug)]
pub(crate) enum WebRuntimeFallback {
    HttpUpstream {
        endpoint: FallbackEndpoint,
        authority: String,
    },
}
