//! Acceptance tests of the `sphere` feature (docs/spec/features/sphere.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/sphere/`.

use crate::feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_count, assert_face_kind, assert_face_names,
    assert_median_build_within, assert_volume, face, load_example,
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
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t006_default_sphere_builds_within_2_ms() {
    let document = load_example("sphere/default.ptsm");

    assert_median_build_within(&document, "f-sph1", 21, Duration::from_millis(2));
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
