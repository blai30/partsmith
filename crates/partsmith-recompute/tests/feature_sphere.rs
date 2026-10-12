//! Acceptance tests of the `sphere` feature (docs/spec/features/sphere.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/sphere/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_names, assert_volume, example_paths, face,
    load_example, median_build_time,
};
use partsmith_occt_sys::GeometryKind;

#[test]
fn examples_are_canonical() {
    let paths = example_paths("sphere");
    assert!(!paths.is_empty(), "examples/sphere has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
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

    let duration = median_build_time(&document, "f-sph1", 21);
    assert!(
        duration < Duration::from_millis(2),
        "the default sphere took {duration:?} to build (median of 21 runs), over the 2 ms budget",
    );
}
