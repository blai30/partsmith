//! A map that remembers the pass in which each entry was last used, and evicts the entries used
//! longest ago (recompute.md section 3.7), for the result cache and the validation memo.

use std::collections::BTreeMap;

/// Values by key, each with the number of the pass that last used it. A pass is one run, plan
/// or graph; what a pass uses is never evicted at its end.
#[derive(Debug)]
pub(crate) struct PassLru<K, V> {
    /// The values with their last use.
    entries: BTreeMap<K, Used<V>>,
    /// Number of the current (or latest) pass.
    pass: u64,
}

/// One value and the pass that last used it.
#[derive(Debug)]
struct Used<V> {
    /// The value.
    value: V,
    /// The pass that last used it.
    last_use: u64,
}

impl<K, V> Default for PassLru<K, V> {
    fn default() -> Self {
        PassLru {
            entries: BTreeMap::new(),
            pass: 0,
        }
    }
}

impl<K: Ord + Copy, V> PassLru<K, V> {
    /// Starts a pass: entries used from now on count as used by it.
    pub(crate) fn begin_pass(&mut self) {
        self.pass += 1;
    }

    /// The value under `key`, marked as used by the current pass.
    pub(crate) fn touch(&mut self, key: &K) -> Option<&V> {
        let entry = self.entries.get_mut(key)?;
        entry.last_use = self.pass;
        Some(&entry.value)
    }

    /// The value under `key`, without marking it used.
    pub(crate) fn peek(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    /// Stores `value` under `key`, used by the current pass.
    // Inlined so a large value (a feature error) is written in place rather than copied.
    #[inline]
    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.entries.insert(
            key,
            Used {
                value,
                last_use: self.pass,
            },
        );
    }

    /// While there are more entries than `limit`, evicts the entry whose last use is oldest (the
    /// smaller key first on ties) among those the current pass did not use.
    pub(crate) fn evict_to(&mut self, limit: usize) {
        let excess = self.entries.len().saturating_sub(limit);
        if excess == 0 {
            return;
        }
        let mut unused: Vec<(u64, K)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_use != self.pass)
            .map(|(key, entry)| (entry.last_use, *key))
            .collect();
        unused.sort_unstable();
        for (_, key) in unused.into_iter().take(excess) {
            self.entries.remove(&key);
        }
    }

    /// Number of entries.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Drops every entry.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The keys in ascending order.
    #[cfg(test)]
    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses `keys` (each looked up, then inserted when missing) in one pass, then evicts to `limit`.
    fn pass(lru: &mut PassLru<u8, ()>, keys: &[u8], limit: usize) {
        lru.begin_pass();
        for &key in keys {
            if lru.touch(&key).is_none() {
                lru.insert(key, ());
            }
        }
        lru.evict_to(limit);
    }

    fn kept(lru: &PassLru<u8, ()>) -> Vec<u8> {
        lru.keys().copied().collect()
    }

    #[test]
    fn values_are_found_by_key() {
        let mut lru = PassLru::default();
        lru.begin_pass();

        lru.insert(1_u8, "one");

        assert_eq!(lru.peek(&1), Some(&"one"));
        assert_eq!(lru.peek(&2), None);
        assert_eq!(lru.touch(&1), Some(&"one"));
        assert_eq!(lru.touch(&2), None);
        assert_eq!(lru.len(), 1);
    }

    #[test]
    fn nothing_is_evicted_within_the_limit() {
        let mut lru = PassLru::default();

        pass(&mut lru, &[1, 2], 3);
        pass(&mut lru, &[3], 3);

        assert_eq!(kept(&lru), [1, 2, 3]);
    }

    #[test]
    fn the_oldest_unused_entries_go_first() {
        let mut lru = PassLru::default();

        pass(&mut lru, &[1], 2);
        pass(&mut lru, &[2], 2);
        pass(&mut lru, &[1], 2);
        pass(&mut lru, &[3], 2);

        assert_eq!(kept(&lru), [1, 3]);
    }

    #[test]
    fn ties_evict_the_smaller_key_first() {
        let mut lru = PassLru::default();

        pass(&mut lru, &[7, 5, 9], 2);
        pass(&mut lru, &[1], 2);

        assert_eq!(kept(&lru), [1, 9]);
    }

    #[test]
    fn entries_of_the_current_pass_are_never_evicted() {
        let mut lru = PassLru::default();

        pass(&mut lru, &[1, 2, 3], 1);

        assert_eq!(lru.len(), 3);
    }

    #[test]
    fn a_zero_limit_keeps_only_the_current_pass() {
        let mut lru = PassLru::default();

        pass(&mut lru, &[1, 2], 0);
        pass(&mut lru, &[3], 0);

        assert_eq!(kept(&lru), [3]);
    }

    #[test]
    fn peeking_does_not_count_as_a_use() {
        let mut lru = PassLru::default();
        pass(&mut lru, &[1], 1);
        pass(&mut lru, &[2], 1);
        lru.peek(&2);

        pass(&mut lru, &[3], 1);

        assert_eq!(kept(&lru), [3]);
    }

    #[test]
    fn clear_drops_every_entry() {
        let mut lru = PassLru::default();
        pass(&mut lru, &[1, 2], 5);

        lru.clear();

        assert_eq!(lru.len(), 0);
    }
}
