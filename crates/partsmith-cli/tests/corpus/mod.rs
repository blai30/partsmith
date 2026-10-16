//! Documents of the reference stability corpus (`docs/design/reference-stability-corpus.md`) that cli.md section 6
//! tests 55 to 62 write as `.ptsm` files named after their scenario (`r16.ptsm`): each scenario's setup with its
//! references picked as the corpus's "Picked references" rule says, after its edit.
//!
//! [`picked`] opens a setup in a headless session, focuses each feature that stores a reference, so that the state is
//! the one just before that feature (engine.md section 3.5), stores the full reference `pick_reference` gives for each
//! element the feature names (engine.md section 3.9) and the reference `solve_sketch` gives for each profile region
//! (with its outline), marks each face sketch's fallback `fallback_frame: body`, and returns the saved text.
//! [`Scenario::edited_text`] opens that text, applies the corpus edit as one `apply_edits` and returns the saved text,
//! which is what the CLI then opens fresh.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

use partsmith_engine::{Session, SessionOptions};
use partsmith_model::FORMAT_VERSION;
use partsmith_protocol::Request;
use serde_json::{Value, json};

// Numbers, references and regions.

/// A number in fingerprint text form (naming.md section 3.7.3): six decimals, trailing zeros and a trailing point
/// removed, `-0` written as `0`.
fn fingerprint_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        let trimmed = text.trim_end_matches('0').trim_end_matches('.').len();
        text.truncate(trimmed);
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
fn region(key: &str, center: [f64; 2], area: f64) -> Value {
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
fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
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
fn circle_region(entity: &str, center: [f64; 2], radius: f64) -> Value {
    region(entity, center, std::f64::consts::PI * radius * radius)
}

// Sketch entities.

/// A line entity.
fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// A circle entity.
fn circle(entity: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": entity, "type": "circle", "center": center, "radius": radius})
}

/// A point entity.
fn point(entity: &str, position: [f64; 2]) -> Value {
    json!({"id": entity, "type": "point", "position": position})
}

/// Four lines `e-<first>` to `e-<first + 3>` drawn counterclockwise from (x0, y0), as features.md section 6.1 `RECT`
/// draws them.
fn rectangle(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
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
fn origin_plane(name: &str) -> Value {
    json!({"kind": "origin", "plane": name})
}

/// A datum plane reference `{"kind": "datum", "feature": <id>}`.
fn datum_plane(feature: &str) -> Value {
    json!({"kind": "datum", "feature": feature})
}

/// A face plane of a sketch whose fallback is the XY frame at height `z`; [`picked`] marks the fallback
/// `fallback_frame: body`.
fn top_face_plane(body: &str, face: Value, z: f64) -> Value {
    json!({
        "kind": "face",
        "body": body,
        "face": face,
        "fallback": {"origin": [0.0, 0.0, z], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}
    })
}

/// A sketch record without constraints.
fn sketch(feature: &str, plane: Value, entities: Vec<Value>) -> Value {
    json!({"id": feature, "type": "sketch", "plane": plane, "entities": entities})
}

/// A datum plane record offset from an origin plane.
fn datum_offset(feature: &str, plane: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "datum_plane", "base": origin_plane(plane), "offset": offset})
}

/// A datum plane record on a face of a body.
fn datum_on_face(feature: &str, body: &str, face: Value, offset: &str) -> Value {
    json!({
        "id": feature,
        "type": "datum_plane",
        "base": {"kind": "face", "body": body, "face": face},
        "offset": offset
    })
}

// Solid features.

/// A distance extent.
fn distance(expr: &str) -> Value {
    json!({"kind": "distance", "expr": expr})
}

/// A symmetric extent.
fn symmetric(expr: &str) -> Value {
    json!({"kind": "symmetric", "expr": expr})
}

/// A through-all extent.
fn through_all() -> Value {
    json!({"kind": "through_all"})
}

