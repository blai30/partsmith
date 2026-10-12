//! Acceptance tests of the `hole` feature (docs/spec/features/hole.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/hole/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    LENGTH_TOLERANCE, Run, assert_bounding_box, assert_canonical_example, assert_coordinates,
    assert_error_field, assert_face_area, assert_face_count, assert_face_in_plane,
    assert_face_kind, assert_face_radius, assert_has_face, assert_no_face, assert_unchanged,
    assert_volume, body_update, example_paths, face, load_example, median_build_time, vertex,
};
use partsmith_features::BodyChange;
use partsmith_occt_sys::GeometryKind;

/// `BOX-BELOW`: spans [-20,-20,-10]-[20,20,0], volume 16000, top face at z = 0.
const BOX_BELOW: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}"#;

/// `SK-PTS`: the points `e-p1` (0, 0), `e-p2` (10, 10) and `e-p3` (100, 0) on XY.
const POINTS_SKETCH: &str = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}, {"id": "e-p2", "type": "point", "position": [10, 10]}, {"id": "e-p3", "type": "point", "position": [100, 0]}]}"#;

/// The extent that goes through the whole body.
const THROUGH_ALL: &str = r#""extent": {"kind": "through_all"}"#;

/// The hole `f-hole1` of diameter `diameter` at `points` (a JSON array), with `extra` members after
/// the diameter.
fn hole_with_diameter(points: &str, diameter: &str, extra: &str) -> String {
    format!(
        r#"{{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": {points}, "diameter": "{diameter}", {extra}}}"#
    )
}

/// `HOLE(points, extra)` of the spec: diameter 6.
fn hole(points: &str, extra: &str) -> String {
    hole_with_diameter(points, "6", extra)
}

/// Runs `BOX-BELOW`, `SK-PTS` and `hole` in body `b-1`.
fn run_hole(hole: &str) -> Run {
    Run::records(&["b-1"], &[BOX_BELOW, POINTS_SKETCH, hole])
}

/// The geometry of test 1, shared by tests 14, 16, 17 and 20: a through hole of diameter 6 at
/// sketch point `point`, which names its wall.
fn assert_through_hole(run: &Run, point: &str) {
    let shape = run.shape("b-1");
    assert_volume(shape, 15717.256661);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-20.0, -20.0, -10.0], [20.0, 20.0, 0.0]);
    let bore = face(run.body("b-1"), &format!("f-hole1/bore/{point}"));
    assert_face_kind(&bore, GeometryKind::Cylinder);
    assert_face_radius(&bore, 3.0);
    assert_face_area(&bore, 188.495559);
    assert_no_face(run.body("b-1"), &format!("f-hole1/entry/{point}"));
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("hole");
    assert!(!paths.is_empty(), "examples/hole has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_simple_through() {
    let run = Run::example("hole/simple-through.ptsm");

    run.assert_all_ok();
    assert_through_hole(&run, "e-p1");
    let change = &body_update(&run, "f-hole1", "b-1").change;
    assert!(matches!(change, BodyChange::Cut), "change {change:?}");
}

