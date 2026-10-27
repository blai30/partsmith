//! Document-running helpers shared by the feature acceptance tests (features.md section 6.1).
//!
//! A test builds a document (from an example under `examples/` or from JSON records), runs it with
//! `Recompute` and the built-in registry, and inspects the `Report`: statuses, rendered messages,
//! and the kernel measurements of body shapes. The records, documents and motion checks of the
//! frames on the model (ADR 0023) are in `frames.rs`, and the records that a feature's document
//! tests share with its release-only budgets (`budgets.rs`) in a module named after the feature.

// The test targets `integration` and `budgets` and the bench each use only some of these helpers.
#![allow(dead_code)]

pub mod fillet;
mod frames;
pub mod hole;
pub mod import;
pub mod linear_pattern;
pub mod shell;

// The test targets `integration` and `budgets` and the bench each use only some of these helpers.
#[allow(unused_imports)]
pub use frames::*;
#[allow(unused_imports)]
pub use import::box_step_asset;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use partsmith_features::{
    BodyState, BodyUpdate, BuildContext, BuildInputs, CancelToken, DatumResult, Dependencies,
    Diagnosis, DocumentNames, FallbackCause, FeatureError, FeatureOutput, FeatureWarning, FieldRef,
    Fix, IgnoredReason, Limit, Link, Loss, Names, OutputData, Placed, Registry, RenderContext,
    Repair, Severity, SketchResult, Values,
};
use partsmith_message_lint::lint_failures;
use partsmith_model::{Document, EmptyContext, FeatureRecord, FieldUnit, Id, Scope};
use partsmith_naming::{ElementKind, ElementName, Follow, Motion, Reference};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Point, Shape, SubShapeKind, Vector};
use partsmith_recompute::{BodyReport, FeatureReport, Input, Message, Recompute, Report, Status};
use partsmith_sketch::Placement;
use serde_json::Value;

/// Relative tolerance for volumes and areas (features.md section 6.1).
pub const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance in millimeters for bounding box coordinates, and for face positions and radii
/// (features.md section 6.1 gives 1e-5 mm per bounding box coordinate).
pub const LENGTH_TOLERANCE: f64 = 1e-5;

/// Absolute tolerance for placements and axis directions (features.md section 6.1).
pub const AXIS_TOLERANCE: f64 = 1e-9;

/// `CUBE20`: a 20 mm cube in `b-1` spanning [-10,-10,0]-[10,10,20].
pub const CUBE20: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}"#;

/// `E-FT`: the edge between the front and top faces of `CUBE20`.
pub const FRONT_TOP_EDGE: &str =
    r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#;

/// The 20 x 20 x 2.4 mm plate of the dress-up "too large" tests.
pub const PLATE: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}"#;

/// The top loop of `PLATE`: `E-FT`, `E-BT`, `E-LT` and `E-RT` with the plate's fingerprints.
pub const PLATE_TOP_EDGES: [&str; 4] = [
    r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-10,0,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=10,0,2.4|l=20"}"#,
];

/// `STEP` of fillet.md tests 19 to 22 and chamfer.md tests 17 and 18 (newcomer error corpus NC05): a 40 x 20 x 10 box
/// with a step 2.4 mm deep over x 0..20 whose riser `f-box2/left` lies at x = 0 (volume 7040).
pub const STEP: [&str; 2] = [
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}"#,
    r#"{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "30", "5"], "position": ["10", "0", "7.6"], "op": "cut"}"#,
];

/// `E-RISER`: the edge between the top of 'Box 1' and the riser of `STEP`.
pub const RISER_EDGE: &str =
    r#"{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}"#;

/// The bottom left edge of `STEP`, which the riser does not limit.
pub const STEP_BOTTOM_LEFT_EDGE: &str =
    r#"{"q": "edge(f-box1/bottom|f-box1/left)", "fp": "line|n=0,1,0|c=-20,0,0|l=20|adj=2,2"}"#;

/// The sentence of NC05 that names the riser of `STEP` as the limit.
pub const RISER_SENTENCE: &str = "The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.";

/// `BOX-BELOW`: a box in `b-1` spanning [-20,-20,-10]-[20,20,0], volume 16000.
pub const BOX_BELOW: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}"#;

/// The `examples/` folder at the repository root.
pub fn examples_folder() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples"))
}

/// Loads `examples/<relative_path>` as a document.
pub fn load_example(relative_path: &str) -> Document {
    let path = examples_folder().join(relative_path);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read example {}: {error}", path.display()));
    parse_document(&text)
}

/// Parses `.ptsm` text into a document.
pub fn parse_document(text: &str) -> Document {
    match partsmith_model::load(text) {
        Ok(loaded) => loaded.document,
        Err(error) => panic!("the test document does not load: {error}\n{text}"),
    }
}

/// The canonical saved text of a document.
pub fn saved(document: &Document) -> String {
    partsmith_model::save(document)
        .unwrap_or_else(|error| panic!("the document does not save: {error}"))
}

/// An id written in a test.
pub fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|error| panic!("'{text}' is not a valid id: {error}"))
}