/// An extrude record into `body`.
fn extrude(
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

/// The extrude record with Reverse on.
fn reversed(mut record: Value) -> Value {
    record["reverse"] = json!(true);
    record
}

/// A box record; `position` is omitted when `None`.
fn box_record(
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
fn fillet(feature: &str, body: &str, edges: Vec<Value>, radius: &str) -> Value {
    json!({"id": feature, "type": "fillet", "body": body, "edges": edges, "radius": radius})
}

/// An equal-distance chamfer record.
fn chamfer(feature: &str, body: &str, edges: Vec<Value>, distance: &str) -> Value {
    json!({
        "id": feature, "type": "chamfer", "body": body, "edges": edges,
        "distance": {"kind": "equal", "expr": distance}
    })
}

/// A simple hole record `f-hole1` of `b-1` through all, on the points of `f-sk1`.
fn hole(points: &[&str], diameter: &str) -> Value {
    json!({
        "id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1",
        "points": points, "diameter": diameter, "extent": through_all()
    })
}

/// A linear pattern record `f-lp1` of `b-1` along the origin X axis.
fn linear_pattern(count: &str, spacing: &str) -> Value {
    json!({
        "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": count, "spacing": spacing}
    })
}

/// A split body record `f-split1` of `b-1` by the datum plane `f-dp1`, the other side going into `b-2`.
fn split_into_second_body() -> Value {
    json!({
        "id": "f-split1", "type": "split_body", "body": "b-1",
        "plane": datum_plane("f-dp1"), "target": "b-2"
    })
}

// Documents.

/// A `.ptsm` text in millimeters with no parameters, these bodies, each named after its number (`b-2` is 'Body 2'),
/// and these features.
fn document(bodies: &[&str], features: &[Value]) -> String {
    let bodies: Vec<Value> = bodies
        .iter()
        .map(|body| json!({"id": body, "name": format!("Body {}", body.trim_start_matches("b-"))}))
        .collect();
    let document = json!({
        "format": "partsmith",
        "format_version": FORMAT_VERSION,
        "units": "mm",
        "parameters": [],
        "bodies": bodies,
        "features": features
    });
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

// The corpus's shared setups and references.

/// CUBE20: `f-box1` box 20 x 20 x 20 in `b-1`.
fn cube20() -> Value {
    box_record("f-box1", "b-1", ["20", "20", "20"], None, "new")
}

/// CUBE10: `f-box1` box 10 x 10 x 10 in `b-1`.
fn cube10() -> Value {
    box_record("f-box1", "b-1", ["10", "10", "10"], None, "new")
}

/// BOX-BELOW: `f-box1` box 40 x 40 x 10 at (0, 0, -10) in `b-1` (top at z = 0).
fn box_below() -> Value {
    box_record(
        "f-box1",
        "b-1",
        ["40", "40", "10"],
        Some(["0", "0", "-10"]),
        "new",
    )
}

/// BLOCK's outline with its right side at `x`: the rectangle `e-1` to `e-4` from (0, 0) to (x, 50).
fn block_outline_to(x: f64) -> Vec<Value> {
    rectangle(1, 0.0, 0.0, x, 50.0)
}

/// BLOCK: the 80 x 50 rectangle `e-1` to `e-4` of `f-sk1` extruded 30 by `f-ext1` into `b-1`, followed by `more`.
fn block_then(more: Vec<Value>) -> Vec<Value> {
    let mut features = vec![
        sketch("f-sk1", origin_plane("XY"), block_outline_to(80.0)),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
            distance("30"),
            "new",
        ),
    ];
    features.extend(more);
    features
}

/// BLOCK's top.
fn block_top() -> Value {
    reference("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4")
}

/// BLOCK's back top edge.
fn block_back_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
    )
}

/// BLOCK's right top edge.
fn block_right_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
        "line|n=0,1,0|c=80,25,30|l=50|adj=2,2",
    )
}

/// CUBE20's front top edge.
fn cube20_front_top() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

// Scenarios.

