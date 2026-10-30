//! Acceptance tests of the `cone` feature (docs/spec/features/cone.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/cone/`.

use crate::feature_support;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_axis_direction, assert_face_axis_through, assert_face_count,
    assert_face_kind, assert_face_names, assert_face_normal, assert_flat_face, assert_no_face,
    assert_volume, body_changes, face, face_index, link_ids,
};
use partsmith_occt_sys::GeometryKind;

/// The spec's `CONE(r1, r2, h)` record.
fn cone(bottom_radius: &str, top_radius: &str, height: &str) -> String {
    format!(
        r#"{{"id": "f-cone1", "type": "cone", "body": "b-1", "bottom_radius": "{bottom_radius}", "top_radius": "{top_radius}", "height": "{height}", "op": "new"}}"#
    )
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("cone");
}

#[test]
fn t001_frustum_is_ok() {
    let run = Run::example("cone/frustum.ptsm");

    run.assert_ok("f-cone1");
}

#[test]
fn t001_frustum_volume_faces_and_bounds() {
    let run = Run::example("cone/frustum.ptsm");

    let shape = run.shape("b-1");
    assert_volume(shape, 1832.595715);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
}

#[test]
fn t001_frustum_faces() {
    let run = Run::example("cone/frustum.ptsm");

    let side = face(run.body("b-1"), "f-cone1/side");
    assert_face_kind(&side, GeometryKind::Cone);
    assert_face_area(&side, 526.861105);
    assert_face_area(&face(run.body("b-1"), "f-cone1/bottom"), 314.159265);
    assert_face_area(&face(run.body("b-1"), "f-cone1/top"), 78.539816);
}

#[test]
fn t002_pointed_cone() {
    let run = Run::example("cone/pointed.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 1047.197551);
    assert_face_count(shape, 2);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
}

#[test]
fn t002_pointed_cone_has_no_top() {
    let run = Run::example("cone/pointed.ptsm");

    assert_face_names(run.body("b-1"), &["f-cone1/bottom", "f-cone1/side"]);
    assert_eq!(face_index(run.body("b-1"), "f-cone1/top"), None);
}

#[test]
fn t003_inverted_cone() {
    let run = Run::example("cone/inverted.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 261.799388);
    assert_face_count(shape, 2);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
    assert_face_names(run.body("b-1"), &["f-cone1/top", "f-cone1/side"]);
}

#[test]
fn t004_equal_radii_make_a_cylinder() {
    let run = Run::example("cone/equal-radii.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 785.398163);
    assert_face_count(shape, 3);
    let side = face(run.body("b-1"), "f-cone1/side");
    assert_face_kind(&side, GeometryKind::Cylinder);
}

#[test]
fn t005_both_radii_zero_is_an_error() {
    let run = Run::records(&["b-1"], &[&cone("0", "0", "10")]);

    run.assert_error("f-cone1", "cone_radii_zero");
    assert_eq!(
        run.messages("f-cone1"),
        ["Cone bottom and top radius cannot both be 0. Give one of them a size."],
    );
}

#[test]
fn t006_negative_radius_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cone("10", "-1", "10")]);

    let error = run.assert_error("f-cone1", "out_of_range");
    assert_error_field(error, "top_radius");
    assert_eq!(
        run.messages("f-cone1"),
        ["Top radius must be at least 0 mm, but it is -1 mm. Use 0 mm or more."],
    );
}

#[test]
fn t007_zero_height_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cone("10", "5", "0")]);

    let error = run.assert_error("f-cone1", "out_of_range");
    assert_error_field(error, "height");
}

