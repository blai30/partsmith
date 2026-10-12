//! Fixtures and helpers shared by the recompute acceptance tests (recompute.md section 6, the body
//! semantics tests of features.md section 6.4 and the naming conformance tests 81 to 90 of
//! naming.md).
//!
//! Documents are written as JSON and parsed with `partsmith_model::load`, as features.md section 6.1
//! describes. A [`Harness`] owns one `Recompute` plus the evaluation context and asset map of its
//! runs.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

pub mod features;

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    BodyState, BodyUpdate, CancelToken, DatumResult, FeatureOutput, OutputData, Registry,
};
use partsmith_model::{Document, ExprError, Id, Quantity};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_occt_sys::{ElementGeometry, Shape, SubShapeKind};
use partsmith_recompute::{
    Action, BodyReport, CacheKey, EdgeKind, FeatureReport, Graph, Input, Message, Node, NodeState,
    Plan, PlanStep, Recompute, Report, Severity, Status,
};
use partsmith_sketch::Placement;
use serde_json::{Value, json};

/// Relative tolerance for volumes and areas (features.md section 6.1).
pub const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance in millimeters per bounding box coordinate (features.md section 6.1).
pub const BOUNDS_TOLERANCE: f64 = 1e-5;

/// Absolute tolerance for placements and directions (features.md section 6.1).
pub const PLACEMENT_TOLERANCE: f64 = 1e-9;

// Ids and documents.

/// A test id.
pub fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|error| panic!("'{text}' is not a valid id: {error}"))
}

/// The document skeleton of features.md section 6.1 with `members` replacing or adding top-level
/// members (for example `parameters`, `bodies`, `features`, `units`, `rollback`).
pub fn document_json(members: Value) -> Value {
    let mut document = json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "parameters": [],
        "bodies": [],
        "features": []
    });
    let members = members
        .as_object()
        .expect("document members are a JSON object");
    for (key, value) in members {
        document[key] = value.clone();
    }
    document
}

/// Parses a document from its JSON value, as features.md section 6.1 runs documents.
pub fn load(document: &Value) -> Document {
    let text = document.to_string();
    match partsmith_model::load(&text) {
        Ok(loaded) => loaded.document,
        Err(error) => panic!("the test document does not load: {error}\n{text}"),
    }
}

/// Parses the skeleton with `members` (see [`document_json`]).
pub fn document(members: Value) -> Document {
    load(&document_json(members))
}

/// Bodies `b-1` to `b-<count>`, named `Body 1` to `Body <count>` (features.md section 6.1).
pub fn bodies(count: usize) -> Value {
    Value::Array(
        (1..=count)
            .map(|number| json!({"id": format!("b-{number}"), "name": format!("Body {number}")}))
            .collect(),
    )
}

/// A parameter record.
pub fn parameter(parameter_id: &str, name: &str, expr: &str) -> Value {
    json!({"id": parameter_id, "name": name, "expr": expr})
}

/// `RECT(x0, y0, x1, y1)` of features.md section 6.1 with entity ids `e-<first>` to
/// `e-<first + 3>`: four lines drawn counterclockwise from the lower-left corner.
pub fn rectangle(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    Value::Array(
        (0..4)
            .map(|i| {
                json!({
                    "id": format!("e-{}", first + i),
                    "type": "line",
                    "start": corners[i],
                    "end": corners[(i + 1) % 4]
                })
            })
            .collect(),
    )
}

/// The region ref of [`rectangle`]: key `e-<first>,...,e-<first + 3>` and fingerprint
/// `c=<center x>,<center y>|a=<area>`.
pub fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let key = (first..first + 4)
        .map(|number| format!("e-{number}"))
        .collect::<Vec<_>>()
        .join(",");
    let center_x = (x0 + x1) / 2.0;
    let center_y = (y0 + y1) / 2.0;
    let area = (x1 - x0) * (y1 - y0);
    // Rust's f64 Display matches the region fingerprint text for these round values (sketch.md).
    json!({"key": key, "fp": format!("c={center_x},{center_y}|a={area}")})
}

/// A sketch record on an origin plane with these entities.
pub fn sketch(feature: &str, plane: &str, entities: Value) -> Value {
    json!({
        "id": feature,
        "type": "sketch",
        "plane": {"kind": "origin", "plane": plane},
        "entities": entities
    })
}

