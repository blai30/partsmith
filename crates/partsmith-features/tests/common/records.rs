//! Feature records of the specs' shared setups, as JSON: `BLOCK` and `SLOT(a, b)` (reference stability corpus,
//! "Shared setups"), `CUBE10`, `CUBE20`, `BOX-BELOW`, the L-shaped body of features/fillet.md test 4, the setups of
//! corpus R32b, R33 and R38, builders for sketches (`FACE-SK2` of features/sketch.md included), extrudes, fillets,
//! shells, linear patterns and the Move body by (100, 0, 0), and the stored references of elements that several tests
//! pick.

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

/// The right top edge of `BLOCK`, as the corpus stores it (R06, R49).
pub const RIGHT_TOP_EDGE: (&str, &str) = (
    "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
    "line|n=0,1,0|c=80,25,30|l=50|adj=2,2",
);

/// The front top edge of `BLOCK` (R19).
pub const FRONT_TOP_EDGE: (&str, &str) = (
    "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
    "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
);

/// The right side of `BLOCK` (R49).
pub const RIGHT_SIDE: (&str, &str) = (
    "f-ext1/lateral/e-2",
    "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4",
);

/// The top of `BLOCK` (R19, R58).
pub const BLOCK_TOP: (&str, &str) = ("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4");

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

/// The Move body `f-mv1` of `b-1` by (100, 0, 0).
pub fn move_by_100() -> Value {
    json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]})
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

/// The L-shaped body of features/fillet.md test 4 in `b-1`: `f-box1` 40 x 20 x 10 (x -20..20, y -10..10) and `f-box2`
/// 20 x 20 x 10 joined on its right half (x 0..20, z 10..20). `f-box1/top` lies at z = 10, `f-box2/top` at z = 20, and
/// the fronts merge into `f-box1/front`.
pub fn l_shaped_body() -> Vec<Value> {
    vec![
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}),
        box_record("f-box2", ["20", "20", "10"], ["10", "0", "10"], "join"),
    ]
}

/// The top of `l_shaped_body` stored with `f-box2/top` merged into `f-box1/top`: one reference that now resolves to
/// faces in two planes (features.md section 6.3 test 27).
pub fn both_tops() -> Value {
    json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=-10,0,10|a=400", "merged": {"f-box1/top": ["f-box2/top"]}})
}

/// The front top edge of `l_shaped_body` stored with `f-box2/top` merged into `f-box1/top`: one reference that now
/// resolves to two parallel edges at z = 10 and z = 20 (features.md section 6.3 test 27).
pub fn both_front_top_edges() -> Value {
    json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=-10,-10,10|l=20", "merged": {"f-box1/top": ["f-box2/top"]}})
}

/// The setup of corpus R38 after its edit: `RECT(0, 0, 40, 20)` with the shared point of `e-3`'s end and `e-4`'s start
/// moved to (`lean`, 20), so the left wall leans by `lean` mm, as `f-sk1` on XY, and `f-ext1`, its extrude of 10 into
/// `b-1`, op new.
pub fn leaning_wall(lean: f64) -> Vec<Value> {
    let entities = vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 0.0], [40.0, 20.0]),
        line("e-3", [40.0, 20.0], [lean, 20.0]),
        line("e-4", [lean, 20.0], [0.0, 0.0]),
    ];
    vec![
        sketch("f-sk1", xy(), entities),
        extrude(
            "f-ext1",
            "f-sk1",
            vec![json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"})],
            distance("10"),
            "new",
        ),
    ]
}

/// The left wall's bottom edge of `leaning_wall` as the corpus stores it (R38).
pub const LEFT_BOTTOM_EDGE: (&str, &str) = (
    "edge(f-ext1/lateral/e-4|f-ext1/start-cap)",
    "line|n=0,1,0|c=0,10,0|l=20|adj=2,2",
);

/// The setup of corpus R33 up to its cut (features/sketch.md test 22): `f-box1` box 20 x 20 x 40 in `b-1`, `f-dp1` at
/// z = 35, `f-sk3` on it with `RECT-11(-15, -15, 15, 15)`, and `f-ext3`, its cut of 10, which removes z 35..40 and so
/// the top `f-box1/top`, leaving its own start cap as the new top at z = 35.
pub fn cut_through_the_top() -> Vec<Value> {
    let square = vec![
        line("e-11", [-15.0, -15.0], [15.0, -15.0]),
        line("e-12", [15.0, -15.0], [15.0, 15.0]),
        line("e-13", [15.0, 15.0], [-15.0, 15.0]),
        line("e-14", [-15.0, 15.0], [-15.0, -15.0]),
    ];
    vec![
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "40"], "op": "new"}),
        json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "35"}),
        sketch(
            "f-sk3",
            json!({"kind": "datum", "feature": "f-dp1"}),
            square,
        ),
        extrude(
            "f-ext3",
            "f-sk3",
            vec![json!({"key": "e-11,e-12,e-13,e-14", "fp": "c=0,0|a=900"})],
            distance("10"),
            "cut",
        ),
    ]
}

/// The top of `f-box1` as a sketch attached when the box was 10 tall stored it (corpus R33).
pub const ATTACHED_BOX_TOP: (&str, &str) = ("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=400|adj=4,4");

/// `FACE-SK2(face, fallback, frame, entities)` of features/sketch.md section 6: the sketch `f-sk2` on `face` of `b-1`
/// with the fallback origin `origin`, x axis +X and normal +Z, in the coordinates `frame` names (`fallback_frame` left
/// out for `None`, as for `world` before file format version 2).
pub fn face_sketch(
    face: Value,
    origin: [f64; 3],
    frame: Option<&str>,
    entities: Vec<Value>,
) -> Value {
    let mut plane = json!({
        "kind": "face",
        "body": "b-1",
        "face": face,
        "fallback": {"origin": origin, "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
    });
    if let Some(frame) = frame {
        plane["fallback_frame"] = json!(frame);
    }
    sketch("f-sk2", plane, entities)
}

/// `BLOCK` with `f-ext1` reversed (corpus R32b): x 0..80, y 0..50, z -30..0, its end cap the bottom at z = -30, facing
/// -Z.
pub fn reversed_block() -> Vec<Value> {
    let mut features = block();
    features[1]["reverse"] = json!(true);
    features
}
