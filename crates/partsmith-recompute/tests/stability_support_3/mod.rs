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

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::sync::Arc;

use feature_support::{Assets, Run, face_names};
use partsmith_features::{BodyState, Repair};
use partsmith_model::Document;
use partsmith_naming::{
    ElementKind, ElementName, MatchMethod, Reference, Resolution, resolve, score,
};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Plane, Point, Shape, SubShapeKind};
use partsmith_recompute::Status;
use serde_json::{Value, json};

/// Tolerance in millimeters for exact positions (the corpus gives 1e-6 mm; the kernel's own
/// measurements carry a little more noise, so 1e-5 mm as in features.md section 6.1).
pub const POSITION_TOLERANCE: f64 = 1e-5;

/// Tolerance in millimeters for positions the corpus gives as "about".
pub const ABOUT_TOLERANCE: f64 = 0.05;

/// Relative tolerance for volumes and areas.
pub const VOLUME_TOLERANCE: f64 = 1e-6;

// Numbers and references.

/// A number in fingerprint text form (naming.md section 3.7.3): six decimals, trailing zeros and
/// a trailing point removed, `-0` written as `0`.
pub fn fingerprint_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_owned();
    }
    text
}

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

/// A stored reference `{q, fp}`.
pub fn reference(query: &str, fingerprint: &str) -> Value {
    json!({"q": query, "fp": fingerprint})
}

/// The parsed form of a stored reference.
pub fn parsed_reference(value: &Value) -> Reference {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{value} is not a valid reference: {error}"))
}

// Sketch entities, constraints and regions.

/// A line entity.
pub fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// A circle entity.
pub fn circle(entity: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": entity, "type": "circle", "center": center, "radius": radius})
}

/// An arc entity running counterclockwise from `start_angle` to `end_angle` (radians from +u).
pub fn arc(entity: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({
        "id": entity, "type": "arc", "center": center, "radius": radius,
        "start_angle": start_angle, "end_angle": end_angle
    })
}

/// A point entity.
pub fn point(entity: &str, position: [f64; 2]) -> Value {
    json!({"id": entity, "type": "point", "position": position})
}

/// A dimensional constraint (`radius` or `diameter`) on a circle or arc.
pub fn size_constraint(constraint: &str, kind: &str, curve: &str, expr: &str) -> Value {
    json!({"id": constraint, "type": kind, "curve": curve, "expr": expr})
}

/// Four lines `e-<first>` to `e-<first + 3>` drawn counterclockwise from (x0, y0), as features.md
/// section 6.1 `RECT` draws them.
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

/// An origin plane `{"kind": "origin", "plane": <name>}`.
pub fn origin_plane(name: &str) -> Value {
    json!({"kind": "origin", "plane": name})
}

/// A datum plane reference `{"kind": "datum", "feature": <id>}`.
pub fn datum_plane(feature: &str) -> Value {
    json!({"kind": "datum", "feature": feature})
}

/// A face plane of a sketch, with its fallback placement written after the reference.
pub fn face_plane(
    body: &str,
    face: Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
) -> Value {
    json!({
        "kind": "face",
        "body": body,
        "face": face,
        "fallback": {"origin": origin, "x_axis": x_axis, "normal": normal}
    })
}

/// A face plane of a sketch whose fallback is the XY frame at height `z`.
pub fn top_face_plane(body: &str, face: Value, z: f64) -> Value {
    face_plane(body, face, [0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
}

/// A sketch record without constraints.
pub fn sketch(feature: &str, plane: Value, entities: Vec<Value>) -> Value {
    json!({"id": feature, "type": "sketch", "plane": plane, "entities": entities})
}

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

/// A datum plane record offset from an origin plane.
pub fn datum_offset(feature: &str, plane: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "datum_plane", "base": origin_plane(plane), "offset": offset})
}

/// A datum plane record on a face of a body.
pub fn datum_on_face(feature: &str, body: &str, face: Value, offset: &str) -> Value {
    json!({
        "id": feature,
        "type": "datum_plane",
        "base": {"kind": "face", "body": body, "face": face},
        "offset": offset
    })
}

// Solid features.

/// A distance extent.
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

