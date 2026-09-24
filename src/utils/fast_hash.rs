//! A fast, non-cryptographic hasher for small keys in hot search tables.
//!
//! `std`'s default SipHash resists hash-flooding, which matters for maps fed
//! untrusted input and not at all for tables keyed by squares and piece types
//! inside the search, where it cost several times the lookup itself. This is
//! the multiply-rotate "Fx" hash used by rustc.
//!
//! Only a map's iteration order depends on the hasher, so switching a map to
//! [`FastMap`] is exact wherever nothing depends on that order.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// `HashMap` with [`FxHasher`].
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// rustc's Fx hash: one rotate, xor and multiply per word.
#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes(c.try_into().unwrap()));
        }
        for &b in chunks.remainder() {
            self.add(b as u64);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}
