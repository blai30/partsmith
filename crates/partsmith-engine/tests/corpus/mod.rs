//! Documents of the reference stability corpus (`docs/design/reference-stability-corpus.md`) and of the newcomer error
//! corpus that engine.md section 6 tests 115 to 153 open, written as `.ptsm` texts from the corpus's shared setups; the
//! picks their setups store; their edits; and readers of the statuses, actions and repairs those tests check.
//!
//! A setup stores its references as picks store them (the corpus's "Picked references" rule): [`picked`] opens it in
//! a headless session, focuses each feature that stores a reference, so that the display state is the state just
//! before that feature (engine.md section 3.5), stores the full reference `pick_reference` gives for each element the
//! feature names (section 3.9) and the reference `solve_sketch` gives for each profile region (`Region::reference()`,
//! with its outline), marks each face sketch's fallback `fallback_frame: body`, and returns the saved text. "After its
//! edit" is the setup opened, then the corpus edit applied as one labeled `apply_edits` ([`after_edit`]).

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

use partsmith_engine::Session;
use partsmith_model::FORMAT_VERSION;
use serde_json::{Map, Value, json};

use crate::common::{
    Sent, apply, doc_handle, elements, get_document, headless_session, measure_body, number,
    open_text, saved_text, send,
};

/// The label of the `apply_edits` that makes a corpus edit.
pub const EDIT_LABEL: &str = "Edit";

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

// Scenarios.

/// One change of a corpus edit, made into model edits against the document's current records by [`edits`].
#[derive(Debug, Clone)]
pub enum Change {
    /// The entities of a sketch replaced.
    Entities(&'static str, Vec<Value>),
    /// The value at a JSON Pointer of a feature record set, the key added when the record does not have it.
    Set(&'static str, &'static str, Value),
    /// Records inserted, in order, directly before a feature.
    InsertBefore(&'static str, Vec<Value>),
    /// Records inserted, in order, at a timeline position.
    InsertAt(usize, Vec<Value>),
    /// Features removed.
    Remove(Vec<&'static str>),
    /// A feature moved to a timeline position.
    Move(&'static str, usize),
}

/// A corpus scenario: its setup with its picks, and the changes of its edit.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// The setup's text as [`picked`] saves it.
    pub setup: String,
    /// The changes of the edit.
    pub changes: Vec<Change>,
}

impl Scenario {
    /// The scenario whose setup is `setup` (picked here) and whose edit makes `changes`.
    pub fn new(setup: &str, changes: Vec<Change>) -> Scenario {
        Scenario {
            setup: picked(setup),
            changes,
        }
    }

    /// The scenario's setup with `more` changes in its edit.
    pub fn and(mut self, more: Vec<Change>) -> Scenario {
        self.changes.extend(more);
        self
    }
}

/// BLOCK followed by `more`, then BLOCK's outline changed to `outline`.
pub fn block_edited(more: Vec<Value>, outline: Vec<Value>) -> Scenario {
    Scenario::new(
        &document(&["b-1"], &block_then(more)),
        vec![Change::Entities("f-sk1", outline)],
    )
}

/// Corpus R03: a corner bevel removed from the outline. Expressions carry `mm`, so the same text in inches keeps every
/// size (test 153).
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
        vec![Change::Entities("f-sk1", block_outline())],
    )
}

/// Corpus R06 (a): BLOCK, a fillet on the right top edge, a face sketch on the right side with a pocket; `e-2` deleted
/// and `e-9` drawn in its place.
pub fn r06a() -> Scenario {
    block_edited(
        vec![
            fillet("f-fil1", "b-1", vec![block_right_top()], "2"),
            sketch(
                "f-sk2",
                face_plane(
                    "b-1",
                    block_right_side(),
                    [80.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0],
                ),
                vec![circle("e-5", [25.0, 15.0], 4.0)],
            ),
            with(
                extrude(
                    "f-ext2",
                    "b-1",
                    "f-sk2",
                    vec![circle_region("e-5", [25.0, 15.0], 4.0)],
                    distance("5"),
                    "cut",
                ),
                "reverse",
                json!(true),
            ),
        ],
        right_side_redrawn(),
    )
}

/// Corpus R06 (a) without its face sketch and pocket (engine.md test 117): BLOCK and a fillet of 2 on the right top
/// edge, then `e-2` deleted and `e-9` drawn in its place.
pub fn r06a_fillet_only() -> Scenario {
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_right_top()], "2")],
        right_side_redrawn(),
    )
}