/// `DOC-A` of recompute.md section 6.
pub fn doc_a_json() -> Value {
    document_json(json!({
        "parameters": [parameter("p-h", "h", "10 mm"), parameter("p-w", "w", "5 mm")],
        "bodies": bodies(2),
        "features": [
            sketch("f-sk1", "XY", rectangle(1, 0.0, 0.0, 80.0, 50.0)),
            {
                "id": "f-ext1", "type": "extrude", "body": "b-1",
                "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
                "extent": {"kind": "distance", "expr": "h"}, "op": "new"
            },
            {
                "id": "f-box2", "type": "box", "body": "b-2",
                "size": ["10", "10", "10"], "position": ["100", "0", "0"], "op": "new"
            },
            {
                "id": "f-fil1", "type": "fillet", "body": "b-1",
                "edges": [{"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=40,0,10|l=80"}],
                "radius": "1"
            },
            {
                "id": "f-dp1", "type": "datum_plane",
                "base": {"kind": "origin", "plane": "XY"}, "offset": "w"
            }
        ]
    }))
}

/// `DOC-A` as a document.
pub fn doc_a() -> Document {
    load(&doc_a_json())
}

/// A chain of test features: datums f-d1 (at 5) and f-d2 (on f-d1, at `h`); blocks f-a (new, 2)
/// and f-b (join, `h`) on b-1; f-c (new, 3) on b-2; a probe f-p of b-1.
pub fn chain_json() -> Value {
    document_json(json!({
        "parameters": [parameter("p-h", "h", "2 mm")],
        "bodies": bodies(2),
        "features": [
            test_datum("f-d1", "5"),
            test_datum_on("f-d2", "f-d1", "h"),
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-b", "b-1", "h", "join"),
            test_block("f-c", "b-2", "3", "new"),
            test_probe("f-p", "b-1")
        ]
    }))
}

/// The feature records of a document JSON, for editing.
pub fn records(document: &mut Value) -> &mut Vec<Value> {
    document["features"]
        .as_array_mut()
        .expect("the document has a features array")
}

/// The feature record with this id in a document JSON, for editing.
pub fn record<'d>(document: &'d mut Value, feature: &str) -> &'d mut Value {
    records(document)
        .iter_mut()
        .find(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has no feature {feature}"))
}

/// The position of a feature record in a document JSON.
pub fn record_position(document: &mut Value, feature: &str) -> usize {
    records(document)
        .iter()
        .position(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has no feature {feature}"))
}

/// The body record with this id in a document JSON, for editing.
pub fn body_record<'d>(document: &'d mut Value, body: &str) -> &'d mut Value {
    document["bodies"]
        .as_array_mut()
        .expect("the document has a bodies array")
        .iter_mut()
        .find(|record| record["id"] == body)
        .unwrap_or_else(|| panic!("the document has no body {body}"))
}

/// The parameter record with this id in a document JSON, for editing.
pub fn parameter_record<'d>(document: &'d mut Value, parameter: &str) -> &'d mut Value {
    document["parameters"]
        .as_array_mut()
        .expect("the document has a parameters array")
        .iter_mut()
        .find(|record| record["id"] == parameter)
        .unwrap_or_else(|| panic!("the document has no parameter {parameter}"))
}

/// A `test_datum` record without a base.
pub fn test_datum(feature: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "test_datum", "offset": offset})
}

/// A `test_datum` record offset from `base`.
pub fn test_datum_on(feature: &str, base: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "test_datum", "base": base, "offset": offset})
}

/// A `test_block` record; `op` is "new" or "join".
pub fn test_block(feature: &str, body: &str, size: &str, op: &str) -> Value {
    json!({"id": feature, "type": "test_block", "body": body, "size": size, "op": op})
}

/// A `test_probe` record.
pub fn test_probe(feature: &str, body: &str) -> Value {
    json!({"id": feature, "type": "test_probe", "body": body})
}

/// A `test_node` record; empty lists are omitted.
pub fn test_node(
    feature: &str,
    uses: &[&str],
    reads: &[&str],
    writes: &[&str],
    values: &[&str],
) -> Value {
    let mut record = json!({"id": feature, "type": "test_node"});
    for (key, list) in [
        ("uses", uses),
        ("reads", reads),
        ("writes", writes),
        ("values", values),
    ] {
        if !list.is_empty() {
            record[key] = json!(list);
        }
    }
    record
}