/// One change of a corpus edit, made into model edits against the document's current records.
#[derive(Debug, Clone)]
enum Change {
    /// The entities of a sketch replaced.
    Entities(&'static str, Vec<Value>),
    /// The value at a JSON Pointer of a feature record set, the key added when the record does not have it.
    Set(&'static str, &'static str, Value),
    /// Records inserted, in order, directly before a feature.
    InsertBefore(&'static str, Vec<Value>),
    /// Features removed.
    Remove(Vec<&'static str>),
}

/// A corpus scenario: its setup with its picks, and the changes of its edit.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// The setup's text as [`picked`] saves it.
    setup: String,
    /// The changes of the edit.
    changes: Vec<Change>,
}

impl Scenario {
    /// The scenario whose setup is `setup` (picked here) and whose edit makes `changes`.
    fn new(setup: &str, changes: Vec<Change>) -> Scenario {
        Scenario {
            setup: picked(setup),
            changes,
        }
    }

    /// The scenario's document after its edit: the picked setup opened in a headless session, the edit applied as
    /// one `apply_edits`, and the text `save_document` returns.
    pub fn edited_text(&self) -> String {
        let mut engine = Engine::open(&self.setup);
        let edits = engine.edits(&self.changes);
        engine.apply("Edit", edits);
        engine.saved_text()
    }
}

/// BLOCK followed by `more`, then BLOCK's outline changed to `outline`.
fn block_edited(more: Vec<Value>, outline: Vec<Value>) -> Scenario {
    Scenario::new(
        &document(&["b-1"], &block_then(more)),
        vec![Change::Entities("f-sk1", outline)],
    )
}

/// Corpus R03: BLOCK with a corner bevel `e-5` from (80, 40) to (70, 50) and a fillet of 2 mm on the bevel's top edge
/// and the front top edge, then the bevel deleted (a square corner again).
pub fn r03() -> Scenario {
    let outline = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        line("e-5", [80.0, 40.0], [70.0, 50.0]),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let features = vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![region(
                "e-1,e-2,e-3,e-4,e-5",
                [39.535865, 24.725738],
                3950.0,
            )],
            distance("30 mm"),
            "new",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![
                reference(
                    "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
                    "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2",
                ),
                reference(
                    "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
                    "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
                ),
            ],
            "2 mm",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Entities("f-sk1", block_outline_to(80.0))],
    )
}

/// Corpus R06 (a) as recompute.md test 41 writes it without its second entry: BLOCK and a fillet of 2 on the right
/// top edge, then `e-2` deleted and `e-9` drawn in its place.
pub fn r06a() -> Scenario {
    let redrawn = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 50.0]),
    ];
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_right_top()], "2")],
        redrawn,
    )
}

/// Corpus R08 (a) at width 200: BLOCK and SLOT(30,50) (`f-dp1`, `f-sk2` with the rectangle `e-5` to `e-8` from
/// (30, -1) to (50, 51), and its reversed cut `f-ext2` 5 deep), a fillet of 2 on the front top edge of the right piece
/// of the top, then BLOCK's right side moved from x = 80 to x = 200. The fillet's reference is stored without `nb`,
/// as a script or a build before format version 2 wrote it: the edit stores it in that form.
pub fn r08a() -> Scenario {
    let right_piece_front_top = reference(
        "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
    );
    let features = block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            rectangle(5, 30.0, -1.0, 50.0, 51.0),
        ),
        reversed(extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![rectangle_region(5, 30.0, -1.0, 50.0, 51.0)],
            distance("5"),
            "cut",
        )),
        fillet("f-fil1", "b-1", vec![right_piece_front_top.clone()], "2"),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![
            Change::Entities("f-sk1", block_outline_to(200.0)),
            Change::Set("f-fil1", "/edges/0", right_piece_front_top),
        ],
    )
}

/// Corpus R12 (the round pocket) at distance 30: BLOCK, a round pocket 5 deep from a datum at its top, a fillet of 2
/// on the pocket's floor edge, then the pocket made 30 deep, through the block.
pub fn r12() -> Scenario {
    let features = block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
        ),
        reversed(extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [40.0, 25.0], 10.0)],
            distance("5"),
            "cut",
        )),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1",
            )],
            "2",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-ext2", "/extent/expr", json!("30"))],
    )
}

/// Corpus R16: CUBE20 with a fillet of 2 on its front top edge, and a 5 x 5 rabbet ('Sketch 1' and 'Extrude 1') along
/// that edge inserted before the fillet, drawn as cli.md test 55 draws it: `e-1` (-11, 15)-(-5, 15), `e-2`
/// (-5, 15)-(-5, 21), `e-3` (-5, 21)-(-11, 21) and `e-4` (-11, 21)-(-11, 15).
pub fn r16() -> Scenario {
    let features = vec![
        cube20(),
        fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"),
    ];
    let rabbet = vec![
        sketch(
            "f-sk2",
            origin_plane("YZ"),
            rectangle(1, -11.0, 15.0, -5.0, 21.0),
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![rectangle_region(1, -11.0, 15.0, -5.0, 21.0)],
            symmetric("30"),
            "cut",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::InsertBefore("f-fil1", rabbet)],
    )
}

