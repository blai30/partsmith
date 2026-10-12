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

#[cfg(test)]
mod tests {
    use partsmith_model::EmptyContext;
    use serde_json::json;

    use super::*;
    use crate::test_support::{document, registry};
    use crate::timeline::StepKind;
    use crate::validation::Validations;

    /// The key of every record of `features` in timeline order, None for records that do not run.
    fn keys(features: serde_json::Value) -> Vec<Option<CacheKey>> {
        let registry = registry();
        let document = document(features);
        let timeline = Timeline::prepare(&registry, &mut Validations::default(), &document, None);
        let parameters = document.evaluate_parameters(&EmptyContext);
        let scope = Scope {
            units: document.units,
            parameters: &parameters,
            context: &EmptyContext,
        };
        let mut keys = Vec::new();
        for (position, step) in timeline.steps.iter().enumerate() {
            let key = match &step.kind {
                StepKind::Active(active) => {
                    Some(evaluate(&timeline, position, active, &keys, &scope).1)
                }
                _ => None,
            };
            keys.push(key);
        }
        keys
    }

    #[test]
    fn a_feature_link_to_a_later_record_counts_as_none() {
        let forward = keys(json!([
            {"id": "f-1", "type": "test_datum", "base": "f-2", "offset": "1"},
            {"id": "f-2", "type": "test_datum", "offset": "1"}
        ]));
        let unknown = keys(json!([
            {"id": "f-1", "type": "test_datum", "base": "f-2", "offset": "1"}
        ]));

        assert_eq!(forward[0], unknown[0]);
    }

    #[test]
    fn upstream_keys_flow_into_downstream_keys() {
        let first = keys(json!([
            {"id": "f-1", "type": "test_datum", "offset": "1"},
            {"id": "f-2", "type": "test_datum", "base": "f-1", "offset": "1"}
        ]));
        let edited = keys(json!([
            {"id": "f-1", "type": "test_datum", "offset": "2"},
            {"id": "f-2", "type": "test_datum", "base": "f-1", "offset": "1"}
        ]));

        assert_ne!(first[0], edited[0]);
        assert_ne!(first[1], edited[1]);
    }

    #[test]
    fn a_body_key_follows_its_provider() {
        let written = keys(json!([
            {"id": "f-1", "type": "test_writer", "body": "b-1"},
            {"id": "f-2", "type": "test_reader", "body": "b-1"}
        ]));
        let suppressed = keys(json!([
            {"id": "f-1", "type": "test_writer", "suppressed": true, "body": "b-1"},
            {"id": "f-2", "type": "test_reader", "body": "b-1"}
        ]));
        let other_body = keys(json!([
            {"id": "f-1", "type": "test_writer", "body": "b-2"},
            {"id": "f-2", "type": "test_reader", "body": "b-1"}
        ]));

        assert_ne!(written[1], suppressed[1]);
        assert_eq!(suppressed[1], other_body[1]);
    }
}
