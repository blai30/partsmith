//! Acceptance tests of the `torus` feature (docs/spec/features/torus.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/torus/`.

use crate::feature_support;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_axis_direction, assert_face_count, assert_face_kind,
    assert_face_names, assert_flat_face, assert_solid_count, assert_unchanged, assert_volume,
    assert_volume_faces_and_bounds, body_changes, face, link_ids,
};
use partsmith_features::{Bound, FeatureError};
use partsmith_occt_sys::GeometryKind;

/// The spec's `TOR` record with this tube radius.
fn torus_with_tube(minor_radius: &str) -> String {
    format!(
        r#"{{"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "{minor_radius}", "op": "new"}}"#
    )
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("torus");
}

#[test]
fn t001_default_is_ok() {
    let run = Run::example("torus/default.ptsm");

    run.assert_ok("f-tor1");
}

#[test]
fn t001_default_volume_faces_and_bounds() {
    let run = Run::example("torus/default.ptsm");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        9869.604401,
        1,
        [-25.0, -25.0, -5.0],
        [25.0, 25.0, 5.0],
    );
}

#[test]
fn t001_default_surface_is_toroidal() {
    let run = Run::example("torus/default.ptsm");

    assert_face_names(run.body("b-1"), &["f-tor1/surface"]);
    let surface = face(run.body("b-1"), "f-tor1/surface");
    assert_face_kind(&surface, GeometryKind::Torus);
    assert_face_area(&surface, 3947.841760);
}

#[test]
fn t002_resting_on_xy() {
    let run = Run::example("torus/resting-on-xy.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-25.0, -25.0, 0.0], [25.0, 25.0, 10.0]);
}

#[test]
fn t003_on_xz_has_its_axis_along_y() {
    let run = Run::example("torus/on-xz.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-25.0, -5.0, -25.0], [25.0, 5.0, 25.0]);
    let surface = face(run.body("b-1"), "f-tor1/surface");
    assert_face_axis_direction(&surface, [0.0, 1.0, 0.0]);
}

#[test]
fn t004_tube_as_large_as_the_ring_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&torus_with_tube("20")]);

    let error = run.assert_error("f-tor1", "out_of_range");
    assert_error_field(error, "minor_radius");
    assert_eq!(
        run.messages("f-tor1"),
        [
            "Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm."
        ],
    );
}

#[test]
fn t005_zero_tube_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&torus_with_tube("0")]);

    let error = run.assert_error("f-tor1", "out_of_range");
    assert_error_field(error, "minor_radius");
    assert!(
        matches!(
            error,
            FeatureError::OutOfRange { bound: Bound::GreaterThan(limit), .. } if *limit == 0.0
        ),
        "the bound should be GreaterThan(0): {error:?}",
    );
}

#[test]
fn t007_tube_below_the_tolerance_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&torus_with_tube("0.0000001")]);

    let error = run.assert_error("f-tor1", "out_of_range");
    assert_error_field(error, "minor_radius");
    assert_eq!(
        run.messages("f-tor1"),
        [
            "Tube radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t008_tube_within_the_tolerance_of_the_ring_is_out_of_range() {
    let record = r#"{"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "10", "minor_radius": "9.9999999", "op": "new"}"#;

    let run = Run::records(&["b-1"], &[record]);

    let error = run.assert_error("f-tor1", "out_of_range");
    assert_error_field(error, "minor_radius");
    assert_eq!(
        run.messages("f-tor1"),
        [
            "Tube radius must be less than the ring radius (10 mm), but it is 9.9999999 mm. Use a value less than 10 mm."
        ],
    );
    assert_empty(&run, "b-1");
}

/// Asserts the faces that tests 10 and 11 share: half of the ring of tube radius 2 on either side of the top face of
/// 'Box 1', which keeps its name with the two discs where the ring meets it taken out.
fn assert_half_ring_through_the_top(run: &Run) {
    let body = run.body("b-1");
    let surface = face(body, "f-tor1/surface");
    assert_face_kind(&surface, GeometryKind::Torus);
    assert_face_area(&surface, 394.784176);
    assert_flat_face(body, "f-box1/top", 2, 20.0, 774.867259);
}

#[test]
fn t009_intersect_keeps_the_upper_half() {
    let run = Run::example("torus/intersect.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        4934.802201,
        2,
        [-25.0, -25.0, 0.0],
        [25.0, 25.0, 5.0],
    );
    assert_eq!(body_changes(&run, "f-tor1"), ["b-1 Intersect"]);
}

#[test]
fn t009_intersect_closes_the_half_with_the_box_bottom() {
    let run = Run::example("torus/intersect.ptsm");

    let body = run.body("b-1");
    let surface = face(body, "f-tor1/surface");
    assert_face_kind(&surface, GeometryKind::Torus);
    assert_face_area(&surface, 1973.920880);
    // The ring between radius 15 and 25.
    assert_flat_face(body, "f-box1/bottom", 2, 0.0, 1256.637061);
}

#[test]
fn t010_join_adds_the_upper_half_as_a_handle() {
    let run = Run::example("torus/join.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 16394.784176);
    assert_face_count(shape, 7);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-20.0, -10.0, 0.0], [20.0, 10.0, 32.0]);
    assert_eq!(body_changes(&run, "f-tor1"), ["b-1 Join"]);
    assert_half_ring_through_the_top(&run);
}

#[test]
fn t011_cut_removes_the_lower_half_as_a_channel() {
    let run = Run::example("torus/cut.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 15605.215824);
    assert_face_count(shape, 7);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-20.0, -10.0, 0.0], [20.0, 10.0, 20.0]);
    assert_eq!(body_changes(&run, "f-tor1"), ["b-1 Cut"]);
    assert_half_ring_through_the_top(&run);
}

#[test]
fn t012_on_a_datum_moves_along_its_normal() {
    let run = Run::example("torus/on-datum.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        9869.604401,
        1,
        [-25.0, -15.0, -25.0],
        [25.0, -5.0, 25.0],
    );
}

#[test]
fn t012_on_a_datum_depends_on_the_datum() {
    let run = Run::example("torus/on-datum.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-tor1").features), ["f-dp1"]);
}

#[test]
fn t013_on_a_face_of_another_body_rests_on_the_face() {
    let run = Run::example("torus/on-face.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-2"),
        631.654682,
        1,
        [-10.0, -10.0, 10.0],
        [10.0, 10.0, 14.0],
    );
    assert_unchanged(&run, "f-tor1", "b-1");
}

#[test]
fn t013_on_a_face_of_another_body_reads_that_body() {
    let run = Run::example("torus/on-face.ptsm");

    let dependencies = run.dependencies("f-tor1");
    assert_eq!(link_ids(&dependencies.bodies_read), ["b-1"]);
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-2"]);
}