/// Corpus R31 (a): CUBE20 in `b-1` of bodies `b-1` and `b-2`, a chamfer of 1 on its bottom front edge, then a datum
/// XY offset 5 and a split of `b-1` by it into `b-2` inserted before the chamfer (`b-1` keeps z 5..20).
pub fn r31a() -> Scenario {
    let features = vec![
        cube20(),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/bottom|f-box1/front)",
                "line|n=1,0,0|c=0,-10,0|l=20|adj=2,2",
            )],
            "1",
        ),
    ];
    Scenario::new(
        &document(&["b-1", "b-2"], &features),
        vec![Change::InsertBefore(
            "f-cha1",
            vec![datum_offset("f-dp1", "XY", "5"), split_into_second_body()],
        )],
    )
}

/// Corpus R41: CUBE10 patterned 3 times along X with spacing 20, a fillet of 1 on the right top edges of the second
/// and third cube, then the count lowered from 3 to 2.
pub fn r41() -> Scenario {
    let copy_right_top_edge = |copy: u32| {
        reference(
            &format!("edge(f-box1/right@f-lp1:{copy}|f-box1/top@f-lp1:{copy})"),
            &format!("line|n=0,1,0|c={},0,10|l=10|adj=2,2", 5 + 20 * copy),
        )
    };
    let features = vec![
        cube10(),
        linear_pattern("3", "20"),
        fillet(
            "f-fil1",
            "b-1",
            vec![copy_right_top_edge(1), copy_right_top_edge(2)],
            "1",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-lp1", "/direction/count", json!("2"))],
    )
}

/// Corpus R54: BLOCK, a boss (`f-sk2` on BLOCK's top, `f-ext2`), a datum `f-dp1` on the boss top, a peg on it
/// (`f-sk3`, `f-ext3`), then the boss's sketch and extrude deleted. After the delete 'Sketch 2' labels `f-sk3` and
/// 'Extrude 2' labels `f-ext3`.
pub fn r54() -> Scenario {
    let features = block_then(vec![
        sketch(
            "f-sk2",
            top_face_plane("b-1", block_top(), 30.0),
            rectangle(11, 20.0, 10.0, 60.0, 30.0),
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![rectangle_region(11, 20.0, 10.0, 60.0, 30.0)],
            distance("10"),
            "join",
        ),
        datum_on_face(
            "f-dp1",
            "b-1",
            reference("f-ext2/end-cap", "plane|n=0,0,1|c=40,20,40|a=800|adj=4,4"),
            "0",
        ),
        sketch(
            "f-sk3",
            datum_plane("f-dp1"),
            vec![circle("e-21", [40.0, 20.0], 5.0)],
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![circle_region("e-21", [40.0, 20.0], 5.0)],
            distance("5"),
            "join",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Remove(vec!["f-sk2", "f-ext2"])],
    )
}

/// Corpus R63 (a): holes at (0, 0) and (10, 10) through BOX-BELOW, a chamfer of 0.5 on the top rim of the second, then
/// the hole type set to a counterbore of 11 by 4.
pub fn r63a() -> Scenario {
    let features = vec![
        box_below(),
        sketch(
            "f-sk1",
            origin_plane("XY"),
            vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        ),
        hole(&["e-p1", "e-p2"], "6"),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/top|f-hole1/bore/e-p2)",
                "circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1",
            )],
            "0.5",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set(
            "f-hole1",
            "/hole_type",
            json!({"kind": "counterbore", "diameter": "11", "depth": "4"}),
        )],
    )
}

/// Corpus R66: BLOCK, a fillet of 1 on the back top edge, then `e-3` replaced by the five lines `e-9` to `e-13` of a
/// 20 x 10 notch.
pub fn r66() -> Scenario {
    let notched = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 50.0], [50.0, 50.0]),
        line("e-10", [50.0, 50.0], [50.0, 40.0]),
        line("e-11", [50.0, 40.0], [30.0, 40.0]),
        line("e-12", [30.0, 40.0], [30.0, 50.0]),
        line("e-13", [30.0, 50.0], [0.0, 50.0]),
    ];
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "1")],
        notched,
    )
}

// Picks.

/// The setup `text` with its references and regions stored as picks store them (module documentation), as saved.
fn picked(text: &str) -> String {
    let mut engine = Engine::open(&with_body_fallbacks(text));
    let records = engine.features();
    let edits: Vec<Value> = records
        .iter()
        .filter_map(|record| {
            let picked = engine.picked_record(record);
            (picked != *record).then(|| json!({"type": "update_feature", "feature": picked}))
        })
        .collect();
    if !edits.is_empty() {
        engine.apply("Pick", edits);
    }
    engine.saved_text()
}