/// An extrude record into `body`.
pub fn extrude(
    feature: &str,
    body: &str,
    sketch: &str,
    regions: Vec<Value>,
    extent: Value,
    op: &str,
) -> Value {
    json!({
        "id": feature, "type": "extrude", "body": body,
        "profile": {"sketch": sketch, "regions": regions},
        "extent": extent, "op": op
    })
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

/// The same record with `key` set to `value`.
pub fn with(mut record: Value, key: &str, value: Value) -> Value {
    record[key] = value;
    record
}

/// The same record with `"suppressed": true`.
pub fn suppressed(record: Value) -> Value {
    with(record, "suppressed", json!(true))
}

/// The same record without its `suppressed` key (unsuppressed, as the host writes it).
pub fn unsuppressed(mut record: Value) -> Value {
    if let Some(object) = record.as_object_mut() {
        object.remove("suppressed");
    }
    record
}

/// A box record; `position` is omitted when `None`.
pub fn box_record(
    feature: &str,
    body: &str,
    size: [&str; 3],
    position: Option<[&str; 3]>,
    op: &str,
) -> Value {
    let mut record = json!({"id": feature, "type": "box", "body": body, "size": size});
    if let Some(position) = position {
        record["position"] = json!(position);
    }
    record["op"] = json!(op);
    record
}

/// A fillet record.
pub fn fillet(feature: &str, body: &str, edges: Vec<Value>, radius: &str) -> Value {
    json!({"id": feature, "type": "fillet", "body": body, "edges": edges, "radius": radius})
}

/// An equal-distance chamfer record.
pub fn chamfer(feature: &str, body: &str, edges: Vec<Value>, distance: &str) -> Value {
    json!({
        "id": feature, "type": "chamfer", "body": body, "edges": edges,
        "distance": {"kind": "equal", "expr": distance}
    })
}

/// A shell record.
pub fn shell(feature: &str, body: &str, thickness: &str, remove_faces: Vec<Value>) -> Value {
    json!({
        "id": feature, "type": "shell", "body": body, "thickness": thickness,
        "remove_faces": remove_faces
    })
}

/// A simple hole record on the points of `sketch`.
pub fn hole(
    feature: &str,
    body: &str,
    sketch: &str,
    points: &[&str],
    diameter: &str,
    extent: Value,
) -> Value {
    json!({
        "id": feature, "type": "hole", "body": body, "sketch": sketch,
        "points": points, "diameter": diameter, "extent": extent
    })
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

/// CUBE20: `f-box1` box 20 x 20 x 20 in `b-1`.
pub fn cube20() -> Value {
    box_record("f-box1", "b-1", ["20", "20", "20"], None, "new")
}

/// CUBE10: `f-box1` box 10 x 10 x 10 in `b-1`.
pub fn cube10() -> Value {
    box_record("f-box1", "b-1", ["10", "10", "10"], None, "new")
}

/// BOX-BELOW: `f-box1` box 40 x 40 x 10 at (0,0,-10) in `b-1` (top at z = 0).
pub fn box_below() -> Value {
    box_record(
        "f-box1",
        "b-1",
        ["40", "40", "10"],
        Some(["0", "0", "-10"]),
        "new",
    )
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

/// The canonical saved text of a document.
pub fn saved(document: &Document) -> String {
    partsmith_model::save(document)
        .unwrap_or_else(|error| panic!("the document does not save: {error}"))
}

/// The features without the records `removed`.
pub fn without(features: &[Value], removed: &[&str]) -> Vec<Value> {
    features
        .iter()
        .filter(|record| {
            !removed
                .iter()
                .any(|feature| record["id"].as_str() == Some(feature))
        })
        .cloned()
        .collect()
}

/// The features with `inserted` placed directly before the record `before`.
pub fn insert_before(features: &[Value], before: &str, inserted: Vec<Value>) -> Vec<Value> {
    let position = features
        .iter()
        .position(|record| record["id"].as_str() == Some(before))
        .unwrap_or_else(|| panic!("no feature {before}"));
    let mut result = features[..position].to_vec();
    result.extend(inserted);
    result.extend_from_slice(&features[position..]);
    result
}

/// The features with the records `moved` taken out and appended at the end, in their order.
pub fn move_to_end(features: &[Value], moved: &[&str]) -> Vec<Value> {
    let mut result = without(features, moved);
    for feature in moved {
        let record = features
            .iter()
            .find(|record| record["id"].as_str() == Some(feature))
            .unwrap_or_else(|| panic!("no feature {feature}"));
        result.push(record.clone());
    }
    result
}

/// The features with the record `feature` replaced by `change(record)`.
pub fn edited(
    features: &[Value],
    feature: &str,
    change: impl FnOnce(Value) -> Value,
) -> Vec<Value> {
    let mut change = Some(change);
    features
        .iter()
        .map(|record| match change.take() {
            Some(apply) if record["id"].as_str() == Some(feature) => apply(record.clone()),
            other => {
                change = other;
                record.clone()
            }
        })
        .collect()
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

/// The state of `body` at the end of the run.
pub fn final_state<'a>(run: &'a Run, body: &str) -> &'a BodyState {
    run.body(body)
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

/// Every face of a body with its name and geometry.
pub fn all_faces(state: &BodyState) -> Vec<(String, ElementGeometry)> {
    faces_with_prefix(state, "")
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

/// The resolution of `reference` against the state of `body` just before `feature`, or `None`
/// when the feature did not run or the body was empty.
pub fn resolution(run: &Run, feature: &str, body: &str, reference: &Value) -> Option<Resolution> {
    let state = state_before(run, feature, body)?;
    if state.is_empty() {
        return None;
    }
    let elements = state
        .elements()
        .unwrap_or_else(|error| panic!("element infos of {body}: {error}"));
    Some(resolve(
        &parsed_reference(reference),
        &elements,
        state.naming().aliases(),
    ))
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
    let resolution = resolve(&parsed, &elements, state.naming().aliases());
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

/// A one-line description of a resolution.
pub fn describe_resolution(resolution: &Resolution) -> String {
    match resolution {
        Resolution::Resolved {
            elements,
            method,
            confidence,
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
        Resolution::Missing { nearest } => match nearest {
            Some(candidate) => format!(
                "missing (nearest {} {:.4})",
                candidate.name, candidate.score
            ),
            None => "missing (no candidate)".to_owned(),
        },
    }
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

/// The repair pointers of the messages of `feature`.
pub fn repair_pointers(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .filter_map(|message| match &message.repair {
            Some(Repair::Element { pointer, .. }) | Some(Repair::Region { pointer, .. }) => {
                Some(pointer.clone())
            }
            None => None,
        })
        .collect()
}

/// The candidate element names of every element repair of `feature`, in message order.
pub fn repair_candidates(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .filter_map(|message| match &message.repair {
            Some(Repair::Element { candidates, .. }) => Some(candidates),
            _ => None,
        })
        .flatten()
        .map(|name| name.as_str().to_owned())
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

/// The state of `body` just before `feature`, if the feature ran.
pub fn state_before<'a>(run: &'a Run, feature: &str, body: &str) -> Option<&'a BodyState> {
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
