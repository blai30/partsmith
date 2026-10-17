//! Helpers shared by the three thirds of the reference stability corpus (`stability_support_1`,
//! `stability_support_2` and `stability_support_3`, R01 to R66 of
//! docs/design/reference-stability-corpus.md) and by the loss diagnosis tests (`diagnosis.rs`,
//! recompute.md section 6), which run the corpus's scenarios: the feature records (`records.rs`,
//! with the shared setups and their picked references), documents, runs and edits, fixes, the
//! picks of the corpus's "Picked references" rule, run readers, resolver diagnostics and checks
//! that are the same in all of them. Each includes it with `#[path]` beside its own
//! `feature_support`; the corpus thirds re-export it.
//! The first third checks against body states with its own `Expectations` at the corpus's exact
//! 1e-6 mm instead of `Checks`, which reads the run.
//!
//! It lives in its own directory, like `common/` and `feature_support/`, so Cargo does not build
//! it as a test target of its own. It is not behind the `stability-study` feature, since the
//! diagnosis tests run in the normal suite.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

mod records;

pub use super::feature_support::{id, saved};
pub use records::*;

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{BodyState, Fix, OutputData, Repair};
use partsmith_model::Document;
use partsmith_naming::{
    ElementKind, ElementName, MatchMethod, Reference, Resolution, resolve, score,
};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Plane, Point, Shape, SubShapeKind};
use partsmith_recompute::Status;
use serde_json::{Map, Value, json};

use super::feature_support::{Assets, Run, document_text, face_names, parse_document};

/// Tolerance in millimeters for exact positions (the corpus gives 1e-6 mm; the kernel's own
/// measurements carry a little more noise, so 1e-5 mm as in features.md section 6.1).
pub const POSITION_TOLERANCE: f64 = 1e-5;

/// Tolerance in millimeters for positions the corpus gives as "about".
pub const ABOUT_TOLERANCE: f64 = 0.05;

/// Relative tolerance for volumes and areas.
pub const VOLUME_TOLERANCE: f64 = 1e-6;

// References, ids and fixes.

/// The parsed form of a stored reference.
pub fn parsed_reference(value: &Value) -> Reference {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{value} is not a valid reference: {error}"))
}

/// `Unsuppress` of `feature`.
pub fn unsuppress(feature: &str) -> Fix {
    Fix::Unsuppress {
        feature: id(feature),
    }
}

/// `Show` of `feature` itself.
pub fn show(feature: &str) -> Fix {
    Fix::Show {
        feature: id(feature),
        entity: None,
    }
}

// Documents and runs.

/// A document in `units` with these parameter records, test bodies and feature records
/// (features.md section 6.1).
pub fn document_in(
    units: &str,
    parameters: &[Value],
    bodies: &[&str],
    features: &[Value],
) -> Document {
    let parameters: Vec<String> = parameters.iter().map(Value::to_string).collect();
    let parameters: Vec<&str> = parameters.iter().map(String::as_str).collect();
    let features: Vec<String> = features.iter().map(Value::to_string).collect();
    let features: Vec<&str> = features.iter().map(String::as_str).collect();
    parse_document(&document_text(units, &parameters, bodies, &features))
}

/// A millimeter document without parameters, with these test bodies and feature records.
pub fn document_of(bodies: &[&str], features: &[Value]) -> Document {
    document_in("mm", &[], bodies, features)
}

/// Runs the setup made of these test bodies and feature records, with its references picked as
/// the corpus's "Picked references" rule says ([`run_picked`]).
pub fn run(bodies: &[&str], features: &[Value]) -> Run {
    run_with_assets(bodies, features, &Assets::new())
}

/// Runs the setup made of these test bodies and feature records with these asset bytes, with its
/// references picked ([`run_picked`]).
pub fn run_with_assets(bodies: &[&str], features: &[Value], assets: &Assets) -> Run {
    run_picked(document_of(bodies, features), assets)
}

/// The edit of the document of `run` made of these test bodies and feature records, with the
/// picks of `run`'s document carried over ([`with_picks`]).
pub fn edited_document(run: &Run, bodies: &[&str], features: &[Value]) -> Document {
    with_picks(&run.document, &document_of(bodies, features))
}

/// Runs the edit of `setup` made of these test bodies and feature records on a new recompute, with
/// the picks of `setup` carried over.
pub fn run_edited(setup: &Run, bodies: &[&str], features: &[Value]) -> Run {
    Run::new(edited_document(setup, bodies, features))
}