/// The text with every face plane's fallback marked `fallback_frame: body`: no setup here has a move or a copy before
/// its face sketches, so body and world coordinates coincide where they are attached.
fn with_body_fallbacks(text: &str) -> String {
    let mut document: Value = serde_json::from_str(text).expect("setups are JSON");
    mark_fallbacks(&mut document);
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// Marks each face plane with a fallback in `value` `fallback_frame: body`.
fn mark_fallbacks(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.get("kind") == Some(&json!("face")) && object.contains_key("fallback") {
                object.insert("fallback_frame".to_owned(), json!("body"));
            }
            object.values_mut().for_each(mark_fallbacks);
        }
        Value::Array(items) => items.iter_mut().for_each(mark_fallbacks),
        _ => {}
    }
}

/// A reference a record stores: its JSON Pointer in the record, the body it is resolved in and its query.
struct StoredReference {
    pointer: String,
    body: String,
    query: String,
}

/// Every reference a record stores, each with the body of the nearest object around it that names one.
fn stored_references(record: &Value) -> Vec<StoredReference> {
    let mut found = Vec::new();
    collect_references(record, "", "", &mut found);
    found
}

/// Adds the references in `value`, at `pointer`, resolved in `body` unless an object names another.
fn collect_references(value: &Value, pointer: &str, body: &str, found: &mut Vec<StoredReference>) {
    match value {
        Value::Object(object) => {
            if let Some(query) = object.get("q").and_then(Value::as_str) {
                found.push(StoredReference {
                    pointer: pointer.to_owned(),
                    body: body.to_owned(),
                    query: query.to_owned(),
                });
                return;
            }
            let body = object.get("body").and_then(Value::as_str).unwrap_or(body);
            for (key, child) in object {
                collect_references(child, &format!("{pointer}/{key}"), body, found);
            }
        }
        Value::Array(items) => {
            for (position, item) in items.iter().enumerate() {
                collect_references(item, &format!("{pointer}/{position}"), body, found);
            }
        }
        _ => {}
    }
}

/// Sets the value at a JSON Pointer, adding the last key to its object when it is not there.
fn set_pointer(target: &mut Value, pointer: &str, value: Value) {
    let (parent_pointer, key) = pointer
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("{pointer} is a JSON Pointer"));
    let parent = target
        .pointer_mut(parent_pointer)
        .unwrap_or_else(|| panic!("the record has {parent_pointer}"));
    match parent {
        Value::Object(object) => {
            object.insert(key.to_owned(), value);
        }
        Value::Array(items) => {
            let index: usize = key.parse().expect("array positions are numbers");
            items[index] = value;
        }
        other => panic!("{parent_pointer} of the record is {other}, not an object or array"),
    }
}

// The headless session.

/// One document open in a headless session, driven with protocol commands as JSON (protocol.md section 2.5).
struct Engine {
    session: Session,
    doc: Value,
}

impl Engine {
    /// Opens a `.ptsm` text without assets.
    fn open(text: &str) -> Engine {
        let mut session = Session::new(SessionOptions::headless());
        let opened = send(
            &mut session,
            json!({"type": "open_document", "text": text, "assets": []}),
        );
        Engine {
            session,
            doc: opened["doc"].clone(),
        }
    }

    /// Sends a command about this document; `command` holds every member but `doc`.
    fn send(&mut self, mut command: Value) -> Value {
        command["doc"] = self.doc.clone();
        send(&mut self.session, command)
    }

    /// The document's feature records.
    fn features(&mut self) -> Vec<Value> {
        self.send(json!({"type": "get_document"}))["document"]["features"]
            .as_array()
            .expect("documents list their features")
            .clone()
    }

    /// Applies `edits` as one `apply_edits` labeled `label`.
    fn apply(&mut self, label: &str, edits: Vec<Value>) {
        self.send(json!({"type": "apply_edits", "label": label, "edits": edits}));
    }

    /// The canonical text `save_document` returns.
    fn saved_text(&mut self) -> String {
        self.send(json!({"type": "save_document"}))["text"]
            .as_str()
            .expect("save_document returns text")
            .to_owned()
    }