/// The JSON of a test body: `b-<n>` is named `Body <n>` (features.md section 6.1).
pub fn body_json(body: &str) -> String {
    let number = body
        .strip_prefix("b-")
        .unwrap_or_else(|| panic!("test body ids look like b-1, not {body}"));
    format!(r#"{{"id": "{body}", "name": "Body {number}"}}"#)
}

/// The document skeleton of features.md section 6.1 in `units`, with these parameter records,
/// test bodies (by id) and feature records (JSON objects).
pub fn document_text(
    units: &str,
    parameters: &[&str],
    bodies: &[&str],
    features: &[&str],
) -> String {
    let bodies: Vec<String> = bodies.iter().map(|body| body_json(body)).collect();
    format!(
        r#"{{"format": "partsmith", "format_version": 2, "units": "{units}", "parameters": [{}], "bodies": [{}], "features": [{}]}}"#,
        parameters.join(", "),
        bodies.join(", "),
        features.join(", "),
    )
}

/// A millimeter document without parameters, with these test bodies (by id) and feature records.
pub fn document(bodies: &[&str], features: &[&str]) -> Document {
    parse_document(&document_text("mm", &[], bodies, features))
}

/// Asset bytes by name, as `Input::assets` takes them.
pub type Assets = BTreeMap<String, Arc<[u8]>>;

/// Runs `document` on `recompute` with an empty evaluation context, these asset bytes and a new
/// cancel token (features.md section 6.1).
pub fn run_document(recompute: &mut Recompute, document: &Document, assets: &Assets) -> Report {
    let input = Input {
        document,
        context: &EmptyContext,
        stop_at: None,
        assets,
    };
    recompute.run(&input, &CancelToken::new())
}

/// A document run once through `Recompute` with the built-in registry.
pub struct Run {
    /// The document that was run.
    pub document: Document,
    /// The recompute that ran it (its registry answers dependency questions).
    pub recompute: Recompute,
    /// The report of the run.
    pub report: Report,
}

impl Run {
    /// Runs `document` with `Registry::with_builtins()`, an empty evaluation context, no assets and
    /// a new cancel token (features.md section 6.1).
    pub fn new(document: Document) -> Run {
        Run::with_assets(document, &Assets::new())
    }

    /// Runs `document` as `Run::new` does, with these asset bytes.
    pub fn with_assets(document: Document, assets: &Assets) -> Run {
        let mut recompute = Recompute::new(Arc::new(Registry::with_builtins()));
        let report = run_document(&mut recompute, &document, assets);
        Run {
            document,
            recompute,
            report,
        }
    }

    /// Runs the document again on the same recompute, so its cached results are reused, with these
    /// asset bytes. The new report replaces the old one.
    pub fn rerun(&mut self, assets: &Assets) {
        self.report = run_document(&mut self.recompute, &self.document, assets);
    }

    /// Loads and runs `examples/<relative_path>`.
    pub fn example(relative_path: &str) -> Run {
        Run::new(load_example(relative_path))
    }

    /// Runs a millimeter document with these test bodies (by id) and feature records.
    pub fn records(bodies: &[&str], features: &[&str]) -> Run {
        Run::new(document(bodies, features))
    }

    /// The record of `feature` in the document.
    pub fn record(&self, feature: &str) -> &FeatureRecord {
        self.document
            .features
            .iter()
            .find(|record| record.id.as_str() == feature)
            .unwrap_or_else(|| panic!("the document has no feature {feature}"))
    }

    /// The report of `feature`.
    pub fn feature(&self, feature: &str) -> &FeatureReport {
        feature_report(&self.report, feature)
    }

    /// The output of `feature`; fails the test when it has none.
    pub fn output(&self, feature: &str) -> &FeatureOutput {
        output(&self.report, feature)
    }

    /// The body updates of `feature`'s output, in output order; fails the test when the output
    /// is not body updates.
    pub fn body_updates(&self, feature: &str) -> &[BodyUpdate] {
        body_updates(&self.report, feature)
    }

    /// The state of `body` at the end of the run.
    pub fn body(&self, body: &str) -> &BodyState {
        final_state(&self.report, body)
    }

    /// The shape of `body` at the end of the run; fails the test when the body is empty.
    pub fn shape(&self, body: &str) -> &Shape {
        self.body(body)
            .shape()
            .unwrap_or_else(|| panic!("body {body} is empty; {}", self.summary()))
    }

    /// The declared dependencies of `feature`'s record.
    pub fn dependencies(&self, feature: &str) -> Dependencies {
        self.recompute
            .registry()
            .dependencies(self.record(feature))
            .unwrap_or_else(|error| panic!("dependencies of {feature} failed: {error}"))
    }

    /// The rendered messages of `feature`, error first, then warnings.
    pub fn messages(&self, feature: &str) -> Vec<&str> {
        message_texts(self.feature(feature))
    }

    /// Asserts that `feature` has status ok.
    pub fn assert_ok(&self, feature: &str) {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Ok),
            "{feature} should be ok, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
    }

    /// Asserts that every feature of the document has status ok.
    pub fn assert_all_ok(&self) {
        for report in &self.report.features {
            self.assert_ok(report.feature.as_str());
        }
    }

    /// Asserts that `feature` has status ok and no message at all, not even a notice.
    pub fn assert_quiet(&self, feature: &str) {
        self.assert_ok(feature);
        assert!(
            self.messages(feature).is_empty(),
            "{:?}",
            self.messages(feature)
        );
    }

    /// Asserts that every feature of the document has status ok and no message at all.
    pub fn assert_all_quiet(&self) {
        self.assert_all_ok();
        for report in &self.report.features {
            let feature = report.feature.as_str();
            assert_eq!(
                self.messages(feature),
                Vec::<&str>::new(),
                "{feature} has no message"
            );
        }
    }

    /// Asserts that `feature` has status error with the error code `code`, and returns the error.
    pub fn assert_error(&self, feature: &str, code: &str) -> &FeatureError {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Error),
            "{feature} should fail with {code}, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
        let error = report
            .error
            .as_ref()
            .unwrap_or_else(|| panic!("{feature} has status error but no error"));
        assert_eq!(error.code(), code, "error code of {feature}: {error}");
        error
    }

    /// Asserts that `feature` has status warning with exactly these warning codes, in order.
    pub fn assert_warnings(&self, feature: &str, codes: &[&str]) {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Warning),
            "{feature} should warn with {codes:?}, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
        let actual: Vec<&str> = report
            .warnings
            .iter()
            .map(|warning| warning.code())
            .collect();
        assert_eq!(actual, codes, "warning codes of {feature}");
    }

    /// Every feature's status and messages, for failure output.
    pub fn summary(&self) -> String {
        summary(&self.report)
    }
}

// Reading reports, shared with `tests/common`.

/// The report of `feature`.
pub fn feature_report<'r>(report: &'r Report, feature: &str) -> &'r FeatureReport {
    report
        .features
        .iter()
        .find(|entry| entry.feature.as_str() == feature)
        .unwrap_or_else(|| panic!("the report has no feature {feature}"))
}

/// The end-of-run report of `body`.
pub fn body_report<'r>(report: &'r Report, body: &str) -> &'r BodyReport {
    report
        .bodies
        .iter()
        .find(|entry| entry.body.as_str() == body)
        .unwrap_or_else(|| panic!("the report has no body {body}"))
}

/// The state of `body` at the end of the run.
pub fn final_state<'r>(report: &'r Report, body: &str) -> &'r BodyState {
    &body_report(report, body).state
}

/// The state of `body` just before `feature` (its `bodies_before`, which holds the bodies it
/// declares).
pub fn state_before<'r>(report: &'r Report, feature: &str, body: &str) -> &'r BodyState {
    feature_report(report, feature)
        .bodies_before
        .iter()
        .find(|(id, _)| id.as_str() == body)
        .map(|(_, state)| state)
        .unwrap_or_else(|| {
            panic!(
                "{feature} has no state of body {body} before it; {}",
                summary(report)
            )
        })
}

