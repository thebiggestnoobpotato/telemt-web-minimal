//! Bounded WEB carrier ingress behind a trusted external TLS terminator.

/// Browser bridge generation for the serialized HTTPS carrier.
pub(crate) mod bridge;
/// Process lifecycle publication shared with the control plane.
pub(crate) mod control;
/// Shared binary frame codec and protocol constants.
pub(crate) mod frame;
/// Plain HTTP ingress and decoy routing behind external TLS termination.
pub(crate) mod http;
/// Operator-facing `tg://webproxy` link formatting.
pub(crate) mod links;
/// Process-wide credentials, quotas, memory budgets, and shutdown ownership.
pub(crate) mod manager;
/// Resumable carrier sessions and logical-stream state machines.
pub(crate) mod session;
/// AsyncRead and AsyncWrite adapter for one logical MTProxy stream.
pub(crate) mod stream;
/// Process-owned fixed-cardinality WEB operational telemetry.
pub(crate) mod telemetry;
/// Transport streams shared by listener ingress and decoy upstream egress.
pub(crate) mod transport;
/// Process-owned bounded WEB debugging records and capture lifecycle.
pub(crate) mod trace;
