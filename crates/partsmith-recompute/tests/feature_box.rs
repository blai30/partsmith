//! Acceptance tests of the `box` feature (docs/spec/features/box.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in `examples/box/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_error_field, assert_face_area, assert_face_count,
    assert_face_kind, assert_face_names, assert_face_normal, assert_face_position, assert_volume,
    face, link_ids, load_example, median_build_time,
};
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
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t010_default_box_builds_within_2_ms() {
    let document = load_example("box/default.ptsm");

    let duration = median_build_time(&document, "f-box1", 21);
    assert!(
        duration < Duration::from_millis(2),
        "the default box took {duration:?} to build (median of 21 runs), over the 2 ms budget",
    );
}
