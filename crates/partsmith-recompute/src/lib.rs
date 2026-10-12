//! Dependency graph, dirty tracking and cached recompute of the Partsmith feature timeline.
//!
//! [`Recompute`] runs a document's timeline: it validates every feature record through the
//! feature registry, builds the dependency [`Graph`], evaluates parameters and feature
//! expressions, computes a content-hash [`CacheKey`] for every active feature, reuses cached
//! results for unchanged keys, builds the rest, isolates failures and panics, and returns a
//! [`Report`] with per-feature status, messages, timing, outputs and body states. [`Plan`]s
//! predict what a run would build or reuse without building.
//!
//! A key is the BLAKE3 hash of a feature's canonical params, evaluated values, upstream keys and
//! the versions of `partsmith-features` and OCCT, so an edit makes exactly the edited feature and
//! its transitive dependents dirty, and undo or reopening a document reuses earlier results. One
//! `Recompute` serves every document of a session; its cache evicts the least recently used
//! results beyond its limit.
//!
//! The crate is pure Rust over `partsmith-model` and `partsmith-features`, holds no global state
//! and is deterministic. The behavior is specified in `docs/spec/recompute.md`.

// Recompute passes `partsmith_features::FeatureError` through unchanged (recompute.md section 4),
// and that error keeps the payloads features.md section 4.1 gives it, so it is larger than
// clippy's threshold. Errors end a feature's build, so their size costs nothing on a hot path.
#![expect(
    clippy::result_large_err,
    reason = "FeatureError is the features crate's error type, passed through as the spec gives it"
)]

mod cache;
mod evaluate;
mod graph;
mod key;
mod plan;
mod report;
mod run;
#[cfg(test)]
mod test_support;
mod timeline;
mod validation;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use partsmith_features::Registry;
use partsmith_model::{Document, EvaluationContext, Id, ParameterValues, Scope};

pub use graph::{Change, Edge, EdgeKind, Graph, Node, NodeState};
pub use key::CacheKey;
pub use partsmith_features::{
    BodyState, CancelToken, FeatureError, FeatureOutput, FeatureWarning, Repair,
};
pub use plan::{Action, Plan, PlanStep};
pub use report::{
    BodyReport, FeatureReport, Message, ParameterReport, Report, Severity, Status, Timing,
};

use crate::cache::Cache;
use crate::run::Inputs;
use crate::timeline::Timeline;
use crate::validation::Validations;

/// Default maximum number of cached results.
pub const DEFAULT_CACHE_ENTRIES: usize = 4096;

/// Runs timelines with a result cache. One per session, shared by its documents.
pub struct Recompute {
    /// The feature registry.
    registry: Arc<Registry>,
    /// Cached build results by key, with the run counter of their last use.
    cache: Cache,
    /// Validation outcomes by record content, so unchanged records are not parsed again.
    validations: Mutex<Validations>,
}

impl fmt::Debug for Recompute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Recompute")
            .field("registry", &self.registry)
            .field("cached", &self.cache.len())
            .field("cache_limit", &self.cache.limit())
            .finish()
    }
}

impl Recompute {
    /// A recompute with the default cache size.
    pub fn new(registry: Arc<Registry>) -> Self {
        Recompute::with_cache_limit(registry, DEFAULT_CACHE_ENTRIES)
    }

    /// A recompute whose cache keeps at most `max_entries` results after each run. Results the
    /// latest run used are never evicted, so a run can leave more.
    pub fn with_cache_limit(registry: Arc<Registry>, max_entries: usize) -> Self {
        Recompute {
            registry,
            cache: Cache::new(max_entries),
            validations: Mutex::new(Validations::default()),
        }
    }

    /// The feature registry.
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// The dependency graph of a document (no evaluation, no building).
    pub fn graph(&self, input: &Input<'_>) -> Graph {
        self.prepare(input).graph
    }

    /// What a run would build or reuse, with every key (evaluates, never builds). Never changes the cache.
    pub fn plan(&self, input: &Input<'_>) -> Plan {
        let timeline = self.prepare(input);
        let parameters = input.document.evaluate_parameters(input.context);
        plan::plan(
            &timeline,
            &input.scope(&parameters),
            &self.cache,
            input.assets,
        )
    }

    /// Runs the timeline up to the stop point.
    pub fn run(&mut self, input: &Input<'_>, cancel: &CancelToken) -> Report {
        let started = Instant::now();
        let timeline = prepare(&self.registry, &self.validations, self.cache.limit(), input);
        let parameters = input.document.evaluate_parameters(input.context);
        let scope = input.scope(&parameters);
        self.cache.begin_run();
        let ran = run::run(
            &Inputs {
                timeline: &timeline,
                registry: &self.registry,
                scope: &scope,
                assets: input.assets,
                cancel,
            },
            &mut self.cache,
        );
        self.cache.end_run();
        Report {
            features: ran.features,
            bodies: ran.bodies,
            parameters: parameter_reports(input.document, &parameters),
            messages: timeline.messages,
            canceled: ran.canceled,
            elapsed: started.elapsed(),
        }
    }

