//! Acceptance tests of the `circular_pattern` feature (docs/spec/features/circular_pattern.md
//! section 6), run as documents through recompute (features.md section 6.1). Geometry documents are
//! the examples in `examples/circular_pattern/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_position, assert_same_body,
    assert_solid_count, assert_unchanged, assert_volume, body_changes, example_paths, face,
    link_ids, load_example, median_build_time,
};
use partsmith_model::Units;
use partsmith_occt_sys::{ElementGeometry, GeometryKind};

/// The bodies of every circular pattern test.
const BODIES: [&str; 2] = ["b-1", "b-2"];

/// The `PEG` fixture: a cylinder of radius 2 and height 5 in b-1 with its axis at x = 20, y = 0.
const PEG: &str = r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "2", "height": "5", "position": ["20", "0", "0"], "op": "new"}"#;

/// The `CP(extra)` fixture: a circular pattern of b-1 about the origin Z axis.
fn pattern(extra: &str) -> String {
    format!(
        r#"{{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {{"kind": "origin", "axis": "Z"}}, {extra}}}"#
    )
}

/// Asserts that the top disc of a peg copy is flat and centered at `center`, and returns it.
fn assert_top_disc(run: &Run, name: &str, center: [f64; 3]) -> ElementGeometry {
    let top = face(run.body("b-1"), name);
    assert_face_kind(&top, GeometryKind::Plane);
    for (axis, coordinate) in center.into_iter().enumerate() {
        assert_face_position(&top, axis, coordinate);
    }
    top
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("circular_pattern");
    assert!(!paths.is_empty(), "examples/circular_pattern has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_six_around_volume_faces_and_bounds() {
    let run = Run::example("circular_pattern/six-around.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 376.991118);
    assert_face_count(shape, 18);
    assert_solid_count(shape, 6);
    assert_bounding_box(shape, [-22.0, -19.320508, 0.0], [22.0, 19.320508, 5.0]);
}

#[test]
fn t001_six_around_third_copy_top_is_a_disc_opposite_the_peg() {
    let run = Run::example("circular_pattern/six-around.ptsm");

    let top = assert_top_disc(&run, "f-cyl1/top@f-cp1:3", [-20.0, 0.0, 5.0]);
    assert_face_area(&top, 12.566371);
}

#[test]
fn t001_six_around_changes_the_source_by_join() {
    let run = Run::example("circular_pattern/six-around.ptsm");

    assert_eq!(body_changes(&run, "f-cp1"), ["b-1 Join"]);
}

#[test]
fn t002_partial_angle_volume_faces_and_bounds() {
    let run = Run::example("circular_pattern/partial-angle.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 188.495559);
    assert_face_count(shape, 9);
    assert_bounding_box(shape, [-2.0, -2.0, 0.0], [22.0, 22.0, 5.0]);
}

#[test]
fn t002_partial_angle_spreads_the_copies_over_the_angle() {
    let run = Run::example("circular_pattern/partial-angle.ptsm");

    assert_top_disc(&run, "f-cyl1/top@f-cp1:1", [14.142136, 14.142136, 5.0]);
    assert_top_disc(&run, "f-cyl1/top@f-cp1:2", [0.0, 20.0, 5.0]);
}

#[test]
fn t003_partial_reversed() {
    let run = Run::example("circular_pattern/partial-reversed.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-2.0, -22.0, 0.0], [22.0, 2.0, 5.0]);
}

#[test]
fn t004_four_around_a_full_turn() {
    let run = Run::example("circular_pattern/four-around.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 251.327412);
    assert_face_count(shape, 12);
    assert_bounding_box(shape, [-22.0, -22.0, 0.0], [22.0, 22.0, 5.0]);
}

#[test]
fn t005_count_1_has_no_effect() {
    let run = Run::example("circular_pattern/count-one.ptsm");

    run.assert_warnings("f-cp1", &["no_effect"]);
    assert_unchanged(&run, "f-cp1", "b-1");
}

// Regression: evaluation rejects a negative count against 0, which suggested "0" and then failed
// again against the pattern's own minimum of 1.
#[test]
fn negative_count_suggests_a_count_that_works() {
    let run = Run::records(&BODIES, &[PEG, &pattern(r#""count": "-1""#)]);

    let error = run.assert_error("f-cp1", "out_of_range");
    assert_error_field(error, "count");
    assert_eq!(
        run.messages("f-cp1"),
        ["Count must be at least 1, but it is -1. Use 1 or more."]
    );
    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
}

// Regression: count 1 with op new skipped the empty-target check, so the error depended on the
// count (features.md section 3.5).
#[test]
fn count_1_with_op_new_needs_an_empty_target() {
    let run = Run::records(
        &BODIES,
        &[
            PEG,
            r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}"#,
            &pattern(r#""count": "1", "op": "new", "target": "b-2""#),
        ],
    );

    run.assert_error("f-cp1", "body_not_empty");
    assert_unchanged(&run, "f-cp1", "b-2");
    assert_volume(run.shape("b-2"), 1.0);
}

#[test]
fn t006_angle_above_360() {
    let run = Run::records(
        &BODIES,
        &[PEG, &pattern(r#""count": "3", "angle": "400 deg""#)],
    );

    let error = run.assert_error("f-cp1", "out_of_range");
    assert_error_field(error, "angle");
    assert_eq!(
        run.messages("f-cp1"),
        ["Total angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less."]
    );
}

#[test]
fn t007_zero_angle() {
    let run = Run::records(&BODIES, &[PEG, &pattern(r#""count": "3", "angle": "0""#)]);

    let error = run.assert_error("f-cp1", "out_of_range");
    assert_error_field(error, "angle");
}

#[test]
fn t008_axis_from_a_hub_face_matches_the_origin_axis() {
    let run = Run::example("circular_pattern/axis-from-face.ptsm");
    let reference = Run::example("circular_pattern/six-around.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 376.991118);
    assert_face_count(shape, 18);
    assert_solid_count(shape, 6);
    assert_bounding_box(shape, [-22.0, -19.320508, 0.0], [22.0, 19.320508, 5.0]);
    assert_same_body(run.body("b-1"), reference.body("b-1"));
}

#[test]
fn t008_axis_from_a_hub_face_reads_the_hub_body() {
    let run = Run::example("circular_pattern/axis-from-face.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-cp1").bodies_read), ["b-2"]);
    assert_unchanged(&run, "f-cp1", "b-2");
}

#[test]
fn t009_new_body() {
    let run = Run::example("circular_pattern/new-body.ptsm");

    run.assert_all_ok();
    assert_unchanged(&run, "f-cp1", "b-1");
    assert_volume(run.shape("b-1"), 62.831853);
    let copies = run.shape("b-2");
    assert_volume(copies, 314.159265);
    assert_face_count(copies, 15);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t010_six_around_builds_within_40_ms() {
    let document = load_example("circular_pattern/six-around.ptsm");

    let duration = median_build_time(&document, "f-cp1", 21);
    assert!(
        duration < Duration::from_millis(40),
        "the pattern took {duration:?} to build (median of 21 runs), over the 40 ms budget",
    );
}

#[test]
fn t011_fractional_count_below_1() {
    let run = Run::records(&BODIES, &[PEG, &pattern(r#""count": "0.5""#)]);

    let error = run.assert_error("f-cp1", "out_of_range");
    assert_error_field(error, "count");
    assert_eq!(
        run.messages("f-cp1"),
        ["Count must be at least 1, but it is 0.5. Use 1 or more."]
    );
    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
    assert_unchanged(&run, "f-cp1", "b-1");
}
