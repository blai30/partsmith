//! Acceptance tests of the `cylinder` feature (docs/spec/features/cylinder.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/cylinder/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_empty, assert_error_field,
    assert_face_area, assert_face_axis_direction, assert_face_axis_through, assert_face_count,
    assert_face_kind, assert_face_names, assert_face_position, assert_face_radius, assert_volume,
    example_paths, face, face_index, load_example, median_build_time,
};
use partsmith_occt_sys::GeometryKind;

/// The `CYL` record of the spec with this radius and height, for the error tests.
fn cylinder(radius: &str, height: &str) -> String {
    format!(
        r#"{{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "{radius}", "height": "{height}", "op": "new"}}"#
    )
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("cylinder");
    assert!(!paths.is_empty(), "examples/cylinder has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_default_is_ok() {
    let run = Run::example("cylinder/default.ptsm");

    run.assert_ok("f-cyl1");
}

#[test]
fn t001_default_volume_faces_and_bounds() {
    let run = Run::example("cylinder/default.ptsm");

    let shape = run.shape("b-1");
    assert_volume(shape, 785.398163);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
}

#[test]
fn t001_default_face_names() {
    let run = Run::example("cylinder/default.ptsm");

    assert_face_names(
        run.body("b-1"),
        &["f-cyl1/side", "f-cyl1/top", "f-cyl1/bottom"],
    );
}

#[test]
fn t001_default_side_is_a_cylinder_around_z() {
    let run = Run::example("cylinder/default.ptsm");

    let side = face(run.body("b-1"), "f-cyl1/side");
    assert_face_kind(&side, GeometryKind::Cylinder);
    assert_face_radius(&side, 5.0);
    assert_face_axis_direction(&side, [0.0, 0.0, 1.0]);
    assert_face_axis_through(&side, [0.0, 0.0, 0.0]);
    assert_face_area(&side, 314.159265);
}

#[test]
fn t001_default_top_and_bottom_discs() {
    let run = Run::example("cylinder/default.ptsm");

    let top = face(run.body("b-1"), "f-cyl1/top");
    assert_face_kind(&top, GeometryKind::Plane);
    assert_face_area(&top, 78.539816);
    assert_face_position(&top, 2, 10.0);
    let bottom = face(run.body("b-1"), "f-cyl1/bottom");
    assert_face_kind(&bottom, GeometryKind::Plane);
    assert_face_area(&bottom, 78.539816);
    assert_face_position(&bottom, 2, 0.0);
}

#[test]
fn t002_on_yz_stands_along_x() {
    let run = Run::example("cylinder/on-yz.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, -5.0, -5.0], [10.0, 5.0, 5.0]);
    let top = face(run.body("b-1"), "f-cyl1/top");
    assert_face_position(&top, 0, 10.0);
}

#[test]
fn t003_offset_moves_the_cylinder() {
    let run = Run::example("cylinder/offset.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [5.0, -5.0, -5.0], [15.0, 5.0, 5.0]);
}

#[test]
fn t004_zero_radius_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cylinder("0", "10")]);

    let error = run.assert_error("f-cyl1", "out_of_range");
    assert_error_field(error, "radius");
}

#[test]
fn t005_negative_height_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cylinder("5", "-2")]);

    let error = run.assert_error("f-cyl1", "out_of_range");
    assert_error_field(error, "height");
    assert_eq!(
        run.messages("f-cyl1"),
        ["Height must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm."],
    );
}

#[test]
fn t006_cut_a_round_hole() {
    let run = Run::example("cylinder/cut-round-hole.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15717.256661);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-20.0, -20.0, -10.0], [20.0, 20.0, 0.0]);
}

#[test]
fn t006_cut_a_round_hole_keeps_the_side_name() {
    let run = Run::example("cylinder/cut-round-hole.ptsm");

    assert!(
        face_index(run.body("b-1"), "f-cyl1/side").is_some(),
        "b-1 should have a face named f-cyl1/side",
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t007_default_cylinder_builds_within_2_ms() {
    let document = load_example("cylinder/default.ptsm");

    let duration = median_build_time(&document, "f-cyl1", 21);
    assert!(
        duration < Duration::from_millis(2),
        "the default cylinder took {duration:?} to build (median of 21 runs), over the 2 ms budget",
    );
}

#[test]
fn t008_radius_below_the_tolerance_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cylinder("0.0000001", "10")]);

    let error = run.assert_error("f-cyl1", "out_of_range");
    assert_error_field(error, "radius");
    assert_eq!(
        run.messages("f-cyl1"),
        ["Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t009_height_below_the_tolerance_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cylinder("5", "0.0000001")]);

    let error = run.assert_error("f-cyl1", "out_of_range");
    assert_error_field(error, "height");
    assert_eq!(
        run.messages("f-cyl1"),
        ["Height must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
}