/// Replaces the document of `run` by the edit made of these test bodies and feature records, with
/// the picks of `run`'s document carried over, and runs it again on the same recompute, as an edit
/// in an open session does: results of the earlier runs stay in its cache.
pub fn edit(run: &mut Run, bodies: &[&str], features: &[Value]) {
    edit_document(run, document_of(bodies, features), &Assets::new());
}

/// Replaces the document of `run` by the edit `document`, with the picks of `run`'s document
/// carried over, and runs it again on the same recompute with these asset bytes.
pub fn edit_document(run: &mut Run, document: Document, assets: &Assets) {
    run.document = with_picks(&run.document, &document);
    run.rerun(assets);
}

// STEP assets.

/// An asset name made of one repeated hex digit, as the hash-named asset of a file revision.
pub fn asset_name(digit: char) -> String {
    format!("{}.step", digit.to_string().repeat(64))
}

/// STEP bytes of a plate 40 x 20 x 5 at x 0..40, y 0..20, z 0..5 with through holes of radius 3
/// at each `(x, y)` of `holes`, written with the kernel.
pub fn plate_bytes(holes: &[[f64; 2]]) -> Arc<[u8]> {
    let plate = partsmith_occt_sys::make_box(&Plane::XY, 40.0, 20.0, 5.0)
        .expect("the kernel makes the plate")
        .shape;
    let tools: Vec<Shape> = holes
        .iter()
        .map(|[x, y]| {
            let placement = Plane {
                origin: Point::new(*x, *y, -1.0),
                ..Plane::XY
            };
            partsmith_occt_sys::make_cylinder(&placement, 3.0, 7.0)
                .expect("the kernel makes the hole tool")
                .shape
        })
        .collect();
    let shape = if tools.is_empty() {
        plate
    } else {
        partsmith_occt_sys::cut(&plate, &tools)
            .expect("the kernel cuts the holes")
            .shape
    };
    let bodies = [partsmith_occt_sys::StepBody {
        shape,
        name: "Plate".to_owned(),
        color: None,
    }];
    Arc::from(partsmith_occt_sys::write_step(&bodies).expect("the kernel writes STEP"))
}

// Picked references (the corpus's "Picked references" rule).

/// The document `run` ran, with its references as picks store them: what `pick_reference` and the
/// sketch editor would have stored had the user picked each one in the state the setup builds.
///
/// Every stored reference `{q, fp}` whose query names an element of its body just before its
/// feature becomes `Reference::from_element` of that element (with anchors and merged names);
/// every profile region `{key, fp}` becomes `Region::reference()` of the region of its sketch with
/// that key (with its outline); every face sketch plane is marked `fallback_frame: body`, keeping
/// the fallback the scenario gives; and every edge or face axis stores the `direction` of its
/// element's fingerprint. A reference's body is the `body` beside it, else its record's `body`.
/// Anything else, a reference to an element the setup does not have included, stays as written.
pub fn picked_document(run: &Run) -> Document {
    let mut text = document_value(&run.document);
    for record in records_mut(&mut text) {
        let feature = record["id"]
            .as_str()
            .expect("every record has an id")
            .to_owned();
        let report = run.feature(&feature);
        let bodies: BTreeMap<&str, &BodyState> = report
            .bodies_before
            .iter()
            .map(|(body, state)| (body.as_str(), state))
            .collect();
        let body = record["body"].as_str().map(str::to_owned);
        pick_references(record, None, body.as_deref(), &bodies);
        if let Some(profile) = record.get_mut("profile") {
            pick_regions(run, profile);
        }
    }
    parse_document(&text.to_string())
}

/// Runs the setup `document` with these asset bytes on a new recompute, picks its references
/// ([`picked_document`]) and runs the picked document on the same recompute, so the setup is run
/// as the user would have made it.
pub fn run_picked(document: Document, assets: &Assets) -> Run {
    let mut run = Run::with_assets(document, assets);
    run.document = picked_document(&run);
    run.rerun(assets);
    run
}

