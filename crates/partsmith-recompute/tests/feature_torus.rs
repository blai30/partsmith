//! Acceptance tests of the `torus` feature (docs/spec/features/torus.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/torus/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_empty, assert_error_field,
    assert_face_area, assert_face_axis_direction, assert_face_count, assert_face_kind,
    assert_face_names, assert_volume, example_paths, face, load_example, median_build_time,
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
    let paths = example_paths("torus");
    assert!(!paths.is_empty(), "examples/torus has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_default_is_ok() {
    let run = Run::example("torus/default.ptsm");

    run.assert_ok("f-tor1");
}

#[test]
fn t001_default_volume_faces_and_bounds() {
    let run = Run::example("torus/default.ptsm");

    let shape = run.shape("b-1");
    assert_volume(shape, 9869.604401);
    assert_face_count(shape, 1);
    assert_bounding_box(shape, [-25.0, -25.0, -5.0], [25.0, 25.0, 5.0]);
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
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t006_default_torus_builds_within_2_ms() {
    let document = load_example("torus/default.ptsm");

    let duration = median_build_time(&document, "f-tor1", 21);
    assert!(
        duration < Duration::from_millis(2),
        "the default torus took {duration:?} to build (median of 21 runs), over the 2 ms budget",
    );
}

#[test]
fn t007_tube_below_the_tolerance_is_out_of_range() {
    let run = Run::records(&["b-1"], &[&torus_with_tube("0.0000001")]);

    let error = run.assert_error("f-tor1", "out_of_range");
    assert_error_field(error, "minor_radius");
    assert_eq!(
        run.messages("f-tor1"),
        ["Tube radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
    assert_empty(&run, "b-1");
}
