//! Helpers for the last third of the reference stability corpus, R45 to R65 of
//! docs/design/reference-stability-corpus.md, used by `reference_stability_3.rs`.
//!
//! Documents are written as `.ptsm` JSON records and run with `Registry::with_builtins()` through
//! the shared feature test helpers. Checks are collected rather than asserted one by one, so a
//! failing scenario reports every way its result differs from the corpus, together with what the
//! resolver did with the reference under test.
//!
//! It lives in its own directory, like `common/` and `feature_support/`, so Cargo does not build
//! it as a test target of its own.

#![cfg(feature = "stability-study")]
// The corpus test binary uses only some of these helpers.
#![allow(dead_code)]

#[path = "../feature_support/mod.rs"]
pub mod feature_support;
#[path = "../stability_common/mod.rs"]
mod stability_common;

pub use stability_common::*;

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::sync::Arc;

use feature_support::{Assets, Run};
use partsmith_features::{BodyState, Repair};
use partsmith_model::Document;
use partsmith_naming::score;
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Plane, Point, Shape};
use serde_json::{Value, json};

// Numbers and references.

/// A number in region fingerprint text form (sketch.md section 3.13.6): rounded to six decimals
/// and written in shortest form, a rounded zero written `0`.
pub fn region_number(value: f64) -> String {
    let rounded = (value * 1e6).round() / 1e6;
    if rounded == 0.0 {
        "0".to_owned()
    } else {
        format!("{rounded}")
    }
}

// Sketch entities, constraints and regions.

/// An arc entity running counterclockwise from `start_angle` to `end_angle` (radians from +u).
pub fn arc(entity: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({
        "id": entity, "type": "arc", "center": center, "radius": radius,
        "start_angle": start_angle, "end_angle": end_angle
    })
}

/// A dimensional constraint (`radius` or `diameter`) on a circle or arc.
pub fn size_constraint(constraint: &str, kind: &str, curve: &str, expr: &str) -> Value {
    json!({"id": constraint, "type": kind, "curve": curve, "expr": expr})
}

/// A region ref with this key and the fingerprint of a region with this centroid and area.
pub fn region(key: &str, center: [f64; 2], area: f64) -> Value {
    json!({
        "key": key,
        "fp": format!(
            "c={},{}|a={}",
            region_number(center[0]),
            region_number(center[1]),
            region_number(area)
        )
    })
}

/// The region ref of [`rectangle`].
pub fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let key = (first..first + 4)
        .map(|number| format!("e-{number}"))
        .collect::<Vec<_>>()
        .join(",");
    region(
        &key,
        [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
        (x1 - x0) * (y1 - y0),
    )
}

/// The region ref of a circle entity.
pub fn circle_region(entity: &str, center: [f64; 2], radius: f64) -> Value {
    region(entity, center, PI * radius * radius)
}

// Planes, sketches and datum planes.

/// A sketch record with constraints.
pub fn constrained_sketch(
    feature: &str,
    plane: Value,
    entities: Vec<Value>,
    constraints: Vec<Value>,
) -> Value {
    json!({
        "id": feature, "type": "sketch", "plane": plane, "entities": entities,
        "constraints": constraints
    })
}

// Solid features.

/// A symmetric extent.
pub fn symmetric(expr: &str) -> Value {
    json!({"kind": "symmetric", "expr": expr})
}

/// A through-all extent.
pub fn through_all() -> Value {
    json!({"kind": "through_all"})
}

/// A revolve record into `body`.
pub fn revolve(
    feature: &str,
    body: &str,
    sketch: &str,
    regions: Vec<Value>,
    axis: Value,
    extent: Value,
    op: &str,
) -> Value {
    json!({
        "id": feature, "type": "revolve", "body": body,
        "profile": {"sketch": sketch, "regions": regions},
        "axis": axis, "extent": extent, "op": op
    })
}

/// An angle extent of a revolve.
pub fn angle(expr: &str) -> Value {
    json!({"kind": "angle", "expr": expr})
}

/// The full-turn extent of a revolve.
pub fn full_turn() -> Value {
    json!({"kind": "full"})
}

/// An axis on an edge of a body.
pub fn edge_axis(body: &str, edge: Value) -> Value {
    json!({"kind": "edge", "body": body, "edge": edge})
}

/// A counterbore hole type.
pub fn counterbore(diameter: &str, depth: &str) -> Value {
    json!({"kind": "counterbore", "diameter": diameter, "depth": depth})
}

/// A linear pattern record of `b-1` along an origin axis (op join, the default).
pub fn linear_pattern(feature: &str, axis: &str, count: &str, spacing: &str) -> Value {
    json!({
        "id": feature, "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": axis}, "count": count, "spacing": spacing}
    })
}

/// A mirror record of `b-1` across `plane` (op join, the default).
pub fn mirror(feature: &str, plane: Value) -> Value {
    json!({"id": feature, "type": "mirror", "body": "b-1", "plane": plane})
}

