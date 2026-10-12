//! Evaluating a step's expressions and computing its cache key (recompute.md section 3.5 step 3).

use partsmith_features::Values;
use partsmith_model::Scope;

use crate::key::{CacheKey, KeyInput};
use crate::timeline::{ActiveStep, Timeline};

/// The evaluated values and the cache key of the active step at `position`. `keys` holds the
/// key of every earlier step (None for records without one).
///
/// A feature link takes the key of the record it names when that record is earlier, and none
/// otherwise; a body link takes the key of its provider, or empty when it has none.
pub(crate) fn evaluate(
    timeline: &Timeline<'_>,
    position: usize,
    step: &ActiveStep<'_>,
    keys: &[Option<CacheKey>],
    scope: &Scope<'_>,
) -> (Values, CacheKey) {
    let record = timeline.steps[position].record;
    let manifest = step.feature.manifest();
    let dependencies = &step.valid.dependencies;
    let values = Values::evaluate(&dependencies.expressions, scope, manifest);
    let earlier_key = |upstream: Option<usize>| {
        upstream
            .filter(|&upstream| upstream < position)
            .and_then(|upstream| keys[upstream])
    };
    let key = KeyInput {
        type_name: &record.kind,
        version: manifest.version,
        feature: &record.id,
        params: &step.valid.params_text,
        values: &values,
        features: dependencies
            .features
            .iter()
            .map(|link| (&link.id, earlier_key(timeline.graph.position(&link.id))))
            .collect(),
        bodies: step
            .body_links()
            .zip(&step.providers)
            .map(|(link, &provider)| (&link.id, earlier_key(provider)))
            .collect(),
    }
    .key();
    (values, key)
}