/// The output of `feature`; fails the test when it has none.
pub fn output<'r>(report: &'r Report, feature: &str) -> &'r FeatureOutput {
    feature_report(report, feature)
        .output
        .as_deref()
        .unwrap_or_else(|| panic!("{feature} has no output; {}", summary(report)))
}

/// The body updates of `feature`'s output, in output order; fails the test when the output is
/// not body updates.
pub fn body_updates<'r>(report: &'r Report, feature: &str) -> &'r [BodyUpdate] {
    let OutputData::Bodies(updates) = &output(report, feature).data else {
        panic!("{feature} did not output body updates");
    };
    updates
}

/// The `BodyUpdate` of `body` in `feature`'s output.
pub fn output_update<'r>(report: &'r Report, feature: &str, body: &str) -> &'r BodyUpdate {
    body_updates(report, feature)
        .iter()
        .find(|update| update.body.as_str() == body)
        .unwrap_or_else(|| panic!("{feature} did not update body {body}"))
}

/// The placement and motion of the datum plane that `feature` produced.
fn datum_output<'r>(report: &'r Report, feature: &str) -> (&'r Placement, &'r Motion) {
    let OutputData::Datum(DatumResult::Plane { placement, motion }) = &output(report, feature).data
    else {
        panic!("{feature} did not output a datum plane");
    };
    (placement, motion)
}

/// The placement of the datum plane that `feature` produced.
pub fn datum_plane<'r>(report: &'r Report, feature: &str) -> &'r Placement {
    datum_output(report, feature).0
}

/// The rendered messages of a feature's report, error first, then warnings.
pub fn message_texts(report: &FeatureReport) -> Vec<&str> {
    report
        .messages
        .iter()
        .map(|message| message.text.as_str())
        .collect()
}

/// Every feature's status and messages, for failure output.
pub fn summary(report: &Report) -> String {
    let lines: Vec<String> = report
        .features
        .iter()
        .map(|entry| {
            format!(
                "{}: {:?} {:?}",
                entry.feature,
                entry.status,
                message_texts(entry)
            )
        })
        .collect();
    lines.join("; ")
}

/// Asserts that the field an error is about has this path, for example `size[0]`.
pub fn assert_error_field(error: &FeatureError, path: &str) {
    let field = error
        .field()
        .unwrap_or_else(|| panic!("the error names no field: {error}"));
    assert_eq!(field.path, path, "field of the error: {error}");
}

/// The ids of dependency links, in order.
pub fn link_ids(links: &[Link]) -> Vec<&str> {
    links.iter().map(|link| link.id.as_str()).collect()
}

/// Asserts `actual` equals `expected` within the relative tolerance `MEASURE_TOLERANCE`.
pub fn assert_relative(what: &str, actual: f64, expected: f64) {
    let difference = (actual - expected).abs();
    assert!(
        difference <= MEASURE_TOLERANCE * expected.abs(),
        "{what}: expected {expected}, got {actual} (relative difference {})",
        difference / expected.abs(),
    );
}

/// Asserts `actual` equals `expected` within an absolute tolerance.
pub fn assert_absolute(what: &str, actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected}, got {actual} (tolerance {tolerance})",
    );
}

/// Asserts the shape's volume (relative tolerance 1e-6).
pub fn assert_volume(shape: &Shape, expected: f64) {
    let volume = shape.volume().expect("the kernel measures the volume");
    assert_relative("volume", volume, expected);
}

/// Asserts the shape's total face area (relative tolerance 1e-6).
pub fn assert_area(shape: &Shape, expected: f64) {
    let area = shape.area().expect("the kernel measures the area");
    assert_relative("area", area, expected);
}

/// Asserts the shape's number of faces (exact).
pub fn assert_face_count(shape: &Shape, expected: u32) {
    assert_eq!(
        shape.count(SubShapeKind::Face),
        expected,
        "face count of {shape:?}"
    );
}

/// Asserts the shape's number of solids (exact).
pub fn assert_solid_count(shape: &Shape, expected: usize) {
    let solids = shape.solids().expect("the kernel lists the solids");
    assert_eq!(solids.len(), expected, "solid count of {shape:?}");
}

/// Asserts the number of shells of a body's shape: one outer shell per solid and one per closed
/// cavity (kernel.md section 3.6 item 6).
pub fn assert_shell_count(shape: &Shape, expected: u32) {
    assert_eq!(
        shape.count(SubShapeKind::Shell),
        expected,
        "shell count of {shape:?}"
    );
}

/// Asserts the shape's tight bounding box, minimum then maximum corner (absolute 1e-5 mm).
pub fn assert_bounding_box(shape: &Shape, min: [f64; 3], max: [f64; 3]) {
    let bounds = shape
        .bounding_box()
        .expect("the kernel bounds a non-empty shape");
    assert_coordinates("bounding box minimum", bounds.min, min, LENGTH_TOLERANCE);
    assert_coordinates("bounding box maximum", bounds.max, max, LENGTH_TOLERANCE);
}

/// The face names of a body, sorted by byte order.
pub fn face_names(state: &BodyState) -> Vec<String> {
    element_names(state, ElementKind::Face)
}

/// Asserts the face names of a body, in any order.
pub fn assert_face_names(state: &BodyState, expected: &[&str]) {
    let mut expected: Vec<&str> = expected.to_vec();
    expected.sort();
    assert_eq!(face_names(state), expected, "face names");
}

/// The kernel index of the face named `name`, if the body has one.
pub fn face_index(state: &BodyState, name: &str) -> Option<u32> {
    let element = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    state.naming().index(&element)
}

/// The geometry of the face named `name` (features.md section 6.1: found by name through the
/// body's naming map, measured with the kernel's `element`).
pub fn face(state: &BodyState, name: &str) -> ElementGeometry {
    element(state, ElementKind::Face, name)
}

/// The geometry of the element of `kind` named `name`; panics with the body's names of that kind
/// when it has none.
fn element(state: &BodyState, kind: ElementKind, name: &str) -> ElementGeometry {
    let plural = match kind {
        ElementKind::Face => "faces",
        ElementKind::Edge => "edges",
        ElementKind::Vertex => "vertices",
    };
    try_element(state, name).unwrap_or_else(|| {
        panic!(
            "no {kind} named {name}; {plural}: {:?}",
            element_names(state, kind)
        )
    })
}

/// The geometry of the face, edge or vertex named `name`, found through the body's naming map and
/// measured with the kernel's `element`, if the body has one.
pub fn try_element(state: &BodyState, name: &str) -> Option<ElementGeometry> {
    let parsed = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    let index = state.naming().index(&parsed)?;
    let kind = match parsed.kind() {
        ElementKind::Face => SubShapeKind::Face,
        ElementKind::Edge => SubShapeKind::Edge,
        ElementKind::Vertex => SubShapeKind::Vertex,
    };
    let shape = state.shape()?;
    Some(
        shape
            .element(kind, index)
            .unwrap_or_else(|error| panic!("geometry of {name}: {error}")),
    )
}