/// `edited`, a document made from a picked setup by a scenario's edit, with the picks of `picked`
/// carried over (the setup as [`picked_document`] stored it, or a document already carrying its
/// picks): in every record of `edited` that `picked` has, each reference still stored in the
/// short form `{q, fp}` with the query `picked` stores at that place becomes `picked`'s
/// reference, each profile region still stored without an outline under `picked`'s key becomes
/// `picked`'s region, and each face sketch plane and axis whose reference is `picked`'s gains its
/// `fallback_frame` and `direction`. References the edit wrote itself stay as written.
pub fn with_picks(picked: &Document, edited: &Document) -> Document {
    let picked = document_value(picked);
    let picked_records: BTreeMap<&str, &Value> = picked["features"]
        .as_array()
        .expect("a saved document has features")
        .iter()
        .map(|record| {
            (
                record["id"].as_str().expect("every record has an id"),
                record,
            )
        })
        .collect();
    let mut text = document_value(edited);
    for record in records_mut(&mut text) {
        let partner = record["id"]
            .as_str()
            .and_then(|feature| picked_records.get(feature));
        if let Some(partner) = partner {
            *record = carried(partner, record);
        }
    }
    parse_document(&text.to_string())
}

/// The reference `feature` stores in `run`'s document with the query of `reference` (its pick),
/// or `reference` itself when the record stores none with that query.
pub fn stored(run: &Run, feature: &str, reference: &Value) -> Value {
    let text = document_value(&run.document);
    text["features"]
        .as_array()
        .expect("a saved document has features")
        .iter()
        .find(|record| record["id"].as_str() == Some(feature))
        .and_then(|record| find_reference(record, &reference["q"]))
        .cloned()
        .unwrap_or_else(|| reference.clone())
}

/// The saved form of a document as JSON.
fn document_value(document: &Document) -> Value {
    serde_json::from_str(&saved(document)).expect("a saved document is JSON")
}

/// The feature records of a document's JSON.
fn records_mut(document: &mut Value) -> &mut Vec<Value> {
    document["features"]
        .as_array_mut()
        .expect("a saved document has features")
}

/// True for a stored element reference `{q, fp, ...}`.
fn is_reference(value: &Value) -> bool {
    value.get("q").is_some_and(Value::is_string) && value.get("fp").is_some_and(Value::is_string)
}

/// True for a stored region reference `{key, fp, ...}`.
fn is_region(value: &Value) -> bool {
    value.get("key").is_some_and(Value::is_string) && value.get("fp").is_some_and(Value::is_string)
}

/// True for a reference without anchors and merged names, as references were written before
/// picks stored them.
fn is_short_reference(value: &Value) -> bool {
    is_reference(value) && value.get("nb").is_none() && value.get("merged").is_none()
}

/// The first element reference with query `query` in a record's JSON.
fn find_reference<'v>(value: &'v Value, query: &Value) -> Option<&'v Value> {
    match value {
        Value::Object(object) if is_reference(value) => {
            (object.get("q") == Some(query)).then_some(value)
        }
        Value::Object(object) => object
            .values()
            .find_map(|child| find_reference(child, query)),
        Value::Array(items) => items.iter().find_map(|item| find_reference(item, query)),
        _ => None,
    }
}

/// Picks every element reference in `value`, a part of a record found under `key`, resolving it
/// in `body` (the record's body unless a `body` beside the reference names another), and marks
/// face sketch planes and axes as picks store them.
fn pick_references(
    value: &mut Value,
    key: Option<&str>,
    body: Option<&str>,
    bodies: &BTreeMap<&str, &BodyState>,
) {
    if is_reference(value) {
        let picked = body
            .and_then(|body| bodies.get(body))
            .and_then(|state| picked_reference(state, value));
        if let Some(picked) = picked {
            *value = serde_json::to_value(picked).expect("a reference serializes");
        }
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                pick_references(item, key, body, bodies);
            }
        }
        Value::Object(object) => {
            let body = object
                .get("body")
                .and_then(Value::as_str)
                .or(body)
                .map(str::to_owned);
            for (child_key, child) in object.iter_mut() {
                pick_references(child, Some(child_key), body.as_deref(), bodies);
            }
            mark_pick(object, key);
        }
        _ => {}
    }
}

/// Marks a face sketch plane `fallback_frame: body`, and gives an edge or face axis the direction
/// of its element's fingerprint.
fn mark_pick(object: &mut Map<String, Value>, key: Option<&str>) {
    let Some(kind) = object
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    if kind == "face" && object.contains_key("fallback") {
        object.insert("fallback_frame".to_owned(), json!("body"));
    }
    if key == Some("axis") && matches!(kind.as_str(), "edge" | "face") {
        let direction = object
            .get(&kind)
            .filter(|reference| is_reference(reference))
            .and_then(|reference| parsed_reference(reference).fingerprint.direction);
        if let Some(direction) = direction {
            object.insert("direction".to_owned(), json!(direction));
        }
    }
}