/// Corpus R12: a round pocket 5 deep under a floor fillet made 30 deep, through the block.
pub fn r12() -> Scenario {
    let features = block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
        ),
        with(
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-5", [40.0, 25.0], 10.0)],
                distance("5"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
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
/// that edge inserted before the fillet, drawn as cli.md test 55 draws it.
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

/// Corpus R23's setup: a 60 x 30 x 10 box, pegs A (`f-sk1`, `f-ext1`) and B (`f-sk2`, `f-ext2`) on it, and a fillet of
/// 1 on peg A's top rim.
fn pegs() -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", ["60", "30", "10"], None, "new"),
        datum_offset("f-dp1", "XY", "10"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-5", [-5.0, 0.0], 4.0)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-5", [-5.0, 0.0], 4.0)],
            distance("20"),
            "join",
        ),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-7", [5.0, 0.0], 4.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-7", [5.0, 0.0], 4.0)],
            distance("20"),
            "join",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
                "circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1",
            )],
            "1",
        ),
    ]
}

/// Corpus R23 (a): peg A's extrude suppressed.
pub fn r23a() -> Scenario {
    Scenario::new(
        &document(&["b-1"], &pegs()),
        vec![Change::Set("f-ext1", "/suppressed", json!(true))],
    )
}

/// Corpus R25: a datum on a chamfer face, with the chamfer suppressed.
pub fn r25() -> Scenario {
    let features = vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2"),
        datum_on_face(
            "f-dp1",
            "b-1",
            reference(
                "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
                "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4",
            ),
            "0",
        ),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-1", [0.0, 7.071068], 0.5)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-1", [0.0, 7.071068], 0.5)],
            distance("1"),
            "join",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-cha1", "/suppressed", json!(true))],
    )
}

/// Corpus R28: BLOCK, a boss on a datum at its top, a fillet of 2 on the boss's top rim, then the fillet dragged
/// directly after 'Extrude 1'.
pub fn r28() -> Scenario {
    let features = block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [40.0, 25.0], 10.0)],
            distance("10"),
            "join",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,40|l=62.831853|r=10|adj=2,1",
            )],
            "2",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Move("f-fil1", 2)],
    )
}

/// Corpus R41: CUBE10 patterned 3 times along X, a fillet of 1 on the top right edges of the second and third cube,
/// then the count lowered from 3 to 2.
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

/// Corpus R50: a circle drawn inside BLOCK's region.
pub fn r50() -> Scenario {
    let mut outline = block_outline();
    outline.push(circle("e-5", [40.0, 25.0], 10.0));
    block_edited(Vec::new(), outline)
}

/// Corpus R54: a boss under a datum, with the boss's sketch and extrude deleted.
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

/// Corpus R56 (b): a small box joined onto CUBE20 by a boolean, a fillet on the small box's top front edge, then the
/// boolean's target and tool swapped.
pub fn r56b() -> Scenario {
    let features = vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "20"]),
            "new",
        ),
        boolean("b-1", &["b-2"], "join"),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box2/front|f-box2/top)",
                "line|n=1,0,0|c=0,-5,30|l=10|adj=2,2",
            )],
            "1",
        ),
    ];
    Scenario::new(
        &document(&["b-1", "b-2"], &features),
        vec![
            Change::Set("f-bool1", "/target", json!("b-2")),
            Change::Set("f-bool1", "/tools", json!(["b-1"])),
        ],
    )
}

