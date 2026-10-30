//! Acceptance tests of the `cylinder` feature (docs/spec/features/cylinder.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/cylinder/`.

use crate::feature_support;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_axis_direction, assert_face_axis_through, assert_face_in_plane,
    assert_face_kind, assert_face_names, assert_face_normal, assert_face_position,
    assert_flat_face, assert_has_face, assert_no_face, assert_round_face,
    assert_volume_faces_and_bounds, body_changes, face, link_ids,
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
    assert_examples_canonical("cylinder");
}

#[test]
fn t001_default_is_ok() {
    let run = Run::example("cylinder/default.ptsm");

    run.assert_ok("f-cyl1");
}

#[test]
fn t001_default_volume_faces_and_bounds() {
    let run = Run::example("cylinder/default.ptsm");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        785.398163,
        3,
        [-5.0, -5.0, 0.0],
        [5.0, 5.0, 10.0],
    );
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

    let side = assert_round_face(run.body("b-1"), "f-cyl1/side", GeometryKind::Cylinder, 5.0);
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
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        15717.256661,
        7,
        [-20.0, -20.0, -10.0],
        [20.0, 20.0, 0.0],
    );
}

#[test]
fn t006_cut_a_round_hole_keeps_the_side_name() {
    let run = Run::example("cylinder/cut-round-hole.ptsm");

    assert_has_face(run.body("b-1"), "f-cyl1/side");
}

#[test]
fn t008_radius_below_the_tolerance_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cylinder("0.0000001", "10")]);

    let error = run.assert_error("f-cyl1", "out_of_range");
    assert_error_field(error, "radius");
    assert_eq!(
        run.messages("f-cyl1"),
        [
            "Radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
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
        [
            "Height must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
    );
}

#[test]
fn t010_intersect_keeps_the_part_inside_the_box() {
    let run = Run::example("cylinder/intersect.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        785.398163,
        3,
        [-5.0, -5.0, 0.0],
        [5.0, 5.0, 10.0],
    );
    assert_eq!(body_changes(&run, "f-cyl1"), ["b-1 Intersect"]);
}

#[test]
fn t010_intersect_takes_its_ends_from_the_box() {
    let run = Run::example("cylinder/intersect.ptsm");

    let body = run.body("b-1");
    let side = assert_round_face(body, "f-cyl1/side", GeometryKind::Cylinder, 5.0);
    assert_face_area(&side, 314.159265);
    assert_flat_face(body, "f-box1/bottom", 2, 0.0, 78.539816);
    assert_flat_face(body, "f-box1/top", 2, 10.0, 78.539816);
    assert_no_face(body, "f-cyl1/top");
    assert_no_face(body, "f-cyl1/bottom");
}

#[test]
fn t011_on_a_datum_grows_along_its_normal() {
    let run = Run::example("cylinder/on-datum.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        785.398163,
        3,
        [-5.0, -5.0, -5.0],
        [5.0, 5.0, 5.0],
    );
    let body = run.body("b-1");
    let bottom = face(body, "f-cyl1/bottom");
    assert_face_in_plane(&bottom, 2, 5.0);
    assert_face_normal(&bottom, [0.0, 0.0, 1.0]);
    let top = face(body, "f-cyl1/top");
    assert_face_in_plane(&top, 2, -5.0);
    assert_face_normal(&top, [0.0, 0.0, -1.0]);
}

#[test]
fn t011_on_a_datum_depends_on_the_datum() {
    let run = Run::example("cylinder/on-datum.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-cyl1").features), ["f-dp1"]);
}

#[test]
fn t012_on_a_face_of_the_body_joins_at_the_face_center() {
    let run = Run::example("cylinder/on-face.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        8785.398163,
        8,
        [-10.0, -10.0, 0.0],
        [20.0, 10.0, 20.0],
    );
    assert_eq!(body_changes(&run, "f-cyl1"), ["b-1 Join"]);
}

#[test]
fn t012_on_a_face_of_the_body_points_its_axis_along_the_face_normal() {
    let run = Run::example("cylinder/on-face.ptsm");

    let body = run.body("b-1");
    let side = assert_round_face(body, "f-cyl1/side", GeometryKind::Cylinder, 5.0);
    assert_face_axis_direction(&side, [1.0, 0.0, 0.0]);
    assert_face_axis_through(&side, [0.0, 0.0, 10.0]);
    let top = assert_flat_face(body, "f-cyl1/top", 0, 20.0, 78.539816);
    assert_face_normal(&top, [1.0, 0.0, 0.0]);
    // The bottom lies on the face, which keeps its name.
    assert_no_face(body, "f-cyl1/bottom");
    assert_face_area(&face(body, "f-box1/right"), 321.460184);
}

#[test]
fn t012_on_a_face_of_the_body_writes_that_body_without_reading_it() {
    let run = Run::example("cylinder/on-face.ptsm");

    let dependencies = run.dependencies("f-cyl1");
    assert!(link_ids(&dependencies.bodies_read).is_empty());
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-1"]);
}
