//! Acceptance tests of the `move_body` feature (docs/spec/features/move_body.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/move_body/`, those of tests 11 and 12 (frames on the model) among them (`leaning-edge`
//! and `turned-edge-axis`), from which the tests make their other documents; test 10 runs the
//! document of features/sketch.md test 20, `examples/references/moved-face-sketch`.

use crate::feature_support;

use feature_support::{
    BOTH_FRONT_TOP_EDGES, FRONT_TOP_EDGES_OF_BOTH_BOXES, Run, TURN_BY_90,
    assert_axis_stored_as_picked, assert_bore_through, assert_bounding_box, assert_choice_between,
    assert_error_field, assert_examples_canonical, assert_face_area, assert_face_count,
    assert_face_in_plane, assert_face_kind, assert_face_normal, assert_face_position,
    assert_face_translated, assert_quarter_turns_about_z, assert_unchanged, assert_volume,
    body_changes, edge_axis, face, face_motion, feature_record_mut, l_shaped_body, link_ids,
    load_example, remove_axis_direction, with_shared_point_moved,
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

// Tests 10 to 13: the output map is the input map `moved` by the transform, so face sketches and stored axis directions
// move with the part, and an axis keeps the side the user picked (features.md sections 3.6 and 3.9).

#[test]
fn t010_a_sketch_after_the_move_keeps_its_point_on_the_moved_face() {
    // The document of features/sketch.md test 20 (corpus R32a): the move inserted before the face sketch, whose
    // fallback is in body coordinates.
    let run = Run::example("references/moved-face-sketch.ptsm");

    run.assert_all_ok();
    assert_face_translated(&run, "f-mv1", "b-1", "f-box1/top", [100.0, 0.0, 0.0]);
    assert_bore_through(&run, [115.0, 15.0, 0.0]);
}

#[test]
fn t010_a_sketch_after_a_turn_keeps_its_point_on_the_turned_face() {
    let mut turned = load_example("references/moved-face-sketch.ptsm");
    *feature_record_mut(&mut turned, "f-mv1") =
        serde_json::from_str(TURN_BY_90).expect("a move body record");

    let run = Run::new(turned);

    run.assert_all_ok();
    assert_quarter_turns_about_z(&face_motion(&run, "f-mv1", "b-1", "f-box1/top"), 1);
    assert_bore_through(&run, [-15.0, 15.0, 0.0]);
}

/// Test 11's example (corpus R38, its left wall leaning to (-1, 20)) with the shared point of `e-3`'s end and `e-4`'s
/// start at (`lean_x`, 20).
fn leaning_to(lean_x: f64) -> Run {
    let example = load_example("move_body/leaning-edge.ptsm");
    Run::new(with_shared_point_moved(
        &example,
        "f-sk1",
        ["e-3", "e-4"],
        [lean_x, 20.0],
    ))
}

#[test]
fn t011_the_part_turns_down_about_the_upright_wall() {
    // Before the edit.
    let run = leaning_to(0.0);

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, -40.0], [10.0, 20.0, 0.0]);
}

#[test]
fn t011_an_axis_edge_that_leans_keeps_the_fingerprints_side() {
    // The axis keeps the side of the fingerprint's +Y, (-0.049938, 0.998752, 0), so the part still turns down.
    let run = Run::example("move_body/leaning-edge.ptsm");
    run.assert_all_ok();
    assert_bounding_box(
        run.shape("b-1"),
        [-1.0, -1.995012, -40.948846],
        [10.087274, 20.499376, 0.0],
    );

    let run = leaning_to(1.0);
    run.assert_all_ok();
    assert_bounding_box(
        run.shape("b-1"),
        [0.0, -0.499376, -39.950094],
        [11.08478, 21.945137, 0.0],
    );
}

#[test]
fn t012_a_half_turn_before_the_move_turns_its_stored_axis() {
    // A half turn about Z inserted later as `f-mv1`, and `f-mv2`, a quarter turn about the box's right top edge as
    // picked before `f-mv1` was inserted, with its direction.
    let run = Run::example("move_body/turned-edge-axis.ptsm");

    run.assert_all_ok();
    // The stored direction turns with the part to -Y, so the box turns over that edge to where it went before.
    assert_bounding_box(run.shape("b-1"), [-5.0, -5.0, 10.0], [5.0, 5.0, 20.0]);
}

#[test]
fn t012_without_a_stored_direction_the_fingerprint_keeps_its_side() {
    let mut unstored = load_example("move_body/turned-edge-axis.ptsm");
    remove_axis_direction(&mut unstored, "f-mv2", &["rotation", "axis"]);

    let run = Run::new(unstored);

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

#[test]
fn t014_rotate_about_a_sketch_line_tips_the_box_onto_its_right_side() {
    let run = Run::example("move_body/rotate-about-sketch-line.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [5.0, -10.0, 0.0], [35.0, 10.0, 10.0]);
}

#[test]
fn t014_rotate_about_a_sketch_line_turns_plus_z_toward_plus_x() {
    let run = Run::example("move_body/rotate-about-sketch-line.ptsm");

    let body = run.body("b-1");
    let top = face(body, "f-box1/top");
    assert_face_in_plane(&top, 0, 35.0);
    assert_face_normal(&top, [1.0, 0.0, 0.0]);
    let right = face(body, "f-box1/right");
    assert_face_in_plane(&right, 2, 0.0);
    assert_face_normal(&right, [0.0, 0.0, -1.0]);
}

#[test]
fn t014_rotate_about_a_sketch_line_depends_on_the_sketch() {
    let run = Run::example("move_body/rotate-about-sketch-line.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-mv1").features), ["f-sk1"]);
}

#[test]
fn t015_rotate_about_a_round_face_turns_about_its_axis() {
    let run = Run::example("move_body/rotate-about-face.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [10.0, -25.0, 0.0], [30.0, -15.0, 30.0]);
    let front = face(run.body("b-1"), "f-box1/front");
    assert_face_in_plane(&front, 0, 30.0);
    assert_face_normal(&front, [1.0, 0.0, 0.0]);
}

#[test]
fn t015_rotate_about_a_round_face_reads_its_body() {
    let run = Run::example("move_body/rotate-about-face.ptsm");

    assert_unchanged(&run, "f-mv1", "b-2");
    assert_eq!(link_ids(&run.dependencies("f-mv1").bodies_read), ["b-2"]);
}

#[test]
fn t015_the_round_face_axis_is_stored_as_a_pick_stores_it() {
    let run = Run::example("move_body/rotate-about-face.ptsm");

    assert_axis_stored_as_picked(&run, "f-mv1", "/rotation/axis", "b-2", "f-cyl1/side");
}
