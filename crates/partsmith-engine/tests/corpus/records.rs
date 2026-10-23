//! Records of the corpus documents: numbers and references, sketch entities, planes, sketches and datum planes,
//! solid features, documents, and the corpus's shared setups and the references their picks store.

use partsmith_model::FORMAT_VERSION;
use serde_json::{Value, json};

// Numbers and references.

/// A number in fingerprint text form (naming.md section 3.7.3): six decimals, trailing zeros and a trailing point
/// removed, `-0` written as `0`.
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

/// A stored reference `{q, fp}` as a setup writes it before its picks.
pub fn reference(query: &str, fingerprint: &str) -> Value {
    json!({"q": query, "fp": fingerprint})
}

/// A region reference with this key and the fingerprint of a region with this centroid and area.
pub fn region(key: &str, center: [f64; 2], area: f64) -> Value {
    json!({
        "key": key,
        "fp": format!(
            "c={},{}|a={}",
            fingerprint_number(center[0]),
            fingerprint_number(center[1]),
            fingerprint_number(area)
        )
    })
}

/// The region reference of the rectangle `e-<first>` to `e-<first + 3>` from (x0, y0) to (x1, y1).
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

/// The region reference of a circle entity.
pub fn circle_region(entity: &str, center: [f64; 2], radius: f64) -> Value {
    region(entity, center, std::f64::consts::PI * radius * radius)
}

// Sketch entities.

/// A line entity.
pub fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// A circle entity.
pub fn circle(entity: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": entity, "type": "circle", "center": center, "radius": radius})
}

/// A point entity.
pub fn point(entity: &str, position: [f64; 2]) -> Value {
    json!({"id": entity, "type": "point", "position": position})
}

/// Four lines `e-<first>` to `e-<first + 3>` drawn counterclockwise from (x0, y0), as features.md section 6.1 `RECT`
/// draws them.
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

// Planes, sketches and datum planes.

/// An origin plane `{"kind": "origin", "plane": <name>}`.
pub fn origin_plane(name: &str) -> Value {
    json!({"kind": "origin", "plane": name})
}

/// A datum plane reference `{"kind": "datum", "feature": <id>}`.
pub fn datum_plane(feature: &str) -> Value {
    json!({"kind": "datum", "feature": feature})
}

/// A face plane of a sketch with its fallback placement; [`picked`] marks the fallback `fallback_frame: body`.
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

/// A sketch record with these constraints.
pub fn constrained_sketch(
    feature: &str,
    plane: Value,
    entities: Vec<Value>,
    constraints: Vec<Value>,
) -> Value {
    with(
        sketch(feature, plane, entities),
        "constraints",
        json!(constraints),
    )
}

/// A Distance constraint on the length of a line.
pub fn line_distance(constraint: &str, line: &str, expr: &str) -> Value {
    json!({"id": constraint, "type": "distance", "line": line, "expr": expr})
}

