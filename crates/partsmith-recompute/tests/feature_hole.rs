//! Acceptance tests of the `hole` feature (docs/spec/features/hole.md section 6), run as documents
//! through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/hole/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    BOX_BELOW, LENGTH_TOLERANCE, MOVE_BY_100, Run, absent_faces, assert_bounding_box,
    assert_coordinates, assert_error_field, assert_examples_canonical, assert_face_area,
    assert_face_count, assert_face_in_plane, assert_face_kind, assert_face_radius, assert_has_face,
    assert_median_build_within, assert_messages_pass_lint, assert_no_face, assert_solid_count,
    assert_translation, assert_unchanged, assert_volume, body_update, document, face, face_motion,
    face_names_with_prefix, id, load_example, message, moved_box_document, output_body, pick,
    remove_entry, sketch_result, vertex,
};
use partsmith_features::{BodyChange, Bound, FeatureError, FeatureWarning, FieldRef, Fix};
use partsmith_model::Document;
use partsmith_occt_sys::GeometryKind;
use partsmith_recompute::Message;

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
    assert_examples_canonical("hole");
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

/// `SetField` of `reverse` to `value`: "Turn on Reverse" for true, "Turn off Reverse" for false (engine.md section
/// 3.22).
fn set_reverse(value: bool) -> Fix {
    Fix::SetField {
        feature: None,
        field: FieldRef {
            path: "reverse".to_string(),
            label: "Reverse".to_string(),
        },
        value: serde_json::json!(value),
    }
}

/// The messages of `f-hole1` in order: its error, then its warnings.
fn hole_messages(run: &Run) -> &[Message] {
    &run.feature("f-hole1").messages
}

/// The box of test 7's reversed variant: 40 x 40 x 10 from z = 0, above the sketch plane.
const BOX_ABOVE: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}"#;

#[test]
fn t007_one_point_off_the_body() {
    let run = Run::example("hole/point-off-body.ptsm");

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    // The hole is named by its sketch coordinates, and reversed it misses too, so no Reverse is offered.
    assert_eq!(
        run.messages("f-hole1"),
        [
            "The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
        ],
    );
    assert_eq!(message(&run, "f-hole1", "hole_misses_body").fixes, []);
    assert_volume(run.shape("b-1"), 15717.256661);
    assert_messages_pass_lint(&run, "f-hole1");
}

#[test]
fn t007_a_point_that_drills_away_from_the_body_offers_reverse() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_ABOVE,
            POINTS_SKETCH,
            &hole(r#"["e-p1", "e-p3"]"#, THROUGH_ALL),
        ],
    );

    run.assert_warnings(
        "f-hole1",
        &["hole_misses_body", "hole_misses_body", "no_effect"],
    );
    let messages = hole_messages(&run);
    assert_eq!(
        messages[0].text,
        "The hole at (0, 0) in 'Sketch 1' does not touch body 'Body 1'."
    );
    assert_eq!(messages[0].fixes, [set_reverse(true)]);
    assert_eq!(
        messages[1].text,
        "The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
    );
    assert_eq!(messages[1].fixes, []);
    assert_eq!(
        messages[2].text,
        "This hole cuts nothing from body 'Body 1': every hole misses it."
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t007_turning_on_reverse_drills_the_hole_that_reaches_the_body() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_ABOVE,
            POINTS_SKETCH,
            &hole(
                r#"["e-p1", "e-p3"]"#,
                &format!(r#"{THROUGH_ALL}, "reverse": true"#),
            ),
        ],
    );

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_eq!(
        run.messages("f-hole1"),
        [
            "The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
        ],
    );
    assert_volume(run.shape("b-1"), 15717.256661);
}