/// Corpus R63 (a): holes at (0, 0) and (10, 10) in BOX-BELOW, a chamfer of 0.5 on the top rim of the second, then the
/// hole type set to a counterbore of 11 by 4.
pub fn r63a() -> Scenario {
    let features = vec![
        box_below(),
        sketch(
            "f-sk1",
            origin_plane("XY"),
            vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        ),
        hole(
            "f-hole1",
            "b-1",
            "f-sk1",
            &["e-p1", "e-p2"],
            "6",
            through_all(),
        ),
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
        vec![Change::Set("f-hole1", "/hole_type", counterbore("11", "4"))],
    )
}

/// Corpus R66: BLOCK, a fillet of 1 on the back top edge, then `e-3` replaced by the five lines of a notch.
pub fn r66() -> Scenario {
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "1")],
        back_line_notched(),
    )
}

/// A datum plane `f-dp1` offset 5 from BLOCK's right side (engine.md section 3.22, `KeepMatch` of a field that holds
/// one reference), then corpus R49's edit, so the face it is on follows to both halves of the side.
pub fn datum_on_a_split_side() -> Scenario {
    block_edited(
        vec![datum_on_face("f-dp1", "b-1", block_right_side(), "5")],
        right_side_split(),
    )
}

/// Newcomer error NC11: BLOCK, a fillet of 2 on the back top edge, then the back line `e-3` deleted.
pub fn nc11() -> Scenario {
    let without_back_line = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "2")],
        without_back_line,
    )
}

/// Engine.md test 119: BLOCK with `f-sk1` changed to the lines of sketch.md test 231, so its region matches two
/// separate areas.
pub fn two_areas() -> Scenario {
    let lines = vec![
        line("e-1", [0.0, 0.0], [200.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-5", [200.0, 0.0], [200.0, 20.0]),
        line("e-6", [200.0, 20.0], [150.0, 20.0]),
        line("e-7", [150.0, 20.0], [150.0, 0.0]),
    ];
    block_edited(Vec::new(), lines)
}

/// Corpus R06 (a) on a profile of two regions (the re-anchoring of `KeepRegions`, engine.md section 3.22): BLOCK and a
/// separate 20 x 20 tile `e-5` to `e-8` at x 100..120 extruded together, and a fillet of 2 on BLOCK's front right
/// vertical edge. The edit deletes `e-2` and draws `e-9` in its place (BLOCK's region follows its outline with the
/// succession `e-2 -> [e-9]`, and the fillet follows it with a notice) and draws a guide circle `e-10` inside the tile
/// (the tile's region is split, with the `region_split` notice and `Keep these regions`).
pub fn redrawn_beside_a_split() -> Scenario {
    redrawn_beside_a_split_then(Vec::new())
}

/// [`redrawn_beside_a_split`] with `more` features after the fillet.
fn redrawn_beside_a_split_then(more: Vec<Value>) -> Scenario {
    let tile = rectangle(5, 100.0, 0.0, 120.0, 20.0);
    let mut outline = block_outline();
    outline.extend(tile.clone());
    let mut features = vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![
                rectangle_region(1, 0.0, 0.0, 80.0, 50.0),
                rectangle_region(5, 100.0, 0.0, 120.0, 20.0),
            ],
            distance("30"),
            "new",
        ),
        fillet("f-fil1", "b-1", vec![block_front_right_vertical()], "2"),
    ];
    features.extend(more);
    let mut edited = right_side_redrawn();
    edited.extend(tile);
    edited.push(circle("e-10", [110.0, 10.0], 5.0));
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Entities("f-sk1", edited)],
    )
}