/// A coincident constraint between two points.
pub fn coincident(constraint: &str, first: &str, second: &str) -> Value {
    json!({"id": constraint, "type": "coincident", "points": [first, second]})
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

/// The same record with `key` set to `value`.
pub fn with(mut record: Value, key: &str, value: Value) -> Value {
    record[key] = value;
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

/// A linear pattern record `f-lp1` of `b-1` along the origin X axis.
pub fn linear_pattern(count: &str, spacing: &str) -> Value {
    json!({
        "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": count, "spacing": spacing}
    })
}

/// A boolean record `f-bool1`.
pub fn boolean(target: &str, tools: &[&str], op: &str) -> Value {
    json!({"id": "f-bool1", "type": "boolean", "target": target, "tools": tools, "op": op})
}

/// A revolve record into `body` about `axis` by `extent`.
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

/// A shell record `f-sh1` of `b-1` with this thickness, whose faces to remove are stored under `key` (`remove_faces`,
/// or a misspelled key).
pub fn shell(thickness: &str, key: &str, faces: Vec<Value>) -> Value {
    json!({"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": thickness, key: faces})
}

// Documents.

/// A `.ptsm` text in `units` with these parameters, these bodies, each named after its number (`b-2` is 'Body 2'), and
/// these features.
pub fn document_with(
    units: &str,
    parameters: &[Value],
    bodies: &[&str],
    features: &[Value],
) -> String {
    let bodies: Vec<Value> = bodies
        .iter()
        .map(|body| json!({"id": body, "name": format!("Body {}", body.trim_start_matches("b-"))}))
        .collect();
    let document = json!({
        "format": "partsmith",
        "format_version": FORMAT_VERSION,
        "units": units,
        "parameters": parameters,
        "bodies": bodies,
        "features": features
    });
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// A `.ptsm` text in millimeters with no parameters, these bodies and these features.
pub fn document(bodies: &[&str], features: &[Value]) -> String {
    document_with("mm", &[], bodies, features)
}

/// A parameter record.
pub fn parameter(id: &str, name: &str, expr: &str) -> Value {
    json!({"id": id, "name": name, "expr": expr})
}

// The corpus's shared setups and references.

/// CUBE20: `f-box1` box 20 x 20 x 20 in `b-1`.
pub fn cube20() -> Value {
    box_record("f-box1", "b-1", ["20", "20", "20"], None, "new")
}

/// CUBE10: `f-box1` box 10 x 10 x 10 in `b-1`.
pub fn cube10() -> Value {
    box_record("f-box1", "b-1", ["10", "10", "10"], None, "new")
}

/// BOX-BELOW: `f-box1` box 40 x 40 x 10 at (0, 0, -10) in `b-1` (top at z = 0).
pub fn box_below() -> Value {
    box_record(
        "f-box1",
        "b-1",
        ["40", "40", "10"],
        Some(["0", "0", "-10"]),
        "new",
    )
}

/// BLOCK's outline: the 80 x 50 rectangle `e-1` to `e-4`.
pub fn block_outline() -> Vec<Value> {
    rectangle(1, 0.0, 0.0, 80.0, 50.0)
}

/// BLOCK: the 80 x 50 rectangle `e-1` to `e-4` of `f-sk1` extruded 30 by `f-ext1` into `b-1`.
pub fn block() -> Vec<Value> {
    vec![
        sketch("f-sk1", origin_plane("XY"), block_outline()),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
            distance("30"),
            "new",
        ),
    ]
}

/// BLOCK followed by `more`.
pub fn block_then(more: Vec<Value>) -> Vec<Value> {
    let mut features = block();
    features.extend(more);
    features
}

/// BLOCK's back top edge.
pub fn block_back_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
    )
}

/// BLOCK's right top edge.
pub fn block_right_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
        "line|n=0,1,0|c=80,25,30|l=50|adj=2,2",
    )
}

/// BLOCK's front right vertical edge.
pub fn block_front_right_vertical() -> Value {
    reference(
        "edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)",
        "line|n=0,0,1|c=80,0,15|l=30|adj=2,2",
    )
}

/// BLOCK's top.
pub fn block_top() -> Value {
    reference("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4")
}

/// BLOCK's right side.
pub fn block_right_side() -> Value {
    reference(
        "f-ext1/lateral/e-2",
        "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4",
    )
}

/// BLOCK's back side.
pub fn block_back_side() -> Value {
    reference(
        "f-ext1/lateral/e-3",
        "plane|n=0,1,0|c=40,50,15|a=2400|adj=4,4",
    )
}

/// CUBE20's front top edge.
pub fn cube20_front_top() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

/// Corpus R06 (a)'s edit of BLOCK's outline: `e-2` deleted and `e-9` drawn in its place.
pub fn right_side_redrawn() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 50.0]),
    ]
}

/// Corpus R49's edit of BLOCK's outline: `e-2` ends at (80, 25) and the collinear `e-6` continues it to (80, 50).
pub fn right_side_split() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 25.0]),
        line("e-6", [80.0, 25.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ]
}

/// BLOCK's outline with `e-2` deleted and drawn again as two collinear lines, `e-9` (80, 0)-(80, 25) and `e-10`
/// (80, 25)-(80, 50), so the right side is replaced by two faces in one plane.
pub fn right_side_redrawn_in_two() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 25.0]),
        line("e-10", [80.0, 25.0], [80.0, 50.0]),
    ]
}

/// Corpus R66's edit of BLOCK's outline: `e-3` replaced by the five lines `e-9` to `e-13` of a notch.
pub fn back_line_notched() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 50.0], [50.0, 50.0]),
        line("e-10", [50.0, 50.0], [50.0, 40.0]),
        line("e-11", [50.0, 40.0], [30.0, 40.0]),
        line("e-12", [30.0, 40.0], [30.0, 50.0]),
        line("e-13", [30.0, 50.0], [0.0, 50.0]),
    ]
}

/// CUBE20's top.
pub fn cube20_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4")
}
