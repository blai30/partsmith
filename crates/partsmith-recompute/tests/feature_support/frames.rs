//! Helpers for the face sketches of features/sketch.md tests 20 to 29 and the copy, move and axis tests of the other
//! feature specs that build on them (frames on the model, ADR 0023): the records and documents they run, and the
//! assertions on sketch placements and on the motions of faces.

use partsmith_model::Document;
use partsmith_naming::{ElementName, Motion};

use super::{
    AXIS_TOLERANCE, LENGTH_TOLERANCE, Run, assert_absolute, assert_coordinates,
    assert_face_axis_through, assert_placement, body_before, document, face, line, output_body,
    sketch_result,
};

/// `PL(ox, oy, oz)` of features/sketch.md section 6 as JSON: origin `(ox, oy, oz)`, x axis +X, normal +Z.
pub fn top_fallback(origin: [f64; 3]) -> String {
    let [x, y, z] = origin;
    format!(r#"{{"origin": [{x}, {y}, {z}], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}"#)
}

/// Asserts that the sketch `sketch` is placed at `PL(origin)`: origin `origin`, x axis +X, normal +Z (absolute 1e-9).
#[track_caller]
pub fn assert_sketch_on_top_plane(run: &Run, sketch: &str, origin: [f64; 3]) {
    assert_placement(
        &sketch_result(run, sketch).placement,
        origin,
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
}

/// `FACE-SK2(face, fallback, frame, entities)` of features/sketch.md section 6 with `fallback` given as JSON: the sketch `f-sk2` on
/// `face` of `b-1`, its `fallback_frame` left out for `None` (`world`, as every document written before file format
/// version 2 has it).
pub fn face_sketch_with(face: &str, fallback: &str, frame: Option<&str>, entities: &str) -> String {
    let frame = frame
        .map(|frame| format!(r#", "fallback_frame": "{frame}""#))
        .unwrap_or_default();
    format!(
        r#"{{"id": "f-sk2", "type": "sketch", "plane": {{"kind": "face", "body": "b-1", "face": {face}, "fallback": {fallback}{frame}}}, "entities": {entities}}}"#
    )
}

/// `FACE-SK2(face, PL(origin), frame, entities)` of features/sketch.md section 6.
pub fn face_sketch(face: &str, origin: [f64; 3], frame: Option<&str>, entities: &str) -> String {
    face_sketch_with(face, &top_fallback(origin), frame, entities)
}

/// The entities of a sketch with one point `e-p1` at `(x, y)`.
pub fn one_point(position: [f64; 2]) -> String {
    let [x, y] = position;
    format!(r#"[{{"id": "e-p1", "type": "point", "position": [{x}, {y}]}}]"#)
}

/// The entities of a sketch with one circle `id` at `(x, y)` with radius `radius`.
pub fn one_circle(id: &str, center: [f64; 2], radius: f64) -> String {
    let [x, y] = center;
    format!(r#"[{{"id": "{id}", "type": "circle", "center": [{x}, {y}], "radius": {radius}}}]"#)
}

/// A hole `f-hole1` in `b-1` at `e-p1` of `f-sk2` with this diameter and extent.
pub fn hole_at_point(diameter: &str, extent: &str) -> String {
    format!(
        r#"{{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "{diameter}", "extent": {extent}}}"#
    )
}

/// The through-all extent.
pub const THROUGH_ALL: &str = r#"{"kind": "through_all"}"#;

/// The Move body `f-mv1` of features/sketch.md test 20: b-1 moved by (100, 0, 0).
pub const MOVE_BY_100: &str =
    r#"{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]}"#;

/// The Move body `f-mv1` of features/sketch.md test 20's rotation variant (corpus R32a): b-1 turned by 90 degrees about
/// the world Z axis.
pub const TURN_BY_90: &str = r#"{"id": "f-mv1", "type": "move_body", "body": "b-1", "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}}"#;

/// The Move body `f-mv1` turning `body` by the angle expression `angle` about the world axis `axis` (`X`, `Y` or `Z`).
pub fn turn(body: &str, axis: &str, angle: &str) -> String {
    format!(
        r#"{{"id": "f-mv1", "type": "move_body", "body": "{body}", "rotation": {{"axis": {{"kind": "origin", "axis": "{axis}"}}, "angle": "{angle}"}}}}"#
    )
}

/// An edge of `body` as an axis reference: the edge stored as the reference `edge` (JSON), followed by `extra` members
/// such as `, "direction": [0, 1, 0]`, or none.
pub fn edge_axis(body: &str, edge: &str, extra: &str) -> String {
    format!(r#"{{"kind": "edge", "body": "{body}", "edge": {edge}{extra}}}"#)
}

/// A box 40 x 40 x 10 in `b-1` (x -20..20, y -20..20, z 0..10), the `inserted` records, `f-sk2` on the box's top with
/// its fallback in `frame` and a point `e-p1` at (15, 15), and a hole `f-hole1` of diameter 4 through all at the point.
pub fn box_top_sketch_document(inserted: &[&str], frame: Option<&str>) -> Document {
    let sketch = face_sketch(
        r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"}"#,
        [0.0, 0.0, 10.0],
        frame,
        &one_point([15.0, 15.0]),
    );
    let hole = hole_at_point("4", THROUGH_ALL);
    let mut features = vec![
        r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}"#,
    ];
    features.extend_from_slice(inserted);
    features.extend([sketch.as_str(), hole.as_str()]);
    document(&["b-1"], &features)
}

/// The document of features/sketch.md test 20 (corpus R32a) with the Move body record `motion`: the box of
/// `box_top_sketch_document` with the move inserted before the face sketch.
pub fn moved_box_document(motion: &str, frame: Option<&str>) -> Document {
    box_top_sketch_document(&[motion], frame)
}

/// The document of features/sketch.md test 27, which linear_pattern.md test 16 also runs: a box 20 x 20 x 10 in `b-1`,
/// its linear pattern `f-lp1` along X with count 2 and `spacing`, a face sketch `f-sk2` on the copy's top in body
/// coordinates with a point `e-p1` at (5, 5), and a hole `f-hole1` of diameter 4 through all at the point.
pub fn sketch_on_pattern_copy_document(spacing: &str) -> Document {
    document(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "op": "new"}"#,
            &format!(
                r#"{{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {{"axis": {{"kind": "origin", "axis": "X"}}, "count": "2", "spacing": "{spacing}"}}}}"#
            ),
            &face_sketch(
                r#"{"q": "f-box1/top@f-lp1:1", "fp": "plane|n=0,0,1|c=60,0,10|a=400|adj=4,4"}"#,
                [0.0, 0.0, 10.0],
                Some("body"),
                &one_point([5.0, 5.0]),
            ),
            &hole_at_point("4", THROUGH_ALL),
        ],
    )
}

/// Asserts that the bore of `f-hole1` at `e-p1` has its axis through `point` at the end of the run.
#[track_caller]
pub fn assert_bore_through(run: &Run, point: [f64; 3]) {
    assert_face_axis_through(&face(run.body("b-1"), "f-hole1/bore/e-p1"), point);
}

/// Asserts that the maximum z of `body`'s bounding box at the end of the run is `z`.
#[track_caller]
pub fn assert_top_at(run: &Run, body: &str, z: f64) {
    let bounds = run
        .shape(body)
        .bounding_box()
        .expect("the kernel bounds a non-empty shape");
    assert_absolute("bounding box maximum z", bounds.max[2], z, LENGTH_TOLERANCE);
}

/// The L-shaped body of fillet.md test 4 in `body`: `f-box1` 40 x 20 x 10 (x -20..20, y -10..10) and `f-box2` 20 x 20
/// x 10 joined on its right half (x 0..20, z 10..20). `f-box1/top` lies at z = 10, `f-box2/top` at z = 20, and the
/// fronts merge into `f-box1/front`.
pub fn l_shaped_body(body: &str) -> [String; 2] {
    [
        format!(
            r#"{{"id": "f-box1", "type": "box", "body": "{body}", "size": ["40", "20", "10"], "op": "new"}}"#
        ),
        format!(
            r#"{{"id": "f-box2", "type": "box", "body": "{body}", "size": ["20", "20", "10"], "position": ["10", "0", "10"], "op": "join"}}"#
        ),
    ]
}

/// The top of `l_shaped_body` stored with `f-box2/top` merged into `f-box1/top`: one reference that resolves to faces
/// in two planes.
pub const BOTH_TOPS: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=-10,0,10|a=400", "merged": {"f-box1/top": ["f-box2/top"]}}"#;

/// The front top edge of `l_shaped_body` stored with `f-box2/top` merged into `f-box1/top`: one reference that
/// resolves to two parallel edges at z = 10 and z = 20.
pub const BOTH_FRONT_TOP_EDGES: &str = r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=-10,-10,10|l=20", "merged": {"f-box1/top": ["f-box2/top"]}}"#;

/// The edges `BOTH_FRONT_TOP_EDGES` resolves to, in name order.
pub const FRONT_TOP_EDGES_OF_BOTH_BOXES: [&str; 2] = [
    "edge(f-box1/front|f-box1/top)",
    "edge(f-box1/front|f-box2/top)",
];

/// The setup of corpus R38 in `b-1`: `RECT(0, 0, 40, 20)` with the shared point of `e-3`'s end and `e-4`'s start at
/// (`lean`, 20) (0 before the corpus edit; -1 or 1 after it, so the left wall leans 1 mm either way), as `f-sk1` on XY,
/// and `f-ext1`, its extrude of 10, op new.
pub fn leaning_wall(lean: i32) -> [String; 2] {
    let lines = [
        line("e-1", [0, 0], [40, 0]),
        line("e-2", [40, 0], [40, 20]),
        line("e-3", [40, 20], [lean, 20]),
        line("e-4", [lean, 20], [0, 0]),
    ];
    [
        format!(
            r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{}]}}"#,
            lines.join(", ")
        ),
        r#"{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "new"}"#.to_owned(),
    ]
}

/// The left wall's bottom edge of `leaning_wall` as an axis of `b-1` (corpus R38), followed by `extra` members such as
/// `, "direction": [0, 1, 0]`, or none.
pub fn left_bottom_edge_axis(extra: &str) -> String {
    edge_axis(
        "b-1",
        r#"{"q": "edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "fp": "line|n=0,1,0|c=0,10,0|l=20|adj=2,2"}"#,
        extra,
    )
}

/// Corpus R38 in `b-1`: `leaning_wall(lean)` and `f-lp1`, two copies 30 apart along `left_bottom_edge_axis(extra)`,
/// the bottom edge of the left wall as picked while the wall stood upright (`n=0,1,0`).
pub fn leaning_wall_pattern(lean: i32, extra: &str) -> Document {
    let [sketch, extrude] = leaning_wall(lean);
    let axis = left_bottom_edge_axis(extra);
    document(
        &["b-1"],
        &[
            &sketch,
            &extrude,
            &format!(
                r#"{{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {{"axis": {axis}, "count": "2", "spacing": "30"}}}}"#
            ),
        ],
    )
}

/// `LEAN(x)` of revolve.md test 28: the sketch `f-sk5` on XZ with `e-11` (-6, 0)-(-4, 0), `e-12` (-4, 0)-(x, 30),
/// `e-13` (x, 30)-(-6, 30) and `e-14` (-6, 30)-(-6, 0), and `f-ext5`, its extrude of 10 into `b-2`: a wall in y -10..0
/// whose face at y = 0 lies in the XZ plane and whose edge along `e-12` leans when `x` is not -4.
pub fn lean(top_x: i32) -> [String; 2] {
    let lines = [
        line("e-11", [-6, 0], [-4, 0]),
        line("e-12", [-4, 0], [top_x, 30]),
        line("e-13", [top_x, 30], [-6, 30]),
        line("e-14", [-6, 30], [-6, 0]),
    ];
    [
        format!(
            r#"{{"id": "f-sk5", "type": "sketch", "plane": {{"kind": "origin", "plane": "XZ"}}, "entities": [{}]}}"#,
            lines.join(", ")
        ),
        r#"{"id": "f-ext5", "type": "extrude", "body": "b-2", "profile": {"sketch": "f-sk5", "regions": [{"key": "e-11,e-12,e-13,e-14", "fp": "c=-5,15|a=60"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "new"}"#.to_owned(),
    ]
}

/// The edge of `lean` along `e-12` on the wall's face at y = 0, as an axis of `b-2`, followed by `extra` members such
/// as `, "direction": [0, 0, 1]`, or none.
pub fn lean_edge_axis(extra: &str) -> String {
    edge_axis(
        "b-2",
        r#"{"q": "edge(f-ext5/lateral/e-12|f-ext5/start-cap)", "fp": "line|n=0,0,1|c=-4,0,15|l=30|adj=2,2"}"#,
        extra,
    )
}

/// The block and slot of datum_plane.md test 17 in `body`: `f-sk1` with `RECT(0, 0, 80, 50)`, `f-ext1`, its extrude of
/// 30, `f-dp9` at z = 30, `f-sk9` on it with the slot x 30..50, y -1..51, and `f-ext9`, its cut of 5 reversed, which
/// splits the end cap into `f-ext1/end-cap#1` and `#2` (naming.md test 84).
pub fn slotted_block(body: &str) -> [String; 5] {
    let rectangle = [
        line("e-1", [0, 0], [80, 0]),
        line("e-2", [80, 0], [80, 50]),
        line("e-3", [80, 50], [0, 50]),
        line("e-4", [0, 50], [0, 0]),
    ];
    let slot = [
        line("e-11", [30, -1], [50, -1]),
        line("e-12", [50, -1], [50, 51]),
        line("e-13", [50, 51], [30, 51]),
        line("e-14", [30, 51], [30, -1]),
    ];
    [
        format!(
            r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{}]}}"#,
            rectangle.join(", ")
        ),
        format!(
            r#"{{"id": "f-ext1", "type": "extrude", "body": "{body}", "profile": {{"sketch": "f-sk1", "regions": [{{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}}]}}, "extent": {{"kind": "distance", "expr": "30"}}, "op": "new"}}"#
        ),
        r#"{"id": "f-dp9", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}"#.to_owned(),
        format!(
            r#"{{"id": "f-sk9", "type": "sketch", "plane": {{"kind": "datum", "feature": "f-dp9"}}, "entities": [{}]}}"#,
            slot.join(", ")
        ),
        format!(
            r#"{{"id": "f-ext9", "type": "extrude", "body": "{body}", "profile": {{"sketch": "f-sk9", "regions": [{{"key": "e-11,e-12,e-13,e-14", "fp": "c=40,25|a=1040"}}]}}, "extent": {{"kind": "distance", "expr": "5"}}, "reverse": true, "op": "cut"}}"#
        ),
    ]
}