/// [`redrawn_beside_a_split`] with a face sketch `f-sk2` on BLOCK's right side after the fillet, a circle `e-1` at
/// (25, 15) of radius 4 on it. After the edit the sketch follows the replaced line to `f-ext1/lateral/e-9` without a
/// message (features.md section 3.6).
pub fn redrawn_beside_a_split_with_a_face_sketch() -> Scenario {
    redrawn_beside_a_split_then(vec![sketch(
        "f-sk2",
        face_plane(
            "b-1",
            block_right_side(),
            [80.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ),
        vec![circle("e-1", [25.0, 15.0], 4.0)],
    )])
}

// Picks.

/// A reference a record stores: its JSON Pointer in the record, the body it is resolved in and its query.
struct StoredReference {
    pointer: String,
    body: String,
    query: String,
}

/// The setup `text` with its references and regions stored as picks store them (module documentation), as saved.
pub fn picked(text: &str) -> String {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, &with_body_fallbacks(text)));
    let records = features(&get_document(&mut session, doc)["document"]);
    let mut edits = Vec::new();
    for record in &records {
        let picked = picked_record(&mut session, doc, record);
        if picked != *record {
            edits.push(json!({"type": "update_feature", "feature": picked}));
        }
    }
    if !edits.is_empty() {
        apply(&mut session, doc, Some("Pick"), Value::Array(edits)).ok();
    }
    saved_text(&mut session, doc)
}

/// The full references of the elements named `names` of `body` as `pick_reference` gives them in the state just
/// before `feature` of the document `text`, in a fresh headless session.
pub fn picked_before(text: &str, feature: &str, body: &str, names: &[&str]) -> Vec<Value> {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, text));
    focus_on(&mut session, doc, feature);
    let references = names
        .iter()
        .map(|name| pick_named(&mut session, doc, body, name)["reference"].clone())
        .collect();
    send(
        &mut session,
        json!({"type": "cancel_transaction", "doc": doc}),
    )
    .ok();
    references
}

/// The descriptor `pick_reference` gives, in a headless session, for the element named `name` of `body` at the display
/// state: a headless pick names the element by its body and its position among the elements of its kind in
/// `list_elements` order (engine.md section 3.9).
pub fn pick_named(session: &mut Session, doc: u64, body: &str, name: &str) -> Value {
    let listed = elements(session, doc, body, None);
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
    send(
        session,
        json!({
            "type": "pick_reference",
            "doc": doc,
            "pick": {"mesh_id": 0, "kind": kind, "index": index, "body": body}
        }),
    )
    .ok()["element"]
        .clone()
}

/// Opens a transaction and focuses `feature`, so that the display state is the state just before it.
fn focus_on(session: &mut Session, doc: u64, feature: &str) {
    send(
        session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Pick"}),
    )
    .ok();
    send(
        session,
        json!({"type": "set_focus", "doc": doc, "feature": feature}),
    )
    .ok();
}