#[test]
fn t002_blind_flat() {
    let run = Run::example("hole/blind-flat.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15858.628331);
    assert_face_count(shape, 8);
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-hole1/bore/e-p1"), 94.247780);
    let bottom = face(body, "f-hole1/bottom/e-p1");
    assert_face_in_plane(&bottom, 2, -5.0);
    assert_face_area(&bottom, 28.274334);
}

#[test]
fn t003_blind_with_a_tip() {
    let run = Run::example("hole/blind-tip.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15841.639397);
    assert_face_count(shape, 8);
    let body = run.body("b-1");
    assert_face_kind(&face(body, "f-hole1/tip/e-p1"), GeometryKind::Cone);
    assert_no_face(run.body("b-1"), "f-hole1/bottom/e-p1");
    let apex = vertex(body, "vertex(f-hole1/tip/e-p1)");
    assert_coordinates(
        "tip apex",
        [apex.centroid.x, apex.centroid.y, apex.centroid.z],
        [0.0, 0.0, -6.802582],
        LENGTH_TOLERANCE,
    );
}

#[test]
fn t004_counterbore() {
    let run = Run::example("hole/counterbore.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15450.221286);
    assert_face_count(shape, 9);
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-hole1/counterbore/e-p1"), 138.230077);
    let floor = face(body, "f-hole1/counterbore-floor/e-p1");
    assert_face_in_plane(&floor, 2, -4.0);
    assert_face_area(&floor, 66.758844);
    assert_face_area(&face(body, "f-hole1/bore/e-p1"), 113.097336);
}

#[test]
fn t005_countersink() {
    let run = Run::example("hole/countersink.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15604.159326);
    assert_face_count(shape, 8);
    let body = run.body("b-1");
    assert_face_kind(&face(body, "f-hole1/countersink/e-p1"), GeometryKind::Cone);
    assert_face_area(&face(body, "f-hole1/bore/e-p1"), 131.946891);
}

#[test]
fn t006_two_points() {
    let run = Run::example("hole/two-points.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 15434.513322);
    assert_face_count(shape, 8);
    assert_has_face(run.body("b-1"), "f-hole1/bore/e-p1");
    assert_has_face(run.body("b-1"), "f-hole1/bore/e-p2");
}

#[test]
fn t007_one_point_off_the_body() {
    let run = Run::example("hole/point-off-body.ptsm");

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_eq!(
        run.messages("f-hole1"),
        [
            "The hole at point 'e-p3' does not touch body 'Body 1'. Move the point, or turn on Reverse."
        ],
    );
    assert_volume(run.shape("b-1"), 15717.256661);
}

#[test]
fn t008_every_point_off_the_body() {
    let run = Run::example("hole/all-points-off-body.ptsm");

    run.assert_warnings("f-hole1", &["hole_misses_body", "no_effect"]);
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t009_reverse_drills_away() {
    let run = Run::example("hole/reverse.ptsm");

    run.assert_warnings("f-hole1", &["hole_misses_body", "no_effect"]);
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t010_head_diameter_too_small() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        &format!(
            r#""hole_type": {{"kind": "counterbore", "diameter": "5", "depth": "4"}}, {THROUGH_ALL}"#
        ),
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.diameter");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Head diameter must be greater than the hole diameter (6 mm), but it is 5 mm. Use a value greater than 6 mm."
        ],
    );
}

#[test]
fn t011_counterbore_deeper_than_the_hole() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "counterbore", "diameter": "11", "depth": "6"}, "extent": {"kind": "distance", "expr": "5"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.depth");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm."
        ],
    );
}

#[test]
fn t012_countersink_angle_180() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        &format!(
            r#""hole_type": {{"kind": "countersink", "diameter": "12", "angle": "180 deg"}}, {THROUGH_ALL}"#
        ),
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.angle");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Countersink angle must be less than 180 deg, but it is 180 deg. Use a value less than 180 deg."
        ],
    );
}

#[test]
fn t013_countersink_deeper_than_the_hole() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "countersink", "diameter": "20", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "5"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.diameter");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 20 mm. Use a value less than 16 mm."
        ],
    );
}

#[test]
fn t014_tip_with_through_all() {
    let run = Run::example("hole/tip-through-all.ptsm");

    run.assert_warnings("f-hole1", &["ignored"]);
    assert_eq!(
        run.messages("f-hole1"),
        ["Tip angle is ignored because the hole goes through all."],
    );
    assert_through_hole(&run, "e-p1");
}