/// A record of one of the body-only test features (`test_peek`, `test_stray`, `test_twice`).
pub fn test_on_body(feature: &str, kind: &str, body: &str) -> Value {
    json!({"id": feature, "type": kind, "body": body})
}

/// The record with `"suppressed": true`.
pub fn suppressed(mut record: Value) -> Value {
    record["suppressed"] = json!(true);
    record
}

// Registries and runs.

/// `Registry::with_builtins()` plus the four test features of recompute.md section 6.
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::with_builtins();
    features::register_spec_features(&mut registry);
    registry
}

/// An empty registry plus every test feature (no built-in feature types).
pub fn test_registry() -> Registry {
    let mut registry = Registry::new();
    features::register_spec_features(&mut registry);
    features::register_runner_features(&mut registry);
    registry
}

/// One `Recompute` with the evaluation context and asset map its runs use (both empty unless a
/// test fills them).
pub struct Harness {
    /// The recompute under test.
    pub recompute: Recompute,
    /// Dotted names for `Input::context`.
    pub context: BTreeMap<String, Quantity>,
    /// Asset bytes for `Input::assets`.
    pub assets: BTreeMap<String, Arc<[u8]>>,
}

impl Harness {
    /// A harness over `registry` with the default cache size.
    pub fn new(registry: Registry) -> Harness {
        Harness::from_recompute(Recompute::new(Arc::new(registry)))
    }

    /// A harness over `registry` that keeps at most `max_entries` results not used by the latest run.
    pub fn with_cache_limit(registry: Registry, max_entries: usize) -> Harness {
        Harness::from_recompute(Recompute::with_cache_limit(Arc::new(registry), max_entries))
    }

    /// A harness over the built-in registry plus the spec's test features.
    pub fn builtins() -> Harness {
        Harness::new(builtin_registry())
    }

    /// A harness over the test features only.
    pub fn test_features() -> Harness {
        Harness::new(test_registry())
    }

    fn from_recompute(recompute: Recompute) -> Harness {
        Harness {
            recompute,
            context: BTreeMap::new(),
            assets: BTreeMap::new(),
        }
    }

    /// Runs the whole timeline (up to the document's rollback marker) with a new token.
    pub fn run(&mut self, document: &Document) -> Report {
        self.run_with(document, None, &CancelToken::new())
    }

    /// Runs up to and including `stop_at` with a new token.
    pub fn run_until(&mut self, document: &Document, stop_at: &str) -> Report {
        let stop_at = id(stop_at);
        self.run_with(document, Some(&stop_at), &CancelToken::new())
    }

    /// Runs with an explicit stop point and token.
    pub fn run_with(
        &mut self,
        document: &Document,
        stop_at: Option<&Id>,
        cancel: &CancelToken,
    ) -> Report {
        let input = Input {
            document,
            context: &self.context,
            stop_at,
            assets: &self.assets,
        };
        self.recompute.run(&input, cancel)
    }

    /// The plan of the whole timeline.
    pub fn plan(&self, document: &Document) -> Plan {
        self.recompute.plan(&self.input(document, None))
    }

    /// The plan up to and including `stop_at`.
    pub fn plan_until(&self, document: &Document, stop_at: &str) -> Plan {
        let stop_at = id(stop_at);
        self.recompute.plan(&self.input(document, Some(&stop_at)))
    }

    /// The dependency graph of the whole timeline.
    pub fn graph(&self, document: &Document) -> Graph {
        self.recompute.graph(&self.input(document, None))
    }

    /// The dependency graph with a stop point.
    pub fn graph_until(&self, document: &Document, stop_at: &str) -> Graph {
        let stop_at = id(stop_at);
        self.recompute.graph(&self.input(document, Some(&stop_at)))
    }

    /// Number of cached results.
    pub fn cache_len(&self) -> usize {
        self.recompute.cache_len()
    }

    fn input<'a>(&'a self, document: &'a Document, stop_at: Option<&'a Id>) -> Input<'a> {
        Input {
            document,
            context: &self.context,
            stop_at,
            assets: &self.assets,
        }
    }
}

// Reading reports, plans and graphs.