/// Asserts a face's surface kind.
pub fn assert_face_kind(geometry: &ElementGeometry, expected: GeometryKind) {
    assert_eq!(geometry.kind, expected, "surface kind of {geometry:?}");
}

/// Asserts a face's area (relative tolerance 1e-6).
pub fn assert_face_area(geometry: &ElementGeometry, expected: f64) {
    assert_relative("face area", geometry.measure, expected);
}

/// Asserts one coordinate (0 for x, 1 for y, 2 for z) of a face's area centroid (absolute 1e-5 mm).
/// For a flat face this is the position of its plane along that axis.
pub fn assert_face_position(geometry: &ElementGeometry, axis: usize, expected: f64) {
    let centroid = geometry.centroid;
    let coordinate = [centroid.x, centroid.y, centroid.z][axis];
    let name = ["x", "y", "z"][axis];
    assert_absolute(
        &format!("face centroid {name}"),
        coordinate,
        expected,
        LENGTH_TOLERANCE,
    );
}

/// Asserts a face's area centroid (absolute 1e-5 mm in each coordinate), as a spec's "centered at" says.
pub fn assert_face_centroid(geometry: &ElementGeometry, expected: [f64; 3]) {
    for (axis, coordinate) in expected.into_iter().enumerate() {
        assert_face_position(geometry, axis, coordinate);
    }
}

/// Asserts a face's radius (absolute 1e-5 mm).
pub fn assert_face_radius(geometry: &ElementGeometry, expected: f64) {
    let radius = geometry
        .radius
        .unwrap_or_else(|| panic!("the face has no radius: {geometry:?}"));
    assert_absolute("face radius", radius, expected, LENGTH_TOLERANCE);
}

/// The components of a vector.
pub fn components(vector: Vector) -> [f64; 3] {
    [vector.x, vector.y, vector.z]
}

/// The coordinates of a kernel point.
pub fn point_components(point: Point) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// Asserts a flat face's outward unit normal, sign included (absolute 1e-9).
pub fn assert_face_normal(geometry: &ElementGeometry, expected: [f64; 3]) {
    let normal = geometry
        .direction
        .unwrap_or_else(|| panic!("the face has no normal: {geometry:?}"));
    assert_coordinates(
        &format!("normal of {geometry:?}"),
        components(normal),
        expected,
        AXIS_TOLERANCE,
    );
}

/// Asserts a round face's axis direction up to its sign (the kernel keeps OCCT's sign).
pub fn assert_face_axis_direction(geometry: &ElementGeometry, expected: [f64; 3]) {
    let direction = geometry
        .direction
        .unwrap_or_else(|| panic!("the face has no axis: {geometry:?}"));
    let actual = components(direction);
    let dot: f64 = actual
        .iter()
        .zip(expected)
        .map(|(component, wanted)| component * wanted)
        .sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    for (axis, (component, wanted)) in actual.into_iter().zip(expected).enumerate() {
        assert_absolute(
            &format!("axis component {axis} of {geometry:?}"),
            sign * component,
            wanted,
            AXIS_TOLERANCE,
        );
    }
}

/// The distance from `point` to the axis line of a round face (through its axis location) or of a
/// straight edge (through its centroid), if the geometry has an axis.
pub fn axis_distance(geometry: &ElementGeometry, point: [f64; 3]) -> Option<f64> {
    let direction = components(geometry.direction?);
    let location = match geometry.kind {
        GeometryKind::Line => geometry.centroid,
        _ => geometry.location?,
    };
    let offset = [
        point[0] - location.x,
        point[1] - location.y,
        point[2] - location.z,
    ];
    let along: f64 = offset
        .iter()
        .zip(direction)
        .map(|(component, axis)| component * axis)
        .sum();
    Some(
        offset
            .iter()
            .zip(direction)
            .map(|(component, axis)| (component - along * axis).powi(2))
            .sum::<f64>()
            .sqrt(),
    )
}

/// Asserts that a round face's axis passes through `point` (absolute 1e-9).
pub fn assert_face_axis_through(geometry: &ElementGeometry, point: [f64; 3]) {
    let distance = axis_distance(geometry, point)
        .unwrap_or_else(|| panic!("the face has no axis: {geometry:?}"));
    assert_absolute(
        &format!("distance from {point:?} to the axis of {geometry:?}"),
        distance,
        0.0,
        AXIS_TOLERANCE,
    );
}

/// Median build time of `feature` over `runs` runs of `document`, each with a fresh `Recompute` so no
/// result comes from the cache. For the performance budgets (features.md section 5).
pub fn median_build_time(document: &Document, feature: &str, runs: usize) -> Duration {
    median_build_time_with_assets(document, feature, runs, &Assets::new())
}

/// Serializes the timed runs, so that a budget's median never shares the cores with another
/// budget's timed runs.
static TIMING: Mutex<()> = Mutex::new(());

/// The median of `runs` durations, each one call of `measure`, timed while no other median is
/// (`TIMING`).
pub fn median_duration(runs: usize, mut measure: impl FnMut() -> Duration) -> Duration {
    assert!(runs > 0, "at least one run");
    // The lock guards no data, so a budget that failed while holding it does not fail the rest.
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut durations: Vec<Duration> = (0..runs).map(|_| measure()).collect();
    durations.sort();
    durations[runs / 2]
}

/// Asserts that the median build time of `feature` over `runs` fresh runs of `document` is under
/// `budget` (features.md section 5).
pub fn assert_median_build_within(
    document: &Document,
    feature: &str,
    runs: usize,
    budget: Duration,
) {
    let duration = median_build_time(document, feature, runs);
    assert!(
        duration < budget,
        "{feature} took {duration:?} to build (median of {runs} runs), over the {budget:?} budget",
    );
}

/// How long `feature` took to build in `run`, which must have built it rather than reused a cached
/// result.
fn build_time(run: &Run, feature: &str) -> Duration {
    let timing = &run.feature(feature).timing;
    assert!(!timing.from_cache, "a fresh recompute builds {feature}");
    timing.duration
}

// Helpers for the dress-up features (fillet, chamfer, shell): output states, unchanged bodies,
// name prefixes, repair data and the "too large" search (features.md sections 4.3 and 6.1).

/// The `BodyUpdate` of `body` in `feature`'s output.
pub fn body_update<'a>(run: &'a Run, feature: &str, body: &str) -> &'a BodyUpdate {
    output_update(&run.report, feature, body)
}

