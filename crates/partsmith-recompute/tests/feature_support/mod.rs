//! Document-running helpers shared by the feature acceptance tests (features.md section 6.1).
//!
//! A test builds a document (from an example under `examples/` or from JSON records), runs it with
//! `Recompute` and the built-in registry, and inspects the `Report`: statuses, rendered messages,
//! and the kernel measurements of body shapes.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use partsmith_features::{BodyState, CancelToken, Dependencies, FeatureError, Link, Registry};
use partsmith_model::{Document, EmptyContext, FeatureRecord};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Shape, SubShapeKind, Vector};
use partsmith_recompute::{FeatureReport, Input, Recompute, Report, Status};

/// Relative tolerance for volumes and areas (features.md section 6.1).
pub const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance in millimeters for bounding box coordinates, and for face positions and radii
/// (features.md section 6.1 gives 1e-5 mm per bounding box coordinate).
pub const LENGTH_TOLERANCE: f64 = 1e-5;

/// Absolute tolerance for placements and axis directions (features.md section 6.1).
pub const AXIS_TOLERANCE: f64 = 1e-9;

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
        r#"{{"format": "partsmith", "format_version": 1, "units": "{units}", "parameters": [{}], "bodies": [{}], "features": [{}]}}"#,
        parameters.join(", "),
        bodies.join(", "),
        features.join(", "),
    )
}

/// A millimeter document without parameters, with these test bodies (by id) and feature records.
pub fn document(bodies: &[&str], features: &[&str]) -> Document {
    parse_document(&document_text("mm", &[], bodies, features))
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
        let mut recompute = Recompute::new(Arc::new(Registry::with_builtins()));
        let assets = BTreeMap::new();
        let input = Input {
            document: &document,
            context: &EmptyContext,
            stop_at: None,
            assets: &assets,
        };
        let report = recompute.run(&input, &CancelToken::new());
        Run {
            document,
            recompute,
            report,
        }
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
        self.report
            .features
            .iter()
            .find(|report| report.feature.as_str() == feature)
            .unwrap_or_else(|| panic!("the report has no feature {feature}"))
    }

    /// The state of `body` at the end of the run.
    pub fn body(&self, body: &str) -> &BodyState {
        &self
            .report
            .bodies
            .iter()
            .find(|report| report.body.as_str() == body)
            .unwrap_or_else(|| panic!("the report has no body {body}"))
            .state
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
        self.feature(feature)
            .messages
            .iter()
            .map(|message| message.text.as_str())
            .collect()
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
        let lines: Vec<String> = self
            .report
            .features
            .iter()
            .map(|report| {
                format!(
                    "{}: {:?} {:?}",
                    report.feature,
                    report.status,
                    self.messages(report.feature.as_str()),
                )
            })
            .collect();
        lines.join("; ")
    }
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

/// Asserts the shape's tight bounding box, minimum then maximum corner (absolute 1e-5 mm).
pub fn assert_bounding_box(shape: &Shape, min: [f64; 3], max: [f64; 3]) {
    let bounds = shape
        .bounding_box()
        .expect("the kernel bounds a non-empty shape");
    for axis in 0..3 {
        let name = ["x", "y", "z"][axis];
        assert_absolute(
            &format!("bounding box minimum {name}"),
            bounds.min[axis],
            min[axis],
            LENGTH_TOLERANCE,
        );
        assert_absolute(
            &format!("bounding box maximum {name}"),
            bounds.max[axis],
            max[axis],
            LENGTH_TOLERANCE,
        );
    }
}

/// The face names of a body, sorted by byte order.
pub fn face_names(state: &BodyState) -> Vec<String> {
    let mut names: Vec<String> = state
        .naming()
        .names(ElementKind::Face)
        .map(|(_, name)| name.as_str().to_owned())
        .collect();
    names.sort();
    names
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
    let index = face_index(state, name)
        .unwrap_or_else(|| panic!("no face named {name}; faces: {:?}", face_names(state)));
    let shape = state.shape().expect("a body with named faces has a solid");
    shape
        .element(SubShapeKind::Face, index)
        .unwrap_or_else(|error| panic!("geometry of {name}: {error}"))
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

/// Asserts a flat face's outward unit normal, sign included (absolute 1e-9).
pub fn assert_face_normal(geometry: &ElementGeometry, expected: [f64; 3]) {
    let normal = geometry
        .direction
        .unwrap_or_else(|| panic!("the face has no normal: {geometry:?}"));
    for (axis, (actual, wanted)) in components(normal).into_iter().zip(expected).enumerate() {
        assert_absolute(
            &format!("normal component {axis} of {geometry:?}"),
            actual,
            wanted,
            AXIS_TOLERANCE,
        );
    }
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

/// Asserts that a round face's axis passes through `point` (absolute 1e-9).
pub fn assert_face_axis_through(geometry: &ElementGeometry, point: [f64; 3]) {
    let location = geometry
        .location
        .unwrap_or_else(|| panic!("the face has no axis location: {geometry:?}"));
    let direction = components(
        geometry
            .direction
            .unwrap_or_else(|| panic!("the face has no axis: {geometry:?}")),
    );
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
    let distance = offset
        .iter()
        .zip(direction)
        .map(|(component, axis)| (component - along * axis).powi(2))
        .sum::<f64>()
        .sqrt();
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
    assert!(runs > 0, "at least one run");
    let mut durations: Vec<Duration> = (0..runs)
        .map(|_| {
            let run = Run::new(document.clone());
            run.assert_ok(feature);
            let timing = &run.feature(feature).timing;
            assert!(!timing.from_cache, "a fresh recompute builds {feature}");
            timing.duration
        })
        .collect();
    durations.sort();
    durations[runs / 2]
}

// Helpers for the dress-up features (fillet, chamfer, shell): output states, unchanged bodies,
// name prefixes, repair data and the "too large" search (features.md sections 4.3 and 6.1).

/// The `BodyUpdate` of `body` in `feature`'s output.
pub fn body_update<'a>(
    run: &'a Run,
    feature: &str,
    body: &str,
) -> &'a partsmith_features::BodyUpdate {
    let output = run
        .feature(feature)
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("{feature} has no output; {}", run.summary()));
    let partsmith_features::OutputData::Bodies(updates) = &output.data else {
        panic!("{feature} did not output body updates");
    };
    updates
        .iter()
        .find(|update| update.body.as_str() == body)
        .unwrap_or_else(|| panic!("{feature} did not update body {body}"))
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
pub fn assert_list_field(field: Option<&partsmith_features::FieldRef>, list: &str, index: usize) {
    let field = field.unwrap_or_else(|| panic!("the message names no field; expected {list}"));
    let entry = format!("{list}[{index}]");
    assert!(
        field.path == list || field.path == entry,
        "the message field is {}, expected {list} or {entry}",
        field.path,
    );
}

