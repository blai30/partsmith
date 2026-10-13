//! Helpers for the reference stability tests R01 to R22 (`reference_stability_1.rs`, from
//! docs/design/reference-stability-corpus.md): feature records of the corpus's shared setups,
//! geometry probes for fillet, chamfer and shell results, a collector of unmet expectations and
//! resolver diagnostics.
//!
//! A scenario states several expectations (the result, the status, the message); [`Checks`] records
//! each one that is not met and fails the test once with all of them, so a failing scenario shows
//! everything that is wrong, with the resolver's view of the reference under test.
//!
//! It lives in its own directory, like `common/` and `feature_support/`, so Cargo does not build
//! it as a test target of its own.

#![cfg(feature = "stability-study")]
// Each user of the module needs only some of the helpers.
#![allow(dead_code)]

use std::f64::consts::PI;

use partsmith_features::{BodyState, Repair, SketchResult};
use partsmith_model::Document;
use partsmith_naming::{ElementKind, ElementName, Reference, Resolution, resolve, score};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, SubShapeKind};
use partsmith_recompute::{FeatureReport, Message, Report, Severity, Status};
use partsmith_sketch::{RegionRef, match_region};
use serde_json::{Value, json};

/// Positions and lengths the corpus compares exactly (1e-6 mm).
pub const POSITION_TOLERANCE: f64 = 1e-6;

/// Relative tolerance of the corpus's volumes and areas.
pub const RELATIVE_TOLERANCE: f64 = 1e-6;

/// Warning codes that report a lost or unusable reference or region, which a notice never is.
pub const LOSS_CODES: [&str; 6] = [
    "reference_missing",
    "reference_ambiguous",
    "region_missing",
    "region_ambiguous",
    "plane_fallback",
    "no_effect",
];

/// The phrase of today's generic lost-reference messages, which says neither what was lost in words
/// nor why.
pub const GENERIC_LOSS: &str = "can no longer be found";

// Sketch entities and feature records.

/// A sketch line.
pub fn line(id: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": id, "type": "line", "start": start, "end": end})
}

/// A sketch circle.
pub fn circle(id: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": id, "type": "circle", "center": center, "radius": radius})
}

/// A sketch arc, counterclockwise from `start_angle` to `end_angle` (radians).
pub fn arc(id: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({
        "id": id, "type": "arc", "center": center, "radius": radius,
        "start_angle": start_angle, "end_angle": end_angle
    })
}

/// Four lines `e-<first>` to `e-<first + 3>` drawn counterclockwise from the lower-left corner.
pub fn rectangle(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    let corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    (0..4)
        .map(|i| {
            line(
                &format!("e-{}", first + i),
                corners[i],
                corners[(i + 1) % 4],
            )
        })
        .collect()
}

/// The region ref of [`rectangle`]: its key and its centroid and area fingerprint.
pub fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let key = (first..first + 4)
        .map(|number| format!("e-{number}"))
        .collect::<Vec<_>>()
        .join(",");
    // Rust's f64 Display matches the region fingerprint text for these round values.
    let fingerprint = format!(
        "c={},{}|a={}",
        (x0 + x1) / 2.0,
        (y0 + y1) / 2.0,
        (x1 - x0) * (y1 - y0)
    );
    region(&key, &fingerprint)
}

/// A region ref.
pub fn region(key: &str, fingerprint: &str) -> Value {
    json!({"key": key, "fp": fingerprint})
}

/// An element reference `{q, fp}`.
pub fn reference(query: &str, fingerprint: &str) -> Value {
    json!({"q": query, "fp": fingerprint})
}

/// A sketch on an origin plane (`XY`, `XZ` or `YZ`).
pub fn sketch_on_origin(id: &str, plane: &str, entities: Vec<Value>) -> Value {
    json!({"id": id, "type": "sketch", "plane": {"kind": "origin", "plane": plane}, "entities": entities})
}

/// A sketch on a datum plane feature.
pub fn sketch_on_datum(id: &str, datum: &str, entities: Vec<Value>) -> Value {
    json!({"id": id, "type": "sketch", "plane": {"kind": "datum", "feature": datum}, "entities": entities})
}

/// A face sketch on `face` of `b-1` with its stored fallback placement.
pub fn sketch_on_face(
    id: &str,
    face: Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
    entities: Vec<Value>,
) -> Value {
    json!({
        "id": id, "type": "sketch",
        "plane": {
            "kind": "face", "body": "b-1", "face": face,
            "fallback": {"origin": origin, "x_axis": x_axis, "normal": normal}
        },
        "entities": entities
    })
}

