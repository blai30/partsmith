//! A fast hasher for the crate's internal name indexes.
//!
//! Building an element set hashes every adjacency name of a body state, tens of thousands of short
//! strings, and with the standard library's flooding-resistant hasher that hashing was more than
//! half of the build. The keys are a body state's own names, so flooding resistance buys nothing
//! here. This is the multiply-rotate hash rustc uses for its own maps (FxHash); it is also
//! deterministic, so iteration order never varies between runs (nothing relies on it).

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// The odd multiplier of the hash (from FxHash).
const MULTIPLIER: u64 = 0x517c_c1b7_2722_0a95;

/// Bytes hashed per step.
const WORD: usize = 8;

/// A multiply-rotate hasher for short keys.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct NameHasher(u64);

impl NameHasher {
    /// Mixes one word into the hash.
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(MULTIPLIER);
    }
}

impl Hasher for NameHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut words = bytes.chunks_exact(WORD);
        for word in &mut words {
            let mut buffer = [0; WORD];
            buffer.copy_from_slice(word);
            self.add(u64::from_le_bytes(buffer));
        }
        let rest = words.remainder();
        if !rest.is_empty() {
            let mut buffer = [0; WORD];
            buffer[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buffer));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        // `usize` is at most 64 bits on every supported target.
        self.add(value as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// A hash map keyed by names or ids, hashed with `NameHasher`.
pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<NameHasher>>;

#[cfg(test)]
mod tests {
    use std::hash::{BuildHasher, BuildHasherDefault};

    use super::*;

    fn hash_of(text: &str) -> u64 {
        BuildHasherDefault::<NameHasher>::default().hash_one(text)
    }

    #[test]
    fn equal_texts_hash_alike_and_different_texts_apart() {
        assert_eq!(hash_of("f-box1/top"), hash_of("f-box1/top"));
        assert_ne!(hash_of("f-box1/top"), hash_of("f-box1/top#1"));
        assert_ne!(hash_of("f-a/x"), hash_of("f-a/y"));
    }

    #[test]
    fn a_map_finds_its_keys() {
        let mut map: FastMap<&str, u32> = FastMap::default();
        for (position, name) in ["f-a/x", "f-a/x#1", "f-b/y", "edge(f-a/x|f-b/y)"]
            .into_iter()
            .enumerate()
        {
            map.insert(name, u32::try_from(position).expect("small"));
        }

        assert_eq!(map.get("f-b/y"), Some(&2));
        assert_eq!(map.get("f-c/z"), None);
    }
}