/// Asserts that `motion` moves each point of `moves` from its first position to its second (absolute 1e-9), so a test
/// checks a translation, rotation or reflection without comparing matrix entries bit for bit.
pub fn assert_moves(motion: &Motion, moves: &[([f64; 3], [f64; 3])]) {
    for (from, to) in moves {
        assert_coordinates(
            &format!("{from:?} moved by {motion:?}"),
            motion.apply_point(*from),
            *to,
            AXIS_TOLERANCE,
        );
    }
}

/// The points a motion is probed with: the origin and the unit points.
const PROBE_POINTS: [[f64; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
];

/// Asserts that `motion` moves each of `PROBE_POINTS` to where `expected` puts it (absolute 1e-9).
fn assert_moves_probe_points(motion: &Motion, expected: impl Fn([f64; 3]) -> [f64; 3]) {
    let moves: Vec<([f64; 3], [f64; 3])> = PROBE_POINTS
        .iter()
        .map(|point| (*point, expected(*point)))
        .collect();
    assert_moves(motion, &moves);
}

/// Asserts that `motion` is the translation by `offset`: it moves the origin and the unit points by `offset`.
pub fn assert_translation(motion: &Motion, offset: [f64; 3]) {
    assert_moves_probe_points(motion, |point| {
        [0, 1, 2].map(|axis| point[axis] + offset[axis])
    });
}

/// The motion of the face named `face` in the state `feature`'s output gives `body`.
pub fn face_motion(run: &Run, feature: &str, body: &str, face: &str) -> Motion {
    let name = ElementName::parse(face).expect("names written in tests are valid");
    output_body(run, feature, body).motion(&name)
}

/// The motion of the face named `face` in the state of `body` just before `feature`.
pub fn face_motion_before(run: &Run, feature: &str, body: &str, face: &str) -> Motion {
    let name = ElementName::parse(face).expect("names written in tests are valid");
    body_before(run, feature, body).motion(&name)
}

/// Asserts that the motion of the face `face` of `body` after `feature` is the translation by `offset`.
#[track_caller]
pub fn assert_face_translated(run: &Run, feature: &str, body: &str, face: &str, offset: [f64; 3]) {
    assert_translation(&face_motion(run, feature, body, face), offset);
}

/// Asserts that the face `face` of `body` after `feature` has the motion of `MOVE_BY_100` earlier in the run, the
/// translation (100, 0, 0): the body's base motion, which a fillet, chamfer, shell or split body gives the faces it
/// makes, so they move with the part like the faces it keeps (features.md section 3.9).
#[track_caller]
pub fn assert_moved_by_100(run: &Run, feature: &str, body: &str, face: &str) {
    assert_face_translated(run, feature, body, face, [100.0, 0.0, 0.0]);
}

/// Asserts that `motion` is the rotation by `turns` quarter turns counterclockwise about the world Z axis.
pub fn assert_quarter_turns_about_z(motion: &Motion, turns: u32) {
    assert_moves_probe_points(motion, |point| {
        (0..turns).fold(point, |point, _| [-point[1], point[0], point[2]])
    });
}
