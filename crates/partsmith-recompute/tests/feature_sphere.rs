//! Acceptance tests of the `sphere` feature (docs/spec/features/sphere.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/sphere/`.

use crate::feature_support;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_count, assert_face_kind, assert_face_names, assert_face_radius,
    assert_flat_face, assert_unchanged, assert_volume, body_changes, face, link_ids,
};
use partsmith_occt_sys::GeometryKind;

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("sphere");
}

#[test]
fn t001_default_is_ok() {
    let run = Run::example("sphere/default.ptsm");

    run.assert_ok("f-sph1");
}

#[test]
fn t001_default_volume_faces_and_bounds() {
    let run = Run::example("sphere/default.ptsm");

    let shape = run.shape("b-1");
    assert_volume(shape, 4188.790205);
    assert_face_count(shape, 1);
    assert_bounding_box(shape, [-10.0, -10.0, -10.0], [10.0, 10.0, 10.0]);
}

#[test]
fn t001_default_surface_is_spherical() {
    let run = Run::example("sphere/default.ptsm");

    assert_face_names(run.body("b-1"), &["f-sph1/surface"]);
    let surface = face(run.body("b-1"), "f-sph1/surface");
    assert_face_kind(&surface, GeometryKind::Sphere);
    assert_face_area(&surface, 1256.637061);
}

#[test]
fn t002_resting_on_xy() {
    let run = Run::example("sphere/resting-on-xy.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
}

#[test]
fn t003_dome_on_a_box() {
    let run = Run::example("sphere/dome-on-box.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 8261.799388);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 25.0]);
}

#[test]
fn t004_dimple_in_a_box() {
    let run = Run::example("sphere/dimple-in-box.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 7738.200612);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
}

#[test]
fn t005_zero_radius_is_out_of_range() {
    let run = Run::records(
        &["b-1"],
        &[r#"{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "0", "op": "new"}"#],
    );

    let error = run.assert_error("f-sph1", "out_of_range");
    assert_error_field(error, "radius");
}

#[test]
fn t007_radius_below_the_tolerance_is_out_of_range() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "0.0000001", "op": "new"}"#,
        ],
    );

    let error = run.assert_error("f-sph1", "out_of_range");
    assert_error_field(error, "radius");
    assert_eq!(
        run.messages("f-sph1"),
        [
            "Radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t008_intersect_keeps_the_lower_half() {
    let run = Run::example("sphere/intersect.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 1072.330292);
    assert_face_count(shape, 2);
    assert_bounding_box(shape, [-8.0, -8.0, 12.0], [8.0, 8.0, 20.0]);
    assert_eq!(body_changes(&run, "f-sph1"), ["b-1 Intersect"]);
}

#[test]
fn t008_intersect_closes_the_half_with_the_box_top() {
    let run = Run::example("sphere/intersect.ptsm");

    let body = run.body("b-1");
    let surface = face(body, "f-sph1/surface");
    assert_face_kind(&surface, GeometryKind::Sphere);
    assert_face_radius(&surface, 8.0);
    assert_face_area(&surface, 402.123860);
    assert_flat_face(body, "f-box1/top", 2, 20.0, 201.061930);
}

#[test]
fn t009_on_a_datum_is_centered_on_its_origin() {
    let run = Run::example("sphere/on-datum.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 4188.790205);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
}

#[test]
fn t009_on_a_datum_depends_on_the_datum() {
    let run = Run::example("sphere/on-datum.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-sph1").features), ["f-dp1"]);
}

#[test]
fn t010_on_a_face_of_another_body_rests_on_the_face() {
    let run = Run::example("sphere/on-face.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-2");
    assert_volume(shape, 523.598776);
    assert_face_count(shape, 1);
    assert_bounding_box(shape, [-5.0, -5.0, 20.0], [5.0, 5.0, 30.0]);
    assert_unchanged(&run, "f-sph1", "b-1");
}

#[test]
fn t010_on_a_face_of_another_body_reads_that_body() {
    let run = Run::example("sphere/on-face.ptsm");

    let dependencies = run.dependencies("f-sph1");
    assert_eq!(link_ids(&dependencies.bodies_read), ["b-1"]);
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-2"]);
}
