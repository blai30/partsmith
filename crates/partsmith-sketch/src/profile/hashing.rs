//! A fast hash for the lookups of region matching and for region key digests.
//!
//! The hashed data comes from the sketch itself and lives only in memory, so a hash without protection against
//! chosen collisions is safe, and its values never reach a document.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hash, Hasher};

/// A hash map with the fast hash.
pub(super) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// A hash set with the fast hash.
pub(super) type FastSet<T> = HashSet<T, BuildHasherDefault<FastHasher>>;

/// The multiplier of the Fx hash.
const MULTIPLIER: u64 = 0x517c_c1b7_2722_0a95;

/// The Fx hash (from Firefox, also used by rustc): one rotate, xor and multiply per word.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FastHasher {
    hash: u64,
}

impl FastHasher {
    /// The hash of one value.
    pub(super) fn of(value: &impl Hash) -> u64 {
        let mut hasher = FastHasher::default();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(MULTIPLIER);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }

    fn finish(&self) -> u64 {
        self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_values_hash_equally() {
        assert_eq!(FastHasher::of(&"e-1"), FastHasher::of(&String::from("e-1")));
    }

    #[test]
    fn a_trailing_byte_changes_the_hash() {
        assert_ne!(FastHasher::of(&"e-1"), FastHasher::of(&"e-10"));
    }

    #[test]
    fn the_order_of_words_matters() {
        assert_ne!(
            FastHasher::of(&(1_u64, 2_u64)),
            FastHasher::of(&(2_u64, 1_u64))
        );
    }
}