/// The state `feature`'s output gives `body`: the shape and naming map of its `BodyUpdate`.
pub fn output_body(run: &Run, feature: &str, body: &str) -> BodyState {
    let update = body_update(run, feature, body);
    match &update.shape {
        Some(shape) => BodyState::new(shape.clone(), update.naming.clone()),
        None => BodyState::empty(),
    }
}

/// The names of a body's elements of one kind, sorted by byte order.
pub fn element_names(state: &BodyState, kind: ElementKind) -> Vec<String> {
    let mut names: Vec<String> = state
        .naming()
        .names(kind)
        .map(|(_, name)| name.as_str().to_owned())
        .collect();
    names.sort();
    names
}

/// The face names of a body that start with `prefix`, sorted by byte order.
pub fn face_names_with_prefix(state: &BodyState, prefix: &str) -> Vec<String> {
    face_names(state)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .collect()
}

/// Asserts that a body is unchanged (features.md section 6.1): the same volume, face count and
/// face, edge and vertex names as `expected`.
pub fn assert_same_body(actual: &BodyState, expected: &BodyState) {
    let expected_shape = expected
        .shape()
        .expect("the body holds a solid before the feature");
    let actual_shape = actual.shape().expect("the body still holds a solid");
    assert_volume(
        actual_shape,
        expected_shape
            .volume()
            .expect("the kernel measures the volume"),
    );
    assert_face_count(actual_shape, expected_shape.count(SubShapeKind::Face));
    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(
            element_names(actual, kind),
            element_names(expected, kind),
            "{kind:?} names of an unchanged body",
        );
    }
}

/// Asserts that a message field is a reference list or one of its entries: feature specs name the
/// list ("field `edges`") and the repair data points at the entry (`/edges/1`).
pub fn assert_list_field(field: Option<&FieldRef>, list: &str, index: usize) {
    let field = field.unwrap_or_else(|| panic!("the message names no field; expected {list}"));
    let entry = format!("{list}[{index}]");
    assert!(
        field.path == list || field.path == entry,
        "the message field is {}, expected {list} or {entry}",
        field.path,
    );
}

/// Asserts element repair data: the JSON Pointer of the lost reference and the body it is resolved in.
pub fn assert_element_repair(repair: Option<Repair>, pointer: &str, body: &str) {
    match repair {
        Some(Repair::Element {
            pointer: actual_pointer,
            body: actual_body,
            ..
        }) => {
            assert_eq!(actual_pointer, pointer, "repair pointer");
            assert_eq!(actual_body.as_str(), body, "repair body");
        }
        other => panic!("expected element repair data, got {other:?}"),
    }
}

/// Asserts that a skipped edge's warning is `Ignored` with the reason `SmoothEdge`.
#[track_caller]
pub fn assert_skipped_as_smooth(warning: &FeatureWarning) {
    assert!(
        matches!(
            warning,
            FeatureWarning::Ignored {
                reason: IgnoredReason::SmoothEdge,
                ..
            }
        ),
        "expected the edge to be skipped as smooth, got {warning:?}"
    );
}

/// Asserts that a "too large" error measured the face `face` across the edge `edge` at `position` of Edges, 2.4 mm
/// across, as its `limit`, and whether the other edges were verified to work without that edge (features.md section
/// 4.3).
#[track_caller]
pub fn assert_limiting_face(
    error: &FeatureError,
    position: usize,
    edge: &str,
    face: &str,
    removable: bool,
) {
    let (FeatureError::FilletTooLarge { limit, .. } | FeatureError::ChamferTooLarge { limit, .. }) =
        error
    else {
        panic!("expected a too large error, got {error:?}");
    };
    let Some(Limit::Face {
        position: found_position,
        edge: found_edge,
        face: found_face,
        width,
        removable: found_removable,
    }) = limit
    else {
        panic!("expected a limiting face, got {limit:?}");
    };
    assert_eq!(*found_position, position, "the limiting edge's entry");
    assert_eq!(found_edge.query.to_string(), edge);
    assert_eq!(found_face.query.to_string(), face);
    assert!((width - 2.4).abs() <= 1e-5, "width {width}");
    assert_eq!(*found_removable, removable, "removable");
}

/// `value` rounded down to two significant digits, as suggested limits are (features.md section 3.12).
pub fn round_down_to_two_significant_digits(value: f64) -> f64 {
    assert!(
        value.is_finite() && value > 0.0,
        "a limit is a positive number, not {value}"
    );
    let decimals = 1 - value.log10().floor() as i32;
    if decimals >= 0 {
        let factor = 10_f64.powi(decimals);
        (value * factor).floor() / factor
    } else {
        let factor = 10_f64.powi(-decimals);
        (value / factor).floor() * factor
    }
}

/// The suggested limit of a "too large" error with largest working value `max`, as the number a
/// millimeter document shows, for example `2.3` for 2.396 (features.md section 3.12).
pub fn suggested_limit(max: f64) -> String {
    format!("{}", round_down_to_two_significant_digits(max))
}

/// Median build time of `feature` over `runs` fresh runs of `document` in which it fails with
/// `code`. For the budgets of failing builds, which include the "too large" search (features.md
/// section 5).
pub fn median_failed_build_time(
    document: &Document,
    feature: &str,
    code: &str,
    runs: usize,
) -> Duration {
    median_duration(runs, || {
        let run = Run::new(document.clone());
        run.assert_error(feature, code);
        build_time(&run, feature)
    })
}

// Helpers for the sketch, extrude, revolve, hole, datum plane and import tests: sketch lines as
// text, a STEP box asset, sketch and datum outputs, states before a feature, message pointers,
// expression uses, placements, vertices, flat faces and canonical examples (features.md section 6.1).

/// A sketch line entity as JSON text, for documents written as text.
pub fn line(entity: &str, start: [i32; 2], end: [i32; 2]) -> String {
    format!(
        r#"{{"id": "{entity}", "type": "line", "start": [{}, {}], "end": [{}, {}]}}"#,
        start[0], start[1], end[0], end[1]
    )
}

/// An asset name made of one repeated hex digit followed by `.step`, as the hash-named asset of a
/// file revision (the tests do not check that it is a hash); `asset_name('0')` is
/// features/import.md's fixture `A`.
pub fn asset_name(digit: char) -> String {
    format!("{}.step", digit.to_string().repeat(64))
}

/// The state of `body` just before `feature` (its `bodies_before`, which holds the bodies it declares).
pub fn body_before<'a>(run: &'a Run, feature: &str, body: &str) -> &'a BodyState {
    state_before(&run.report, feature, body)
}

