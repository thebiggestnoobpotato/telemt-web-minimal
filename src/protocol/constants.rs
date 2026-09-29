//! Protocol constants and datacenter addresses

#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr};

use crate::crypto::SecureRandom;
use crate::protocol::framing::{
    secure_version_d_body_len_from_wire_len, secure_version_d_padding_len,
};
use std::sync::LazyLock;

// ============= Telegram Datacenters =============

pub const TG_DATACENTER_PORT: u16 = 443;

pub static TG_DATACENTERS_V4: LazyLock<Vec<IpAddr>> = LazyLock::new(|| {
    vec![
        IpAddr::V4(Ipv4Addr::new(149, 154, 175, 50)),
        IpAddr::V4(Ipv4Addr::new(149, 154, 167, 51)),
        IpAddr::V4(Ipv4Addr::new(149, 154, 175, 100)),
        IpAddr::V4(Ipv4Addr::new(149, 154, 167, 91)),
        IpAddr::V4(Ipv4Addr::new(149, 154, 171, 5)),
    ]
});

pub static TG_DATACENTERS_V6: LazyLock<Vec<IpAddr>> = LazyLock::new(|| {
    vec![
        IpAddr::V6("2001:b28:f23d:f001::a".parse().unwrap()),
        IpAddr::V6("2001:67c:04e8:f002::a".parse().unwrap()),
        IpAddr::V6("2001:b28:f23d:f003::a".parse().unwrap()),
        IpAddr::V6("2001:67c:04e8:f004::a".parse().unwrap()),
        IpAddr::V6("2001:b28:f23f:f005::a".parse().unwrap()),
    ]
});

// ============= Protocol Tags =============

/// MTProto transport protocol variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ProtoTag {
    /// Abridged protocol - compact framing
    Abridged = 0xefefefef,
    /// Intermediate protocol - simple 4-byte length prefix
    Intermediate = 0xeeeeeeee,
    /// Secure intermediate - with random padding
    Secure = 0xdddddddd,
}

impl ProtoTag {
    /// Parse protocol tag from 4 bytes
    pub fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        match u32::from_le_bytes(bytes) {
            0xefefefef => Some(ProtoTag::Abridged),
            0xeeeeeeee => Some(ProtoTag::Intermediate),
            0xdddddddd => Some(ProtoTag::Secure),
            _ => None,
        }
    }

    /// Convert to 4 bytes (little-endian)
    pub fn to_bytes(self) -> [u8; 4] {
        (self as u32).to_le_bytes()
    }

    /// Get protocol tag as bytes slice
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            ProtoTag::Abridged => &PROTO_TAG_ABRIDGED,
            ProtoTag::Intermediate => &PROTO_TAG_INTERMEDIATE,
            ProtoTag::Secure => &PROTO_TAG_SECURE,
        }
    }
}

/// Protocol tag bytes
pub const PROTO_TAG_ABRIDGED: [u8; 4] = [0xef, 0xef, 0xef, 0xef];
pub const PROTO_TAG_INTERMEDIATE: [u8; 4] = [0xee, 0xee, 0xee, 0xee];
pub const PROTO_TAG_SECURE: [u8; 4] = [0xdd, 0xdd, 0xdd, 0xdd];

// ============= Handshake Layout =============

/// Bytes to skip at the start of handshake
pub const SKIP_LEN: usize = 8;
/// Pre-key length (before hashing with secret)
pub const PREKEY_LEN: usize = 32;
/// AES key length
pub const KEY_LEN: usize = 32;
/// AES IV length  
pub const IV_LEN: usize = 16;
/// Total handshake length
pub const HANDSHAKE_LEN: usize = 64;
/// Position of protocol tag in decrypted handshake
pub const PROTO_TAG_POS: usize = 56;
/// Position of datacenter index
pub const DC_IDX_POS: usize = 60;

// ============= Message Limits =============

/// Minimum message length
pub const MIN_MSG_LEN: usize = 12;
/// Maximum message length (16 MB)
pub const MAX_MSG_LEN: usize = 1 << 24;
/// CBC block padding size
pub const CBC_PADDING: usize = 16;
/// Padding filler bytes
pub const PADDING_FILLER: [u8; 4] = [0x04, 0x00, 0x00, 0x00];

