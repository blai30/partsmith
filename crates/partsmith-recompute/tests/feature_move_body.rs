//! Acceptance tests of the `move_body` feature (docs/spec/features/move_body.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/move_body/`; the documents of tests 10 to 13 (frames on the model) are written here
//! until their examples exist.

use crate::feature_support;

use std::time::Duration;

use feature_support::{
    BOTH_FRONT_TOP_EDGES, FRONT_TOP_EDGES_OF_BOTH_BOXES, Run, assert_bore_through,
    assert_bounding_box, assert_choice_between, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_count, assert_face_kind, assert_face_normal,
    assert_face_position, assert_face_translated, assert_median_build_within,
    assert_quarter_turns_about_z, assert_unchanged, assert_volume, body_changes, edge_axis, face,
    face_motion, l_shaped_body, leaning_wall, left_bottom_edge_axis, load_example,
    moved_box_document,
};
use partsmith_occt_sys::GeometryKind;

/// The bodies of every move body test.
const BODIES: [&str; 1] = ["b-1"];

/// The `BOX123` fixture: b-1 spanning [-5,-10,0]-[5,10,30].
const BOX123: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}"#;

/// The `MV(extra)` fixture: a move of b-1 with these further members.
fn move_body(extra: &str) -> String {
    format!(r#"{{"id": "f-mv1", "type": "move_body", "body": "b-1", {extra}}}"#)
}

/// The `ROT-Z(angle)` fixture: a rotation about the origin Z axis.
fn rotation_z(angle: &str) -> String {
    format!(r#"{{"axis": {{"kind": "origin", "axis": "Z"}}, "angle": "{angle}"}}"#)
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("move_body");
}

#[test]
fn t001_translate_volume_faces_and_bounds() {
    let run = Run::example("move_body/translate.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, -10.0, 0.0], [10.0, 10.0, 30.0]);
}

#[test]
fn t001_translate_keeps_the_top_face_name() {
    let run = Run::example("move_body/translate.ptsm");

    // Planar at z = 30, spanning x 0..10: centered at x = 5 with the full 10 by 20 area.
    let top = face(run.body("b-1"), "f-box1/top");
    assert_face_kind(&top, GeometryKind::Plane);
    assert_face_position(&top, 2, 30.0);
    assert_face_position(&top, 0, 5.0);
    assert_face_area(&top, 200.0);
}

#[test]
fn t001_translate_modifies_the_body() {
    let run = Run::example("move_body/translate.ptsm");

    assert_eq!(body_changes(&run, "f-mv1"), ["b-1 Modify"]);
}

#[test]
fn t002_rotate_bounds() {
    let run = Run::example("move_body/rotate.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-10.0, -5.0, 0.0], [10.0, 5.0, 30.0]);
}

#[test]
fn t002_rotate_turns_the_front_face_to_plus_x() {
    let run = Run::example("move_body/rotate.ptsm");

    let front = face(run.body("b-1"), "f-box1/front");
    assert_face_kind(&front, GeometryKind::Plane);
    assert_face_position(&front, 0, 10.0);
    assert_face_normal(&front, [1.0, 0.0, 0.0]);
}

#[test]
fn t003_rotate_then_translate() {
    let run = Run::example("move_body/rotate-translate.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-5.0, -5.0, 0.0], [15.0, 5.0, 30.0]);
}

#[test]
fn t004_rotate_about_its_own_edge() {
    let run = Run::example("move_body/rotate-about-edge.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 1000.0);
    assert_bounding_box(shape, [5.0, -5.0, 10.0], [15.0, 5.0, 20.0]);
}

#[test]
fn t005_references_survive_a_move() {
    let run = Run::example("move_body/move-then-fillet.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 991.415927);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [45.0, -5.0, 0.0], [55.0, 5.0, 10.0]);
}

#[test]
fn t006_nothing_moves() {
    let run = Run::example("move_body/nothing-moves.ptsm");

    run.assert_warnings("f-mv1", &["no_effect"]);
    assert_eq!(
        run.messages("f-mv1"),
        [
            "This move body leaves body 'Body 1' where it is, because its move is 0 and it turns by whole turns or not at all."
        ]
    );
    assert_unchanged(&run, "f-mv1", "b-1");
}

#[test]
fn t007_empty_body() {
    let run = Run::records(&BODIES, &[&move_body(r#""translation": ["5", "0", "0"]"#)]);

    run.assert_error("f-mv1", "body_empty");
    assert_eq!(
        run.messages("f-mv1"),
        [
            "Body 'Body 1' has no solid at this point, so this move body has nothing to work on. Pick a body that has a solid."
        ]
    );
}

#[test]
fn t008_wrong_angle_dimension() {
    let run = Run::records(
        &BODIES,
        &[
            BOX123,
            &move_body(&format!(r#""rotation": {}"#, rotation_z("5 mm"))),
        ],
    );

    let error = run.assert_error("f-mv1", "expression");
    assert_error_field(error, "rotation.angle");
    assert_eq!(
        run.messages("f-mv1"),
        ["Rotation angle: Expected an angle, but this expression gives a length."]
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t009_rotate_then_translate_builds_within_5_ms() {
    let document = load_example("move_body/rotate-translate.ptsm");

    assert_median_build_within(&document, "f-mv1", 21, Duration::from_millis(5));
}

// Tests 10 to 13: the output map is the input map `moved` by the transform, so face sketches and stored axis directions
// move with the part, and an axis keeps the side the user picked (features.md sections 3.6 and 3.9).

/// The document of features/sketch.md test 20 (corpus R32a) with `extra` as the members of the Move body inserted
/// before the face sketch, whose fallback is in body coordinates.
fn sketch_after_a_move_document(extra: &str) -> partsmith_model::Document {
    moved_box_document(&move_body(extra), Some("body"))
}

#[test]
fn t010_a_sketch_after_the_move_keeps_its_point_on_the_moved_face() {
    let run = Run::new(sketch_after_a_move_document(
        r#""translation": ["100", "0", "0"]"#,
    ));

    run.assert_all_ok();
    assert_face_translated(&run, "f-mv1", "b-1", "f-box1/top", [100.0, 0.0, 0.0]);
    assert_bore_through(&run, [115.0, 15.0, 0.0]);
}

#[test]
fn t010_a_sketch_after_a_turn_keeps_its_point_on_the_turned_face() {
    let run = Run::new(sketch_after_a_move_document(&format!(
        r#""rotation": {}"#,
        rotation_z("90 deg")
    )));

    run.assert_all_ok();
    assert_quarter_turns_about_z(&face_motion(&run, "f-mv1", "b-1", "f-box1/top"), 1);
    assert_bore_through(&run, [-15.0, 15.0, 0.0]);
}

/// The R38 setup with its left wall leaning by `lean`, turned by 90 degrees about the wall's bottom edge.
fn turned_about_leaning_wall(lean: i32) -> Run {
    let [sketch, extrude] = leaning_wall(lean);
    Run::records(
        &BODIES,
        &[
            &sketch,
            &extrude,
            &move_body(&format!(
                r#""rotation": {{"axis": {}, "angle": "90 deg"}}"#,
                left_bottom_edge_axis("")
            )),
        ],
    )
}

#[test]
fn t011_the_part_turns_down_about_the_upright_wall() {
    let run = turned_about_leaning_wall(0);

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, -40.0], [10.0, 20.0, 0.0]);
}

#[test]
fn t011_an_axis_edge_that_leans_keeps_the_fingerprints_side() {
    // The axis keeps the side of the fingerprint's +Y, (-0.049938, 0.998752, 0), so the part still turns down.
    let run = turned_about_leaning_wall(-1);
    run.assert_all_ok();
    assert_bounding_box(
        run.shape("b-1"),
        [-1.0, -1.995012, -40.948846],
        [10.087274, 20.499376, 0.0],
    );

    let run = turned_about_leaning_wall(1);
    run.assert_all_ok();
    assert_bounding_box(
        run.shape("b-1"),
        [0.0, -0.499376, -39.950094],
        [11.08478, 21.945137, 0.0],
    );
}

/// A box 10 x 10 x 10, a half turn about Z inserted later as `f-mv1`, and `f-mv2`, a quarter turn about the box's
/// right top edge as picked before `f-mv1` was inserted, with the axis's `extra` members.
fn quarter_turn_after_a_half_turn(extra: &str) -> Run {
    Run::records(
        &BODIES,
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}"#,
            &move_body(&format!(r#""rotation": {}"#, rotation_z("180 deg"))),
            &format!(
                r#"{{"id": "f-mv2", "type": "move_body", "body": "b-1", "rotation": {{"axis": {}, "angle": "90 deg"}}}}"#,
                edge_axis(
                    "b-1",
                    r#"{"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=5,0,10|l=10"}"#,
                    extra
                )
            ),
        ],
    )
}

#[test]
fn t012_a_half_turn_before_the_move_turns_its_stored_axis() {
    let run = quarter_turn_after_a_half_turn(r#", "direction": [0, 1, 0]"#);

    run.assert_all_ok();
    // The stored direction turns with the part to -Y, so the box turns over that edge to where it went before.
    assert_bounding_box(run.shape("b-1"), [-5.0, -5.0, 10.0], [5.0, 5.0, 20.0]);
}

#[test]
fn t012_without_a_stored_direction_the_fingerprint_keeps_its_side() {
    let run = quarter_turn_after_a_half_turn("");

    run.assert_all_ok();
    // The fingerprint's +Y.
    assert_bounding_box(run.shape("b-1"), [-15.0, -5.0, 0.0], [-5.0, 5.0, 10.0]);
}

#[test]
fn t013_axis_edges_not_on_one_line() {
    let [box_1, box_2] = l_shaped_body("b-1");
    let run = Run::records(
        &BODIES,
        &[
            &box_1,
            &box_2,
            &move_body(&format!(
                r#""rotation": {{"axis": {}, "angle": "90 deg"}}"#,
                edge_axis("b-1", BOTH_FRONT_TOP_EDGES, "")
            )),
        ],
    );

    assert_choice_between(
        &run,
        "f-mv1",
        "not_collinear",
        ("rotation.axis", "edge"),
        "b-1",
        &FRONT_TOP_EDGES_OF_BOTH_BOXES,
        "The edge selected for Rotation axis now matches 2 edges that do not lie on one line. Pick the one you mean.",
    );
    assert_unchanged(&run, "f-mv1", "b-1");
}

#[test]
fn a_translation_beyond_the_modeling_limit_is_out_of_range() {
    // As features.md section 6.4 test 15 (d), along Z and above the limit.
    let run = Run::records(
        &BODIES,
        &[BOX123, &move_body(r#""translation": ["0", "0", "1e200"]"#)],
    );

    let error = run.assert_error("f-mv1", "out_of_range");
    assert_error_field(error, "translation[2]");
    assert_eq!(
        run.messages("f-mv1"),
        ["Move Z must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."]
    );
    assert_unchanged(&run, "f-mv1", "b-1");
}
