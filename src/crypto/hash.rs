//! Cryptographic hash functions
//!
//! SHA-256 and CRC32 primitives used by config loading, replay checks,
//! and frame checksums.

use hmac::{Hmac, Mac};
use sha2::Digest;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// SHA-256
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// SHA-256 HMAC
pub fn sha256_hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// CRC32
pub fn crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}
