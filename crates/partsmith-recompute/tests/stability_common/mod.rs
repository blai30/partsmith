//! Helpers shared by the three thirds of the reference stability corpus
//! (`reference_stability_{1,2,3}.rs`, R01 to R66 of docs/design/reference-stability-corpus.md), by
//! the loss diagnosis tests (`diagnosis.rs`, recompute.md section 6), which run the corpus's
//! scenarios, and by the newcomer error corpus (`newcomer_errors.rs`, test 70): the feature records
//! (`records.rs`, with the shared setups and their picked references), documents, runs and edits,
//! fixes, the picks of the corpus's "Picked references" rule (`picks.rs`), run readers, resolver
//! diagnostics and the collected checks (`checks.rs`) that are the same in all of them. The test
//! target `integration` declares it beside `feature_support`; the second and last thirds use it
//! directly, and the first third's own helpers (`stability_support_1`) re-export it.
//! The first third checks against body states with its own `Expectations` at the corpus's exact
//! 1e-6 mm instead of `Checks`.

mod checks;
mod picks;
mod records;

pub use super::feature_support::{asset_name, axis_distance, id, saved, try_element};
pub use checks::{Checks, ExactMessage, plain_words_problem};
pub use picks::{run_picked, stored, with_picks};
pub use records::*;

use std::collections::BTreeSet;
use std::sync::Arc;

use partsmith_features::{BodyState, Fix, Repair};
use partsmith_model::Document;
use partsmith_naming::{ElementKind, ElementName, Reference, Resolution, resolve, score};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Plane, Point, Shape, SubShapeKind};
use partsmith_recompute::Message;
use serde_json::Value;

use super::feature_support::{
    Assets, Run, document_text, face_names, parse_document, point_components,
};

/// Tolerance in millimeters for the positions the second and last thirds compare exactly, 1e-5 mm
/// as in features.md section 6.1; the first third compares at the corpus's 1e-6 mm
/// (`EXACT_POSITION_TOLERANCE` of `stability_support_1`).
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

/// Runs the setup made of these test bodies and the feature records `setup`, then its edit made of
/// the same bodies and the feature records `edited` on the same recompute, with the setup's picks
/// carried over ([`edit`]): the same-session edit of an open document.
pub fn run_then_edit(bodies: &[&str], setup: &[Value], edited: &[Value]) -> Run {
    let mut session = run(bodies, setup);
    edit(&mut session, bodies, edited);
    session
}

// STEP assets.

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

// Reading results.

/// The codes of the messages of `feature`, in message order.
pub fn codes(run: &Run, feature: &str) -> Vec<&'static str> {
    run.feature(feature)
        .messages
        .iter()
        .map(|message| message.code)
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

/// The volume of `body` at the end of the run, 0 when it is empty.
pub fn volume_of(run: &Run, body: &str) -> f64 {
    volume(run.body(body)).unwrap_or(0.0)
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
    let mut faces: Vec<(String, ElementGeometry)> = state
        .naming()
        .names(ElementKind::Face)
        .filter(|(_, name)| name.as_str().starts_with(prefix))
        .map(|(index, name)| {
            let geometry = shape
                .element(SubShapeKind::Face, index)
                .unwrap_or_else(|error| panic!("geometry of {name}: {error}"));
            (name.as_str().to_owned(), geometry)
        })
        .collect();
    faces.sort_by(|(first, _), (second, _)| first.cmp(second));
    faces
}

/// A short description of faces for failure output: name, kind, centroid and radius.
pub fn describe_faces(faces: &[(String, ElementGeometry)]) -> String {
    if faces.is_empty() {
        return "none".to_owned();
    }
    faces
        .iter()
        .map(|(name, geometry)| {
            let [x, y, z] = point_components(geometry.centroid);
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

/// Every face of a body with its name and geometry.
pub fn all_faces(state: &BodyState) -> Vec<(String, ElementGeometry)> {
    faces_with_prefix(state, "")
}

/// The total area of the flat faces of a body whose outward normal is `normal` and that lie in the
/// plane through `point` with that normal.
pub fn area_in_plane(state: &BodyState, point: [f64; 3], normal: [f64; 3]) -> f64 {
    all_faces(state)
        .iter()
        .filter(|(_, geometry)| {
            let Some(direction) = geometry.direction else {
                return false;
            };
            let along = direction.x * normal[0] + direction.y * normal[1] + direction.z * normal[2];
            let [x, y, z] = point_components(geometry.centroid);
            let offset = (x - point[0]) * normal[0]
                + (y - point[1]) * normal[1]
                + (z - point[2]) * normal[2];
            geometry.kind == GeometryKind::Plane
                && (along - 1.0).abs() < 1e-9
                && offset.abs() < POSITION_TOLERANCE
        })
        .map(|(_, geometry)| geometry.measure)
        .sum()
}

/// The number of groups of faces of `shape` connected through shared edges: 1 for a solid
/// without voids, more when the solid encloses a cavity or the shape has separate pieces.
pub fn face_groups(shape: &Shape) -> usize {
    let topology = shape.topology().expect("the kernel reads the topology");
    let count = topology.faces.len();
    let mut parent: Vec<usize> = (0..count).collect();
    for edge in &topology.edges {
        let faces: Vec<usize> = edge.faces.iter().map(|face| *face as usize - 1).collect();
        for pair in faces.windows(2) {
            let first = root(&mut parent, pair[0]);
            let second = root(&mut parent, pair[1]);
            parent[first] = second;
        }
    }
    (0..count)
        .map(|index| root(&mut parent, index))
        .collect::<BTreeSet<_>>()
        .len()
}

/// The root of `index` in the union-find forest `parent`, halving the path on the way.
fn root(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

/// Every message of `feature` as `code: text`, for notes.
pub fn describe_messages(run: &Run, feature: &str) -> String {
    let report = run.feature(feature);
    let messages: Vec<String> = report
        .messages
        .iter()
        .map(|message| {
            let suggestion = message
                .suggestion
                .as_ref()
                .map(|text| format!(" [suggestion {text}]"))
                .unwrap_or_default();
            format!("{}: {}{suggestion}", message.code, message.text)
        })
        .collect();
    format!(
        "{feature} is {:?} with {}",
        report.status,
        if messages.is_empty() {
            "no messages".to_owned()
        } else {
            messages.join(" | ")
        }
    )
}

/// The element repair data a message carries, one per loss: each repair's pointer and the names
/// of its loss's candidates (features.md section 4.2: an element repair carries the loss).
pub fn element_repairs(message: &Message) -> Vec<(String, Vec<ElementName>)> {
    message
        .repairs
        .iter()
        .filter_map(|repair| match repair {
            Repair::Element { pointer, loss, .. } => Some((
                pointer.clone(),
                loss.candidates
                    .iter()
                    .map(|candidate| candidate.reference.query.clone())
                    .collect(),
            )),
            Repair::Region { .. } => None,
        })
        .collect()
}

/// The candidate element names of every element repair of `feature`, in message order.
pub fn repair_candidates(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .flat_map(element_repairs)
        .flat_map(|(_, candidates)| candidates)
        .map(|candidate| candidate.as_str().to_owned())
        .collect()
}

/// The suggestions of the messages of `feature`.
pub fn suggestions(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .filter_map(|message| message.suggestion.clone())
        .collect()
}
