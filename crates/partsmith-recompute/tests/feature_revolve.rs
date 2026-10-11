//! Acceptance tests of the `revolve` feature (docs/spec/features/revolve.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/revolve/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_empty, assert_error_field,
    assert_face_area, assert_face_count, assert_face_in_plane, assert_face_kind,
    assert_face_radius, assert_has_face, assert_no_face, assert_unchanged, assert_volume,
    example_paths, face, link_ids, load_example, median_build_time,
};
use partsmith_occt_sys::{ElementGeometry, GeometryKind};

/// `SK-RING`: `RECT(10, 0, 20, 30)` on XZ with id `f-sk1`, so x from 10 to 20 and z from 0 to 30.
const RING_SKETCH: &str = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XZ"}, "entities": [{"id": "e-1", "type": "line", "start": [10, 0], "end": [20, 0]}, {"id": "e-2", "type": "line", "start": [20, 0], "end": [20, 30]}, {"id": "e-3", "type": "line", "start": [20, 30], "end": [10, 30]}, {"id": "e-4", "type": "line", "start": [10, 30], "end": [10, 0]}]}"#;

/// `P-RING`: the region of `SK-RING`.
const RING_PROFILE: &str =
    r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]}"#;

/// `AXIS-Z`: the origin Z axis.
const AXIS_Z: &str = r#"{"kind": "origin", "axis": "Z"}"#;

/// A full turn.
const FULL: &str = r#"{"kind": "full"}"#;

/// The cylinder of radius 30 and height 30 in `b-1` of tests 17 to 19.
const CYLINDER_30: &str = r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "30", "height": "30", "op": "new"}"#;

/// An angle extent.
fn angle(expr: &str) -> String {
    format!(r#"{{"kind": "angle", "expr": "{expr}"}}"#)
}

/// `RECT(x0, y0, x1, y1)` (features.md section 6.1) as the sketch `f-sk1` on the origin plane `plane`.
fn rectangle_sketch(plane: &str, corner: [i32; 2], opposite: [i32; 2]) -> String {
    let [x0, y0] = corner;
    let [x1, y1] = opposite;
    format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "{plane}"}}, "entities": [{{"id": "e-1", "type": "line", "start": [{x0}, {y0}], "end": [{x1}, {y0}]}}, {{"id": "e-2", "type": "line", "start": [{x1}, {y0}], "end": [{x1}, {y1}]}}, {{"id": "e-3", "type": "line", "start": [{x1}, {y1}], "end": [{x0}, {y1}]}}, {{"id": "e-4", "type": "line", "start": [{x0}, {y1}], "end": [{x0}, {y0}]}}]}}"#
    )
}

/// The region of a `RECT` sketch with this fingerprint.
fn rectangle_profile(fingerprint: &str) -> String {
    format!(
        r#"{{"sketch": "f-sk1", "regions": [{{"key": "e-1,e-2,e-3,e-4", "fp": "{fingerprint}"}}]}}"#
    )
}

/// The revolve `f-rev1` into `b-1`; `REV(axis, extent, op)` of the spec when `profile` is `P-RING`.
fn revolve(profile: &str, axis: &str, extent: &str, op: &str) -> String {
    format!(
        r#"{{"id": "f-rev1", "type": "revolve", "body": "b-1", "profile": {profile}, "axis": {axis}, "extent": {extent}, "op": "{op}"}}"#
    )
}

/// Runs `SK-RING` then `REV(AXIS-Z, extent, "new")` in body `b-1`.
fn run_ring(extent: &str) -> Run {
    Run::records(
        &["b-1"],
        &[RING_SKETCH, &revolve(RING_PROFILE, AXIS_Z, extent, "new")],
    )
}

/// Runs `SK-RING` then a full `REV(axis, {"kind": "full"}, "new")` in body `b-1`.
fn run_ring_about(axis: &str) -> Run {
    Run::records(
        &["b-1"],
        &[RING_SKETCH, &revolve(RING_PROFILE, axis, FULL, "new")],
    )
}

/// Asserts that a face is a cylinder of this radius.
fn assert_cylinder(geometry: &ElementGeometry, radius: f64) {
    assert_face_kind(geometry, GeometryKind::Cylinder);
    assert_face_radius(geometry, radius);
}

/// The result of test 1, shared by tests 5 and 12.
fn assert_full_ring(run: &Run) {
    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 28274.333882);
    assert_face_count(shape, 4);
    assert_bounding_box(shape, [-20.0, -20.0, 0.0], [20.0, 20.0, 30.0]);
    let body = run.body("b-1");
    let bottom = face(body, "f-rev1/lateral/e-1");
    assert_face_area(&bottom, 942.477796);
    assert_face_in_plane(&bottom, 2, 0.0);
    let outer = face(body, "f-rev1/lateral/e-2");
    assert_face_area(&outer, 3769.911184);
    assert_cylinder(&outer, 20.0);
    let top = face(body, "f-rev1/lateral/e-3");
    assert_face_area(&top, 942.477796);
    assert_face_in_plane(&top, 2, 30.0);
    let inner = face(body, "f-rev1/lateral/e-4");
    assert_face_area(&inner, 1884.955592);
    assert_cylinder(&inner, 10.0);
    assert_no_face(body, "f-rev1/start-cap");
    assert_no_face(body, "f-rev1/end-cap");
}