/// `Reference::from_element` of the element `reference` names in `state`, if `state` has it.
fn picked_reference(state: &BodyState, reference: &Value) -> Option<Reference> {
    let name = ElementName::parse(reference["q"].as_str()?).ok()?;
    let set = state.element_set().ok()?;
    let info = set.elements().iter().find(|info| info.name == name)?;
    Some(Reference::from_element(info, &set))
}

/// Replaces each region of a profile `{sketch, regions}` by `Region::reference()` of the region
/// of the sketch's result in `run` with its key.
fn pick_regions(run: &Run, profile: &mut Value) {
    let Some(sketch) = profile["sketch"].as_str().map(str::to_owned) else {
        return;
    };
    let Some(output) = run.feature(&sketch).output.as_deref() else {
        return;
    };
    let OutputData::Sketch(result) = &output.data else {
        return;
    };
    let Some(regions) = profile["regions"].as_array_mut() else {
        return;
    };
    for stored in regions.iter_mut().filter(|stored| is_region(stored)) {
        let picked = result
            .profile
            .regions
            .iter()
            .map(|region| serde_json::to_value(region.reference()).expect("a region serializes"))
            .find(|picked| picked["key"] == stored["key"]);
        if let Some(picked) = picked {
            *stored = picked;
        }
    }
}

/// `edited` with the picks of `picked`, the same part of the picked setup, carried over (see
/// [`with_picks`]).
fn carried(picked: &Value, edited: &Value) -> Value {
    if is_reference(edited) {
        let same = is_reference(picked) && picked["q"] == edited["q"];
        return if same && is_short_reference(edited) {
            picked.clone()
        } else {
            edited.clone()
        };
    }
    if is_region(edited) {
        let same = is_region(picked) && picked["key"] == edited["key"];
        return if same && edited.get("outline").is_none() {
            picked.clone()
        } else {
            edited.clone()
        };
    }
    match (picked, edited) {
        (Value::Object(picked_object), Value::Object(edited_object)) => {
            let mut object: Map<String, Value> = edited_object
                .iter()
                .map(|(key, value)| {
                    let value = match picked_object.get(key) {
                        Some(partner) => carried(partner, value),
                        None => value.clone(),
                    };
                    (key.clone(), value)
                })
                .collect();
            carry_marks(picked_object, &mut object);
            Value::Object(object)
        }
        (Value::Array(picked_items), Value::Array(edited_items)) => Value::Array(
            edited_items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let partner = picked_items
                        .iter()
                        .find(|candidate| same_reference(candidate, item))
                        .or_else(|| picked_items.get(i));
                    partner.map_or_else(|| item.clone(), |partner| carried(partner, item))
                })
                .collect(),
        ),
        _ => edited.clone(),
    }
}

/// True when both values are element references with one query, or regions with one key.
fn same_reference(first: &Value, second: &Value) -> bool {
    (is_reference(first) && is_reference(second) && first["q"] == second["q"])
        || (is_region(first) && is_region(second) && first["key"] == second["key"])
}

/// Gives a face sketch plane or an axis the `fallback_frame` and `direction` of its picked
/// partner when its reference is the partner's.
fn carry_marks(picked: &Map<String, Value>, object: &mut Map<String, Value>) {
    let Some(kind) = object
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    if picked.get("kind").and_then(Value::as_str) != Some(kind.as_str())
        || picked.get(&kind) != object.get(&kind)
    {
        return;
    }
    for mark in ["fallback_frame", "direction"] {
        if let Some(value) = picked.get(mark) {
            object.entry(mark).or_insert_with(|| value.clone());
        }
    }
}

// Reading results.

/// The status of `feature` as a word: ok, warning, error, suppressed, rolled_back or not_run.
pub fn status(run: &Run, feature: &str) -> &'static str {
    match run.feature(feature).status {
        Status::Ok => "ok",
        Status::Warning => "warning",
        Status::Error => "error",
        Status::Suppressed => "suppressed",
        Status::RolledBack => "rolled_back",
        Status::NotRun => "not_run",
    }
}

