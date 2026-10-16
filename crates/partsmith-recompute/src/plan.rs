//! What a run would do, predicted without building (recompute.md section 3.8).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_model::{Id, Scope};

use crate::cache::{Cache, Outcome};
use crate::evaluate::{Progress, evaluate};
use crate::key::CacheKey;
use crate::timeline::{StepKind, Timeline};

/// What a run would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// One step per feature record, in timeline order.
    pub steps: Vec<PlanStep>,
}

/// What a run would do with one feature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    /// The feature.
    pub feature: Id,
    /// The predicted action.
    pub action: Action,
    /// Its cache key, when it has one.
    pub key: Option<CacheKey>,
}

/// Predicted action for a feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// No cached result for its key: it will be built.
    Build,
    /// Its key is cached: the result will be reused.
    Reuse,
    /// It fails before building (invalid, unknown references, unavailable upstream).
    Fail,
    /// Suppressed.
    Suppressed,
    /// At or after the rollback marker, or after the stop point.
    RolledBack,
}

/// Predicts each step of `timeline`. Values and keys are evaluated as a run evaluates them; the
/// pre-build checks count a record as failed when it is invalid, is predicted to fail, or reuses
/// a cached error; keys are looked up in `cache` without using its entries.
pub(crate) fn plan(
    timeline: &Timeline<'_>,
    scope: &Scope<'_>,
    cache: &Cache,
    assets: &BTreeMap<String, Arc<[u8]>>,
) -> Plan {
    let mut progress = Progress::with_capacity(timeline.steps.len());
    let mut steps = Vec::with_capacity(timeline.steps.len());
    for (position, step) in timeline.steps.iter().enumerate() {
        let (action, key, fails) = match &step.kind {
            StepKind::RolledBack(_) => (Action::RolledBack, None, false),
            StepKind::Suppressed(_) => (Action::Suppressed, None, false),
            StepKind::Invalid(_) => (Action::Fail, None, true),
            StepKind::Active(active) => {
                let (_, key) = evaluate(timeline, position, active, &progress.keys, scope);
                let (action, fails) = if timeline
                    .check_links(position, active, &progress.failed, assets)
                    .is_err()
                {
                    (Action::Fail, true)
                } else {
                    match cache.peek(&key) {
                        Some(Outcome::Failed(_)) => (Action::Reuse, true),
                        Some(Outcome::Built(_)) => (Action::Reuse, false),
                        None => (Action::Build, false),
                    }
                };
                (action, Some(key), fails)
            }
        };
        progress.push(key, fails);
        steps.push(PlanStep {
            feature: step.record.id.clone(),
            action,
            key,
        });
    }
    Plan { steps }
}

#[cfg(test)]
mod tests {
    use partsmith_features::{FeatureError, FeatureOutput, OutputData};
    use partsmith_model::{Document, EmptyContext};
    use serde_json::json;

    use super::*;
    use crate::cache::Built;
    use crate::test_support::{datum, datum_on, document, id, prepared};

    /// The plan of `document` over `cache`, with no assets.
    fn plan_of(document: &Document, cache: &Cache) -> Plan {
        let timeline = prepared(document);
        let parameters = document.evaluate_parameters(&EmptyContext);
        let scope = Scope {
            units: document.units,
            parameters: &parameters,
            context: &EmptyContext,
        };
        plan(&timeline, &scope, cache, &BTreeMap::new())
    }

    fn actions(plan: &Plan) -> Vec<Action> {
        plan.steps.iter().map(|step| step.action).collect()
    }

    fn nothing() -> Outcome {
        Outcome::Built(Built::new(FeatureOutput {
            data: OutputData::Nothing,
            warnings: Vec::new(),
            absent: Vec::new(),
        }))
    }

    /// A cache holding `outcome` under the planned key of the step at `position`.
    fn cache_with(plan: &Plan, position: usize, outcome: Outcome) -> Cache {
        let mut cache = Cache::new(10);
        cache.begin_run();
        cache.insert(plan.steps[position].key.expect("a key"), outcome);
        cache
    }

    #[test]
    fn an_empty_cache_builds_every_active_record() {
        let document = document(json!([
            datum("f-1", "1"),
            {"id": "f-2", "type": "test_datum", "suppressed": true, "offset": "1"},
            {"id": "f-3", "type": "gear_demo"},
            datum_on("f-4", "f-1")
        ]));

        let plan = plan_of(&document, &Cache::new(10));

        assert_eq!(
            actions(&plan),
            [
                Action::Build,
                Action::Suppressed,
                Action::Fail,
                Action::Build
            ]
        );
        let has_key: Vec<bool> = plan.steps.iter().map(|step| step.key.is_some()).collect();
        assert_eq!(has_key, [true, false, false, true]);
    }

    #[test]
    fn a_cached_key_is_reused() {
        let document = document(json!([datum("f-1", "1"), datum("f-2", "2")]));
        let fresh = plan_of(&document, &Cache::new(10));

        let plan = plan_of(&document, &cache_with(&fresh, 1, nothing()));

        assert_eq!(actions(&plan), [Action::Build, Action::Reuse]);
        assert_eq!(plan.steps[1].key, fresh.steps[1].key);
    }

    #[test]
    fn a_cached_error_fails_the_records_that_read_its_output() {
        let document = document(json!([
            datum("f-1", "1"),
            datum_on("f-2", "f-1"),
            datum_on("f-3", "f-2")
        ]));
        let fresh = plan_of(&document, &Cache::new(10));

        let plan = plan_of(
            &document,
            &cache_with(
                &fresh,
                0,
                Outcome::Failed(Box::new(FeatureError::BothSidesZero)),
            ),
        );

        assert_eq!(actions(&plan), [Action::Reuse, Action::Fail, Action::Fail]);
    }

    #[test]
    fn records_that_read_an_invalid_record_fail() {
        let document = document(json!([
            {"id": "f-1", "type": "test_datum", "offset": 5},
            datum_on("f-2", "f-1")
        ]));

        let plan = plan_of(&document, &Cache::new(10));

        assert_eq!(actions(&plan), [Action::Fail, Action::Fail]);
    }

    #[test]
    fn a_failed_pre_build_check_keeps_the_key() {
        let document = document(json!([datum_on("f-1", "f-nope")]));

        let plan = plan_of(&document, &Cache::new(10));

        assert_eq!(actions(&plan), [Action::Fail]);
        assert!(plan.steps[0].key.is_some());
    }

    #[test]
    fn rolled_back_records_are_predicted_rolled_back() {
        let mut document = document(json!([datum("f-1", "1"), datum("f-2", "1")]));
        document.rollback = Some(id("f-2"));

        let plan = plan_of(&document, &Cache::new(10));

        assert_eq!(actions(&plan), [Action::Build, Action::RolledBack]);
        assert_eq!(plan.steps[1].key, None);
    }
}