/// The result of test 2, shared by test 4.
fn assert_quarter_turn(run: &Run) {
    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 7068.583471);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [20.0, 20.0, 30.0]);
    let start_cap = face(run.body("b-1"), "f-rev1/start-cap");
    assert_face_area(&start_cap, 300.0);
    assert_face_in_plane(&start_cap, 1, 0.0);
    assert_end_cap(run, 10.0, 20.0);
}

/// Asserts that the end cap has area 300 and lies in the plane x = 0 with y between `low` and `high`.
fn assert_end_cap(run: &Run, low: f64, high: f64) {
    let end_cap = face(run.body("b-1"), "f-rev1/end-cap");
    assert_face_area(&end_cap, 300.0);
    assert_face_in_plane(&end_cap, 0, 0.0);
    let y = end_cap.centroid.y;
    assert!(
        low < y && y < high,
        "the end cap's centroid y {y} lies between {low} and {high}",
    );
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("revolve");
    assert!(!paths.is_empty(), "examples/revolve has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_full_ring() {
    let run = Run::example("revolve/full-ring.ptsm");

    assert_full_ring(&run);
}

#[test]
fn t002_quarter_turn() {
    let run = Run::example("revolve/quarter.ptsm");

    assert_quarter_turn(&run);
}

#[test]
fn t003_quarter_turn_reversed() {
    let run = Run::example("revolve/quarter-reverse.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 7068.583471);
    assert_bounding_box(shape, [0.0, -20.0, 0.0], [20.0, 0.0, 30.0]);
    assert_end_cap(&run, -20.0, -10.0);
}

#[test]
fn t004_bare_angle_is_degrees() {
    let run = Run::example("revolve/bare-angle.ptsm");

    assert_quarter_turn(&run);
}

#[test]
fn t005_360_deg_is_full() {
    let run = Run::example("revolve/full-angle.ptsm");

    assert_full_ring(&run);
}

#[test]
fn t006_too_large() {
    let run = run_ring(&angle("400 deg"));

    let error = run.assert_error("f-rev1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-rev1"),
        ["Angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less."],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t007_zero() {
    let run = run_ring(&angle("0"));

    run.assert_error("f-rev1", "out_of_range");
    assert_eq!(
        run.messages("f-rev1"),
        ["Angle must be greater than 0 deg, but it is 0 deg. Use a value greater than 0 deg."],
    );
}

#[test]
fn t008_wrong_dimension() {
    let run = run_ring(&angle("10 mm"));

    run.assert_error("f-rev1", "expression");
    assert_eq!(
        run.messages("f-rev1"),
        ["Angle: Expected an angle, but this expression gives a length."],
    );
}

#[test]
fn t009_profile_crosses_the_axis() {
    let run = Run::records(
        &["b-1"],
        &[
            &rectangle_sketch("XZ", [-5, 0], [5, 30]),
            &revolve(&rectangle_profile("c=0,15|a=300"), AXIS_Z, FULL, "new"),
        ],
    );

    run.assert_error("f-rev1", "profile_crosses_axis");
}

#[test]
fn t010_profile_touches_the_axis() {
    let run = Run::example("revolve/profile-on-axis.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 9424.777961);
    assert_face_count(shape, 3);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 30.0]);
    assert_has_face(run.body("b-1"), "f-rev1/lateral/e-1");
    assert_has_face(run.body("b-1"), "f-rev1/lateral/e-2");
    assert_has_face(run.body("b-1"), "f-rev1/lateral/e-3");
    assert_no_face(run.body("b-1"), "f-rev1/lateral/e-4");
}

#[test]
fn t011_sphere_from_a_half_disc() {
    let run = Run::example("revolve/sphere.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 4188.790205);
    assert_face_count(shape, 1);
    assert_bounding_box(shape, [-10.0, -10.0, -10.0], [10.0, 10.0, 10.0]);
    assert_face_area(&face(run.body("b-1"), "f-rev1/lateral/e-1"), 1256.637061);
}

#[test]
fn t012_axis_from_a_construction_line() {
    let run = Run::example("revolve/construction-line-axis.ptsm");

    assert_full_ring(&run);
}

#[test]
fn t013_sketch_line_direction_sets_the_sense() {
    let run = Run::example("revolve/line-direction-sense.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, -20.0, 0.0], [20.0, 0.0, 30.0]);
}

#[test]
fn t014_axis_not_in_the_sketch_plane() {
    let run = Run::records(
        &["b-1"],
        &[
            &rectangle_sketch("XY", [10, 0], [20, 30]),
            &revolve(&rectangle_profile("c=15,15|a=300"), AXIS_Z, FULL, "new"),
        ],
    );

    run.assert_error("f-rev1", "axis_not_in_plane");
}

#[test]
fn t015_revolve_about_x_on_xy() {
    let run = Run::example("revolve/about-x.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 28274.333882);
    assert_face_count(shape, 4);
    assert_bounding_box(shape, [0.0, -20.0, -20.0], [30.0, 20.0, 20.0]);
}

#[test]
fn t016_axis_from_a_body_edge() {
    let run = Run::example("revolve/edge-axis.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 37699.111843);
    assert_face_count(shape, 4);
    assert_bounding_box(shape, [-30.0, -25.0, 0.0], [20.0, 25.0, 30.0]);
    assert_unchanged(&run, "f-rev1", "b-2");
    assert_eq!(link_ids(&run.dependencies("f-rev1").bodies_read), ["b-2"]);
}

#[test]
fn t017_groove_cut() {
    let run = Run::example("revolve/groove-cut.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 75398.223686);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-30.0, -30.0, 0.0], [30.0, 30.0, 30.0]);
    let body = run.body("b-1");
    assert_face_in_plane(&face(body, "f-rev1/lateral/e-1"), 2, 20.0);
    assert_cylinder(&face(body, "f-rev1/lateral/e-2"), 20.0);
    assert_cylinder(&face(body, "f-rev1/lateral/e-4"), 10.0);
}

#[test]
fn t018_intersect() {
    let run = Run::example("revolve/intersect.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 28274.333882);
    assert_face_count(shape, 4);
    assert_bounding_box(shape, [-20.0, -20.0, 0.0], [20.0, 20.0, 30.0]);
}

#[test]
fn t019_new_on_a_body_with_a_solid() {
    let run = Run::records(
        &["b-1"],
        &[
            CYLINDER_30,
            &rectangle_sketch("XZ", [10, 20], [20, 30]),
            &revolve(&rectangle_profile("c=15,25|a=100"), AXIS_Z, FULL, "new"),
        ],
    );

    run.assert_error("f-rev1", "body_not_empty");
    assert_volume(run.shape("b-1"), 84823.001647);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t020_full_ring_builds_within_5_ms() {
    let document = load_example("revolve/full-ring.ptsm");

    let duration = median_build_time(&document, "f-rev1", 21);
    assert!(
        duration < Duration::from_millis(5),
        "test 1 took {duration:?} to build (median of 21 runs), over the 5 ms budget",
    );
}

#[test]
fn t021_unknown_axis_line() {
    let run = run_ring_about(r#"{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}"#);

    let error = run.assert_error("f-rev1", "unknown_entity");
    assert_error_field(error, "axis");
    assert_eq!(
        run.messages("f-rev1"),
        ["'Sketch 1' has no line 'e-9'. Re-pick it for Axis."],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t022_axis_entity_is_not_a_line() {
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XZ"}, "entities": [{"id": "e-1", "type": "line", "start": [10, 0], "end": [20, 0]}, {"id": "e-2", "type": "line", "start": [20, 0], "end": [20, 30]}, {"id": "e-3", "type": "line", "start": [20, 30], "end": [10, 30]}, {"id": "e-4", "type": "line", "start": [10, 30], "end": [10, 0]}, {"id": "e-5", "type": "circle", "center": [50, 50], "radius": 1, "construction": true}]}"#;
    let run = Run::records(
        &["b-1"],
        &[
            sketch,
            &revolve(
                RING_PROFILE,
                r#"{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-5"}"#,
                FULL,
                "new",
            ),
        ],
    );

    run.assert_error("f-rev1", "not_a_line");
    assert_eq!(
        run.messages("f-rev1"),
        ["'e-5' in 'Sketch 1' is not a line with length. Pick a line for Axis."],
    );
}

#[test]
fn t023_axis_face_without_an_axis() {
    let run = Run::records(
        &["b-1", "b-2"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "op": "new"}"#,
            RING_SKETCH,
            &revolve(
                RING_PROFILE,
                r#"{"kind": "face", "body": "b-2", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}}"#,
                FULL,
                "new",
            ),
        ],
    );

    run.assert_error("f-rev1", "no_axis");
    assert_eq!(
        run.messages("f-rev1"),
        [
            "The face selected for Axis has no axis. Pick a cylindrical or conical face, a straight edge, a sketch line or an origin axis."
        ],
    );
    assert_unchanged(&run, "f-rev1", "b-2");
}

#[test]
fn t024_curved_axis_edge() {
    let run = Run::records(
        &["b-1", "b-2"],
        &[
            r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-2", "radius": "5", "height": "10", "op": "new"}"#,
            RING_SKETCH,
            &revolve(
                RING_PROFILE,
                r#"{"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-cyl1/side|f-cyl1/top)", "fp": "circle|n=0,0,1|c=0,0,10|l=31.415927|r=5"}}"#,
                FULL,
                "new",
            ),
        ],
    );

    run.assert_error("f-rev1", "not_straight");
    assert_eq!(
        run.messages("f-rev1"),
        [
            "The edge selected for Axis is not straight. Pick a straight edge, a sketch line or an origin axis."
        ],
    );
}
