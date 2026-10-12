//! Acceptance tests of the `cone` feature (docs/spec/features/cone.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/cone/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_names, assert_volume, example_paths, face,
    face_index, load_example, median_build_time,
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
    let paths = example_paths("cone");
    assert!(!paths.is_empty(), "examples/cone has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
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

// Radii within the kernel's modeling tolerance (kernel.md section 3.4.1): radii closer than 1e-6
// mm make a cylinder, and a radius below 1e-6 mm is an apex, so every pair of radii the checks
// accept builds.

#[test]
fn nearly_equal_radii_make_a_cylinder() {
    let run = Run::example("cone/nearly-equal-radii.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 785.398163);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
    assert_face_kind(&face(run.body("b-1"), "f-cone1/side"), GeometryKind::Cylinder);
}

#[test]
fn a_radius_below_the_tolerance_is_an_apex() {
    let run = Run::example("cone/apex-below-tolerance.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 1047.197551);
    assert_face_count(shape, 2);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
    assert_face_names(run.body("b-1"), &["f-cone1/bottom", "f-cone1/side"]);
}

#[test]
fn radii_below_the_tolerance_are_both_zero() {
    let run = Run::records(&["b-1"], &[&cone("0.0000000001", "0", "10")]);

    run.assert_error("f-cone1", "cone_radii_zero");
    assert_eq!(
        run.messages("f-cone1"),
        ["Cone bottom and top radius cannot both be 0. Give one of them a size."],
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t008_frustum_builds_within_2_ms() {
    let document = load_example("cone/frustum.ptsm");

    let duration = median_build_time(&document, "f-cone1", 21);
    assert!(
        duration < Duration::from_millis(2),
        "the frustum took {duration:?} to build (median of 21 runs), over the 2 ms budget",
    );
}
