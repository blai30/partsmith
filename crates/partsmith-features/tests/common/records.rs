//! Feature records of the specs' shared setups, as JSON: `BLOCK` and `SLOT(a, b)` (reference stability corpus,
//! "Shared setups"), `CUBE10`, `CUBE20`, `BOX-BELOW`, and builders for sketches, extrudes, fillets and shells.

use serde_json::{Value, json};

/// A line entity.
pub fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// A circle entity.
pub fn circle(entity: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": entity, "type": "circle", "center": center, "radius": radius})
}

/// An arc entity, counterclockwise from `start_angle` to `end_angle` (radians).
pub fn arc(entity: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({"id": entity, "type": "arc", "center": center, "radius": radius, "start_angle": start_angle, "end_angle": end_angle})
}

/// A point entity.
pub fn point(entity: &str, position: [f64; 2]) -> Value {
    json!({"id": entity, "type": "point", "position": position})
}

/// `RECT(0, 0, 80, 50)`: the lines `e-1` to `e-4`, drawn counterclockwise.
pub fn rectangle() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ]
}

/// `entities` without those whose id is in `removed`, followed by `added`: a sketch edit.
pub fn edited(entities: Vec<Value>, removed: &[&str], added: Vec<Value>) -> Vec<Value> {
    entities
        .into_iter()
        .filter(|entity| !removed.contains(&entity["id"].as_str().unwrap_or_default()))
        .chain(added)
        .collect()
}

/// The origin plane XY.
pub fn xy() -> Value {
    json!({"kind": "origin", "plane": "XY"})
}

/// A sketch record on `plane`.
pub fn sketch(feature: &str, plane: Value, entities: Vec<Value>) -> Value {
    json!({"id": feature, "type": "sketch", "plane": plane, "entities": entities})
}

/// The region of `RECT(0, 0, 80, 50)` as a pick stores it since file format version 2 (`P-RECT-V2`, sketch.md
/// test 220's `R0`).
pub fn rectangle_region() -> Value {
    json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"})
}

/// A distance extent.
pub fn distance(expr: &str) -> Value {
    json!({"kind": "distance", "expr": expr})
}

/// An extrude into `b-1` of `regions` of `sketch`.
pub fn extrude(feature: &str, sketch: &str, regions: Vec<Value>, extent: Value, op: &str) -> Value {
    json!({
        "id": feature,
        "type": "extrude",
        "body": "b-1",
        "profile": {"sketch": sketch, "regions": regions},
        "extent": extent,
        "op": op
    })
}

/// `BLOCK` with the sketch entities `entities` (the rectangle, or an edit of it): `f-sk1` on XY and `f-ext1`, the
/// extrude of the stored rectangle region (`P-RECT-V2`), distance 30, op new (x 0..80, y 0..50, z 0..30).
pub fn block_with(entities: Vec<Value>) -> Vec<Value> {
    vec![
        sketch("f-sk1", xy(), entities),
        extrude(
            "f-ext1",
            "f-sk1",
            vec![rectangle_region()],
            distance("30"),
            "new",
        ),
    ]
}

/// `BLOCK`.
pub fn block() -> Vec<Value> {
    block_with(rectangle())
}

/// `SLOT(a, b)` on `BLOCK`: `f-dp1` datum XY offset 30, `f-sk2` on it with a rectangle x a..b, y -1..51 (lines
/// `e-21` to `e-24`), and `f-ext2`, its extrude, distance 5, reverse, op cut.
pub fn slot(from: f64, to: f64) -> Vec<Value> {
    let entities = vec![
        line("e-21", [from, -1.0], [to, -1.0]),
        line("e-22", [to, -1.0], [to, 51.0]),
        line("e-23", [to, 51.0], [from, 51.0]),
        line("e-24", [from, 51.0], [from, -1.0]),
    ];
    let center = (from + to) / 2.0;
    let area = (to - from) * 52.0;
    let mut cut = extrude(
        "f-ext2",
        "f-sk2",
        vec![json!({"key": "e-21,e-22,e-23,e-24", "fp": format!("c={center},25|a={area}")})],
        distance("5"),
        "cut",
    );
    cut["reverse"] = json!(true);
    vec![
        json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}),
        sketch(
            "f-sk2",
            json!({"kind": "datum", "feature": "f-dp1"}),
            entities,
        ),
        cut,
    ]
}

/// A box record in `b-1` with these sizes and position.
pub fn box_record(feature: &str, size: [&str; 3], position: [&str; 3], op: &str) -> Value {
    json!({"id": feature, "type": "box", "body": "b-1", "size": size, "position": position, "op": op})
}

/// `CUBE10`: `f-box1` box 10 x 10 x 10, op new (x -5..5, y -5..5, z 0..10).
pub fn cube10() -> Value {
    json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"})
}

/// `CUBE20`: `f-box1` box 20 x 20 x 20, op new (x -10..10, y -10..10, z 0..20).
pub fn cube20() -> Value {
    json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"})
}

/// `BOX-BELOW`: `f-box1` box 40 x 40 x 10 at (0, 0, -10), op new (top face at z = 0).
pub fn box_below() -> Value {
    box_record("f-box1", ["40", "40", "10"], ["0", "0", "-10"], "new")
}

/// A stored reference.
pub fn reference(query: &str, fingerprint: &str) -> Value {
    json!({"q": query, "fp": fingerprint})
}

/// A fillet `f-fil1` of `edges` in `b-1`.
pub fn fillet(edges: Vec<Value>, radius: &str) -> Value {
    json!({"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": edges, "radius": radius})
}

/// A shell `f-sh1` of `b-1` removing `faces`.
pub fn shell(thickness: &str, faces: Vec<Value>) -> Value {
    json!({"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": thickness, "remove_faces": faces})
}

/// `f-lp1`, a linear pattern of `b-1` along origin X with this count and spacing, op join.
pub fn linear_pattern(count: &str, spacing: &str) -> Value {
    json!({
        "id": "f-lp1",
        "type": "linear_pattern",
        "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": count, "spacing": spacing}
    })
}