#[test]
fn t015_unknown_point() {
    let run = run_hole(&hole(r#"["e-p9"]"#, THROUGH_ALL));

    let error = run.assert_error("f-hole1", "unknown_entity");
    assert_error_field(error, "points[0]");
    assert_eq!(
        run.messages("f-hole1"),
        ["'Sketch 1' has no point 'e-p9'. Re-pick it for Points."],
    );
}

#[test]
fn t016_line_end_as_a_point() {
    let run = Run::example("hole/line-end.ptsm");

    run.assert_all_ok();
    assert_through_hole(&run, "e-1.start");
}

#[test]
fn t017_circle_center_as_a_point() {
    let run = Run::example("hole/circle-center.ptsm");

    run.assert_all_ok();
    assert_through_hole(&run, "e-5.center");
}

#[test]
fn t018_zero_diameter() {
    let run = run_hole(&hole_with_diameter(r#"["e-p1"]"#, "0", THROUGH_ALL));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "diameter");
}

#[test]
fn t019_no_points() {
    let run = run_hole(&hole("[]", THROUGH_ALL));

    run.assert_error("f-hole1", "empty_selection");
    assert_eq!(
        run.messages("f-hole1"),
        ["Select at least one point for Points."]
    );
}

#[test]
fn t020_sketch_on_the_body_face() {
    let run = Run::example("hole/sketch-on-face.ptsm");

    run.assert_all_ok();
    assert_through_hole(&run, "e-p1");
}

#[test]
fn repeated_point_is_drilled_once() {
    // Section 3.3 step 5: points with identical refs are used once, so the wall keeps its name
    // without a `#n` suffix.
    let run = Run::example("hole/repeated-point.ptsm");

    run.assert_all_ok();
    assert_through_hole(&run, "e-p1");
}

#[test]
fn countersink_at_its_depth_limit_is_out_of_range() {
    // A head diameter equal to D + 2 L tan(angle / 2) as computed (here 11.21572042689736) is not
    // less than that bound, even where the cone depth computed from it rounds just below L.
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "countersink", "diameter": "11.21572042689736", "angle": "82 deg"}, "extent": {"kind": "distance", "expr": "3"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.diameter");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t021_two_points_build_within_25_ms() {
    let document = load_example("hole/two-points.ptsm");

    let duration = median_build_time(&document, "f-hole1", 21);
    assert!(
        duration < Duration::from_millis(25),
        "test 6 took {duration:?} to build (median of 21 runs), over the 25 ms budget",
    );
}

#[test]
fn t022_zero_depth_with_a_counterbore() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}, "extent": {"kind": "distance", "expr": "0"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-hole1"),
        ["Depth must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t023_negative_depth_with_a_countersink() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "-2"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-hole1"),
        ["Depth must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm."],
    );
}

#[test]
fn t024_through_all_with_a_head_deeper_than_the_body() {
    let plate = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "2"], "position": ["0", "0", "-2"], "op": "new"}"#;
    let counterbore = hole(
        r#"["e-p1"]"#,
        &format!(
            r#""hole_type": {{"kind": "counterbore", "diameter": "11", "depth": "4"}}, {THROUGH_ALL}"#
        ),
    );

    let run = Run::records(&["b-1"], &[plate, POINTS_SKETCH, &counterbore]);

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 3009.933644);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-20.0, -20.0, -2.0], [20.0, 20.0, 0.0]);
    let body = run.body("b-1");
    let wall = face(body, "f-hole1/counterbore/e-p1");
    assert_face_kind(&wall, GeometryKind::Cylinder);
    assert_face_radius(&wall, 5.5);
    assert_face_area(&wall, 69.115038);
    assert_no_face(body, "f-hole1/bore/e-p1");
    assert_no_face(body, "f-hole1/counterbore-floor/e-p1");
    let change = &body_update(&run, "f-hole1", "b-1").change;
    assert!(matches!(change, BodyChange::Cut), "change {change:?}");
}

#[test]
fn t025_counterbore_ending_at_the_shoulder_within_the_tolerance() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4.9999999"}, "extent": {"kind": "distance", "expr": "5"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.depth");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Counterbore depth must be less than the hole depth (5 mm), but it is 5 mm. Use a value less than 5 mm."
        ],
    );
}

#[test]
fn t026_countersink_ending_at_the_shoulder_within_the_tolerance() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""hole_type": {"kind": "countersink", "diameter": "15.9999999", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "5"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.diameter");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 16 mm. Use a value less than 16 mm."
        ],
    );
}

/// Runs a through hole of test 1 with this diameter and asserts that it fails with `OutOfRange` on
/// `diameter`, leaving the body unchanged.
fn assert_diameter_out_of_range(diameter: &str) -> Run {
    let run = run_hole(&hole_with_diameter(r#"["e-p1"]"#, diameter, THROUGH_ALL));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "diameter");
    assert_unchanged(&run, "f-hole1", "b-1");
    run
}

#[test]
fn t027_diameter_below_the_tolerance() {
    let run = assert_diameter_out_of_range("0.0000001");

    assert_eq!(
        run.messages("f-hole1"),
        ["Diameter must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
}

#[test]
fn t028_bore_radius_below_the_tolerance() {
    let run = assert_diameter_out_of_range("0.0000015");

    let error = run.assert_error("f-hole1", "out_of_range");
    assert!(
        matches!(
            error,
            FeatureError::OutOfRange {
                bound: Bound::GreaterThan(0.0),
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn t029_depth_below_the_tolerance() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""extent": {"kind": "distance", "expr": "0.0000001"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-hole1"),
        ["Depth must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t030_counterbore_depth_below_the_tolerance() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        &format!(
            r#""hole_type": {{"kind": "counterbore", "diameter": "11", "depth": "0.0000001"}}, {THROUGH_ALL}"#
        ),
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.depth");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Counterbore depth must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."
        ],
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t031_head_within_the_tolerance_of_the_hole_diameter() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        &format!(
            r#""hole_type": {{"kind": "counterbore", "diameter": "6.0000001", "depth": "4"}}, {THROUGH_ALL}"#
        ),
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "hole_type.diameter");
    assert_eq!(
        run.messages("f-hole1"),
        [
            "Head diameter must be greater than the hole diameter (6 mm), but it is 6 mm. Use a value greater than 6 mm."
        ],
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}
