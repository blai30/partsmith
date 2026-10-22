//! Acceptance tests of the `box` feature (docs/spec/features/box.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in `examples/box/`.

use crate::feature_support;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_count, assert_face_kind, assert_face_names, assert_face_normal,
    assert_face_position, assert_messages_pass_lint, assert_solid_count, assert_volume, face,
    link_ids, message, set_field,
};
use partsmith_features::Fix;
use partsmith_occt_sys::GeometryKind;

/// Asserts a flat face of `f-box1` in `b-1`: its area, the position of its plane along `axis`
/// (0 for x, 1 for y, 2 for z), and its outward normal.
fn assert_box_face(run: &Run, name: &str, area: f64, axis: usize, position: f64, normal: [f64; 3]) {
    let geometry = face(run.body("b-1"), name);
    assert_face_kind(&geometry, GeometryKind::Plane);
    assert_face_area(&geometry, area);
    assert_face_position(&geometry, axis, position);
    assert_face_normal(&geometry, normal);
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("box");
}

#[test]
fn t001_default_placement_is_ok() {
    let run = Run::example("box/default.ptsm");

    run.assert_ok("f-box1");
}

#[test]
fn t001_default_placement_volume_faces_and_bounds() {
    let run = Run::example("box/default.ptsm");

    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [-5.0, -10.0, 0.0], [5.0, 10.0, 30.0]);
}

#[test]
fn t001_default_placement_face_names() {
    let run = Run::example("box/default.ptsm");

    assert_face_names(
        run.body("b-1"),
        &[
            "f-box1/bottom",
            "f-box1/top",
            "f-box1/front",
            "f-box1/back",
            "f-box1/left",
            "f-box1/right",
        ],
    );
}

#[test]
fn t001_default_placement_bottom_and_top_faces() {
    let run = Run::example("box/default.ptsm");

    assert_box_face(&run, "f-box1/bottom", 200.0, 2, 0.0, [0.0, 0.0, -1.0]);
    assert_box_face(&run, "f-box1/top", 200.0, 2, 30.0, [0.0, 0.0, 1.0]);
}

#[test]
fn t001_default_placement_front_and_back_faces() {
    let run = Run::example("box/default.ptsm");

    assert_box_face(&run, "f-box1/front", 300.0, 1, -10.0, [0.0, -1.0, 0.0]);
    assert_box_face(&run, "f-box1/back", 300.0, 1, 10.0, [0.0, 1.0, 0.0]);
}

#[test]
fn t001_default_placement_left_and_right_faces() {
    let run = Run::example("box/default.ptsm");

    assert_box_face(&run, "f-box1/left", 600.0, 0, -5.0, [-1.0, 0.0, 0.0]);
    assert_box_face(&run, "f-box1/right", 600.0, 0, 5.0, [1.0, 0.0, 0.0]);
}

#[test]
fn t002_offset_moves_the_box() {
    let run = Run::example("box/offset.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-4.0, -8.0, 3.0], [6.0, 12.0, 33.0]);
}

#[test]
fn t003_on_xz_volume_and_bounds() {
    let run = Run::example("box/on-xz.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_bounding_box(shape, [-5.0, -30.0, -10.0], [5.0, 0.0, 10.0]);
}

#[test]
fn t003_on_xz_top_faces_minus_y() {
    let run = Run::example("box/on-xz.ptsm");

    let top = face(run.body("b-1"), "f-box1/top");
    assert_face_position(&top, 1, -30.0);
    assert_face_normal(&top, [0.0, -1.0, 0.0]);
}

#[test]
fn t003_on_xz_front_is_at_z_minus_10() {
    let run = Run::example("box/on-xz.ptsm");

    let front = face(run.body("b-1"), "f-box1/front");
    assert_face_position(&front, 2, -10.0);
}

#[test]
fn t004_on_a_datum_stands_on_the_datum() {
    let run = Run::example("box/on-datum.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-5.0, -10.0, 5.0], [5.0, 10.0, 35.0]);
}

#[test]
fn t004_on_a_datum_depends_on_the_datum() {
    let run = Run::example("box/on-datum.ptsm");

    let dependencies = run.dependencies("f-box1");
    assert_eq!(link_ids(&dependencies.features), ["f-dp1"]);
}

#[test]
fn t005_on_a_face_of_another_body_stands_on_the_face() {
    let run = Run::example("box/on-face.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-2"), [-2.0, -2.0, 30.0], [2.0, 2.0, 34.0]);
}

#[test]
fn t005_on_a_face_of_another_body_reads_that_body() {
    let run = Run::example("box/on-face.ptsm");

    let dependencies = run.dependencies("f-box2");
    assert_eq!(link_ids(&dependencies.bodies_read), ["b-1"]);
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-2"]);
}

#[test]
fn t006_zero_size_is_out_of_range() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["0", "20", "30"], "op": "new"}"#,
        ],
    );

    let error = run.assert_error("f-box1", "out_of_range");
    assert_error_field(error, "size[0]");
    assert_eq!(
        run.messages("f-box1"),
        ["Length must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
}

#[test]
fn t007_join_overlapping_merges_coplanar_faces() {
    let run = Run::example("box/join-overlapping.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 1500.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [10.0, 5.0, 10.0]);
}

#[test]
fn t008_cut_a_square_hole() {
    let run = Run::example("box/cut-square-hole.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15000.0);
    assert_face_count(shape, 10);
    assert_bounding_box(shape, [-20.0, -20.0, 0.0], [20.0, 20.0, 10.0]);
}

#[test]
fn t009_sizes_use_parameter_expressions() {
    let run = Run::example("box/expressions.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 512.0);
    assert_bounding_box(shape, [-4.0, -8.0, 0.0], [4.0, 8.0, 4.0]);
}

#[test]
fn t011_size_below_the_tolerance_is_out_of_range() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "0.0000001", "30"], "op": "new"}"#,
        ],
    );

    let error = run.assert_error("f-box1", "out_of_range");
    assert_error_field(error, "size[1]");
    assert_eq!(
        run.messages("f-box1"),
        ["Width must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."],
    );
    assert_empty(&run, "b-1");
}