/// A one-sided distance extent.
pub fn distance(expr: &str) -> Value {
    json!({"kind": "distance", "expr": expr})
}

/// A symmetric extent.
pub fn symmetric(expr: &str) -> Value {
    json!({"kind": "symmetric", "expr": expr})
}

/// A through-all extent.
pub fn through_all() -> Value {
    json!({"kind": "through_all"})
}

/// An extrude into `b-1`.
pub fn extrude(
    id: &str,
    sketch: &str,
    regions: Vec<Value>,
    extent: Value,
    op: &str,
    reverse: bool,
) -> Value {
    let mut record = json!({
        "id": id, "type": "extrude", "body": "b-1",
        "profile": {"sketch": sketch, "regions": regions},
        "extent": extent, "op": op
    });
    if reverse {
        record["reverse"] = json!(true);
    }
    record
}

/// A datum plane parallel to an origin plane at `offset`.
pub fn datum_on_origin(id: &str, plane: &str, offset: &str) -> Value {
    json!({"id": id, "type": "datum_plane", "base": {"kind": "origin", "plane": plane}, "offset": offset})
}

/// A datum plane on a face of `b-1` at `offset`.
pub fn datum_on_face(id: &str, face: Value, offset: &str) -> Value {
    json!({
        "id": id, "type": "datum_plane",
        "base": {"kind": "face", "body": "b-1", "face": face},
        "offset": offset
    })
}

/// A box in `body` at `position` (the default position when None).
pub fn box_record(
    id: &str,
    body: &str,
    size: [&str; 3],
    position: Option<[&str; 3]>,
    op: &str,
) -> Value {
    let mut record = json!({"id": id, "type": "box", "body": body, "size": size, "op": op});
    if let Some(position) = position {
        record["position"] = json!(position);
    }
    record
}

/// A fillet of `b-1`.
pub fn fillet(id: &str, edges: Vec<Value>, radius: &str) -> Value {
    json!({"id": id, "type": "fillet", "body": "b-1", "edges": edges, "radius": radius})
}

/// An equal-distance chamfer of `b-1`.
pub fn chamfer_equal(id: &str, edges: Vec<Value>, distance: &str) -> Value {
    json!({
        "id": id, "type": "chamfer", "body": "b-1", "edges": edges,
        "distance": {"kind": "equal", "expr": distance}
    })
}

/// A two-distance chamfer of `b-1`.
pub fn chamfer_two_distances(id: &str, edges: Vec<Value>, first: &str, second: &str) -> Value {
    json!({
        "id": id, "type": "chamfer", "body": "b-1", "edges": edges,
        "distance": {"kind": "two_distances", "expr": first, "second_expr": second}
    })
}

/// A shell of `b-1` removing `faces`.
pub fn shell(id: &str, thickness: &str, faces: Vec<Value>) -> Value {
    json!({"id": id, "type": "shell", "body": "b-1", "thickness": thickness, "remove_faces": faces})
}

/// A linear pattern of `b-1` along an origin axis (op join, the default).
pub fn linear_pattern(id: &str, axis: &str, count: &str, spacing: &str) -> Value {
    json!({
        "id": id, "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": axis}, "count": count, "spacing": spacing}
    })
}

/// A split of `b-1` by a datum plane, the other side going to `target`.
pub fn split_body(id: &str, datum: &str, target: &str) -> Value {
    json!({
        "id": id, "type": "split_body", "body": "b-1",
        "plane": {"kind": "datum", "feature": datum}, "target": target
    })
}

/// `record` with `suppressed` set.
pub fn suppressed(mut record: Value) -> Value {
    record["suppressed"] = json!(true);
    record
}

/// A parameter record.
pub fn parameter(id: &str, name: &str, expr: &str) -> Value {
    json!({"id": id, "name": name, "expr": expr})
}

// Documents.