#[test]
fn t009_nearly_equal_radii_make_a_cylinder() {
    let run = Run::example("cone/nearly-equal-radii.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 785.398163);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
    assert_face_kind(
        &face(run.body("b-1"), "f-cone1/side"),
        GeometryKind::Cylinder,
    );
}

#[test]
fn t010_apex_below_the_tolerance() {
    let run = Run::example("cone/apex-below-tolerance.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 1047.197551);
    assert_face_count(shape, 2);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
    assert_face_names(run.body("b-1"), &["f-cone1/bottom", "f-cone1/side"]);
}

#[test]
fn t011_both_radii_below_the_tolerance_are_zero() {
    let run = Run::records(&["b-1"], &[&cone("0.0000000001", "0", "10")]);

    run.assert_error("f-cone1", "cone_radii_zero");
    assert_eq!(
        run.messages("f-cone1"),
        ["Cone bottom and top radius cannot both be 0. Give one of them a size."],
    );
}

#[test]
fn t012_height_below_the_tolerance_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&cone("10", "5", "0.0000001")]);

    let error = run.assert_error("f-cone1", "out_of_range");
    assert_error_field(error, "height");
    assert_eq!(
        run.messages("f-cone1"),
        [
            "Height must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t013_intersect_cuts_the_cone_at_the_box_top() {
    let run = Run::example("cone/intersect.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    // The frustum of test 1.
    assert_volume(shape, 1832.595715);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
    assert_eq!(body_changes(&run, "f-cone1"), ["b-1 Intersect"]);
}

#[test]
fn t013_intersect_takes_its_top_from_the_box() {
    let run = Run::example("cone/intersect.ptsm");

    let body = run.body("b-1");
    assert_face_names(body, &["f-box1/top", "f-cone1/bottom", "f-cone1/side"]);
    let side = face(body, "f-cone1/side");
    assert_face_kind(&side, GeometryKind::Cone);
    assert_face_area(&side, 526.861105);
    assert_flat_face(body, "f-cone1/bottom", 2, 0.0, 314.159265);
    assert_flat_face(body, "f-box1/top", 2, 10.0, 78.539816);
}

#[test]
fn t014_cut_opens_a_cone_into_the_top_face() {
    let run = Run::example("cone/cut.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 7869.100306);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    assert_eq!(body_changes(&run, "f-cone1"), ["b-1 Cut"]);
}

#[test]
fn t014_cut_leaves_the_cone_side_and_no_cone_top() {
    let run = Run::example("cone/cut.ptsm");

    let body = run.body("b-1");
    let side = face(body, "f-cone1/side");
    assert_face_kind(&side, GeometryKind::Cone);
    assert_face_area(&side, 111.072073);
    assert_no_face(body, "f-cone1/top");
    assert_flat_face(body, "f-box1/top", 2, 20.0, 321.460184);
}

#[test]
fn t015_on_a_datum_points_its_axis_along_the_datum_normal() {
    let run = Run::example("cone/on-datum.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 1832.595715);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [5.0, -10.0, -10.0], [15.0, 10.0, 10.0]);
    let body = run.body("b-1");
    let bottom = assert_flat_face(body, "f-cone1/bottom", 0, 5.0, 314.159265);
    assert_face_normal(&bottom, [-1.0, 0.0, 0.0]);
    let top = assert_flat_face(body, "f-cone1/top", 0, 15.0, 78.539816);
    assert_face_normal(&top, [1.0, 0.0, 0.0]);
}

#[test]
fn t015_on_a_datum_depends_on_the_datum() {
    let run = Run::example("cone/on-datum.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-cone1").features), ["f-dp1"]);
}

#[test]
fn t016_on_a_face_of_the_body_joins_at_the_face_center() {
    let run = Run::example("cone/on-face.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 8261.799388);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-10.0, -20.0, 0.0], [10.0, 10.0, 20.0]);
    assert_eq!(body_changes(&run, "f-cone1"), ["b-1 Join"]);
}

#[test]
fn t016_on_a_face_of_the_body_points_its_axis_along_the_face_normal() {
    let run = Run::example("cone/on-face.ptsm");

    let body = run.body("b-1");
    let side = face(body, "f-cone1/side");
    assert_face_kind(&side, GeometryKind::Cone);
    assert_face_area(&side, 175.620368);
    assert_face_axis_direction(&side, [0.0, -1.0, 0.0]);
    // The base is centered at (0,-10,10), the center of the face.
    assert_face_axis_through(&side, [0.0, -10.0, 10.0]);
    // The bottom lies on the face, which keeps its name.
    assert_no_face(body, "f-cone1/bottom");
    assert_face_area(&face(body, "f-box1/front"), 321.460184);
}

#[test]
fn t016_on_a_face_of_the_body_writes_that_body_without_reading_it() {
    let run = Run::example("cone/on-face.ptsm");

    let dependencies = run.dependencies("f-cone1");
    assert!(link_ids(&dependencies.bodies_read).is_empty());
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-1"]);
}