/// A boolean record.
pub fn boolean(feature: &str, target: &str, tools: &[&str], op: &str) -> Value {
    json!({"id": feature, "type": "boolean", "target": target, "tools": tools, "op": op})
}

/// A split body record of `body` by a datum plane into `target`.
pub fn split_by_datum(feature: &str, body: &str, datum: &str, target: &str) -> Value {
    json!({
        "id": feature, "type": "split_body", "body": body, "plane": datum_plane(datum),
        "target": target
    })
}

/// An import record of an asset.
pub fn import(feature: &str, body: &str, asset: &str, op: &str) -> Value {
    json!({"id": feature, "type": "import", "body": body, "asset": asset, "op": op})
}

// Shared setups of the corpus.

/// BLOCK (naming.md test 81): `f-sk1` with the 80 x 50 rectangle on XY and `f-ext1` extruding it
/// by `height` into `b-1`.
pub fn block_with_height(height: &str) -> Vec<Value> {
    vec![
        sketch(
            "f-sk1",
            origin_plane("XY"),
            rectangle(1, 0.0, 0.0, 80.0, 50.0),
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
            distance(height),
            "new",
        ),
    ]
}

/// BLOCK with distance 30.
pub fn block() -> Vec<Value> {
    block_with_height("30")
}

/// BLOCK's extrude of `f-sk1` whose sketch has the given entities (the region ref is the one the
/// user picked on the 80 x 50 rectangle).
pub fn block_from_outline(outline: Vec<Value>) -> Vec<Value> {
    let mut features = block();
    features[0] = sketch("f-sk1", origin_plane("XY"), outline);
    features
}

