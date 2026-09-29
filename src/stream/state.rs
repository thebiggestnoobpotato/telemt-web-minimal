//! State machine foundation types for async streams
//!
//! This module provides core types and traits for implementing
//! stateful async streams with proper partial read/write handling.

#![allow(dead_code)]

use bytes::Bytes;

// ============= Core Traits =============

/// Trait for stream states
pub trait StreamState: Sized {
    /// Check if this is a terminal state (no more transitions possible)
    fn is_terminal(&self) -> bool;

    /// Check if stream is in poisoned/error state
    fn is_poisoned(&self) -> bool;

    /// Get human-readable state name for debugging
    fn state_name(&self) -> &'static str;
}

// ============= Yield Buffer =============

/// Buffer for yielding data to caller in chunks
#[derive(Debug)]
pub struct YieldBuffer {
    data: Bytes,
    position: usize,
}

impl YieldBuffer {
    /// Create new yield buffer
    pub fn new(data: Bytes) -> Self {
        Self { data, position: 0 }
    }

    /// Check if all data has been yielded
    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }

    /// Get remaining bytes
    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }

    /// Copy data to output slice, return bytes copied
    pub fn copy_to(&mut self, dst: &mut [u8]) -> usize {
        let available = &self.data[self.position..];
        let to_copy = available.len().min(dst.len());
        dst[..to_copy].copy_from_slice(&available[..to_copy]);
        self.position += to_copy;
        to_copy
    }

    /// Get remaining data as slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data[self.position..]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_yield_buffer() {
        let mut buf = YieldBuffer::new(Bytes::from_static(b"hello world"));

        let mut dst = [0u8; 5];
        assert_eq!(buf.copy_to(&mut dst), 5);
        assert_eq!(&dst, b"hello");

        assert_eq!(buf.remaining(), 6);

        let mut dst = [0u8; 10];
        assert_eq!(buf.copy_to(&mut dst), 6);
        assert_eq!(&dst[..6], b" world");

        assert!(buf.is_empty());
    }
}