/// The record with each stored reference picked in the state just before it and each profile region taken from the
/// solved sketch.
fn picked_record(session: &mut Session, doc: u64, record: &Value) -> Value {
    let mut picked = record.clone();
    let references = stored_references(record);
    if !references.is_empty() {
        let feature = record["id"].as_str().expect("records have ids");
        focus_on(session, doc, feature);
        for stored in &references {
            let reference =
                pick_named(session, doc, &stored.body, &stored.query)["reference"].clone();
            *picked
                .pointer_mut(&stored.pointer)
                .expect("the pointer names the stored reference") = reference;
        }
        send(session, json!({"type": "cancel_transaction", "doc": doc})).ok();
    }
    if let Some(regions) = record.pointer("/profile/regions").and_then(Value::as_array) {
        let sketch = record["profile"]["sketch"].clone();
        let solved = send(
            session,
            json!({"type": "solve_sketch", "doc": doc, "feature": sketch}),
        );
        let solved_regions: Vec<Value> = solved.ok()["solution"]["regions"]
            .as_array()
            .expect("a solution lists its regions")
            .iter()
            .map(|region| region["reference"].clone())
            .collect();
        let picked_regions: Vec<Value> = regions
            .iter()
            .map(|stored| {
                solved_regions
                    .iter()
                    .find(|solved| solved["key"] == stored["key"])
                    .cloned()
                    .unwrap_or_else(|| panic!("sketch {sketch} has no region {}", stored["key"]))
            })
            .collect();
        picked["profile"]["regions"] = Value::Array(picked_regions);
    }
    picked
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

/// The text with every face plane's fallback marked `fallback_frame: body`: no setup has a move or a copy before its
/// face sketches, so body and world coordinates coincide where they are attached.
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

// Edits.

/// The feature records of a document value.
pub fn features(document: &Value) -> Vec<Value> {
    document["features"]
        .as_array()
        .expect("documents list their features")
        .clone()
}

/// The current record of `feature`.
pub fn record(session: &mut Session, doc: u64, feature: &str) -> Value {
    features(&get_document(session, doc)["document"])
        .into_iter()
        .find(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has no feature {feature}"))
}

/// The model edits that make `changes` against the document's current records: one `update_feature` per changed
/// record (each with all its changes, in the order first changed), then the insertions, removals and moves in order.
pub fn edits(session: &mut Session, doc: u64, changes: &[Change]) -> Vec<Value> {
    let timeline = features(&get_document(session, doc)["document"]);
    let position = |feature: &str| {
        timeline
            .iter()
            .position(|record| record["id"] == feature)
            .unwrap_or_else(|| panic!("the document has no feature {feature}"))
    };
    let mut updated: Vec<Value> = Vec::new();
    let mut structural = Vec::new();
    for change in changes {
        match change {
            Change::Entities(feature, entities) => {
                set_in(
                    &mut updated,
                    &timeline,
                    feature,
                    "/entities",
                    json!(entities),
                );
            }
            Change::Set(feature, pointer, value) => {
                set_in(&mut updated, &timeline, feature, pointer, value.clone());
            }
            Change::InsertBefore(feature, records) => {
                let index = position(feature);
                for (offset, inserted) in records.iter().enumerate() {
                    structural.push(json!({"type": "add_feature", "index": index + offset, "feature": inserted}));
                }
            }
            Change::InsertAt(index, records) => {
                for (offset, inserted) in records.iter().enumerate() {
                    structural.push(json!({"type": "add_feature", "index": index + offset, "feature": inserted}));
                }
            }
            Change::Remove(removed) => {
                structural.extend(
                    removed
                        .iter()
                        .map(|feature| json!({"type": "remove_feature", "id": feature})),
                );
            }
            Change::Move(feature, index) => {
                structural.push(json!({"type": "move_feature", "id": feature, "index": index}));
            }
        }
    }
    updated
        .into_iter()
        .map(|record| json!({"type": "update_feature", "feature": record}))
        .chain(structural)
        .collect()
}

/// Sets `pointer` of `feature`'s record among the `updated` records, taking the record from `timeline` the first time.
fn set_in(
    updated: &mut Vec<Value>,
    timeline: &[Value],
    feature: &str,
    pointer: &str,
    value: Value,
) {
    let position = match updated.iter().position(|record| record["id"] == feature) {
        Some(position) => position,
        None => {
            let record = timeline
                .iter()
                .find(|record| record["id"] == feature)
                .unwrap_or_else(|| panic!("the document has no feature {feature}"))
                .clone();
            updated.push(record);
            updated.len() - 1
        }
    };
    set_pointer(&mut updated[position], pointer, value);
}

/// Sets the value at a JSON Pointer, adding the last key to its object when it is not there, or appending to its array
/// when the position is the array's length.
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
            if index == items.len() {
                items.push(value);
            } else {
                items[index] = value;
            }
        }
        other => panic!("{parent_pointer} of the record is {other}, not an object or array"),
    }
}

/// Opens the scenario's setup in `session` (revision 0) and applies its edit as one `apply_edits` labeled
/// [`EDIT_LABEL`] (revision 1); returns the handle and the edit's reply.
pub fn after_edit(session: &mut Session, scenario: &Scenario) -> (u64, Sent) {
    let doc = doc_handle(&open_text(session, &scenario.setup));
    let edits = edits(session, doc, &scenario.changes);
    let edited = apply(session, doc, Some(EDIT_LABEL), Value::Array(edits));
    edited.ok();
    (doc, edited)
}

/// The scenario's text after its edit, saved from a headless session.
pub fn edited_text(scenario: &Scenario) -> String {
    let mut session = headless_session();
    let (doc, _) = after_edit(&mut session, scenario);
    saved_text(&mut session, doc)
}