/// A slot cut `depth` deep from the plane of datum `datum`, x from `x0` to `x1` across the whole
/// width of BLOCK, made by `sketch_id` and extrude `extrude_id`.
pub fn slot_cut(
    datum: &str,
    sketch_id: &str,
    extrude_id: &str,
    x0: f64,
    x1: f64,
    depth: &str,
) -> Vec<Value> {
    vec![
        sketch(
            sketch_id,
            datum_plane(datum),
            rectangle(5, x0, -1.0, x1, 51.0),
        ),
        with(
            extrude(
                extrude_id,
                "b-1",
                sketch_id,
                vec![rectangle_region(5, x0, -1.0, x1, 51.0)],
                distance(depth),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ]
}

/// SLOT(a, b) (naming.md test 84): `f-dp1` datum XY offset 30, `f-sk2` and `f-ext2` cutting 5 mm.
pub fn slot(x0: f64, x1: f64) -> Vec<Value> {
    let mut features = vec![datum_offset("f-dp1", "XY", "30")];
    features.extend(slot_cut("f-dp1", "f-sk2", "f-ext2", x0, x1, "5"));
    features
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

// Documents and runs.

/// The `.ptsm` text of a document in `units` with these parameter records, bodies as `(id, name)`
/// and feature records.
pub fn document_text(
    units: &str,
    parameters: &[Value],
    bodies: &[(&str, &str)],
    features: &[Value],
) -> String {
    let bodies: Vec<Value> = bodies
        .iter()
        .map(|(id, name)| json!({"id": id, "name": name}))
        .collect();
    json!({
        "format": "partsmith",
        "format_version": 1,
        "units": units,
        "parameters": parameters,
        "bodies": bodies,
        "features": features
    })
    .to_string()
}

/// The test bodies `b-<n>` named `Body <n>` (features.md section 6.1).
pub fn test_bodies<'a>(bodies: &[&'a str]) -> Vec<(&'a str, String)> {
    bodies
        .iter()
        .map(|body| {
            let number = body
                .strip_prefix("b-")
                .unwrap_or_else(|| panic!("test body ids look like b-1, not {body}"));
            (*body, format!("Body {number}"))
        })
        .collect()
}

/// A document in `units` with these test bodies and feature records.
pub fn document_in(units: &str, bodies: &[&str], features: &[Value]) -> Document {
    let named = test_bodies(bodies);
    let pairs: Vec<(&str, &str)> = named
        .iter()
        .map(|(id, name)| (*id, name.as_str()))
        .collect();
    feature_support::parse_document(&document_text(units, &[], &pairs, features))
}

/// A millimeter document with these test bodies and feature records.
pub fn document_of(bodies: &[&str], features: &[Value]) -> Document {
    document_in("mm", bodies, features)
}

/// Runs a millimeter document with these test bodies and feature records.
pub fn run(bodies: &[&str], features: &[Value]) -> Run {
    Run::new(document_of(bodies, features))
}

/// Runs a millimeter document with these asset bytes.
pub fn run_with_assets(bodies: &[&str], features: &[Value], assets: &Assets) -> Run {
    Run::with_assets(document_of(bodies, features), assets)
}

/// Replaces the document of `run` and runs it again on the same recompute, as an edit in an open
/// session does: results of the earlier runs stay in its cache.
pub fn edit(run: &mut Run, bodies: &[&str], features: &[Value]) {
    edit_document(run, document_of(bodies, features), &Assets::new());
}

/// Replaces the document of `run` and runs it again on the same recompute with these assets.
pub fn edit_document(run: &mut Run, document: Document, assets: &Assets) {
    run.document = document;
    run.rerun(assets);
}

// Reading results.

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
            let [x, y, z] = centroid(geometry);
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
    fn root(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }
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

/// What the resolver does with `reference` against the state of `body` just before `feature`:
/// the resolution and the four best fingerprint candidates of the reference's kind.
pub fn diagnose(run: &Run, feature: &str, body: &str, reference: &Value) -> String {
    let Some(state) = state_before(run, feature, body) else {
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
        "{feature} is {} with {}",
        status(run, feature),
        if messages.is_empty() {
            "no messages".to_owned()
        } else {
            messages.join(" | ")
        }
    )
}

/// The candidate element names of every element repair of `feature`, in message order (features.md section 4.2:
/// an element repair carries the loss with its candidates).
pub fn repair_candidates(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .filter_map(|message| match &message.repair {
            Some(Repair::Element { loss, .. }) => Some(&loss.candidates),
            _ => None,
        })
        .flatten()
        .map(|candidate| candidate.reference.query.as_str().to_owned())
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

// Collected checks.

impl Checks {
    /// Checks that no message of `feature` contains `word` (ignoring case).
    pub fn message_never_says(&mut self, run: &Run, feature: &str, word: &str) {
        let messages = run.messages(feature);
        let found = messages
            .iter()
            .any(|text| text.to_lowercase().contains(&word.to_lowercase()));
        self.check(
            !found,
            format!("a message of {feature} says {word:?}: {messages:?}"),
        );
    }

    /// Checks that `feature` has at least one message (a notice, warning or error).
    pub fn has_message(&mut self, run: &Run, feature: &str, what: &str) {
        self.check(
            !run.feature(feature).messages.is_empty(),
            format!(
                "{feature} should tell the user {what}, but it has no message ({})",
                status(run, feature)
            ),
        );
    }

    /// Checks that the repair data of `feature` offers every one of `names` as a candidate.
    pub fn repair_offers(&mut self, run: &Run, feature: &str, names: &[&str]) {
        let candidates = repair_candidates(run, feature);
        for name in names {
            self.check(
                candidates.iter().any(|candidate| candidate == name),
                format!(
                    "{feature} does not offer {name} in its repair data (candidates {candidates:?})"
                ),
            );
        }
    }

    /// Checks that the repair data of `feature` offers at least one of `names` as a candidate.
    pub fn repair_offers_one_of(&mut self, run: &Run, feature: &str, names: &[&str]) {
        let candidates = repair_candidates(run, feature);
        self.check(
            names
                .iter()
                .any(|name| candidates.iter().any(|candidate| candidate == name)),
            format!(
                "{feature} offers none of {names:?} in its repair data (candidates {candidates:?})"
            ),
        );
    }

    /// Checks that some message of `feature` suggests exactly `expected` as a one-click fix.
    pub fn suggests(&mut self, run: &Run, feature: &str, expected: &str) {
        let offered = suggestions(run, feature);
        self.check(
            offered.iter().any(|text| text == expected),
            format!("{feature} should suggest {expected:?}, it suggests {offered:?}"),
        );
    }

    /// Checks that `body` is empty at the end of the run.
    pub fn empty(&mut self, run: &Run, body: &str) {
        self.check(
            run.body(body).is_empty(),
            format!(
                "{body} should be empty, but it has volume {}",
                volume_of(run, body)
            ),
        );
    }

    /// Checks a placement within `tolerance`.
    pub fn placement(
        &mut self,
        what: &str,
        placement: &partsmith_sketch::Placement,
        origin: [f64; 3],
        x_axis: Option<[f64; 3]>,
        normal: [f64; 3],
        tolerance: f64,
    ) {
        let mut parts = vec![
            ("origin", placement.origin, origin),
            ("normal", placement.normal, normal),
        ];
        if let Some(x_axis) = x_axis {
            parts.push(("x axis", placement.x_axis, x_axis));
        }
        for (part, actual, expected) in parts {
            let close = actual
                .iter()
                .zip(expected)
                .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
            self.check(
                close,
                format!("{what} placement {part}: expected {expected:?}, got {actual:?}"),
            );
        }
    }

    /// Checks the placement of the sketch `feature` within `tolerance`.
    pub fn sketch_placement(
        &mut self,
        run: &Run,
        feature: &str,
        origin: [f64; 3],
        x_axis: [f64; 3],
        normal: [f64; 3],
        tolerance: f64,
    ) {
        if run.feature(feature).output.is_none() {
            self.check(false, format!("{feature} has no output"));
            return;
        }
        let placement = feature_support::sketch_result(run, feature).placement;
        self.placement(feature, &placement, origin, Some(x_axis), normal, tolerance);
    }
}