#[test]
fn t007_reverse_is_not_offered_when_another_point_reaches_the_body() {
    // A box below the sketch plane under e-p1 and one above it at e-p3: drilled down, e-p1 reaches the body and e-p3
    // misses, and Reverse would turn both, so e-p1 would miss instead (features/hole.md section 3.3 step 5).
    let box_above_e_p3 = r#"{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["100", "0", "0"], "op": "join"}"#;
    let run = Run::records(
        &["b-1"],
        &[
            BOX_BELOW,
            box_above_e_p3,
            POINTS_SKETCH,
            &hole(r#"["e-p1", "e-p3"]"#, THROUGH_ALL),
        ],
    );

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_eq!(
        run.messages("f-hole1"),
        [
            "The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
        ],
    );
    assert_eq!(message(&run, "f-hole1", "hole_misses_body").fixes, []);
    assert_volume(run.shape("b-1"), 16717.256661);
}

#[test]
fn t008_every_point_off_the_body() {
    let run = Run::example("hole/all-points-off-body.ptsm");

    // One point, one message: its own warning already says that nothing changed and why (features.md section 3.15).
    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t008_every_one_of_two_points_off_the_body() {
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}, {"id": "e-p2", "type": "point", "position": [10, 10]}, {"id": "e-p3", "type": "point", "position": [100, 0]}, {"id": "e-p4", "type": "point", "position": [-100, 0]}]}"#;
    let run = Run::records(
        &["b-1"],
        &[BOX_BELOW, sketch, &hole(r#"["e-p3", "e-p4"]"#, THROUGH_ALL)],
    );

    run.assert_warnings(
        "f-hole1",
        &["hole_misses_body", "hole_misses_body", "no_effect"],
    );
    let warning = run.warning("f-hole1", "no_effect");
    assert!(
        matches!(
            warning,
            FeatureWarning::NoEffect {
                how: partsmith_features::NoEffectHow::Missed,
                ..
            }
        ),
        "{warning:?}"
    );
    assert_eq!(
        message(&run, "f-hole1", "no_effect").text,
        "This hole cuts nothing from body 'Body 1': every hole misses it."
    );
    assert_unchanged(&run, "f-hole1", "b-1");
    assert_messages_pass_lint(&run, "f-hole1");
}

#[test]
fn t009_reverse_drills_away() {
    let run = Run::example("hole/reverse.ptsm");

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_eq!(
        run.messages("f-hole1"),
        ["The hole at (0, 0) in 'Sketch 1' does not touch body 'Body 1'."],
    );
    // Verified: drilled the other way, the hole reaches the body, so Turn off Reverse is offered.
    assert_eq!(
        message(&run, "f-hole1", "hole_misses_body").fixes,
        [set_reverse(false)]
    );
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
    // No point is left, so the hole fails (features/hole.md section 3.3 step 4).
    assert_eq!(
        run.messages("f-hole1"),
        ["'Sketch 1' no longer has the point picked for Points. Pick a point of 'Sketch 1' again."],
    );
    assert_eq!(
        message(&run, "f-hole1", "unknown_entity").fixes,
        [pick("/points/0")]
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

    assert_median_build_within(&document, "f-hole1", 21, Duration::from_millis(25));
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
        // Features/hole.md test 22: a depth of exactly 0 reads as 0.
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
            "Counterbore depth must be less than the hole depth (5 mm), but it is 4.9999999 mm. Use a value less than 5 mm."
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
            "Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 15.9999999 mm. Use a value less than 16 mm."
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
        [
            "Diameter must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
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
        ["Depth must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."],
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
            "Counterbore depth must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
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
            "Head diameter must be greater than the hole diameter (6 mm), but it is 6.0000001 mm. Use a value greater than 6 mm."
        ],
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t032_tools_decided_by_the_cut_and_by_their_shared_volume() {
    let run = Run::example("hole/swallowed-solid.ptsm");

    // The small cube does not touch the box, so its join leaves two pieces (features.md section 3.5).
    run.assert_warnings("f-box2", &["separate_solid"]);
    let joined = output_body(&run, "f-box2", "b-1");
    let joined_shape = joined.shape().expect("the body holds the box and the cube");
    assert_solid_count(joined_shape, 2);
    assert_volume(joined_shape, 16008.0);
    assert_bounding_box(joined_shape, [-20.0, -20.0, -10.0], [31.0, 20.0, 0.0]);
    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    // Reversed, it misses too.
    assert_eq!(
        run.messages("f-hole1"),
        [
            "The hole at (25, 15) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
        ],
    );
    let shape = run.shape("b-1");
    assert_volume(shape, 15717.256661);
    assert_face_count(shape, 7);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-20.0, -20.0, -10.0], [20.0, 20.0, 0.0]);
    let body = run.body("b-1");
    assert_has_face(body, "f-hole1/bore/e-p1");
    assert_no_face(body, "f-hole1/bore/e-p4");
    assert_no_face(body, "f-hole1/bore/e-p5");
    assert_eq!(
        face_names_with_prefix(body, "f-box2/"),
        Vec::<String>::new(),
        "the swallowed cube is gone"
    );
}

/// Test 33: the 200 x 200 x 10 plate with 194 through holes of diameter 8 (200 faces), then ten
/// holes of diameter 6 between them.
fn ten_holes_in_a_200_face_body() -> Document {
    let plate = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["200", "200", "10"], "position": ["0", "0", "-10"], "op": "new"}"#;
    let sketch = |id: &str, points: &[[f64; 2]]| {
        let entities: Vec<String> = (1..)
            .zip(points)
            .map(|(number, [x, y])| {
                format!(r#"{{"id": "e-p{number}", "type": "point", "position": [{x}, {y}]}}"#)
            })
            .collect();
        format!(
            r#"{{"id": "{id}", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{}]}}"#,
            entities.join(", ")
        )
    };
    let hole = |id: &str, sketch: &str, count: usize, diameter: &str| {
        let points: Vec<String> = (1..=count)
            .map(|number| format!(r#""e-p{number}""#))
            .collect();
        format!(
            r#"{{"id": "{id}", "type": "hole", "body": "b-1", "sketch": "{sketch}", "points": [{}], "diameter": "{diameter}", {THROUGH_ALL}}}"#,
            points.join(", ")
        )
    };
    let existing: Vec<[f64; 2]> = (0..194_u32)
        .map(|index| {
            [
                -90.0 + 13.5 * f64::from(index % 14),
                -90.0 + 13.5 * f64::from(index / 14),
            ]
        })
        .collect();
    let new: Vec<[f64; 2]> = (0..10_u32)
        .map(|index| [-83.25 + 13.5 * f64::from(index), -83.25])
        .collect();
    document(
        &["b-1"],
        &[
            plate,
            &sketch("f-sk1", &existing),
            &hole("f-hole1", "f-sk1", existing.len(), "8"),
            &sketch("f-sk2", &new),
            &hole("f-hole2", "f-sk2", new.len(), "6"),
        ],
    )
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t033_ten_holes_in_a_200_face_body_within_80_ms() {
    let document = ten_holes_in_a_200_face_body();
    let run = Run::new(document.clone());
    run.assert_all_ok();
    let plate = output_body(&run, "f-hole1", "b-1");
    assert_face_count(plate.shape().expect("the plate is a solid"), 200);
    assert_face_count(run.shape("b-1"), 210);

    assert_median_build_within(&document, "f-hole2", 7, Duration::from_millis(80));
}

#[test]
fn t036_a_through_hole_records_its_bottom_absent() {
    let run = run_hole(&hole(r#"["e-p1"]"#, THROUGH_ALL));

    run.assert_ok("f-hole1");
    assert_eq!(
        absent_faces(&run, "f-hole1"),
        [("f-hole1/bottom/e-p1".to_string(), "Through".to_string())]
    );
}

#[test]
fn t036_a_blind_hole_records_nothing_absent() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""extent": {"kind": "distance", "expr": "5"}"#,
    ));

    run.assert_ok("f-hole1");
    assert!(absent_faces(&run, "f-hole1").is_empty());
}

#[test]
fn t036_a_hole_that_starts_inside_the_body_ends_outside() {
    let datum = r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "-5"}"#;
    let sketch = POINTS_SKETCH.replace(
        r#""plane": {"kind": "origin", "plane": "XY"}"#,
        r#""plane": {"kind": "datum", "feature": "f-dp1"}"#,
    );
    let through = hole(r#"["e-p1"]"#, THROUGH_ALL);

    let run = Run::records(&["b-1"], &[BOX_BELOW, datum, &sketch, &through]);

    run.assert_ok("f-hole1");
    assert_eq!(
        absent_faces(&run, "f-hole1"),
        [("f-hole1/bottom/e-p1".to_string(), "EndOutside".to_string())]
    );
}

#[test]
fn t037_the_holes_faces_move_with_the_part() {
    let run = Run::new(moved_box_document(MOVE_BY_100, Some("body")));

    run.assert_all_ok();
    let motion = face_motion(&run, "f-hole1", "b-1", "f-hole1/bore/e-p1");
    assert_translation(&motion, [100.0, 0.0, 0.0]);
    assert_eq!(motion, sketch_result(&run, "f-sk2").motion);
}

#[test]
fn t034_a_deleted_point_is_skipped() {
    // Corpus R43b: `SK-PTS` has no `e-p9`, so the hole at e-p1 is drilled and e-p9 is skipped.
    let run = run_hole(&hole(r#"["e-p1", "e-p9"]"#, THROUGH_ALL));

    run.assert_warnings("f-hole1", &["point_missing"]);
    let missing = message(&run, "f-hole1", "point_missing");
    assert_eq!(missing.pointer.as_deref(), Some("/points/1"));
    assert_eq!(
        missing.text,
        "This hole no longer makes the hole at a point that was deleted from 'Sketch 1', so it was skipped."
    );
    // Remove from Points, then Pick.
    assert_eq!(
        missing.fixes,
        [remove_entry("/points/1"), pick("/points/1")]
    );
    let warning = run.warning("f-hole1", "point_missing");
    assert!(
        matches!(
            warning,
            FeatureWarning::PointMissing { position: 1, sketch, point, .. }
                if *sketch == id("f-sk1") && point == "e-p9"
        ),
        "{warning:?}"
    );
    assert_volume(run.shape("b-1"), 15717.256661);
    assert_has_face(run.body("b-1"), "f-hole1/bore/e-p1");
    assert_messages_pass_lint(&run, "f-hole1");
}

#[test]
fn t034_removing_the_deleted_point_leaves_the_hole_without_a_warning() {
    let run = run_hole(&hole(r#"["e-p1"]"#, THROUGH_ALL));

    run.assert_all_quiet();
    assert_volume(run.shape("b-1"), 15717.256661);
}

#[test]
fn a_deleted_point_between_two_holes_skips_only_that_point() {
    let run = run_hole(&hole(r#"["e-p1", "e-p9", "e-p2"]"#, THROUGH_ALL));

    run.assert_warnings("f-hole1", &["point_missing"]);
    assert_eq!(
        message(&run, "f-hole1", "point_missing").pointer.as_deref(),
        Some("/points/1")
    );
    assert_volume(run.shape("b-1"), 15434.513322);
    assert_has_face(run.body("b-1"), "f-hole1/bore/e-p1");
    assert_has_face(run.body("b-1"), "f-hole1/bore/e-p2");
}

#[test]
fn every_point_deleted_fails_on_the_first_point() {
    let run = run_hole(&hole(r#"["e-p8", "e-p9"]"#, THROUGH_ALL));

    let error = run.assert_error("f-hole1", "unknown_entity");
    assert_error_field(error, "points[0]");
    assert!(
        matches!(error, FeatureError::UnknownEntity { entity, .. } if entity == "e-p8"),
        "{error:?}"
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}

#[test]
fn t035_a_hole_beside_the_body() {
    // Newcomer error corpus NC12: a face sketch on the top of `BLOCK` with its point at (95, 25), past the right side.
    let rectangle = r#"[{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}]"#;
    let sketch = format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": {rectangle}}}"#
    );
    let block = r#"{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}, "extent": {"kind": "distance", "expr": "30"}, "op": "new"}"#;
    let face_sketch = r#"{"id": "f-sk2", "type": "sketch", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}, "entities": [{"id": "e-p1", "type": "point", "position": [95, 25]}]}"#;
    let hole = r#"{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "5", "extent": {"kind": "through_all"}}"#;
    let run = Run::records(&["b-1"], &[&sketch, block, face_sketch, hole]);

    // One point, so no `no_effect`; reversed it misses too, so no Reverse.
    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_eq!(
        run.messages("f-hole1"),
        [
            "The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body."
        ],
    );
    assert_eq!(message(&run, "f-hole1", "hole_misses_body").fixes, []);
    assert_volume(run.shape("b-1"), 120000.0);
    assert_messages_pass_lint(&run, "f-hole1");
}

#[test]
fn a_diameter_beyond_the_modeling_limit() {
    let run = assert_diameter_out_of_range("1e200");

    assert_eq!(
        run.messages("f-hole1"),
        ["Diameter must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."],
    );
    let error = run.assert_error("f-hole1", "out_of_range");
    assert!(
        matches!(
            error,
            FeatureError::OutOfRange {
                bound: Bound::AtMost(limit),
                ..
            } if *limit == 1_000_000.0
        ),
        "{error:?}"
    );
}

#[test]
fn a_depth_beyond_the_modeling_limit_is_checked_after_its_own_bound() {
    let run = run_hole(&hole(
        r#"["e-p1"]"#,
        r#""extent": {"kind": "distance", "expr": "1e200"}"#,
    ));

    let error = run.assert_error("f-hole1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-hole1"),
        ["Depth must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."],
    );
    assert_unchanged(&run, "f-hole1", "b-1");
}