/// The solved sketch that `feature` produced.
pub fn sketch_result<'a>(run: &'a Run, feature: &str) -> &'a SketchResult {
    let OutputData::Sketch(result) = &run.output(feature).data else {
        panic!("{feature} did not output a sketch");
    };
    result
}

/// The placement of the datum plane that `feature` produced.
pub fn datum_placement<'a>(run: &'a Run, feature: &str) -> &'a Placement {
    datum_plane(&run.report, feature)
}

/// The motion of the datum plane that `feature` produced (features/datum_plane.md section 3.2 step 4).
pub fn datum_motion<'a>(run: &'a Run, feature: &str) -> &'a Motion {
    datum_output(&run.report, feature).1
}

/// The rendered message of `feature` with this code.
pub fn message<'a>(run: &'a Run, feature: &str, code: &str) -> &'a Message {
    run.feature(feature)
        .messages
        .iter()
        .find(|message| message.code == code)
        .unwrap_or_else(|| panic!("{feature} has no {code} message; {}", run.summary()))
}

/// Each expression use of `dependencies` as its field path, expression source and unit, in order.
pub fn expression_uses(dependencies: &Dependencies) -> Vec<(&str, &str, FieldUnit)> {
    dependencies
        .expressions
        .iter()
        .map(|usage| (usage.field.as_str(), usage.expr.as_str(), usage.kind))
        .collect()
}

/// Asserts every coordinate of a point or direction within an absolute tolerance.
pub fn assert_coordinates<const N: usize>(
    what: &str,
    actual: [f64; N],
    expected: [f64; N],
    tolerance: f64,
) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
    assert!(
        close,
        "{what}: expected {expected:?}, got {actual:?} (tolerance {tolerance})",
    );
}

/// Asserts a placement's origin, x axis and normal (absolute 1e-9, features.md section 6.1).
pub fn assert_placement(
    placement: &Placement,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
) {
    assert_coordinates("placement origin", placement.origin, origin, AXIS_TOLERANCE);
    assert_coordinates("placement x axis", placement.x_axis, x_axis, AXIS_TOLERANCE);
    assert_coordinates("placement normal", placement.normal, normal, AXIS_TOLERANCE);
}

/// The geometry of the vertex named `name`, for example `vertex(f-hole1/tip/e-p1)`.
pub fn vertex(state: &BodyState, name: &str) -> ElementGeometry {
    element(state, ElementKind::Vertex, name)
}

/// The geometry of the edge named `name`, for example `edge(f-box1/front|f-box1/top)`.
pub fn edge(state: &BodyState, name: &str) -> ElementGeometry {
    element(state, ElementKind::Edge, name)
}

/// The JSON text of a stored reference, for a record written after a pick or a fix stored it.
pub fn reference_json(reference: &Reference) -> String {
    serde_json::to_string(reference).expect("references serialize")
}

/// Asserts that a face lies in the plane where coordinate `axis` (0 for x, 1 for y, 2 for z) equals
/// `position`: a planar surface whose normal is parallel to that axis (either sign) and whose
/// centroid lies on the plane. This is what a spec's "plane z=30" means.
pub fn assert_face_in_plane(geometry: &ElementGeometry, axis: usize, position: f64) {
    assert_face_kind(geometry, GeometryKind::Plane);
    let mut direction = [0.0; 3];
    direction[axis] = 1.0;
    assert_face_axis_direction(geometry, direction);
    assert_face_position(geometry, axis, position);
}

/// Asserts that the face named `name` of `state` lies in the plane where coordinate `axis` (0 for x, 1 for y, 2 for
/// z) equals `position` and has this area, as a spec's "planar at z=10 with area 78.539816" says, and returns its
/// geometry for further checks.
pub fn assert_flat_face(
    state: &BodyState,
    name: &str,
    axis: usize,
    position: f64,
    area: f64,
) -> ElementGeometry {
    let geometry = face(state, name);
    assert_face_in_plane(&geometry, axis, position);
    assert_face_area(&geometry, area);
    geometry
}

/// The `.ptsm` files in `examples/<folder>/`, as paths relative to `examples/`, sorted.
pub fn example_paths(folder: &str) -> Vec<String> {
    let directory = examples_folder().join(folder);
    let entries = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()));
    let mut paths: Vec<String> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".ptsm"))
        .map(|name| format!("{folder}/{name}"))
        .collect();
    paths.sort();
    paths
}

