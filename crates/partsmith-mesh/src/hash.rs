//! A fast, deterministic hasher for the crate's internal hash maps.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A hash map with [`FastHasher`]. Maps of this type are only ever looked up, never iterated,
/// so no result depends on their order.
pub(crate) type FastHashMap<Key, Value> = HashMap<Key, Value, BuildHasherDefault<FastHasher>>;

/// Odd multiplier of the folded multiply: the 64 fraction bits of the golden ratio.
const MULTIPLIER: u64 = 0x9E37_79B9_7F4A_7C15;
/// Starting state: the first 64 fraction bits of π, so a zero key does not hash to zero.
const SEED: u64 = 0x243F_6A88_85A3_08D3;

/// Hashes small fixed-size keys (cell coordinates, vertex and face ids) with one folded multiply
/// per 64-bit word.
///
/// The standard library's SipHash resists hash flooding, which these maps do not need: their
/// keys come from mesh geometry, and the worst a crafted mesh can do is slow its own analysis.
/// SipHash costs more than the rest of a weld lookup, so this hasher keeps welding fast. The
/// full 128-bit product mixes every key bit into both halves, so keys that differ only in high
/// bits (as `f64` bit patterns of nearby integers do) still spread over the table.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FastHasher {
    /// The hash of the words written so far.
    state: u64,
}

impl Default for FastHasher {
    fn default() -> Self {
        FastHasher { state: SEED }
    }
}

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0_u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        let product = u128::from(self.state ^ value) * u128::from(MULTIPLIER);
        // Folding the high half onto the low half keeps the mixing of both.
        self.state = (product as u64) ^ ((product >> 64) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{BuildHasher, Hash};

    fn hash_of(value: impl Hash) -> u64 {
        BuildHasherDefault::<FastHasher>::default().hash_one(value)
    }

    #[test]
    fn hashing_is_deterministic() {
        assert_eq!(hash_of([1_u64, 2, 3]), hash_of([1_u64, 2, 3]));
        assert_eq!(hash_of(7_u32), hash_of(7_u32));
    }

    #[test]
    fn zero_does_not_hash_to_zero() {
        assert_ne!(hash_of(0_u64), 0);
    }

    #[test]
    fn word_order_matters() {
        assert_ne!(hash_of((1_u64, 2_u64)), hash_of((2_u64, 1_u64)));
    }

    #[test]
    fn keys_differing_only_in_high_bits_differ_in_low_bits() {
        // The bit patterns of the f64 values 1 to 64 differ only in their top 19 bits; a table
        // indexes by the low bits of the hash.
        let mut low_bits: Vec<u64> = (1..=64)
            .map(|cell| hash_of(f64::from(cell).to_bits()) & 0xFF)
            .collect();
        low_bits.sort_unstable();
        low_bits.dedup();

        assert!(low_bits.len() > 32, "only {} distinct", low_bits.len());
    }

    #[test]
    fn byte_writes_hash_whole_words() {
        let mut bytes = FastHasher::default();
        bytes.write(&5_u64.to_le_bytes());
        let mut word = FastHasher::default();
        word.write_u64(5);

        assert_eq!(bytes.finish(), word.finish());
    }
}