    /// Number of cached results.
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Drops every cached result.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// The prepared timeline of `input`.
    fn prepare<'a>(&'a self, input: &Input<'a>) -> Timeline<'a> {
        prepare(&self.registry, &self.validations, self.cache.limit(), input)
    }
}

/// Prepares the timeline of `input` through the validation memo in a new pass, then prunes the
/// memo down to `limit` outcomes besides those the pass used.
fn prepare<'a>(
    registry: &'a Registry,
    validations: &Mutex<Validations>,
    limit: usize,
    input: &Input<'a>,
) -> Timeline<'a> {
    // The memo is a pure cache, so one left behind by a panicking feature is still consistent.
    let mut validations = validations.lock().unwrap_or_else(PoisonError::into_inner);
    validations.begin_pass();
    let timeline = Timeline::prepare(registry, &mut validations, input.document, input.stop_at);
    validations.prune(limit);
    timeline
}

/// What to recompute.
#[derive(Clone, Copy)]
pub struct Input<'a> {
    /// The document (structurally valid, as `partsmith_model` loads and edits keep it).
    pub document: &'a Document,
    /// Resolves dotted names such as `printer.fit.slip` (expressions.md `EvaluationContext`).
    pub context: &'a dyn EvaluationContext,
    /// Stop after this feature (inclusive) instead of at the document's rollback marker. Used for previews.
    pub stop_at: Option<&'a Id>,
    /// Asset bytes by name (the document's asset store).
    pub assets: &'a BTreeMap<String, Arc<[u8]>>,
}

impl Input<'_> {
    /// The evaluation scope of the document's expressions with these evaluated parameters.
    fn scope<'s>(&'s self, parameters: &'s ParameterValues) -> Scope<'s> {
        Scope {
            units: self.document.units,
            parameters,
            context: self.context,
        }
    }
}