/// Asserts that `examples/<folder>/` has examples and that every one of them is canonical.
pub fn assert_examples_canonical(folder: &str) {
    let paths = example_paths(folder);
    assert!(!paths.is_empty(), "examples/{folder} has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

/// Asserts that `examples/<relative_path>` is canonical, as `partsmith fmt` writes it (file-format.md
/// sections 3.2 and 3.5): every record validates without unknown keys, and saving the document with
/// the registry's canonical params reproduces the file byte for byte.
pub fn assert_canonical_example(relative_path: &str) {
    let path = examples_folder().join(relative_path);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read example {}: {error}", path.display()));
    let mut document = parse_document(&text);
    let registry = Registry::with_builtins();
    for record in &mut document.features {
        let validated = registry
            .validate_and_canonicalize(record)
            .unwrap_or_else(|error| {
                panic!(
                    "{} in examples/{relative_path} is invalid: {error}",
                    record.id
                )
            });
        assert!(
            validated.unknown_keys.is_empty(),
            "{} in examples/{relative_path} has unknown keys {:?}",
            record.id,
            validated.unknown_keys,
        );
        *record = validated.record;
    }
    let saved = partsmith_model::save(&document)
        .unwrap_or_else(|error| panic!("examples/{relative_path} does not save: {error}"));
    assert_eq!(
        saved, text,
        "examples/{relative_path} is not canonical (partsmith fmt would rewrite it)"
    );
}

// Helpers for the combine features (boolean, mirror, linear and circular patterns, move body, split
// body, import): every body update of an output, empty and unchanged bodies.

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

/// Median build time of `feature` over `runs` runs of `document` with these asset bytes, each with
/// a fresh `Recompute` so no result comes from the cache (features.md section 5).
pub fn median_build_time_with_assets(
    document: &Document,
    feature: &str,
    runs: usize,
    assets: &Assets,
) -> Duration {
    median_duration(runs, || {
        let run = Run::with_assets(document.clone(), assets);
        run.assert_ok(feature);
        build_time(&run, feature)
    })
}

/// Asserts that a body has a face named `name`.
pub fn assert_has_face(state: &BodyState, name: &str) {
    assert!(
        face_index(state, name).is_some(),
        "no face named {name}; faces: {:?}",
        face_names(state),
    );
}

/// Asserts that a body has no face named `name`.
pub fn assert_no_face(state: &BodyState, name: &str) {
    assert!(
        face_index(state, name).is_none(),
        "unexpected face named {name}; faces: {:?}",
        face_names(state),
    );
}

// Helpers for messages, fixes and repairs (features.md sections 3.7, 3.8, 3.12, 4 and 6.5): rendering a
// feature's errors and warnings with the diagnosis a scenario implies, notices, followed references, picked
// references, the fixes tests expect, successions, absent faces, the message lint and the choice a reference
// that no longer lies in one plane or on one line offers.

impl Run {
    /// Calls `use_context` with the render context of `feature` in this run (features.md section 2.8): the document's
    /// names with the run's outputs, the document's units, the states of the feature's declared bodies just before
    /// it, and `diagnosis`. Tests pass the diagnosis a scenario's cause implies (recompute.md section 3.10).
    pub fn with_context<T>(
        &self,
        feature: &str,
        diagnosis: &Diagnosis,
        use_context: impl FnOnce(&RenderContext<'_>) -> T,
    ) -> T {
        let bodies: BTreeMap<Id, BodyState> = self
            .feature(feature)
            .bodies_before
            .iter()
            .cloned()
            .collect();
        let feature = self.feature(feature).feature.clone();
        self.with_names(|names| {
            use_context(&RenderContext {
                feature: &feature,
                names,
                units: self.document.units,
                bodies: &bodies,
                diagnosis,
            })
        })
    }

    /// Calls `use_names` with the document's names (features.md section 2.8): labels, records, and the sketch results
    /// of this run.
    pub fn with_names<T>(&self, use_names: impl FnOnce(&dyn Names) -> T) -> T {
        let outputs: BTreeMap<Id, Arc<FeatureOutput>> = self
            .report
            .features
            .iter()
            .filter_map(|report| {
                report
                    .output
                    .as_ref()
                    .map(|output| (report.feature.clone(), Arc::clone(output)))
            })
            .collect();
        let names = DocumentNames {
            document: &self.document,
            registry: self.recompute.registry().as_ref(),
            outputs: &outputs,
        };
        use_names(&names)
    }

    /// The error of `feature` rendered with `diagnosis`.
    pub fn render_error(&self, feature: &str, diagnosis: &Diagnosis) -> String {
        let error = self.error(feature);
        self.with_context(feature, diagnosis, |context| error.render(context))
    }

    /// The fixes of `feature`'s error with `diagnosis`.
    pub fn error_fixes(&self, feature: &str, diagnosis: &Diagnosis) -> Vec<Fix> {
        let error = self.error(feature);
        self.with_context(feature, diagnosis, |context| error.fixes(context))
    }

    /// The only warning or notice of `feature` with this code, rendered with `diagnosis`.
    pub fn render_warning(&self, feature: &str, code: &str, diagnosis: &Diagnosis) -> String {
        let warning = self.warning(feature, code);
        self.with_context(feature, diagnosis, |context| warning.render(context))
    }

    /// The fixes of the only warning or notice of `feature` with this code, with `diagnosis`.
    pub fn warning_fixes(&self, feature: &str, code: &str, diagnosis: &Diagnosis) -> Vec<Fix> {
        let warning = self.warning(feature, code);
        self.with_context(feature, diagnosis, |context| warning.fixes(context))
    }

    /// The error of `feature`; fails the test when it has none.
    pub fn error(&self, feature: &str) -> &FeatureError {
        self.feature(feature)
            .error
            .as_ref()
            .unwrap_or_else(|| panic!("{feature} has no error; {}", self.summary()))
    }

    /// The only warning or notice of `feature` with this code.
    pub fn warning(&self, feature: &str, code: &str) -> &FeatureWarning {
        let matching: Vec<&FeatureWarning> = self
            .feature(feature)
            .warnings
            .iter()
            .filter(|warning| warning.code() == code)
            .collect();
        match matching.as_slice() {
            [warning] => warning,
            _ => panic!(
                "{feature} should have exactly one {code}, but its messages are {:?}",
                self.messages(feature)
            ),
        }
    }

    /// The only `plane_fallback` warning of `sketch` (features/sketch.md section 3.5): where it placed the sketch, its
    /// cause and its loss.
    pub fn plane_fallback(&self, sketch: &str) -> (&Placed, FallbackCause, Option<&Loss>) {
        let warning = self.warning(sketch, "plane_fallback");
        let FeatureWarning::PlaneFallback {
            placed,
            cause,
            loss,
            ..
        } = warning
        else {
            panic!("expected a plane fallback warning, got {warning:?}");
        };
        (placed, *cause, loss.as_ref())
    }

    /// Asserts that `feature` built with status ok and exactly these notices, in order, each of severity info
    /// (recompute.md section 3.5 step 11: notices leave the status ok).
    pub fn assert_notices(&self, feature: &str, codes: &[&str]) {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Ok),
            "{feature} should be ok with notices {codes:?}, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
        let actual: Vec<&str> = report
            .warnings
            .iter()
            .map(|warning| warning.code())
            .collect();
        assert_eq!(actual, codes, "notice codes of {feature}");
        for warning in &report.warnings {
            assert_eq!(
                warning.severity(),
                Severity::Info,
                "{feature}'s {} is a notice",
                warning.code()
            );
        }
    }

    /// Asserts that the only notice of `feature` is a `reference_followed` that followed by `follow`, at the field
    /// `pointer` (features.md section 3.8).
    pub fn assert_followed(&self, feature: &str, follow: Follow, pointer: &str) {
        self.assert_notices(feature, &["reference_followed"]);
        let warning = self.warning(feature, "reference_followed");
        assert!(
            matches!(warning, FeatureWarning::ReferenceFollowed { follow: actual, .. } if *actual == follow),
            "{feature} should follow by {follow:?}, but its notice is {warning:?}"
        );
        assert_eq!(
            message(self, feature, "reference_followed")
                .pointer
                .as_deref(),
            Some(pointer),
        );
    }

    /// `Reference::from_element` of the element named `name` in `body` just before `feature`: what a pick there
    /// stores, and what a candidate or fix of that feature holds.
    pub fn picked_before(&self, feature: &str, body: &str, name: &str) -> Reference {
        picked_in(body_before(self, feature, body), name)
    }
}

/// `Reference::from_element` of the element named `name` in `state`.
pub fn picked_in(state: &BodyState, name: &str) -> Reference {
    let set = state
        .element_set()
        .expect("the element set of a built body");
    let name = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    let info = set
        .elements()
        .iter()
        .find(|info| info.name == name)
        .unwrap_or_else(|| panic!("the body has no element {name}"));
    Reference::from_element(info, &set)
}

/// Asserts that the edge or face axis `feature` stores at `pointer` (a JSON Pointer into its params, such as
/// `/axis`) is written as a pick of the element `name` of `body` just before `feature` stores it (features.md
/// section 6.1): its reference is `Reference::from_element` of that element, and its `direction` is the
/// direction of the element's fingerprint, which is the pick's `axis_direction` while the body has not moved
/// (engine.md section 3.9).
pub fn assert_axis_stored_as_picked(
    run: &Run,
    feature: &str,
    pointer: &str,
    body: &str,
    name: &str,
) {
    let params = Value::Object(run.record(feature).params.clone());
    let axis = params
        .pointer(pointer)
        .unwrap_or_else(|| panic!("{feature} stores no axis at {pointer}"));
    let kind = axis["kind"]
        .as_str()
        .unwrap_or_else(|| panic!("the axis at {pointer} has a kind"));
    let stored: Reference = serde_json::from_value(axis[kind].clone()).unwrap_or_else(|error| {
        panic!("the {kind} axis at {pointer} stores no reference: {error}")
    });
    let picked = run.picked_before(feature, body, name);
    assert_eq!(
        stored, picked,
        "the reference at {pointer}/{kind} as a pick stores it"
    );
    let direction: [f64; 3] = serde_json::from_value(axis["direction"].clone())
        .unwrap_or_else(|error| panic!("the axis at {pointer} stores no direction: {error}"));
    let fingerprint_direction = picked
        .fingerprint
        .direction
        .unwrap_or_else(|| panic!("the fingerprint of {name} has a direction"));
    assert_coordinates(
        &format!("the direction at {pointer}"),
        direction,
        fingerprint_direction,
        AXIS_TOLERANCE,
    );
}

/// `UseReferences` of one reference at `pointer`.
pub fn use_reference(pointer: &str, reference: Reference) -> Fix {
    Fix::UseReferences {
        pointer: pointer.to_string(),
        references: vec![reference],
    }
}

/// `Pick` of the reference at `pointer`.
pub fn pick(pointer: &str) -> Fix {
    Fix::Pick {
        pointer: pointer.to_owned(),
    }
}

/// `SetField` of this feature's field `path`, labeled `label`, to `value`.
pub fn set_field(path: &str, label: &str, value: Value) -> Fix {
    Fix::SetField {
        feature: None,
        field: FieldRef {
            path: path.to_owned(),
            label: label.to_owned(),
        },
        value,
    }
}

/// `SetField` of `reverse` to `on`: "Turn on Reverse" for true, "Turn off Reverse" for false
/// (engine.md section 3.22).
pub fn set_reverse(on: bool) -> Fix {
    set_field("reverse", "Reverse", Value::Bool(on))
}

/// `KeepMatch` of the reference at `pointer`.
pub fn keep_match(pointer: &str) -> Fix {
    Fix::KeepMatch {
        pointer: pointer.to_owned(),
    }
}

/// `RemoveEntry` of the list entry at `pointer`.
pub fn remove_entry(pointer: &str) -> Fix {
    Fix::RemoveEntry {
        pointer: pointer.to_owned(),
    }
}

/// `KeepMatch` then `Pick` at `pointer`: the fixes of a followed reference (features.md section 4.2).
pub fn keep_and_pick(pointer: &str) -> [Fix; 2] {
    [keep_match(pointer), pick(pointer)]
}

/// The successions a body's naming map records, as (stored face, faces now) names.
pub fn successions(state: &BodyState) -> Vec<(String, Vec<String>)> {
    names_now(&state.naming().successions().0)
}

/// The merge aliases a body's naming map records (naming.md section 3.3 step 2), as (former name,
/// names now) in key order.
pub fn aliases(state: &BodyState) -> Vec<(String, Vec<String>)> {
    names_now(&state.naming().aliases().0)
}

/// The entries of a naming map's successions or aliases as text, as (stored name, names now) in key
/// order.
fn names_now(entries: &BTreeMap<ElementName, Vec<ElementName>>) -> Vec<(String, Vec<String>)> {
    entries
        .iter()
        .map(|(stored, now)| {
            (
                stored.to_string(),
                now.iter().map(ToString::to_string).collect(),
            )
        })
        .collect()
}

/// The faces `feature` records absent (features.md section 2.7), as (name, why) with `why` in its `Debug` form.
pub fn absent_faces(run: &Run, feature: &str) -> Vec<(String, String)> {
    run.output(feature)
        .absent
        .iter()
        .map(|face| (face.name.to_string(), format!("{:?}", face.why)))
        .collect()
}

/// Asserts that every rendered message of `feature` passes the message lint.
pub fn assert_messages_pass_lint(run: &Run, feature: &str) {
    for text in run.messages(feature) {
        let failures = lint_failures(text);
        assert!(
            failures.is_empty(),
            "the message {text:?} of {feature} breaks the message lint: {failures:?}"
        );
    }
}

/// Asserts that `feature` failed with `code` (`not_coplanar` or `not_collinear`) about the reference of the param
/// `field`, rendered `text`, and that its fixes are `UseReferences` with each element `names` gives, as a pick in
/// `body` just before the feature stores it, then `Pick`, all at the JSON Pointer of the reference inside the param
/// (`/base/face` for `base`, `/direction/axis/edge` for `direction.axis`). Feature specs name the param ("field
/// `base`"); the error may also name the reference inside it, as `NotPlanar` does (features.md section 4.1).
#[track_caller]
pub fn assert_choice_between(
    run: &Run,
    feature: &str,
    code: &str,
    (field, reference_key): (&str, &str),
    body: &str,
    names: &[&str],
    text: &str,
) {
    let error = run.assert_error(feature, code);
    let path = &error
        .field()
        .unwrap_or_else(|| panic!("the error names no field: {error}"))
        .path;
    let reference_path = format!("{field}.{reference_key}");
    assert!(
        *path == field || *path == reference_path,
        "the error is about {path}, expected {field} or {reference_path}"
    );
    assert_eq!(run.messages(feature), [text]);
    let pointer = format!("/{}", reference_path.replace('.', "/"));
    let mut expected: Vec<Fix> = names
        .iter()
        .map(|name| use_reference(&pointer, run.picked_before(feature, body, name)))
        .collect();
    expected.push(pick(&pointer));
    assert_eq!(message(run, feature, code).fixes, expected);
    assert_messages_pass_lint(run, feature);
}
