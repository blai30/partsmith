//! Acceptance tests of the `move_body` feature (docs/spec/features/move_body.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/move_body/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_normal, assert_face_position,
    assert_unchanged, assert_volume, body_changes, example_paths, face, load_example,
    median_build_time,
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
    let paths = example_paths("move_body");
    assert!(!paths.is_empty(), "examples/move_body has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
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
    let run = Run::records(
        &BODIES,
        &[
            BOX123,
            &move_body(&format!(
                r#""translation": ["0", "0", "0"], "rotation": {}"#,
                rotation_z("360 deg")
            )),
        ],
    );

    run.assert_warnings("f-mv1", &["no_effect"]);
    assert_unchanged(&run, "f-mv1", "b-1");
}

#[test]
fn t007_empty_body() {
    let run = Run::records(&BODIES, &[&move_body(r#""translation": ["5", "0", "0"]"#)]);

    run.assert_error("f-mv1", "body_empty");
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

    let duration = median_build_time(&document, "f-mv1", 21);
    assert!(
        duration < Duration::from_millis(5),
        "the move took {duration:?} to build (median of 21 runs), over the 5 ms budget",
    );
}