/// A millimeter document with these parameters, bodies (`b-<n>` named `Body <n>`) and features.
pub fn document_with_parameters(
    parameters: Vec<Value>,
    bodies: &[&str],
    features: Vec<Value>,
) -> Document {
    let bodies: Vec<Value> = bodies
        .iter()
        .map(|body| {
            let number = body.strip_prefix("b-").expect("test bodies are b-<n>");
            json!({"id": body, "name": format!("Body {number}")})
        })
        .collect();
    let text = json!({
        "format": "partsmith", "format_version": 1, "units": "mm",
        "parameters": parameters, "bodies": bodies, "features": features
    })
    .to_string();
    match partsmith_model::load(&text) {
        Ok(loaded) => loaded.document,
        Err(error) => panic!("the test document does not load: {error}\n{text}"),
    }
}

/// A millimeter document without parameters.
pub fn document(bodies: &[&str], features: Vec<Value>) -> Document {
    document_with_parameters(Vec::new(), bodies, features)
}

// Shared setups of the corpus.

/// The BLOCK outline: `e-1` to `e-4`, an 80 x 50 rectangle.
pub fn block_outline() -> Vec<Value> {
    rectangle(1, 0.0, 0.0, 80.0, 50.0)
}

/// The block outline with its right side at `x` instead of 80.
pub fn block_outline_to(x: f64) -> Vec<Value> {
    rectangle(1, 0.0, 0.0, x, 50.0)
}

/// The BLOCK profile region as stored when the block was made.
pub fn block_region() -> Value {
    region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")
}

/// BLOCK with this outline in `f-sk1` and this extrude distance: `f-sk1` on XY and `f-ext1`, an
/// extrude of the stored block region, op new.
pub fn block_with(outline: Vec<Value>, height: &str) -> Vec<Value> {
    vec![
        sketch_on_origin("f-sk1", "XY", outline),
        extrude(
            "f-ext1",
            "f-sk1",
            vec![block_region()],
            distance(height),
            "new",
            false,
        ),
    ]
}

/// BLOCK: x 0..80, y 0..50, z 0..30.
pub fn block() -> Vec<Value> {
    block_with(block_outline(), "30")
}

/// `f-dp1`: datum XY offset 30 (the block top).
pub fn top_datum() -> Value {
    datum_on_origin("f-dp1", "XY", "30")
}

/// A slot cut 5 deep down from `f-dp1`: sketch `sketch` with a rectangle x `x0..x1`, y -1..51
/// (entities `e-5` to `e-8`) and its reversed extrude `extrude_id`, op cut.
pub fn slot(sketch: &str, extrude_id: &str, x0: f64, x1: f64) -> Vec<Value> {
    vec![
        sketch_on_datum(sketch, "f-dp1", rectangle(5, x0, -1.0, x1, 51.0)),
        extrude(
            extrude_id,
            sketch,
            vec![rectangle_region(5, x0, -1.0, x1, 51.0)],
            distance("5"),
            "cut",
            true,
        ),
    ]
}

/// SLOT(a, b) on BLOCK: `f-dp1`, `f-sk2` and `f-ext2`.
pub fn slot_setup(x0: f64, x1: f64) -> Vec<Value> {
    let mut records = vec![top_datum()];
    records.extend(slot("f-sk2", "f-ext2", x0, x1));
    records
}

/// CUBE20: `f-box1`, a 20 mm cube x -10..10, y -10..10, z 0..20.
pub fn cube20() -> Value {
    box_record("f-box1", "b-1", ["20", "20", "20"], None, "new")
}

/// The front top edge of CUBE20, as picked.
pub fn cube20_front_top() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

// Geometry probes.

/// The sub-shape kind of an element kind.
fn sub_shape(kind: ElementKind) -> SubShapeKind {
    match kind {
        ElementKind::Face => SubShapeKind::Face,
        ElementKind::Edge => SubShapeKind::Edge,
        ElementKind::Vertex => SubShapeKind::Vertex,
    }
}

/// The kernel geometry of the element named `name` (face, edge or vertex), if the body has one.
pub fn find_geometry(state: &BodyState, name: &str) -> Option<ElementGeometry> {
    let element = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    let index = state.naming().index(&element)?;
    let geometry = state
        .shape()?
        .element(sub_shape(element.kind()), index)
        .unwrap_or_else(|error| panic!("geometry of {name}: {error}"));
    Some(geometry)
}

/// The names of a body's elements of one kind, sorted.
pub fn names(state: &BodyState, kind: ElementKind) -> Vec<String> {
    let mut names: Vec<String> = state
        .naming()
        .names(kind)
        .map(|(_, name)| name.as_str().to_owned())
        .collect();
    names.sort();
    names
}

