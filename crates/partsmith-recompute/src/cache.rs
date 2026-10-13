//! The in-memory result cache with least-recently-used eviction (recompute.md section 3.7).

use std::sync::Arc;

use partsmith_features::{BodyState, FeatureError, FeatureOutput, OutputData};
use partsmith_model::Id;

use crate::key::CacheKey;
use crate::pass_lru::PassLru;

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
                .map(|update| (update.body.clone(), BodyState::from_update(update)))
                .collect(),
            OutputData::Sketch(_) | OutputData::Datum(_) | OutputData::Nothing => Vec::new(),
        };
        Built {
            output: Arc::new(output),
            bodies,
        }
    }
}

/// Cached outcomes by key, shared by every document of a session. Each run is one pass.
#[derive(Debug)]
pub(crate) struct Cache {
    /// The cached outcomes with the run that last used each.
    entries: PassLru<CacheKey, Outcome>,
    /// How many entries a run leaves at most, besides those it used.
    limit: usize,
}

impl Cache {
    /// An empty cache that keeps at most `limit` entries after a run, plus those the run used.
    pub(crate) fn new(limit: usize) -> Cache {
        Cache {
            entries: PassLru::default(),
            limit,
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
        self.entries.peek(key)
    }

    /// Starts a run: entries used from now on count as used by it.
    pub(crate) fn begin_run(&mut self) {
        self.entries.begin_pass();
    }

    /// The outcome cached under `key`, marked as used by the current run.
    pub(crate) fn get(&mut self, key: &CacheKey) -> Option<Outcome> {
        self.entries.touch(key).cloned()
    }

    /// Caches `outcome` under `key`, used by the current run.
    pub(crate) fn insert(&mut self, key: CacheKey, outcome: Outcome) {
        self.entries.insert(key, outcome);
    }

    /// Ends a run: evicts the entries used longest ago, beyond the limit, that the run did not
    /// use.
    pub(crate) fn end_run(&mut self) {
        self.entries.evict_to(self.limit);
    }
}

#[cfg(test)]
mod tests {
    use partsmith_features::DatumResult;
    use partsmith_sketch::OriginPlane;

    use super::*;
    use crate::test_support::id;

    fn key(byte: u8) -> CacheKey {
        CacheKey([byte; 32])
    }

    fn failed() -> Outcome {
        Outcome::Failed(FeatureError::BothSidesZero)
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
    fn built_outputs_without_bodies_have_no_states() {
        let built = Built::new(FeatureOutput {
            data: OutputData::Datum(DatumResult::Plane(OriginPlane::Xy.placement())),
            warnings: Vec::new(),
        });

        assert!(built.bodies.is_empty());
    }

    #[test]
    fn an_emptied_body_gets_an_empty_state() {
        let body = id("b-1");
        let built = Built::new(FeatureOutput {
            data: OutputData::Bodies(vec![partsmith_features::BodyUpdate {
                body: body.clone(),
                shape: None,
                naming: BodyState::empty().naming().clone(),
                change: partsmith_features::BodyChange::Clear,
                measured: partsmith_features::Measurements::default(),
            }]),
            warnings: Vec::new(),
        });

        assert_eq!(built.bodies.len(), 1);
        assert_eq!(built.bodies[0].0, body);
        assert!(built.bodies[0].1.is_empty());
    }
}
