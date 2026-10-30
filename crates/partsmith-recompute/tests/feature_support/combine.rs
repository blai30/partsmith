//! Helpers for the combine features (boolean, mirror, linear and circular patterns, move body,
//! split body, import): every body update of an output, empty and unchanged bodies, and a build
//! canceled before it starts.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, FeatureError, FeatureOutput, Values,
};
use partsmith_model::{EmptyContext, Id, Scope};
use serde_json::Value;

use super::{Assets, Run, assert_same_body, body_before};

/// Every body update of `feature`'s output in output order, as `<body> <change>`, for example
/// `b-2 Clear`, so a test compares them with a spec's `[b-1 Join, b-2 Clear]`.
pub fn body_changes(run: &Run, feature: &str) -> Vec<String> {
    run.body_updates(feature)
        .iter()
        .map(|update| format!("{} {:?}", update.body, update.change))
        .collect()
}

/// Asserts that `body` is empty at the end of the run.
pub fn assert_empty(run: &Run, body: &str) {
    assert!(
        run.body(body).is_empty(),
        "body {body} should be empty; {}",
        run.summary()
    );
}

/// Asserts that `feature` left `body` unchanged (features.md section 6.1): the body's state at the
/// end of the run is its state just before `feature`, which must be the last feature writing it.
pub fn assert_unchanged(run: &Run, feature: &str, body: &str) {
    assert_same_body(run.body(body), body_before(run, feature, body));
}

impl Run {
    /// Asserts that building `feature` once more through `Registry::build`, outside recompute, from
    /// what the run gave it (its canonical record, its evaluated values and the states of its
    /// declared bodies just before it), with a `CancelToken` that is already canceled, returns
    /// `Err(Canceled)` (features.md section 2.6). The context holds no writer chains, upstream
    /// outputs or assets, which a pattern does not read.
    pub fn assert_build_canceled(&self, feature: &str) {
        let registry = self.recompute.registry();
        let record = registry
            .validate_and_canonicalize(self.record(feature))
            .unwrap_or_else(|error| panic!("the record of {feature} is invalid: {error}"))
            .record;
        let manifest = registry
            .get(&record.kind)
            .unwrap_or_else(|| panic!("{} is a built-in type", record.kind))
            .manifest();
        let dependencies = self.dependencies(feature);
        let parameters = self.document.evaluate_parameters(&EmptyContext);
        let scope = Scope {
            units: self.document.units,
            parameters: &parameters,
            context: &EmptyContext,
        };
        let values = Values::evaluate(&dependencies.expressions, &scope, manifest);
        let bodies: BTreeMap<Id, BodyState> = self
            .feature(feature)
            .bodies_before
            .iter()
            .cloned()
            .collect();
        let body = record
            .params
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{feature} has a body"));
        assert!(
            bodies.keys().any(|id| id.as_str() == body),
            "the run recorded the state of {body} before {feature}"
        );
        let history: BTreeMap<Id, Vec<(Id, BodyState)>> = BTreeMap::new();
        let upstream: BTreeMap<Id, Arc<FeatureOutput>> = BTreeMap::new();
        let assets = Assets::new();
        let cancel = CancelToken::new();
        cancel.cancel();
        let context = BuildContext::new(BuildInputs {
            feature: &record.id,
            manifest,
            units: self.document.units,
            values: &values,
            bodies: &bodies,
            history: &history,
            upstream: &upstream,
            assets: &assets,
            cancel: &cancel,
        });
        let result = registry.build(&record, &context);
        assert!(
            matches!(result, Err(FeatureError::Canceled)),
            "a canceled build of {feature} returns Canceled, not {result:?}",
        );
    }
}