/// `names` as owned strings, sorted.
pub fn sorted(names: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    names.sort();
    names
}

/// The faces whose names start with `prefix`, with their geometry, sorted by name.
pub fn faces_with_prefix(state: &BodyState, prefix: &str) -> Vec<(String, ElementGeometry)> {
    names(state, ElementKind::Face)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .filter_map(|name| find_geometry(state, &name).map(|geometry| (name, geometry)))
        .collect()
}

/// The volume of a body's solid, or None when it is empty.
pub fn volume(state: &BodyState) -> Option<f64> {
    let shape = state.shape()?;
    Some(shape.volume().expect("the kernel measures the volume"))
}

/// The bounding box of a body's solid, minimum then maximum corner, or None when it is empty.
pub fn bounds(state: &BodyState) -> Option<([f64; 3], [f64; 3])> {
    let bounds = state
        .shape()?
        .bounding_box()
        .expect("the kernel bounds a solid");
    Some((bounds.min, bounds.max))
}

/// The number of solids of a body.
pub fn solid_count(state: &BodyState) -> usize {
    state.shape().map_or(0, |shape| {
        shape.solids().expect("the kernel lists the solids").len()
    })
}

/// The span along x of a quarter-cylinder fillet face of `radius` on an edge along x between two
/// perpendicular faces: centered at its centroid, as long as its area over `PI * radius / 2`.
pub fn fillet_span_along_x(geometry: &ElementGeometry, radius: f64) -> (f64, f64) {
    let length = geometry.measure / (PI * radius / 2.0);
    let center = geometry.centroid.x;
    (center - length / 2.0, center + length / 2.0)
}

/// The span along x of an equal-distance chamfer face on an edge along x between two
/// perpendicular faces: centered at its centroid, as long as its area over the chamfer width.
pub fn chamfer_span_along_x(geometry: &ElementGeometry, distance: f64) -> (f64, f64) {
    let length = geometry.measure / (distance * 2_f64.sqrt());
    let center = geometry.centroid.x;
    (center - length / 2.0, center + length / 2.0)
}

/// The spans along x of the fillet faces of `radius` whose names start with `prefix`, sorted.
pub fn fillet_spans(state: &BodyState, prefix: &str, radius: f64) -> Vec<(f64, f64)> {
    sorted_spans(
        faces_with_prefix(state, prefix)
            .iter()
            .map(|(_, geometry)| fillet_span_along_x(geometry, radius))
            .collect(),
    )
}

/// The spans along x of the equal-distance chamfer faces whose names start with `prefix`, sorted.
pub fn chamfer_spans(state: &BodyState, prefix: &str, distance: f64) -> Vec<(f64, f64)> {
    sorted_spans(
        faces_with_prefix(state, prefix)
            .iter()
            .map(|(_, geometry)| chamfer_span_along_x(geometry, distance))
            .collect(),
    )
}

/// Spans sorted by their start.
pub fn sorted_spans(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    spans
}

/// The coordinates of a kernel point.
pub fn coordinates(point: partsmith_occt_sys::Point) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// True when a straight edge's geometry has this centroid and length (1e-6 mm).
pub fn is_edge_at(geometry: &ElementGeometry, centroid: [f64; 3], length: f64) -> bool {
    coordinates(geometry.centroid)
        .iter()
        .zip(centroid)
        .all(|(a, b)| (a - b).abs() <= POSITION_TOLERANCE)
        && (geometry.measure - length).abs() <= POSITION_TOLERANCE
}

/// The names among `candidates` of edges in `state` with this centroid and length.
pub fn edges_at(
    state: &BodyState,
    candidates: &[ElementName],
    centroid: [f64; 3],
    length: f64,
) -> Vec<String> {
    candidates
        .iter()
        .filter(|name| name.kind() == ElementKind::Edge)
        .filter(|name| {
            find_geometry(state, name.as_str())
                .is_some_and(|geometry| is_edge_at(&geometry, centroid, length))
        })
        .map(|name| name.as_str().to_owned())
        .collect()
}

// Messages.

/// True when `text` has an id-like token at byte `start`: one of `f-`, `b-`, `e-`, `p-`, `c-` at a
/// word start, then lowercase letters, then a digit (`f-ext1`, `b-1`, `e-5`).
fn id_at(bytes: &[u8], start: usize) -> bool {
    let at_word_start = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
    if !at_word_start || !b"fbepc".contains(&bytes[start]) || bytes.get(start + 1) != Some(&b'-') {
        return false;
    }
    let mut position = start + 2;
    while bytes.get(position).is_some_and(u8::is_ascii_lowercase) {
        position += 1;
    }
    bytes.get(position).is_some_and(u8::is_ascii_digit)
}