// ============= TLS Constants =============

/// Minimum certificate length for detection
pub const MIN_CERT_LEN: usize = 1024;
/// TLS 1.3 version bytes
pub const TLS_VERSION: [u8; 2] = [0x03, 0x03];
/// TLS record type: Handshake
pub const TLS_RECORD_HANDSHAKE: u8 = 0x16;
/// TLS record type: Change Cipher Spec
pub const TLS_RECORD_CHANGE_CIPHER: u8 = 0x14;
/// TLS record type: Application Data
pub const TLS_RECORD_APPLICATION: u8 = 0x17;
/// TLS record type: Alert
pub const TLS_RECORD_ALERT: u8 = 0x15;
/// Maximum TLS plaintext record payload size.
/// RFC 8446 §5.1: "The length MUST NOT exceed 2^14 bytes."
/// Use this for validating incoming unencrypted records
/// (ClientHello, ChangeCipherSpec, unprotected Handshake messages).
pub const MAX_TLS_PLAINTEXT_SIZE: usize = 16_384;

/// Structural minimum for a valid TLS 1.3 ClientHello with SNI.
/// Derived from RFC 8446 §4.1.2 field layout + Appendix D.4 compat mode.
/// Deliberately conservative (below any real client) to avoid false
/// positives on legitimate connections with compact extension sets.
pub const MIN_TLS_CLIENT_HELLO_SIZE: usize = 100;

/// Maximum TLS ciphertext record payload size.
/// RFC 8446 §5.2: "The length MUST NOT exceed 2^14 + 256 bytes."
/// The +256 accounts for maximum AEAD expansion overhead.
/// Use this for validating or sizing buffers for encrypted records.
pub const MAX_TLS_CIPHERTEXT_SIZE: usize = 16_384 + 256;

#[deprecated(note = "use MAX_TLS_PLAINTEXT_SIZE")]
pub const MAX_TLS_RECORD_SIZE: usize = MAX_TLS_PLAINTEXT_SIZE;

#[deprecated(note = "use MAX_TLS_CIPHERTEXT_SIZE")]
pub const MAX_TLS_CHUNK_SIZE: usize = MAX_TLS_CIPHERTEXT_SIZE;

/// Secure Intermediate payload is expected to be 4-byte aligned.
pub fn is_valid_secure_payload_len(data_len: usize) -> bool {
    data_len.is_multiple_of(4)
}

/// Compute Secure Intermediate payload length from wire length.
/// Secure mode cannot distinguish full-word padding from payload, so only the
/// non-aligned tail bytes are stripped.
pub fn secure_payload_len_from_wire_len(wire_len: usize) -> Option<usize> {
    secure_version_d_body_len_from_wire_len(wire_len)
}

/// Generate padding length for Secure Intermediate protocol.
/// Outbound padding is 1..=3 so a receiver can strip it by 4-byte alignment.
pub fn secure_padding_len(data_len: usize, rng: &SecureRandom) -> usize {
    debug_assert!(
        is_valid_secure_payload_len(data_len),
        "Secure payload must be 4-byte aligned, got {data_len}"
    );
    secure_version_d_padding_len(rng)
}

// ============= Timeouts =============

/// Default handshake timeout in seconds
pub const DEFAULT_HANDSHAKE_TIMEOUT_SECS: u64 = 10;
/// Default connect timeout in seconds
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
/// Default keepalive interval in seconds
pub const DEFAULT_KEEPALIVE_SECS: u64 = 600;
/// Default ACK timeout in seconds
pub const DEFAULT_ACK_TIMEOUT_SECS: u64 = 300;

// ============= Buffer Sizes =============

/// Default buffer size
pub const DEFAULT_BUFFER_SIZE: usize = 16384;

/// Small buffer size for bad client handling
pub const SMALL_BUFFER_SIZE: usize = 8192;

// ============= Statistics =============