/// The report of one feature.
pub fn feature<'r>(report: &'r Report, feature: &str) -> &'r FeatureReport {
    report
        .features
        .iter()
        .find(|entry| entry.feature.as_str() == feature)
        .unwrap_or_else(|| panic!("the report has no feature {feature}"))
}

/// The end-of-run report of one body.
pub fn body<'r>(report: &'r Report, body: &str) -> &'r BodyReport {
    report
        .bodies
        .iter()
        .find(|entry| entry.body.as_str() == body)
        .unwrap_or_else(|| panic!("the report has no body {body}"))
}

/// The end-of-run state of one body.
pub fn final_state<'r>(report: &'r Report, body_id: &str) -> &'r BodyState {
    &body(report, body_id).state
}

/// The state of `body_id` just before `feature_id` (from its `bodies_before`).
pub fn state_before<'r>(report: &'r Report, feature_id: &str, body_id: &str) -> &'r BodyState {
    feature(report, feature_id)
        .bodies_before
        .iter()
        .find(|(body, _)| body.as_str() == body_id)
        .map(|(_, state)| state)
        .unwrap_or_else(|| panic!("{feature_id} has no state of {body_id} before it"))
}

/// The ids in a feature's `bodies_before`, in order.
pub fn bodies_before_ids(report: &Report, feature_id: &str) -> Vec<String> {
    feature(report, feature_id)
        .bodies_before
        .iter()
        .map(|(body, _)| body.to_string())
        .collect()
}

/// The output of a feature that ended `Ok` or `Warning`.
pub fn output<'r>(report: &'r Report, feature_id: &str) -> &'r FeatureOutput {
    feature(report, feature_id)
        .output
        .as_deref()
        .unwrap_or_else(|| panic!("{feature_id} has no output"))
}

/// The body update for `body_id` in a feature's output.
pub fn output_update<'r>(report: &'r Report, feature_id: &str, body_id: &str) -> &'r BodyUpdate {
    match &output(report, feature_id).data {
        OutputData::Bodies(updates) => updates
            .iter()
            .find(|update| update.body.as_str() == body_id)
            .unwrap_or_else(|| panic!("{feature_id} has no update of {body_id}")),
        other => panic!("{feature_id} output is not a body update: {other:?}"),
    }
}

/// The placement of a datum output.
pub fn datum_placement<'r>(report: &'r Report, feature_id: &str) -> &'r Placement {
    match &output(report, feature_id).data {
        OutputData::Datum(DatumResult::Plane(placement)) => placement,
        other => panic!("{feature_id} output is not a datum: {other:?}"),
    }
}

/// The height of a datum output's origin (test features place their datums at (0, 0, z)).
pub fn datum_height(report: &Report, feature_id: &str) -> f64 {
    datum_placement(report, feature_id).origin[2]
}

/// The reported value of one parameter.
pub fn parameter_value<'r>(
    report: &'r Report,
    parameter_id: &str,
) -> &'r Result<Quantity, ExprError> {
    &report
        .parameters
        .iter()
        .find(|entry| entry.parameter.as_str() == parameter_id)
        .unwrap_or_else(|| panic!("the report has no parameter {parameter_id}"))
        .value
}

/// The snake_case name of a status.
pub fn status_name(status: &Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Warning => "warning",
        Status::Error => "error",
        Status::Suppressed => "suppressed",
        Status::RolledBack => "rolled_back",
        Status::NotRun => "not_run",
    }
}

/// The status of one feature, as a snake_case name.
pub fn status(report: &Report, feature_id: &str) -> &'static str {
    status_name(&feature(report, feature_id).status)
}

/// Every feature with its status, in report order.
pub fn statuses(report: &Report) -> Vec<(String, &'static str)> {
    report
        .features
        .iter()
        .map(|entry| (entry.feature.to_string(), status_name(&entry.status)))
        .collect()
}

/// The error code of a feature that failed.
pub fn error_code(report: &Report, feature_id: &str) -> &'static str {
    feature(report, feature_id)
        .error
        .as_ref()
        .unwrap_or_else(|| panic!("{feature_id} has no error"))
        .code()
}

/// The warning codes of a feature, in order.
pub fn warning_codes(report: &Report, feature_id: &str) -> Vec<&'static str> {
    feature(report, feature_id)
        .warnings
        .iter()
        .map(|warning| warning.code())
        .collect()
}