/// The error code and warning codes of `feature`, error first.
pub fn codes(run: &Run, feature: &str) -> Vec<&'static str> {
    run.feature(feature)
        .messages
        .iter()
        .map(|message| message.code)
        .collect()
}

/// The volume of `body` at the end of the run, 0 when it is empty.
pub fn volume_of(run: &Run, body: &str) -> f64 {
    run.body(body)
        .shape()
        .map(|shape| shape.volume().expect("the kernel measures the volume"))
        .unwrap_or(0.0)
}

/// The volume and face names of `body` at the end of the run, to compare two results.
pub fn snapshot(run: &Run, body: &str) -> (f64, Vec<String>) {
    (volume_of(run, body), face_names(run.body(body)))
}

/// Every face of a body whose name starts with `prefix`, with its geometry, sorted by name.
pub fn faces_with_prefix(state: &BodyState, prefix: &str) -> Vec<(String, ElementGeometry)> {
    let Some(shape) = state.shape() else {
        return Vec::new();
    };
    state
        .naming()
        .names(ElementKind::Face)
        .filter(|(_, name)| name.as_str().starts_with(prefix))
        .map(|(index, name)| {
            let geometry = shape
                .element(SubShapeKind::Face, index)
                .unwrap_or_else(|error| panic!("geometry of {name}: {error}"));
            (name.as_str().to_owned(), geometry)
        })
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .collect()
}

