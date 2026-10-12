//! The in-memory result cache with least-recently-used eviction (recompute.md section 3.7).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{BodyState, FeatureError, FeatureOutput, OutputData};
use partsmith_model::Id;

use crate::key::CacheKey;

/// What building a feature gave: its output or its error.
#[derive(Clone, Debug)]
pub(crate) enum Outcome {
    /// The feature built.
    Built(Built),
    /// The feature failed.
    Failed(FeatureError),
}

/// A built output with the body states derived from it, so element infos computed once are
/// shared by every run that reuses it.
#[derive(Clone, Debug)]
pub(crate) struct Built {
    /// The output.
    pub(crate) output: Arc<FeatureOutput>,
    /// The new state of each updated body, in update order.
    pub(crate) bodies: Vec<(Id, BodyState)>,
}

impl Built {
    /// An output with the states of the bodies it updates.
    pub(crate) fn new(output: FeatureOutput) -> Built {
        let bodies = match &output.data {
            OutputData::Bodies(updates) => updates
                .iter()
                .map(|update| {
                    let state = match &update.shape {
                        Some(shape) => BodyState::new(shape.clone(), Arc::clone(&update.naming)),
                        None => BodyState::empty(),
                    };
                    (update.body.clone(), state)
                })
                .collect(),
            OutputData::Sketch(_) | OutputData::Datum(_) | OutputData::Nothing => Vec::new(),
        };
        Built {
            output: Arc::new(output),
            bodies,
        }
    }
}

/// Cached outcomes by key, shared by every document of a session.
#[derive(Debug)]
pub(crate) struct Cache {
    /// The cached outcomes.
    entries: BTreeMap<CacheKey, Entry>,
    /// How many entries a run leaves at most, besides those it used.
    limit: usize,
    /// Number of the current (or latest) run.
    run: u64,
}

/// One cached outcome.
#[derive(Debug)]
struct Entry {
    /// The outcome.
    outcome: Outcome,
    /// The run that last used it.
    last_use: u64,
}

impl Cache {
    /// An empty cache that keeps at most `limit` entries after a run, plus those the run used.
    pub(crate) fn new(limit: usize) -> Cache {
        Cache {
            entries: BTreeMap::new(),
            limit,
            run: 0,
        }
    }

    /// The entry limit.
    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    /// Number of cached outcomes.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Drops every cached outcome.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The outcome cached under `key`, without marking it used (for plans).
    pub(crate) fn peek(&self, key: &CacheKey) -> Option<&Outcome> {
        self.entries.get(key).map(|entry| &entry.outcome)
    }

    /// Starts a run: entries used from now on count as used by it.
    pub(crate) fn begin_run(&mut self) {
        self.run += 1;
    }

    /// The outcome cached under `key`, marked as used by the current run.
    pub(crate) fn get(&mut self, key: &CacheKey) -> Option<Outcome> {
        let entry = self.entries.get_mut(key)?;
        entry.last_use = self.run;
        Some(entry.outcome.clone())
    }

    /// Caches `outcome` under `key`, used by the current run.
    pub(crate) fn insert(&mut self, key: CacheKey, outcome: Outcome) {
        self.entries.insert(
            key,
            Entry {
                outcome,
                last_use: self.run,
            },
        );
    }

    /// Ends a run: while there are more entries than the limit, evicts the entry whose last use
    /// is oldest (the smaller key first on ties) among those the run did not use.
    pub(crate) fn end_run(&mut self) {
        let excess = self.entries.len().saturating_sub(self.limit);
        if excess == 0 {
            return;
        }
        let mut unused: Vec<(u64, CacheKey)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_use != self.run)
            .map(|(key, entry)| (entry.last_use, *key))
            .collect();
        unused.sort_unstable();
        for (_, key) in unused.into_iter().take(excess) {
            self.entries.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_features::DatumResult;
    use partsmith_sketch::OriginPlane;

    use super::*;

    fn key(byte: u8) -> CacheKey {
        CacheKey([byte; 32])
    }

    fn failed() -> Outcome {
        Outcome::Failed(FeatureError::BothSidesZero)
    }

    /// Runs `keys` (each looked up, then inserted when missing) as one run.
    fn run(cache: &mut Cache, keys: &[u8]) {
        cache.begin_run();
        for &byte in keys {
            if cache.get(&key(byte)).is_none() {
                cache.insert(key(byte), failed());
            }
        }
        cache.end_run();
    }

    fn cached(cache: &Cache) -> Vec<u8> {
        cache.entries.keys().map(|key| key.0[0]).collect()
    }

    #[test]
    fn entries_are_found_by_key() {
        let mut cache = Cache::new(10);
        cache.begin_run();

        cache.insert(key(1), failed());

        assert!(cache.peek(&key(1)).is_some());
        assert!(cache.peek(&key(2)).is_none());
        assert!(matches!(cache.get(&key(1)), Some(Outcome::Failed(_))));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn nothing_is_evicted_within_the_limit() {
        let mut cache = Cache::new(3);

        run(&mut cache, &[1, 2]);
        run(&mut cache, &[3]);

        assert_eq!(cached(&cache), [1, 2, 3]);
    }

    #[test]
    fn the_oldest_unused_entries_go_first() {
        let mut cache = Cache::new(2);

        run(&mut cache, &[1]);
        run(&mut cache, &[2]);
        run(&mut cache, &[1]);
        run(&mut cache, &[3]);

        assert_eq!(cached(&cache), [1, 3]);
    }

    #[test]
    fn ties_evict_the_smaller_key_first() {
        let mut cache = Cache::new(2);

        run(&mut cache, &[7, 5, 9]);
        run(&mut cache, &[1]);

        assert_eq!(cached(&cache), [1, 9]);
    }

    #[test]
    fn entries_of_the_latest_run_are_never_evicted() {
        let mut cache = Cache::new(1);

        run(&mut cache, &[1, 2, 3]);

        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn a_zero_limit_keeps_only_the_latest_run() {
        let mut cache = Cache::new(0);

        run(&mut cache, &[1, 2]);
        run(&mut cache, &[3]);

        assert_eq!(cached(&cache), [3]);
    }

    #[test]
    fn peeking_does_not_count_as_a_use() {
        let mut cache = Cache::new(1);
        run(&mut cache, &[1]);
        run(&mut cache, &[2]);
        cache.peek(&key(2));

        run(&mut cache, &[3]);

        assert_eq!(cached(&cache), [3]);
    }

    #[test]
    fn clear_drops_every_entry() {
        let mut cache = Cache::new(5);
        run(&mut cache, &[1, 2]);

        cache.clear();

        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn built_outputs_without_bodies_have_no_states() {
        let built = Built::new(FeatureOutput {
            data: OutputData::Datum(DatumResult::Plane(OriginPlane::Xy.placement())),
            warnings: Vec::new(),
        });

        assert!(built.bodies.is_empty());
    }

    #[test]
    fn an_emptied_body_gets_an_empty_state() {
        let body = Id::new("b-1").expect("valid id");
        let built = Built::new(FeatureOutput {
            data: OutputData::Bodies(vec![partsmith_features::BodyUpdate {
                body: body.clone(),
                shape: None,
                naming: BodyState::empty().naming().clone(),
                change: partsmith_features::BodyChange::Clear,
            }]),
            warnings: Vec::new(),
        });

        assert_eq!(built.bodies.len(), 1);
        assert_eq!(built.bodies[0].0, body);
        assert!(built.bodies[0].1.is_empty());
    }
}