/// Duration buckets for histogram metrics
pub static DURATION_BUCKETS: &[f64] = &[0.1, 0.5, 1.0, 2.0, 5.0, 15.0, 60.0, 300.0, 600.0, 1800.0];

// ============= Reserved Nonce Patterns =============

/// Reserved first bytes of nonce (must avoid)
pub static RESERVED_NONCE_FIRST_BYTES: &[u8] = &[0xef];

/// Reserved 4-byte beginnings of nonce
pub static RESERVED_NONCE_BEGINNINGS: &[[u8; 4]] = &[
    [0x48, 0x45, 0x41, 0x44], // HEAD
    [0x50, 0x4F, 0x53, 0x54], // POST
    [0x47, 0x45, 0x54, 0x20], // GET
    [0xee, 0xee, 0xee, 0xee], // Intermediate
    [0xdd, 0xdd, 0xdd, 0xdd], // Secure
    [0x16, 0x03, 0x01, 0x02], // TLS
];

/// Reserved continuation bytes (bytes 4-7)
pub static RESERVED_NONCE_CONTINUES: &[[u8; 4]] = &[[0x00, 0x00, 0x00, 0x00]];

#[cfg(test)]
#[path = "tests/tls_size_constants_security_tests.rs"]
mod tls_size_constants_security_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_tag_roundtrip() {
        for tag in [ProtoTag::Abridged, ProtoTag::Intermediate, ProtoTag::Secure] {
            let bytes = tag.to_bytes();
            let parsed = ProtoTag::from_bytes(bytes).unwrap();
            assert_eq!(tag, parsed);
        }
    }

    #[test]
    fn test_proto_tag_values() {
        assert_eq!(ProtoTag::Abridged.to_bytes(), PROTO_TAG_ABRIDGED);
        assert_eq!(ProtoTag::Intermediate.to_bytes(), PROTO_TAG_INTERMEDIATE);
        assert_eq!(ProtoTag::Secure.to_bytes(), PROTO_TAG_SECURE);
    }

    #[test]
    fn test_invalid_proto_tag() {
        assert!(ProtoTag::from_bytes([0, 0, 0, 0]).is_none());
        assert!(ProtoTag::from_bytes([0xff, 0xff, 0xff, 0xff]).is_none());
    }

    #[test]
    fn test_datacenters_count() {
        assert_eq!(TG_DATACENTERS_V4.len(), 5);
        assert_eq!(TG_DATACENTERS_V6.len(), 5);
    }

    #[test]
    fn secure_padding_never_produces_aligned_total() {
        let rng = SecureRandom::new();
        for data_len in (0..1000).step_by(4) {
            for _ in 0..100 {
                let padding = secure_padding_len(data_len, &rng);
                assert!(
                    (1..=3).contains(&padding),
                    "padding out of range: data_len={data_len}, padding={padding}"
                );
                assert_ne!(
                    (data_len + padding) % 4,
                    0,
                    "invariant violated: data_len={data_len}, padding={padding}, total={}",
                    data_len + padding
                );
            }
        }
    }

    #[test]
    fn secure_wire_len_roundtrip_for_aligned_payload() {
        for payload_len in (4..4096).step_by(4) {
            for padding in 0..=3usize {
                let wire_len = payload_len + padding;
                let recovered = secure_payload_len_from_wire_len(wire_len);
                assert_eq!(recovered, Some(payload_len));
            }
        }
    }

    #[test]
    fn secure_wire_len_preserves_full_word_tail() {
        let payload_len = 64;
        for padding in [4usize, 8, 12] {
            let wire_len = payload_len + padding;
            let recovered = secure_payload_len_from_wire_len(wire_len);
            assert_eq!(recovered, Some(wire_len));
        }
    }

    #[test]
    fn secure_wire_len_rejects_too_short_frames() {
        assert_eq!(secure_payload_len_from_wire_len(0), None);
        assert_eq!(secure_payload_len_from_wire_len(1), None);
        assert_eq!(secure_payload_len_from_wire_len(2), None);
        assert_eq!(secure_payload_len_from_wire_len(3), None);
    }
}