    /// The record with each stored reference picked in the state just before it and each profile region taken from
    /// the solved sketch.
    fn picked_record(&mut self, record: &Value) -> Value {
        let mut picked = record.clone();
        let references = stored_references(record);
        if !references.is_empty() {
            self.send(json!({"type": "begin_transaction", "label": "Pick"}));
            self.send(json!({"type": "set_focus", "feature": record["id"]}));
            for stored in &references {
                *picked
                    .pointer_mut(&stored.pointer)
                    .expect("the pointer names the stored reference") =
                    self.pick(&stored.body, &stored.query);
            }
            self.send(json!({"type": "cancel_transaction"}));
        }
        if let Some(regions) = record.pointer("/profile/regions").and_then(Value::as_array) {
            let solved =
                self.send(json!({"type": "solve_sketch", "feature": record["profile"]["sketch"]}));
            let solved_regions = solved["solution"]["regions"]
                .as_array()
                .expect("a solution lists its regions");
            let picked_regions: Vec<Value> = regions
                .iter()
                .map(|stored| {
                    solved_regions
                        .iter()
                        .find(|solved| solved["reference"]["key"] == stored["key"])
                        .map(|solved| solved["reference"].clone())
                        .unwrap_or_else(|| panic!("the sketch has no region {}", stored["key"]))
                })
                .collect();
            picked["profile"]["regions"] = Value::Array(picked_regions);
        }
        picked
    }

    /// The full reference `pick_reference` gives, in this headless session, for the element named `name` of `body`:
    /// a headless pick names the element by its body and its position among the elements of its kind in
    /// `list_elements` order (engine.md section 3.9).
    fn pick(&mut self, body: &str, name: &str) -> Value {
        let listed = self.send(json!({"type": "list_elements", "body": body}))["elements"]
            .as_array()
            .expect("list_elements returns elements")
            .clone();
        let kind = listed
            .iter()
            .find(|element| element["name"] == name)
            .unwrap_or_else(|| panic!("body {body} has no element named {name}"))["kind"]
            .clone();
        let index = listed
            .iter()
            .filter(|element| element["kind"] == kind)
            .position(|element| element["name"] == name)
            .expect("the element is among the elements of its kind");
        self.send(json!({
            "type": "pick_reference",
            "pick": {"mesh_id": 0, "kind": kind, "index": index, "body": body}
        }))["element"]["reference"]
            .clone()
    }

    /// The model edits that make `changes` against the document's current records: one `update_feature` per changed
    /// record (each with all its changes, in the order first changed), then the insertions and removals in order.
    fn edits(&mut self, changes: &[Change]) -> Vec<Value> {
        let timeline = self.features();
        let record = |feature: &str| {
            timeline
                .iter()
                .find(|record| record["id"] == feature)
                .unwrap_or_else(|| panic!("the document has no feature {feature}"))
        };
        let mut updated: Vec<Value> = Vec::new();
        let mut structural = Vec::new();
        for change in changes {
            let (feature, pointer, value) = match change {
                Change::Entities(feature, entities) => (*feature, "/entities", json!(entities)),
                Change::Set(feature, pointer, value) => (*feature, *pointer, value.clone()),
                Change::InsertBefore(feature, records) => {
                    let index = timeline
                        .iter()
                        .position(|record| record["id"] == *feature)
                        .unwrap_or_else(|| panic!("the document has no feature {feature}"));
                    structural.extend(records.iter().enumerate().map(|(offset, inserted)| {
                        json!({"type": "add_feature", "index": index + offset, "feature": inserted})
                    }));
                    continue;
                }
                Change::Remove(removed) => {
                    structural.extend(
                        removed
                            .iter()
                            .map(|feature| json!({"type": "remove_feature", "id": feature})),
                    );
                    continue;
                }
            };
            let position = match updated.iter().position(|record| record["id"] == feature) {
                Some(position) => position,
                None => {
                    updated.push(record(feature).clone());
                    updated.len() - 1
                }
            };
            set_pointer(&mut updated[position], pointer, value);
        }
        updated
            .into_iter()
            .map(|record| json!({"type": "update_feature", "feature": record}))
            .chain(structural)
            .collect()
    }
}

/// Sends one command (protocol JSON without the request envelope) through `Session::execute` and returns its
/// success value; panics when the command fails.
fn send(session: &mut Session, command: Value) -> Value {
    let request: Request = serde_json::from_value(json!({"id": 1, "command": command.clone()}))
        .unwrap_or_else(|error| panic!("{command} is a valid request ({error})"));
    let reply = session.execute(request, Vec::new());
    let response = serde_json::to_value(&reply.response).expect("responses serialize");
    let result = &response["result"];
    assert_eq!(result["kind"], "ok", "{command} succeeds: {result}");
    result.clone()
}