/// The rendered messages of a feature.
pub fn messages<'r>(report: &'r Report, feature_id: &str) -> &'r [Message] {
    &feature(report, feature_id).messages
}

/// The first rendered message of a feature.
pub fn first_message<'r>(report: &'r Report, feature_id: &str) -> &'r Message {
    messages(report, feature_id)
        .first()
        .unwrap_or_else(|| panic!("{feature_id} has no message"))
}

/// The rendered message texts of a feature.
pub fn message_texts(report: &Report, feature_id: &str) -> Vec<String> {
    messages(report, feature_id)
        .iter()
        .map(|message| message.text.clone())
        .collect()
}

/// The snake_case name of a severity.
pub fn severity_name(severity: &Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

/// The cache key of a feature.
pub fn key(report: &Report, feature_id: &str) -> Option<CacheKey> {
    feature(report, feature_id).key
}

/// Every feature with its key, in report order.
pub fn keys(report: &Report) -> Vec<(String, Option<CacheKey>)> {
    report
        .features
        .iter()
        .map(|entry| (entry.feature.to_string(), entry.key))
        .collect()
}

/// The features whose result came from the cache, in timeline order.
pub fn reused(report: &Report) -> Vec<String> {
    report
        .features
        .iter()
        .filter(|entry| entry.timing.from_cache)
        .map(|entry| entry.feature.to_string())
        .collect()
}

/// The features whose result did not come from the cache, in timeline order.
pub fn not_reused(report: &Report) -> Vec<String> {
    report
        .features
        .iter()
        .filter(|entry| !entry.timing.from_cache)
        .map(|entry| entry.feature.to_string())
        .collect()
}

/// The ids of every feature in a report, in order.
pub fn all_features(report: &Report) -> Vec<String> {
    report
        .features
        .iter()
        .map(|entry| entry.feature.to_string())
        .collect()
}

/// The snake_case name of a plan action.
pub fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Build => "build",
        Action::Reuse => "reuse",
        Action::Fail => "fail",
        Action::Suppressed => "suppressed",
        Action::RolledBack => "rolled_back",
    }
}

/// The plan step of one feature.
pub fn step<'p>(plan: &'p Plan, feature_id: &str) -> &'p PlanStep {
    plan.steps
        .iter()
        .find(|step| step.feature.as_str() == feature_id)
        .unwrap_or_else(|| panic!("the plan has no step for {feature_id}"))
}

/// The action of one feature in a plan, as a snake_case name.
pub fn action(plan: &Plan, feature_id: &str) -> &'static str {
    action_name(&step(plan, feature_id).action)
}

/// Every feature with its predicted action, in plan order.
pub fn actions(plan: &Plan) -> Vec<(String, &'static str)> {
    plan.steps
        .iter()
        .map(|step| (step.feature.to_string(), action_name(&step.action)))
        .collect()
}

/// The features with this predicted action, in plan order.
pub fn with_action(plan: &Plan, action: &str) -> Vec<String> {
    plan.steps
        .iter()
        .filter(|step| action_name(&step.action) == action)
        .map(|step| step.feature.to_string())
        .collect()
}

/// The snake_case name of a node state.
pub fn state_name(state: &NodeState) -> &'static str {
    match state {
        NodeState::Active => "active",
        NodeState::Suppressed => "suppressed",
        NodeState::RolledBack => "rolled_back",
        NodeState::Invalid => "invalid",
    }
}

/// The node of one feature.
pub fn graph_node<'g>(graph: &'g Graph, feature_id: &str) -> &'g Node {
    graph
        .node(&id(feature_id))
        .unwrap_or_else(|| panic!("the graph has no node {feature_id}"))
}

/// The upstream edges of a node, written `output <feature>` or `body <body> <feature>`.
pub fn upstream(graph: &Graph, feature_id: &str) -> Vec<String> {
    graph_node(graph, feature_id)
        .upstream
        .iter()
        .map(|edge| match &edge.via {
            EdgeKind::Output => format!("output {}", edge.feature),
            EdgeKind::Body(body) => format!("body {body} {}", edge.feature),
        })
        .collect()
}

/// Ids as strings.
pub fn strings(ids: &[Id]) -> Vec<String> {
    ids.iter().map(Id::to_string).collect()
}

/// Strings from string slices, for comparisons.
pub fn owned(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| text.to_string()).collect()
}