/// Why a message is not in plain words (element names, ids or raw kernel text), if it is not.
pub fn plain_words_problem(text: &str) -> Option<String> {
    if text.contains("edge(") || text.contains("vertex(") {
        return Some(format!("it contains an element name: {text}"));
    }
    let bytes = text.as_bytes();
    if let Some(start) = (0..bytes.len()).find(|&start| id_at(bytes, start)) {
        return Some(format!("it contains an id at byte {start}: {text}"));
    }
    if text.contains("Standard_") || text.contains("BRep") {
        return Some(format!("it contains raw kernel text: {text}"));
    }
    None
}

/// The message texts of a feature.
pub fn texts(report: &FeatureReport) -> Vec<&str> {
    report
        .messages
        .iter()
        .map(|message| message.text.as_str())
        .collect()
}

/// The element repair data a message carries: its pointer and candidates.
pub fn element_repair(message: &Message) -> Option<(String, Vec<ElementName>)> {
    match &message.repair {
        Some(Repair::Element {
            pointer,
            candidates,
            ..
        }) => Some((pointer.clone(), candidates.clone())),
        _ => None,
    }
}

/// Every feature's status and messages, one per line.
pub fn summary(report: &Report) -> String {
    report
        .features
        .iter()
        .map(|feature| {
            format!(
                "  {}: {:?} {:?}",
                feature.feature,
                feature.status,
                texts(feature)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Diagnostics.

/// A reference from its JSON.
pub fn parse_reference(value: &Value) -> Reference {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{value} is not a valid reference: {error}"))
}

/// How the resolver resolves `reference` against a body state.
pub fn resolution(state: &BodyState, reference: &Value) -> Resolution {
    let reference = parse_reference(reference);
    let elements = state.elements().expect("element infos of the body");
    resolve(&reference, &elements, state.naming().aliases())
}

/// The resolution of `reference` against a body state and the five best fingerprint scores of
/// elements of its kind, for failure reports.
pub fn diagnose(state: &BodyState, reference: &Value) -> String {
    if state.is_empty() {
        return "the body is empty".to_owned();
    }
    let parsed = parse_reference(reference);
    let kind = parsed.query.kind();
    let elements = state.elements().expect("element infos of the body");
    let mut scores: Vec<(f64, &str)> = elements
        .iter()
        .filter(|info| info.kind == kind)
        .map(|info| {
            (
                score(&parsed.fingerprint, &info.fingerprint, kind),
                info.name.as_str(),
            )
        })
        .collect();
    scores.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
    scores.truncate(5);
    format!(
        "{} resolves to {:?}; best fingerprint scores {scores:?}",
        parsed.query,
        resolve(&parsed, &elements, state.naming().aliases())
    )
}

/// How a sketch's regions match a stored region ref, with every region's key, area and centroid.
pub fn diagnose_region(sketch: &SketchResult, region: &Value) -> String {
    let region_ref: RegionRef = serde_json::from_value(region.clone())
        .unwrap_or_else(|error| panic!("{region} is not a valid region ref: {error}"));
    let regions: Vec<String> = sketch
        .profile
        .regions
        .iter()
        .map(|region| {
            format!(
                "{} (area {}, centroid {:?})",
                region.key, region.area, region.centroid
            )
        })
        .collect();
    format!(
        "region {region} matches {:?}; the sketch's regions are {regions:?}",
        match_region(&sketch.profile.regions, &region_ref)
    )
}

// Expectations.

/// The unmet expectations of one scenario, with notes (diagnostics) for the failure report.
#[derive(Default)]
pub struct Checks {
    /// One line per unmet expectation.
    failures: Vec<String>,
    /// Diagnostics shown with the failures.
    notes: Vec<String>,
}

impl Checks {
    /// No expectations checked yet.
    pub fn new() -> Checks {
        Checks::default()
    }

    /// Adds a diagnostic to show if the scenario fails.
    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// Records an unmet expectation.
    pub fn fail(&mut self, failure: impl Into<String>) {
        self.failures.push(failure.into());
    }

    /// Records `failure` unless `met`.
    pub fn expect(&mut self, met: bool, failure: impl FnOnce() -> String) {
        if !met {
            self.fail(failure());
        }
    }

    /// `actual` within `tolerance` of `expected`.
    pub fn near(&mut self, what: &str, actual: f64, expected: f64, tolerance: f64) {
        self.expect((actual - expected).abs() <= tolerance, || {
            format!("{what}: expected {expected}, got {actual} (tolerance {tolerance})")
        });
    }

    /// `actual` within the corpus's relative tolerance of `expected`.
    pub fn relative(&mut self, what: &str, actual: f64, expected: f64) {
        self.near(what, actual, expected, RELATIVE_TOLERANCE * expected.abs());
    }

    /// Every coordinate of `actual` within 1e-6 of `expected`.
    pub fn point(&mut self, what: &str, actual: [f64; 3], expected: [f64; 3]) {
        let close = actual
            .iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() <= POSITION_TOLERANCE);
        self.expect(close, || {
            format!("{what}: expected {expected:?}, got {actual:?}")
        });
    }

    /// A body's volume (relative 1e-6); an empty body fails.
    pub fn volume(&mut self, what: &str, state: &BodyState, expected: f64) {
        match volume(state) {
            Some(actual) => self.relative(what, actual, expected),
            None => self.fail(format!("{what}: the body is empty, expected {expected}")),
        }
    }

    /// A body's bounding box (1e-6 mm); an empty body fails.
    pub fn bounds(&mut self, what: &str, state: &BodyState, min: [f64; 3], max: [f64; 3]) {
        match bounds(state) {
            Some((actual_min, actual_max)) => {
                self.point(&format!("{what} minimum"), actual_min, min);
                self.point(&format!("{what} maximum"), actual_max, max);
            }
            None => self.fail(format!("{what}: the body is empty")),
        }
    }

    /// The largest z of a body's bounding box (1e-6 mm).
    pub fn top_at(&mut self, what: &str, state: &BodyState, z: f64) {
        match bounds(state) {
            Some((_, max)) => self.near(what, max[2], z, POSITION_TOLERANCE),
            None => self.fail(format!("{what}: the body is empty")),
        }
    }

    /// The face, edge or vertex named `name`; records a failure when the body has none.
    pub fn element(&mut self, state: &BodyState, name: &str) -> Option<ElementGeometry> {
        let geometry = find_geometry(state, name);
        if geometry.is_none() {
            let kind = ElementName::parse(name)
                .expect("a valid element name")
                .kind();
            self.fail(format!(
                "no element named {name}; names of that kind: {:?}",
                names(state, kind)
            ));
        }
        geometry
    }

    /// The body has no face named `name`.
    pub fn no_face(&mut self, state: &BodyState, name: &str) {
        self.expect(find_geometry(state, name).is_none(), || {
            format!("unexpected face {name}")
        });
    }

    /// The body has no face whose name starts with `prefix`.
    pub fn no_faces_with_prefix(&mut self, state: &BodyState, prefix: &str) {
        let found: Vec<String> = faces_with_prefix(state, prefix)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        self.expect(found.is_empty(), || {
            format!("unexpected faces starting with {prefix}: {found:?}")
        });
    }

    /// The face names of a body, exactly.
    pub fn face_names(&mut self, what: &str, state: &BodyState, expected: &[&str]) {
        let actual = names(state, ElementKind::Face);
        let expected = sorted(expected);
        self.expect(actual == expected, || {
            format!("{what}: expected faces {expected:?}, got {actual:?}")
        });
    }

    /// A face is a cylinder of `radius` whose axis runs along `direction` (either sign) through
    /// `point`: how the corpus locates a fillet ("axis y=48, z=28").
    pub fn cylinder(
        &mut self,
        what: &str,
        geometry: &ElementGeometry,
        radius: f64,
        direction: [f64; 3],
        point: [f64; 3],
    ) {
        if geometry.kind != GeometryKind::Cylinder {
            self.fail(format!(
                "{what}: expected a cylinder, got {:?}",
                geometry.kind
            ));
            return;
        }
        if let Some(actual) = geometry.radius {
            self.near(
                &format!("{what} radius"),
                actual,
                radius,
                POSITION_TOLERANCE,
            );
        }
        let (Some(axis), Some(location)) = (geometry.direction, geometry.location) else {
            self.fail(format!("{what}: the cylinder has no axis"));
            return;
        };
        let axis = [axis.x, axis.y, axis.z];
        let dot: f64 = axis.iter().zip(direction).map(|(a, b)| a * b).sum();
        self.near(
            &format!("{what} axis direction (absolute cosine with {direction:?})"),
            dot.abs(),
            1.0,
            1e-9,
        );
        let location = coordinates(location);
        let offset = [0, 1, 2].map(|i| point[i] - location[i]);
        let along: f64 = offset.iter().zip(axis).map(|(a, b)| a * b).sum();
        let distance = offset
            .iter()
            .zip(axis)
            .map(|(component, unit)| (component - along * unit).powi(2))
            .sum::<f64>()
            .sqrt();
        self.near(
            &format!("{what}: distance from {point:?} to the axis"),
            distance,
            0.0,
            POSITION_TOLERANCE,
        );
    }

    /// A face is flat with this outward unit normal (1e-9).
    pub fn plane_normal(&mut self, what: &str, geometry: &ElementGeometry, normal: [f64; 3]) {
        if geometry.kind != GeometryKind::Plane {
            self.fail(format!("{what}: expected a plane, got {:?}", geometry.kind));
            return;
        }
        let Some(direction) = geometry.direction else {
            self.fail(format!("{what}: the plane has no normal"));
            return;
        };
        let actual = [direction.x, direction.y, direction.z];
        let close = actual
            .iter()
            .zip(normal)
            .all(|(a, b)| (a - b).abs() <= 1e-9);
        self.expect(close, || {
            format!("{what}: expected normal {normal:?}, got {actual:?}")
        });
    }

    /// Spans along x, in order (1e-6 mm).
    pub fn spans(&mut self, what: &str, actual: &[(f64, f64)], expected: &[(f64, f64)]) {
        let close = actual.len() == expected.len()
            && actual.iter().zip(expected).all(|(a, b)| {
                (a.0 - b.0).abs() <= POSITION_TOLERANCE && (a.1 - b.1).abs() <= POSITION_TOLERANCE
            });
        self.expect(close, || {
            format!("{what}: expected spans {expected:?}, got {actual:?}")
        });
    }

    /// The fillet faces whose names start with `prefix` are exactly these spans along x, each a
    /// cylinder of `radius` about an axis along x through (any x, `y`, `z`).
    pub fn fillets_along_x(
        &mut self,
        state: &BodyState,
        prefix: &str,
        radius: f64,
        y: f64,
        z: f64,
        expected: &[(f64, f64)],
    ) {
        for (name, geometry) in faces_with_prefix(state, prefix) {
            self.cylinder(&name, &geometry, radius, [1.0, 0.0, 0.0], [0.0, y, z]);
        }
        let spans = fillet_spans(state, prefix, radius);
        self.spans(&format!("faces {prefix}*"), &spans, expected);
    }

    /// The feature is ok: status ok with no warnings.
    pub fn clean(&mut self, report: &FeatureReport) {
        self.expect(report.status == Status::Ok, || {
            format!(
                "{} should be ok, but it is {:?}: {:?}",
                report.feature,
                report.status,
                texts(report)
            )
        });
    }

    /// The feature built (ok or warning) with at most notices: no warning reports a lost reference
    /// or region.
    pub fn built_with_at_most_notices(&mut self, report: &FeatureReport) {
        if !matches!(report.status, Status::Ok | Status::Warning) {
            self.fail(format!(
                "{} should build, but it is {:?}: {:?}",
                report.feature,
                report.status,
                texts(report)
            ));
            return;
        }
        let losses: Vec<&str> = report
            .warnings
            .iter()
            .map(|warning| warning.code())
            .filter(|code| LOSS_CODES.contains(code))
            .collect();
        self.expect(losses.is_empty(), || {
            format!(
                "{} reports losses {losses:?}: {:?}",
                report.feature,
                texts(report)
            )
        });
    }

    /// The feature failed; returns its error message.
    pub fn failed<'a>(&mut self, report: &'a FeatureReport) -> Option<&'a Message> {
        if report.status != Status::Error {
            self.fail(format!(
                "{} should fail, but it is {:?}: {:?}",
                report.feature,
                report.status,
                texts(report)
            ));
            return None;
        }
        report
            .messages
            .iter()
            .find(|message| message.severity == Severity::Error)
    }

    /// A message is in plain words (no element names, ids or kernel text).
    pub fn plain_words(&mut self, message: &Message) {
        if let Some(problem) = plain_words_problem(&message.text) {
            self.fail(format!("message not in plain words: {problem}"));
        }
    }

    /// A message names every one of `words` and is in plain words.
    pub fn message_names(&mut self, what: &str, message: &Message, words: &[&str]) {
        let missing: Vec<&&str> = words
            .iter()
            .filter(|word| !message.text.contains(**word))
            .collect();
        self.expect(missing.is_empty(), || {
            format!("{what} does not name {missing:?}: {}", message.text)
        });
        self.plain_words(message);
    }

    /// The feature has a message that is not an error and names every one of `words` (a notice
    /// on a feature that built), in plain words; returns it.
    pub fn notice_naming<'a>(
        &mut self,
        report: &'a FeatureReport,
        words: &[&str],
    ) -> Option<&'a Message> {
        let found = report.messages.iter().find(|message| {
            message.severity != Severity::Error
                && words.iter().all(|word| message.text.contains(word))
        });
        match found {
            Some(message) => {
                self.plain_words(message);
                Some(message)
            }
            None => {
                self.fail(format!(
                    "{} has no notice naming {words:?}; its messages: {:?}",
                    report.feature,
                    texts(report)
                ));
                None
            }
        }
    }

    /// The feature has a warning (not an error) whose field pointer is `pointer`; returns it.
    pub fn warning_at<'a>(
        &mut self,
        report: &'a FeatureReport,
        pointer: &str,
    ) -> Option<&'a Message> {
        let found = report.messages.iter().find(|message| {
            message.severity == Severity::Warning && message.pointer.as_deref() == Some(pointer)
        });
        if found.is_none() {
            self.fail(format!(
                "{} has no warning at {pointer}; its messages: {:?}",
                report.feature,
                report
                    .messages
                    .iter()
                    .map(|message| (message.severity, &message.pointer, &message.text))
                    .collect::<Vec<_>>()
            ));
        }
        found
    }

    /// The element repair data at `pointer` among a feature's messages; returns its candidates.
    pub fn repair_at(&mut self, report: &FeatureReport, pointer: &str) -> Vec<ElementName> {
        let found = report
            .messages
            .iter()
            .filter_map(element_repair)
            .find(|(at, _)| at == pointer);
        match found {
            Some((_, candidates)) => candidates,
            None => {
                self.fail(format!(
                    "{} has no repair data for {pointer}; its messages and repair data: {:?}",
                    report.feature,
                    report
                        .messages
                        .iter()
                        .map(|message| (&message.text, &message.repair))
                        .collect::<Vec<_>>()
                ));
                Vec::new()
            }
        }
    }

    /// Among `candidates`, an edge of `state` with this centroid and length (a one-click
    /// replacement the corpus names by position).
    pub fn candidate_edge(
        &mut self,
        what: &str,
        state: &BodyState,
        candidates: &[ElementName],
        centroid: [f64; 3],
        length: f64,
    ) {
        let found = edges_at(state, candidates, centroid, length);
        self.expect(!found.is_empty(), || {
            // The body's own edges at that position, which the candidates should have included.
            let edges: Vec<ElementName> = names(state, ElementKind::Edge)
                .iter()
                .map(|name| ElementName::parse(name).expect("a body's names are valid"))
                .collect();
            let present = edges_at(state, &edges, centroid, length);
            format!(
                "{what}: no candidate edge at {centroid:?} with length {length}; candidates \
                 {candidates:?}; the body's edges there: {present:?}"
            )
        });
    }

    /// A reference resolves exactly (method exact) against a body state.
    pub fn resolves_exactly(&mut self, what: &str, state: &BodyState, reference: &Value) {
        let resolution = resolution(state, reference);
        let exact = matches!(
            &resolution,
            Resolution::Resolved { method, .. } if *method == partsmith_naming::MatchMethod::Exact
        );
        self.expect(exact, || {
            format!("{what} should resolve exactly, but resolves to {resolution:?}")
        });
    }

    /// Fails the test with every unmet expectation and the notes, if any expectation is unmet.
    pub fn finish(self) {
        if self.failures.is_empty() {
            return;
        }
        panic!(
            "{} unmet expectation(s):\n- {}\nnotes:\n{}",
            self.failures.len(),
            self.failures.join("\n- "),
            self.notes.join("\n")
        );
    }
}