impl fmt::Debug for Input<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Input")
            .field("document", &self.document)
            .field("context", &self.context.names())
            .field("stop_at", &self.stop_at)
            .field("assets", &self.assets.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// The value of every parameter of `document`, in document order.
fn parameter_reports(document: &Document, values: &ParameterValues) -> Vec<ParameterReport> {
    document
        .parameters
        .iter()
        .map(|parameter| ParameterReport {
            parameter: parameter.id.clone(),
            value: values
                .get(&parameter.name)
                .expect("Document::evaluate_parameters has a result for every parameter name")
                .clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::test_support::{document, id, input, registry};

    fn recompute() -> Recompute {
        Recompute::new(Arc::new(registry()))
    }

    fn statuses(report: &Report) -> Vec<Status> {
        report
            .features
            .iter()
            .map(|feature| feature.status)
            .collect()
    }

    fn datum(feature: &str, offset: &str) -> serde_json::Value {
        json!({"id": feature, "type": "test_datum", "offset": offset})
    }

    #[test]
    fn a_run_builds_then_reuses() {
        let mut recompute = recompute();
        let document = document(json!([datum("f-1", "w"), datum("f-2", "1")]));
        let assets = BTreeMap::new();

        let first = recompute.run(&input(&document, &assets), &CancelToken::new());
        let second = recompute.run(&input(&document, &assets), &CancelToken::new());

        assert_eq!(statuses(&first), [Status::Ok, Status::Ok]);
        assert!(
            first
                .features
                .iter()
                .all(|feature| !feature.timing.from_cache)
        );
        assert!(
            second
                .features
                .iter()
                .all(|feature| feature.timing.from_cache)
        );
        assert_eq!(recompute.cache_len(), 2);
        assert!(!first.canceled);
    }

    #[test]
    fn parameters_are_reported_in_document_order() {
        let mut recompute = recompute();
        let document = document(json!([]));
        let assets = BTreeMap::new();

        let report = recompute.run(&input(&document, &assets), &CancelToken::new());

        let parameters: Vec<&str> = report
            .parameters
            .iter()
            .map(|parameter| parameter.parameter.as_str())
            .collect();
        assert_eq!(parameters, ["p-h", "p-w"]);
        assert_eq!(
            report.parameters[1].value,
            Ok(partsmith_model::Quantity::length(3.0))
        );
    }

    #[test]
    fn an_invalid_record_reports_its_validation_error() {
        let mut recompute = recompute();
        let document = document(json!([{"id": "f-g", "type": "gear_demo"}]));
        let assets = BTreeMap::new();

        let report = recompute.run(&input(&document, &assets), &CancelToken::new());

        let feature = &report.features[0];
        assert_eq!(feature.status, Status::Error);
        assert_eq!(feature.key, None);
        assert_eq!(feature.messages[0].code, "unknown_type");
    }

    #[test]
    fn a_canceled_token_runs_nothing() {
        let mut recompute = recompute();
        let document = document(json!([datum("f-1", "1")]));
        let assets = BTreeMap::new();
        let cancel = CancelToken::new();
        cancel.cancel();

        let report = recompute.run(&input(&document, &assets), &cancel);

        assert_eq!(statuses(&report), [Status::NotRun]);
        assert!(report.canceled);
        assert_eq!(recompute.cache_len(), 0);
    }

    #[test]
    fn a_panic_is_isolated_and_never_cached() {
        let mut recompute = recompute();
        let document = document(json!([{"id": "f-p", "type": "test_panic"}, datum("f-1", "1")]));
        let assets = BTreeMap::new();

        let report = recompute.run(&input(&document, &assets), &CancelToken::new());

        assert_eq!(statuses(&report), [Status::Error, Status::Ok]);
        assert_eq!(
            report.features[0].error,
            Some(FeatureError::Internal {
                message: "boom".to_string()
            })
        );
        assert_eq!(recompute.cache_len(), 1);
    }

    #[test]
    fn unknown_keys_add_a_warning_after_the_output_warnings() {
        let mut recompute = recompute();
        let document = document(json!([
            {"id": "f-1", "type": "test_datum", "offset": "1", "zoom": 2}
        ]));
        let assets = BTreeMap::new();

        let report = recompute.run(&input(&document, &assets), &CancelToken::new());

        let feature = &report.features[0];
        assert_eq!(feature.status, Status::Warning);
        assert_eq!(
            feature.warnings,
            [FeatureWarning::UnknownParams {
                keys: vec!["zoom".to_string()]
            }]
        );
        assert_eq!(feature.messages[0].severity, Severity::Warning);
    }

    #[test]
    fn writers_provide_their_bodies_and_body_reports_name_them() {
        let mut recompute = recompute();
        let document = document(json!([
            {"id": "f-w", "type": "test_writer", "body": "b-2"},
            {"id": "f-r", "type": "test_reader", "body": "b-2"}
        ]));
        let assets = BTreeMap::new();

        let report = recompute.run(&input(&document, &assets), &CancelToken::new());

        let bodies: Vec<(&str, Option<&str>)> = report
            .bodies
            .iter()
            .map(|body| {
                (
                    body.body.as_str(),
                    body.last_feature.as_ref().map(Id::as_str),
                )
            })
            .collect();
        assert_eq!(bodies, [("b-1", None), ("b-2", Some("f-w")), ("b-3", None)]);
        assert_eq!(report.bodies[1].key, report.features[0].key);
        assert_eq!(report.features[1].bodies_before.len(), 1);
    }

    #[test]
    fn plans_and_graphs_leave_the_cache_alone() {
        let mut recompute = recompute();
        let other = document(json!([datum("f-9", "9")]));
        let document = document(json!([datum("f-1", "1")]));
        let assets = BTreeMap::new();
        recompute.run(&input(&document, &assets), &CancelToken::new());

        let plan = recompute.plan(&input(&other, &assets));
        let graph = recompute.graph(&input(&other, &assets));

        assert_eq!(plan.steps[0].action, Action::Build);
        assert_eq!(graph.nodes().len(), 1);
        assert_eq!(recompute.cache_len(), 1);
    }

    #[test]
    fn a_stop_point_rolls_back_the_later_records() {
        let mut recompute = recompute();
        let document = document(json!([datum("f-1", "1"), datum("f-2", "2")]));
        let assets = BTreeMap::new();
        let stop = id("f-1");
        let input = Input {
            stop_at: Some(&stop),
            ..input(&document, &assets)
        };

        let report = recompute.run(&input, &CancelToken::new());

        assert_eq!(statuses(&report), [Status::Ok, Status::RolledBack]);
        assert!(report.messages.is_empty());
    }

    #[test]
    fn a_feature_change_is_queried_in_the_graph_where_the_feature_is_active() {
        let recompute = recompute();
        let writer = |feature: &str, suppressed: bool| json!({"id": feature, "type": "test_writer", "suppressed": suppressed, "body": "b-1"});
        let reader = json!({"id": "f-3", "type": "test_reader", "body": "b-1"});
        let before = document(json!([writer("f-1", false), writer("f-2", true), reader]));
        let after = document(json!([writer("f-1", false), writer("f-2", false), reader]));
        let assets = BTreeMap::new();
        let change = [Change::Feature(id("f-2"))];

        let graph_before = recompute.graph(&input(&before, &assets));
        let graph_after = recompute.graph(&input(&after, &assets));

        assert!(graph_before.affected_by(&change).is_empty());
        assert_eq!(graph_after.affected_by(&change), [id("f-2"), id("f-3")]);
    }

    #[test]
    fn clear_cache_empties_the_cache() {
        let mut recompute = recompute();
        let document = document(json!([datum("f-1", "1")]));
        let assets = BTreeMap::new();
        recompute.run(&input(&document, &assets), &CancelToken::new());

        recompute.clear_cache();

        assert_eq!(recompute.cache_len(), 0);
    }

    #[test]
    fn recompute_can_be_shared_between_threads() {
        fn assert_send_and_sync<T: Send + Sync>() {}

        assert_send_and_sync::<Recompute>();
    }

    #[test]
    fn debug_output_names_the_cache_size() {
        let recompute = Recompute::with_cache_limit(Arc::new(registry()), 7);

        let text = format!("{recompute:?}");

        assert!(text.contains("cache_limit: 7"), "{text}");
    }
}