/// A key as 64 lowercase hex characters, computed from its bytes.
pub fn hex(key: &CacheKey) -> String {
    key.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

// Measurements.

/// The shape of a body that holds a solid.
pub fn shape(state: &BodyState) -> &Shape {
    state.shape().expect("the body holds a solid")
}

/// The volume of a body that holds a solid.
pub fn volume(state: &BodyState) -> f64 {
    shape(state)
        .volume()
        .expect("the kernel measures the volume")
}

/// Number of faces of a body that holds a solid.
pub fn face_count(state: &BodyState) -> u32 {
    shape(state).count(SubShapeKind::Face)
}

/// Number of solids of a body that holds a solid.
pub fn solid_count(state: &BodyState) -> usize {
    shape(state)
        .solids()
        .expect("the kernel lists the solids")
        .len()
}

/// Asserts `actual` is within `MEASURE_TOLERANCE` of `expected`, relatively.
pub fn assert_relative(actual: f64, expected: f64, what: &str) {
    let difference = (actual - expected).abs();
    assert!(
        difference <= MEASURE_TOLERANCE * expected.abs().max(1.0),
        "{what}: expected {expected}, got {actual}"
    );
}

/// Asserts `actual` is within `tolerance` of `expected`.
pub fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected}, got {actual}"
    );
}

/// Asserts the volume of a body that holds a solid.
pub fn assert_volume(state: &BodyState, expected: f64) {
    assert_relative(volume(state), expected, "volume");
}

/// Asserts the tight bounding box of a body that holds a solid.
pub fn assert_bounds(state: &BodyState, minimum: [f64; 3], maximum: [f64; 3]) {
    let bounds = shape(state)
        .bounding_box()
        .expect("the kernel measures the bounding box");
    for axis in 0..3 {
        assert_close(
            bounds.min[axis],
            minimum[axis],
            BOUNDS_TOLERANCE,
            "bounding box minimum",
        );
        assert_close(
            bounds.max[axis],
            maximum[axis],
            BOUNDS_TOLERANCE,
            "bounding box maximum",
        );
    }
}

/// Asserts two vectors or points are equal within `tolerance` per component.
pub fn assert_vector(actual: [f64; 3], expected: [f64; 3], tolerance: f64, what: &str) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() <= tolerance,
            "{what}: expected {expected:?}, got {actual:?}"
        );
    }
}

/// Names of every element of `kind` of a body state, in index order.
pub fn element_names(state: &BodyState, kind: ElementKind) -> Vec<String> {
    state
        .naming()
        .names(kind)
        .map(|(_, name)| name.as_str().to_string())
        .collect()
}

/// Face names of a body state, sorted.
pub fn face_names(state: &BodyState) -> Vec<String> {
    let mut names = element_names(state, ElementKind::Face);
    names.sort();
    names
}

/// Every face, edge and vertex name of a body state, in index order per kind.
pub fn all_element_names(state: &BodyState) -> Vec<String> {
    [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex]
        .into_iter()
        .flat_map(|kind| element_names(state, kind))
        .collect()
}

/// Asserts that a body state has an element named `name` of the name's kind.
pub fn assert_has_element(state: &BodyState, name: &str) {
    let element = ElementName::parse(name).expect("the expected name is valid");
    assert!(
        state.naming().index(&element).is_some(),
        "no element named {name}; faces are {:?}",
        face_names(state)
    );
}

/// The kernel geometry of the face named `name`.
pub fn face_geometry(state: &BodyState, name: &str) -> ElementGeometry {
    let element = ElementName::parse(name).expect("the expected name is valid");
    let index = state
        .naming()
        .index(&element)
        .unwrap_or_else(|| panic!("no face named {name}; faces are {:?}", face_names(state)));
    shape(state)
        .element(SubShapeKind::Face, index)
        .expect("the kernel describes the face")
}

/// The outward normal of a planar face.
pub fn face_normal(state: &BodyState, name: &str) -> [f64; 3] {
    let direction = face_geometry(state, name)
        .direction
        .unwrap_or_else(|| panic!("face {name} has no direction"));
    [direction.x, direction.y, direction.z]
}

/// The area centroid of a face.
pub fn face_centroid(state: &BodyState, name: &str) -> [f64; 3] {
    let centroid = face_geometry(state, name).centroid;
    [centroid.x, centroid.y, centroid.z]
}