/// The geometry of the element named `name` in a body, if it has one.
pub fn element(state: &BodyState, name: &str) -> Option<ElementGeometry> {
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

/// The centroid of an element geometry as an array.
pub fn centroid(geometry: &ElementGeometry) -> [f64; 3] {
    [
        geometry.centroid.x,
        geometry.centroid.y,
        geometry.centroid.z,
    ]
}

/// Distance from `point` to the axis line of a round face or straight edge.
pub fn axis_distance(geometry: &ElementGeometry, point: [f64; 3]) -> Option<f64> {
    let direction = geometry.direction?;
    let location = match geometry.kind {
        GeometryKind::Line => geometry.centroid,
        _ => geometry.location?,
    };
    let direction = [direction.x, direction.y, direction.z];
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

/// A short description of faces for failure output: name, kind, centroid and radius.
pub fn describe_faces(faces: &[(String, ElementGeometry)]) -> String {
    if faces.is_empty() {
        return "none".to_owned();
    }
    faces
        .iter()
        .map(|(name, geometry)| {
            let [x, y, z] = centroid(geometry);
            let radius = geometry
                .radius
                .map(|radius| format!(" r={radius:.3}"))
                .unwrap_or_default();
            format!(
                "{name} ({:?} at {x:.3},{y:.3},{z:.3} size {:.3}{radius})",
                geometry.kind, geometry.measure
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The resolution of the reference `feature` stores with the query of `reference` (its pick,
/// [`stored`]) against the state of `body` just before `feature`, or `None` when the feature did
/// not run or the body was empty.
pub fn resolution(run: &Run, feature: &str, body: &str, reference: &Value) -> Option<Resolution> {
    let state = try_body_before(run, feature, body)?;
    if state.is_empty() {
        return None;
    }
    let stored = stored(run, feature, reference);
    Some(resolve_in_state(&parsed_reference(&stored), state))
}

/// The resolution of `reference` against a body state.
pub fn resolve_in_state(reference: &Reference, state: &BodyState) -> Resolution {
    let set = state
        .element_set()
        .unwrap_or_else(|error| panic!("the element set of the body: {error}"));
    resolve(reference, &set)
}

/// A one-line description of a resolution.
pub fn describe_resolution(resolution: &Resolution) -> String {
    match resolution {
        Resolution::Resolved {
            elements,
            method,
            confidence,
            ..
        } => {
            let names: Vec<&str> = elements.iter().map(ElementName::as_str).collect();
            format!("resolved {method:?} (confidence {confidence:.4}) to {names:?}")
        }
        Resolution::Ambiguous { candidates } => {
            let names: Vec<String> = candidates
                .iter()
                .map(|candidate| format!("{} {:.4}", candidate.name, candidate.score))
                .collect();
            format!("ambiguous among {names:?}")
        }
        Resolution::Missing { candidates, .. } => match candidates.first() {
            Some(candidate) => format!(
                "missing (nearest {} {:.4})",
                candidate.name, candidate.score
            ),
            None => "missing (no candidate)".to_owned(),
        },
    }
}

/// What the resolver does with `reference` against the state of `body` just before `feature`:
/// the resolution and the four best fingerprint candidates of the reference's kind.
pub fn diagnose(run: &Run, feature: &str, body: &str, reference: &Value) -> String {
    let Some(state) = try_body_before(run, feature, body) else {
        return format!("{feature} has no state of {body} before it (it did not run)");
    };
    diagnose_in(state, &format!("{body} before {feature}"), reference)
}

/// What the resolver does with `reference` against a body state.
pub fn diagnose_in(state: &BodyState, place: &str, reference: &Value) -> String {
    if state.is_empty() {
        return format!("{place}: the body is empty");
    }
    let parsed = parsed_reference(reference);
    let elements = state
        .elements()
        .unwrap_or_else(|error| panic!("element infos in {place}: {error}"));
    let resolution = resolve_in_state(&parsed, state);
    let kind = parsed.query.kind();
    let mut scored: Vec<(f64, &str)> = elements
        .iter()
        .filter(|info| info.kind == kind)
        .map(|info| {
            (
                score(&parsed.fingerprint, &info.fingerprint, kind),
                info.name.as_str(),
            )
        })
        .collect();
    scored.sort_by(|first, second| second.0.total_cmp(&first.0).then(first.1.cmp(second.1)));
    let best: Vec<String> = scored
        .iter()
        .take(4)
        .map(|(value, name)| format!("{name} {value:.4}"))
        .collect();
    format!(
        "{} in {place}: {} | best fingerprint scores: {}",
        parsed.query,
        describe_resolution(&resolution),
        best.join(", ")
    )
}

/// The repair pointers of the messages of `feature`.
pub fn repair_pointers(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .flat_map(|message| &message.repairs)
        .map(|repair| match repair {
            Repair::Element { pointer, .. } | Repair::Region { pointer, .. } => pointer.clone(),
        })
        .collect()
}

/// The state of `body` just before `feature`, if the feature ran.
pub fn try_body_before<'a>(run: &'a Run, feature: &str, body: &str) -> Option<&'a BodyState> {
    run.feature(feature)
        .bodies_before
        .iter()
        .find(|(id, _)| id.as_str() == body)
        .map(|(_, state)| state)
}

// Collected checks.

/// Checks collected over one scenario; `finish` fails the test with all of them and the notes.
#[derive(Default)]
pub struct Checks {
    failures: Vec<String>,
    notes: Vec<String>,
}

impl Checks {
    /// No checks yet.
    pub fn new() -> Checks {
        Checks::default()
    }

    /// Records a failure unless `passed`.
    pub fn check(&mut self, passed: bool, what: impl Into<String>) {
        if !passed {
            self.failures.push(what.into());
        }
    }

    /// Adds a diagnostic line shown when the scenario fails.
    pub fn note(&mut self, text: impl Into<String>) {
        self.notes.push(text.into());
    }

    /// Checks `actual` against `expected` within an absolute tolerance.
    pub fn close(&mut self, what: &str, actual: f64, expected: f64, tolerance: f64) {
        self.check(
            (actual - expected).abs() <= tolerance,
            format!("{what}: expected {expected}, got {actual}"),
        );
    }

    /// Checks `actual` against `expected` within a relative tolerance.
    pub fn relative(&mut self, what: &str, actual: f64, expected: f64, tolerance: f64) {
        self.check(
            (actual - expected).abs() <= tolerance * expected.abs(),
            format!("{what}: expected {expected}, got {actual}"),
        );
    }

    /// Checks that `feature` has this status (ok means no warnings).
    pub fn status(&mut self, run: &Run, feature: &str, expected: &str) {
        let actual = status(run, feature);
        self.check(
            actual == expected,
            format!(
                "{feature} should be {expected}, but it is {actual} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that `feature` is ok or has only warnings (it built).
    pub fn built(&mut self, run: &Run, feature: &str) {
        let actual = status(run, feature);
        self.check(
            actual == "ok" || actual == "warning",
            format!(
                "{feature} should build, but it is {actual} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that every feature of the run is ok.
    pub fn all_ok(&mut self, run: &Run) {
        for report in &run.report.features {
            self.status(run, report.feature.as_str(), "ok");
        }
    }

    /// Checks that the error of `feature` has this code.
    pub fn error_code(&mut self, run: &Run, feature: &str, code: &str) {
        let actual = run
            .feature(feature)
            .error
            .as_ref()
            .map(|error| error.code());
        self.check(
            actual == Some(code),
            format!(
                "{feature} should fail with {code}, but its error is {actual:?} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that `body` at the end of `run` has the volume and face names of `expected`.
    pub fn same_result(&mut self, run: &Run, body: &str, expected: &(f64, Vec<String>)) {
        let actual = snapshot(run, body);
        self.relative(
            &format!("volume of {body} against the original"),
            actual.0,
            expected.0,
            VOLUME_TOLERANCE,
        );
        self.check(
            actual.1 == expected.1,
            format!(
                "face names of {body} differ from the original: {:?} against {:?}",
                actual.1, expected.1
            ),
        );
    }

    /// The geometry of the element `name`; records a failure when the body has none.
    pub fn geometry(&mut self, state: &BodyState, name: &str) -> Option<ElementGeometry> {
        let geometry = element(state, name);
        if geometry.is_none() {
            self.check(
                false,
                format!("no element named {name}; faces: {:?}", face_names(state)),
            );
        }
        geometry
    }

    /// Checks that `reference` resolves, against `body` just before `feature`, with `method` to
    /// exactly the elements `expected`.
    pub fn resolves(
        &mut self,
        run: &Run,
        feature: &str,
        body: &str,
        reference: &Value,
        method: MatchMethod,
        expected: &[&str],
    ) {
        let actual = resolution(run, feature, body, reference);
        let matches = match &actual {
            Some(Resolution::Resolved {
                elements,
                method: actual_method,
                ..
            }) => {
                *actual_method == method
                    && elements
                        .iter()
                        .map(ElementName::as_str)
                        .eq(expected.iter().copied())
            }
            _ => false,
        };
        self.check(
            matches,
            format!(
                "{} should resolve {method:?} to {expected:?} before {feature}, but it is {}",
                reference["q"],
                actual
                    .as_ref()
                    .map(describe_resolution)
                    .unwrap_or_else(|| "not resolved (the feature did not run)".to_owned())
            ),
        );
    }

    /// Checks that some message of `feature` contains every one of `words` (ignoring case).
    pub fn message_names(&mut self, run: &Run, feature: &str, words: &[&str]) {
        let messages = run.messages(feature);
        let found = messages.iter().any(|text| {
            let lower = text.to_lowercase();
            words
                .iter()
                .all(|word| lower.contains(&word.to_lowercase()))
        });
        self.check(
            found,
            format!("no message of {feature} names all of {words:?}: {messages:?}"),
        );
    }

    /// Checks that some message of `feature` contains one of `words` (ignoring case).
    pub fn message_names_one_of(&mut self, run: &Run, feature: &str, words: &[&str]) {
        let messages = run.messages(feature);
        let found = messages.iter().any(|text| {
            let lower = text.to_lowercase();
            words
                .iter()
                .any(|word| lower.contains(&word.to_lowercase()))
        });
        self.check(
            found,
            format!("no message of {feature} names any of {words:?}: {messages:?}"),
        );
    }

    /// Checks that the messages of `feature` are in words: no element names, feature ids or body
    /// ids (corpus, "Messages").
    pub fn plain_words(&mut self, run: &Run, feature: &str) {
        for text in run.messages(feature) {
            let has_element = text.contains("edge(") || text.contains("vertex(");
            let has_id = text
                .split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
                .any(|word| (word.starts_with("f-") || word.starts_with("b-")) && word.len() > 2);
            self.check(
                !has_element && !has_id,
                format!("a message of {feature} shows an element name or id: {text}"),
            );
        }
    }

    /// Checks that a message of `feature` carries repair data at `pointer`.
    pub fn repair_at(&mut self, run: &Run, feature: &str, pointer: &str) {
        let pointers = repair_pointers(run, feature);
        self.check(
            pointers.iter().any(|actual| actual == pointer),
            format!("{feature} has no repair data at {pointer} (repair pointers {pointers:?})"),
        );
    }

    /// Checks the volume of `body` at the end of the run.
    pub fn volume(&mut self, run: &Run, body: &str, expected: f64) {
        match run.body(body).shape() {
            Some(shape) => {
                let volume = shape.volume().expect("the kernel measures the volume");
                self.relative(
                    &format!("volume of {body}"),
                    volume,
                    expected,
                    VOLUME_TOLERANCE,
                );
            }
            None => self.check(
                false,
                format!("{body} is empty, expected volume {expected}"),
            ),
        }
    }

    /// Checks the number of solids of `body` at the end of the run.
    pub fn solids(&mut self, run: &Run, body: &str, expected: usize) {
        let count = run
            .body(body)
            .shape()
            .map(|shape| shape.solids().expect("the kernel lists the solids").len())
            .unwrap_or(0);
        self.check(
            count == expected,
            format!("{body} should have {expected} solids, it has {count}"),
        );
    }

    /// Checks the number of faces of `body` at the end of the run.
    pub fn face_count(&mut self, run: &Run, body: &str, expected: u32) {
        let count = run
            .body(body)
            .shape()
            .map(|shape| shape.count(SubShapeKind::Face))
            .unwrap_or(0);
        self.check(
            count == expected,
            format!("{body} should have {expected} faces, it has {count}"),
        );
    }

    /// Checks the tight bounding box of `body` at the end of the run.
    pub fn bounding_box(&mut self, run: &Run, body: &str, min: [f64; 3], max: [f64; 3]) {
        let Some(shape) = run.body(body).shape() else {
            self.check(false, format!("{body} is empty, expected a bounding box"));
            return;
        };
        let bounds = shape.bounding_box().expect("the kernel bounds the shape");
        for axis in 0..3 {
            let name = ["x", "y", "z"][axis];
            self.close(
                &format!("{body} bounding box minimum {name}"),
                bounds.min[axis],
                min[axis],
                POSITION_TOLERANCE,
            );
            self.close(
                &format!("{body} bounding box maximum {name}"),
                bounds.max[axis],
                max[axis],
                POSITION_TOLERANCE,
            );
        }
    }

    /// Checks that the body has (or has not) an element named `name`.
    pub fn has_element(&mut self, state: &BodyState, name: &str, expected: bool) {
        let present = element(state, name).is_some();
        self.check(
            present == expected,
            if expected {
                format!("no element named {name}; faces: {:?}", face_names(state))
            } else {
                format!("unexpected element named {name}")
            },
        );
    }

    /// Checks the number of faces of a body whose name starts with `prefix`, noting them.
    pub fn faces_count(&mut self, state: &BodyState, prefix: &str, expected: usize) {
        let faces = faces_with_prefix(state, prefix);
        self.check(
            faces.len() == expected,
            format!(
                "expected {expected} faces named {prefix}*, found {}: {}",
                faces.len(),
                describe_faces(&faces)
            ),
        );
    }

    /// Checks that the round face or straight edge `name` exists and its axis passes through
    /// `point` within `tolerance`.
    pub fn axis_through(&mut self, state: &BodyState, name: &str, point: [f64; 3], tolerance: f64) {
        match element(state, name) {
            Some(geometry) => match axis_distance(&geometry, point) {
                Some(distance) => self.check(
                    distance <= tolerance,
                    format!("the axis of {name} misses {point:?} by {distance}"),
                ),
                None => self.check(false, format!("{name} has no axis: {geometry:?}")),
            },
            None => self.check(
                false,
                format!("no element named {name}; faces: {:?}", face_names(state)),
            ),
        }
    }

    /// Checks the centroid of the element `name` within `tolerance` per coordinate.
    pub fn centroid_at(
        &mut self,
        state: &BodyState,
        name: &str,
        expected: [f64; 3],
        tolerance: f64,
    ) {
        match element(state, name) {
            Some(geometry) => {
                let actual = centroid(&geometry);
                let close = actual
                    .iter()
                    .zip(expected)
                    .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
                self.check(
                    close,
                    format!("centroid of {name}: expected {expected:?}, got {actual:?}"),
                );
            }
            None => self.check(
                false,
                format!("no element named {name}; faces: {:?}", face_names(state)),
            ),
        }
    }

    /// Fails the test with every failed check and the notes, if any check failed.
    pub fn finish(self) {
        if self.failures.is_empty() {
            return;
        }
        let mut text = String::from("the scenario differs from the corpus:\n");
        for failure in &self.failures {
            text.push_str(&format!("  - {failure}\n"));
        }
        if !self.notes.is_empty() {
            text.push_str("diagnostics:\n");
            for note in &self.notes {
                text.push_str(&format!("  * {note}\n"));
            }
        }
        panic!("{text}");
    }
}