// Statuses, messages and actions.

/// The statuses of the only `recomputed` event of `sent`, in timeline order.
pub fn statuses(sent: &Sent) -> Vec<Value> {
    sent.event("recomputed")["features"]
        .as_array()
        .expect("recomputed lists features")
        .clone()
}

/// The messages of `feature` in the only `recomputed` event of `sent`.
pub fn messages(sent: &Sent, feature: &str) -> Vec<Value> {
    sent.feature_status(feature)["messages"]
        .as_array()
        .expect("a status lists its messages")
        .clone()
}

/// The only message of `feature` with `code` in the only `recomputed` event of `sent`.
pub fn message(sent: &Sent, feature: &str, code: &str) -> Value {
    let found: Vec<Value> = messages(sent, feature)
        .into_iter()
        .filter(|message| message["code"] == code)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{feature} has exactly one {code} message: {}",
        sent.feature_status(feature)
    );
    found[0].clone()
}

/// The repairs of `feature` in the only `recomputed` event of `sent`.
pub fn repairs(sent: &Sent, feature: &str) -> Vec<Value> {
    sent.feature_status(feature)["repairs"]
        .as_array()
        .expect("a status lists its repairs")
        .clone()
}

/// The actions of a message (or of a cause).
pub fn actions(message: &Value) -> Vec<Value> {
    message["actions"].as_array().cloned().unwrap_or_default()
}

/// The labels of the actions of a message, in order.
pub fn labels(message: &Value) -> Vec<String> {
    actions(message)
        .iter()
        .map(|action| {
            action["label"]
                .as_str()
                .expect("actions have labels")
                .to_owned()
        })
        .collect()
}

/// The action of a message labeled `label`.
pub fn action(message: &Value, label: &str) -> Value {
    actions(message)
        .into_iter()
        .find(|action| action["label"] == label)
        .unwrap_or_else(|| panic!("no action labeled {label:?} in {message}"))
}

/// Sends `apply_fix` with the `FixRef` of an `apply` action.
pub fn apply_fix(session: &mut Session, doc: u64, action: &Value) -> Sent {
    assert_eq!(
        action["run"]["kind"], "apply",
        "{action} is an apply action"
    );
    send_fix(session, doc, action["run"]["fix"].clone())
}

/// Sends `apply_fix` with this `FixRef` JSON.
pub fn send_fix(session: &mut Session, doc: u64, fix: Value) -> Sent {
    send(
        session,
        json!({"type": "apply_fix", "doc": doc, "fix": fix}),
    )
}

/// The `FixRef` JSON of an `apply` action of `feature`'s message with `code` and `field`.
pub fn fix_ref(feature: &str, code: &str, field: &str, index: u32, revision: u64) -> Value {
    json!({
        "feature": feature, "code": code, "field": field,
        "cause": false, "index": index, "revision": revision
    })
}

// Results.

/// The volume of a body at the display state.
pub fn volume(session: &mut Session, doc: u64, body: &str) -> f64 {
    number(&measure_body(session, doc, body)["volume"])
}

/// The names of a body's faces at the display state.
pub fn face_names(session: &mut Session, doc: u64, body: &str) -> Vec<String> {
    elements(session, doc, body, Some("face"))
        .iter()
        .map(|face| face["name"].as_str().expect("faces have names").to_owned())
        .collect()
}

/// How many faces of a body a feature named first (their names start with `<feature>/`).
pub fn faces_of(session: &mut Session, doc: u64, body: &str, feature: &str) -> usize {
    let prefix = format!("{feature}/");
    face_names(session, doc, body)
        .iter()
        .filter(|name| name.starts_with(&prefix))
        .count()
}

/// The value with every key named in `keys` removed, at any depth.
pub fn without_keys(value: &Value, keys: &[&str]) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !keys.contains(&key.as_str()))
                .map(|(key, child)| (key.clone(), without_keys(child, keys)))
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| without_keys(item, keys)).collect())
        }
        other => other.clone(),
    }
}