/// The box of test 1: 10 x 20 x 30, spanning [-5,-10,0]-[5,10,30].
const BOX_OF_TEST_1: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}"#;

/// `f-box2`, a 2 mm cube joined to `b-1` at `position`.
fn small_cube(position: [&str; 3]) -> String {
    let [x, y, z] = position;
    format!(
        r#"{{"id": "f-box2", "type": "box", "body": "b-1", "size": ["2", "2", "2"], "position": ["{x}", "{y}", "{z}"], "op": "join"}}"#
    )
}

#[test]
fn t012_a_join_inside_the_body_adds_nothing() {
    let run = Run::records(&["b-1"], &[BOX_OF_TEST_1, &small_cube(["0", "0", "10"])]);

    run.assert_warnings("f-box2", &["no_effect"]);
    assert_eq!(
        run.messages("f-box2"),
        ["This box adds nothing to body 'Body 1': it lies entirely inside the body."]
    );
    // A primitive has no profile sketch to attach, and a join inside gets no Reverse.
    assert_eq!(message(&run, "f-box2", "no_effect").fixes, []);
    assert_volume(run.shape("b-1"), 6000.0);
}

#[test]
fn t012_a_join_that_floats_leaves_two_pieces() {
    let run = Run::records(&["b-1"], &[BOX_OF_TEST_1, &small_cube(["0", "0", "40"])]);

    run.assert_warnings("f-box2", &["separate_solid"]);
    assert_eq!(
        run.messages("f-box2"),
        ["This box does not touch body 'Body 1', so the body now has 2 separate pieces."]
    );
    assert_eq!(
        message(&run, "f-box2", "separate_solid").fixes,
        [Fix::NewBody]
    );
    assert_solid_count(run.shape("b-1"), 2);
    assert_volume(run.shape("b-1"), 6008.0);
    assert_messages_pass_lint(&run, "f-box2");
}

#[test]
fn t013_a_size_beyond_the_modeling_limit() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["1e200", "20", "30"], "op": "new"}"#,
        ],
    );

    let error = run.assert_error("f-box1", "out_of_range");
    assert_error_field(error, "size[0]");
    // Checked before the kernel: the message is the range check's, never "The box could not be built."
    assert_eq!(
        run.messages("f-box1"),
        ["Length must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."],
    );
    assert_eq!(
        message(&run, "f-box1", "out_of_range").fixes,
        [set_field(
            "size[0]",
            "Length",
            serde_json::json!("1000000 mm")
        )]
    );
    assert_empty(&run, "b-1");
}

#[test]
fn every_size_is_checked_against_the_modeling_limit_in_field_order() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "2e6", "3e6"], "op": "new"}"#,
        ],
    );

    let error = run.assert_error("f-box1", "out_of_range");
    assert_error_field(error, "size[1]");
    assert_eq!(
        run.messages("f-box1"),
        ["Width must be at most 1000000 mm, but it is 2000000 mm. Use 1000000 mm or less."],
    );
}

#[test]
fn a_position_beyond_the_modeling_limit_is_out_of_range() {
    // A position may be negative, so its lower limit is -1000000 mm (features.md section 3.4).
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "position": ["-2e6", "0", "0"], "op": "new"}"#,
        ],
    );

    let error = run.assert_error("f-box1", "out_of_range");
    assert_error_field(error, "position[0]");
    assert_eq!(
        run.messages("f-box1"),
        [
            "Position X must be at least -1000000 mm, but it is -2000000 mm. Use -1000000 mm or more."
        ],
    );
    assert_empty(&run, "b-1");
}