/// Asserts element repair data: the JSON Pointer of the lost reference and the body it is resolved in.
pub fn assert_element_repair(
    repair: Option<partsmith_features::Repair>,
    pointer: &str,
    body: &str,
) {
    match repair {
        Some(partsmith_features::Repair::Element {
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
    assert!(runs > 0, "at least one run");
    let mut durations: Vec<Duration> = (0..runs)
        .map(|_| {
            let run = Run::new(document.clone());
            run.assert_error(feature, code);
            let timing = &run.feature(feature).timing;
            assert!(!timing.from_cache, "a fresh recompute builds {feature}");
            timing.duration
        })
        .collect();
    durations.sort();
    durations[runs / 2]
}

// Helpers for the sketch, extrude, revolve, hole and datum plane tests: sketch and datum outputs,
// states before a feature, message pointers, expression uses, placements, vertices, flat faces and
// canonical examples (features.md section 6.1).

/// The state of `body` just before `feature` (its `bodies_before`, which holds the bodies it declares).
pub fn body_before<'a>(run: &'a Run, feature: &str, body: &str) -> &'a BodyState {
    run.feature(feature)
        .bodies_before
        .iter()
        .find(|(id, _)| id.as_str() == body)
        .map(|(_, state)| state)
        .unwrap_or_else(|| {
            panic!(
                "{feature} has no state of body {body} before it; {}",
                run.summary()
            )
        })
}

/// The solved sketch that `feature` produced.
pub fn sketch_result<'a>(run: &'a Run, feature: &str) -> &'a partsmith_features::SketchResult {
    let output = run
        .feature(feature)
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("{feature} has no output; {}", run.summary()));
    let partsmith_features::OutputData::Sketch(result) = &output.data else {
        panic!("{feature} did not output a sketch");
    };
    result
}

/// The placement of the datum plane that `feature` produced.
pub fn datum_placement<'a>(run: &'a Run, feature: &str) -> &'a partsmith_sketch::Placement {
    let output = run
        .feature(feature)
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("{feature} has no output; {}", run.summary()));
    let partsmith_features::OutputData::Datum(partsmith_features::DatumResult::Plane(placement)) =
        &output.data
    else {
        panic!("{feature} did not output a datum plane");
    };
    placement
}

/// The rendered message of `feature` with this code.
pub fn message<'a>(run: &'a Run, feature: &str, code: &str) -> &'a partsmith_recompute::Message {
    run.feature(feature)
        .messages
        .iter()
        .find(|message| message.code == code)
        .unwrap_or_else(|| panic!("{feature} has no {code} message; {}", run.summary()))
}

/// Each expression use of `dependencies` as its field path, expression source and unit, in order.
pub fn expression_uses(
    dependencies: &Dependencies,
) -> Vec<(&str, &str, partsmith_model::FieldUnit)> {
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
    placement: &partsmith_sketch::Placement,
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
    let element = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    let index = state.naming().index(&element).unwrap_or_else(|| {
        panic!(
            "no vertex named {name}; vertices: {:?}",
            element_names(state, ElementKind::Vertex)
        )
    });
    let shape = state
        .shape()
        .expect("a body with named vertices has a solid");
    shape
        .element(SubShapeKind::Vertex, index)
        .unwrap_or_else(|error| panic!("geometry of {name}: {error}"))
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
// body, import): runs with asset bytes, every body update of an output, empty and unchanged bodies.

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

impl Run {
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
}

/// Every body update of `feature`'s output in output order, as `<body> <change>`, for example
/// `b-2 Clear`, so a test compares them with a spec's `[b-1 Join, b-2 Clear]`.
pub fn body_changes(run: &Run, feature: &str) -> Vec<String> {
    let output = run
        .feature(feature)
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("{feature} has no output; {}", run.summary()));
    let partsmith_features::OutputData::Bodies(updates) = &output.data else {
        panic!("{feature} did not output body updates");
    };
    updates
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

/// Median build time of `feature` over `runs` runs of `document` with these asset bytes, each with
/// a fresh `Recompute` so no result comes from the cache (features.md section 5).
pub fn median_build_time_with_assets(
    document: &Document,
    feature: &str,
    runs: usize,
    assets: &Assets,
) -> Duration {
    assert!(runs > 0, "at least one run");
    let mut durations: Vec<Duration> = (0..runs)
        .map(|_| {
            let run = Run::with_assets(document.clone(), assets);
            run.assert_ok(feature);
            let timing = &run.feature(feature).timing;
            assert!(!timing.from_cache, "a fresh recompute builds {feature}");
            timing.duration
        })
        .collect();
    durations.sort();
    durations[runs / 2]
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
