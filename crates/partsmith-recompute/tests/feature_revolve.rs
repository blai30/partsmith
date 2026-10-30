//! Acceptance tests of the `revolve` feature (docs/spec/features/revolve.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/revolve/`, from which the tests make their other documents; the edits of tests 31 to 33
//! are in their examples, with the references stored before them.

use crate::feature_support;

use feature_support::{
    BOTH_FRONT_TOP_EDGES, FRONT_TOP_EDGES_OF_BOTH_BOXES, LENGTH_TOLERANCE, Run, absent_faces,
    assert_absolute, assert_axis_stored_as_picked, assert_bounding_box, assert_choice_between,
    assert_empty, assert_error_field, assert_examples_canonical, assert_face_area,
    assert_face_count, assert_face_in_plane, assert_face_names, assert_flat_face, assert_has_face,
    assert_messages_pass_lint, assert_no_face, assert_round_face, assert_same_body,
    assert_solid_count, assert_unchanged, assert_volume, body_changes, edge_axis, face,
    face_names_with_prefix, keep_and_pick, l_shaped_body, line, link_ids, load_example, message,
    output_body, pick, remove_axis_direction, successions, with_shared_point_moved,
};
use partsmith_features::{Diagnosis, Fix};
use partsmith_naming::Follow;
use partsmith_occt_sys::GeometryKind;

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

/// The result of test 1, shared by tests 5, 12 and 25.
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
    let outer = assert_round_face(body, "f-rev1/lateral/e-2", GeometryKind::Cylinder, 20.0);
    assert_face_area(&outer, 3769.911184);
    let top = face(body, "f-rev1/lateral/e-3");
    assert_face_area(&top, 942.477796);
    assert_face_in_plane(&top, 2, 30.0);
    let inner = assert_round_face(body, "f-rev1/lateral/e-4", GeometryKind::Cylinder, 10.0);
    assert_face_area(&inner, 1884.955592);
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
    assert_examples_canonical("revolve");
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
    assert_round_face(body, "f-rev1/lateral/e-2", GeometryKind::Cylinder, 20.0);
    assert_round_face(body, "f-rev1/lateral/e-4", GeometryKind::Cylinder, 10.0);
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
fn t021_unknown_axis_line() {
    let run = run_ring_about(r#"{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}"#);

    let error = run.assert_error("f-rev1", "unknown_entity");
    assert_error_field(error, "axis");
    assert_eq!(
        run.messages("f-rev1"),
        ["'Sketch 1' no longer has the line picked for Axis. Pick a line of 'Sketch 1' again."],
    );
    assert_eq!(
        message(&run, "f-rev1", "unknown_entity").fixes,
        [pick("/axis")]
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
    // The entity is named by its kind and coordinates, with its radius in document units (features.md section 4.1,
    // sketch.md section 4 and test 241).
    assert_eq!(
        run.messages("f-rev1"),
        [
            "The circle at (50, 50) with radius 1 mm in 'Sketch 1', picked for Axis, is not a line with length. Pick a line."
        ],
    );
    assert_messages_pass_lint(&run, "f-rev1");
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
            "The top of 'Box 1' selected for Axis has no axis. Pick a round face, a straight edge, a sketch line or an origin axis."
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
            "The top rim of 'Cylinder 1' selected for Axis is not straight. Pick a straight edge, a sketch line or an origin axis."
        ],
    );
}

#[test]
fn t025_union_of_regions_split_by_a_line() {
    // Features.md sections 3.7 and 3.10: the pieces of e-1 and e-3 on either side of the dropped
    // line merge, so the union revolves to the ring of test 1.
    let run = Run::example("revolve/union-split-by-line.ptsm");

    assert_full_ring(&run);
    assert_no_face(run.body("b-1"), "f-rev1/lateral/e-5");
}

#[test]
fn t026_angle_within_the_tolerance_of_zero() {
    let run = run_ring(&angle("0.00000001 deg"));

    let error = run.assert_error("f-rev1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-rev1"),
        [
            "Angle must be greater than 0 deg, but it is 0.00000001 deg. Use a value greater than 0 deg."
        ],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t027_a_full_turn_records_both_caps_absent() {
    // Tests 1 and 5.
    for example in ["revolve/full-ring.ptsm", "revolve/full-angle.ptsm"] {
        let run = Run::example(example);

        assert_eq!(
            absent_faces(&run, "f-rev1"),
            [
                ("f-rev1/end-cap".to_string(), "FullTurn".to_string()),
                ("f-rev1/start-cap".to_string(), "FullTurn".to_string()),
            ],
            "{example}"
        );
    }
}

#[test]
fn t027_a_quarter_turn_records_nothing_absent() {
    // Test 2.
    let run = Run::example("revolve/quarter.ptsm");

    assert!(absent_faces(&run, "f-rev1").is_empty());
}

/// `P-RING-V2`: the region of `SK-RING` as a pick stores it since file format version 2.
const RING_PROFILE_V2: &str = r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300", "outline": "e-1,e-2,e-3,e-4"}]}"#;

/// `SK-RING` without the line `e-2`, followed by `added` in its place.
fn ring_sketch_without_outer_side(added: &[String]) -> String {
    let lines: Vec<String> = [
        line("e-1", [10, 0], [20, 0]),
        line("e-3", [20, 30], [10, 30]),
        line("e-4", [10, 30], [10, 0]),
    ]
    .into_iter()
    .chain(added.iter().cloned())
    .collect();
    format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XZ"}}, "entities": [{}]}}"#,
        lines.join(", ")
    )
}

/// `REV(AXIS-Z, extent, "new")` of the edited `sketch` with `P-RING-V2`, followed by `other`.
fn ring_run(sketch: &str, extent: &str, other: &str) -> Run {
    Run::records(
        &["b-1"],
        &[
            sketch,
            &revolve(RING_PROFILE_V2, AXIS_Z, extent, "new"),
            other,
        ],
    )
}

#[test]
fn t031_a_side_line_deleted_and_drawn_again_is_followed() {
    // Corpus R06a for revolve: `e-2` deleted and `e-9` drawn in its place, with the fillet's edge as stored before the
    // edit.
    let run = Run::example("revolve/side-line-redrawn.ptsm");

    run.assert_ok("f-rev1");
    let after_revolve = output_body(&run, "f-rev1", "b-1");
    assert_has_face(&after_revolve, "f-rev1/lateral/e-9");
    assert_eq!(
        successions(&after_revolve),
        [(
            "f-rev1/lateral/e-2".to_string(),
            vec!["f-rev1/lateral/e-9".to_string()]
        )],
    );
    run.assert_followed("f-fil1", Follow::Replaced, "/edges/0");
    assert_eq!(
        run.warning_fixes("f-fil1", "reference_followed", &Diagnosis::default()),
        keep_and_pick("/edges/0"),
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t031_the_fillet_stays_on_the_outer_top_rim() {
    let run = Run::example("revolve/side-line-redrawn.ptsm");

    assert_has_face(
        run.body("b-1"),
        "f-fil1/fillet/edge(f-rev1/lateral/e-3|f-rev1/lateral/e-9)",
    );
    assert_volume(run.shape("b-1"), 28168.872718);
}

#[test]
fn t032_the_follow_lasts_through_the_next_edit() {
    // Corpus R07a for revolve: test 31's document after edit (1), then edit (2), a half turn.
    let after_first_edit = Run::example("revolve/side-line-redrawn.ptsm");
    after_first_edit.assert_notices("f-fil1", &["reference_followed"]);

    let run = Run::example("revolve/side-line-redrawn-half.ptsm");

    run.assert_ok("f-rev1");
    run.assert_notices("f-fil1", &["reference_followed"]);
    assert_eq!(
        run.messages("f-fil1"),
        after_first_edit.messages("f-fil1"),
        "the same notice as after the first edit"
    );
    assert_eq!(
        face_names_with_prefix(run.body("b-1"), "f-fil1/"),
        ["f-fil1/fillet/edge(f-rev1/lateral/e-3|f-rev1/lateral/e-9)"],
        "the bottom outer rim stays sharp"
    );
    assert_bounding_box(run.shape("b-1"), [-20.0, 0.0, 0.0], [20.0, 20.0, 30.0]);
    assert_volume(run.shape("b-1"), 14084.436359);
}

/// The outer side of `SK-RING` turned all the way round, as a pick stores it before the edits of tests 33 and 34.
const OUTER_SIDE: &str =
    r#"{"q": "f-rev1/lateral/e-2", "fp": "cylinder|n=0,0,1|c=0,0,15|a=3769.911184|r=20|adj=2,3"}"#;

/// The shell `f-sh1` of thickness 2 in `b-1` that removes the outer side.
fn shell_without_outer_side() -> String {
    format!(
        r#"{{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [{OUTER_SIDE}]}}"#
    )
}

#[test]
fn t033_a_side_line_split_by_a_collinear_line_keeps_the_whole_opening() {
    // Corpus R49 for revolve: `e-2` ends at (20, 15) and `e-6` continues it on its line, with the shell's face as
    // stored before the edit.
    let run = Run::example("revolve/side-line-split.ptsm");

    run.assert_ok("f-rev1");
    let after_revolve = output_body(&run, "f-rev1", "b-1");
    assert_eq!(
        successions(&after_revolve),
        [(
            "f-rev1/lateral/e-2".to_string(),
            vec![
                "f-rev1/lateral/e-2".to_string(),
                "f-rev1/lateral/e-6".to_string()
            ]
        )],
    );
    for side in ["f-rev1/lateral/e-2", "f-rev1/lateral/e-6"] {
        let geometry = assert_round_face(&after_revolve, side, GeometryKind::Cylinder, 20.0);
        assert_face_area(&geometry, 1884.955592);
    }
    run.assert_followed("f-sh1", Follow::Extended, "/remove_faces/0");
    let shelled = run.body("b-1");
    assert_no_face(shelled, "f-rev1/lateral/e-2");
    assert_no_face(shelled, "f-rev1/lateral/e-6");
    assert_volume(run.shape("b-1"), 7363.893180);
    assert_messages_pass_lint(&run, "f-sh1");
}

/// The lines of a notch drawn in place of `e-2` (corpus R66 for revolve): a groove 4 deep between z = 10 and 20.
fn notch_lines() -> Vec<String> {
    vec![
        line("e-9", [20, 0], [20, 10]),
        line("e-10", [20, 10], [16, 10]),
        line("e-11", [16, 10], [16, 20]),
        line("e-12", [16, 20], [20, 20]),
        line("e-13", [20, 20], [20, 30]),
    ]
}

/// The ring with `e-2` replaced by the notch, turned all the way round, and the shell opening the outer side as stored
/// before the edit.
fn notched_outer_side() -> Run {
    ring_run(
        &ring_sketch_without_outer_side(&notch_lines()),
        FULL,
        &shell_without_outer_side(),
    )
}

/// The faces made from the lines of the notch, in profile order.
const NOTCH_FACES: [&str; 5] = [
    "f-rev1/lateral/e-9",
    "f-rev1/lateral/e-10",
    "f-rev1/lateral/e-11",
    "f-rev1/lateral/e-12",
    "f-rev1/lateral/e-13",
];

#[test]
fn t034_a_side_line_replaced_by_a_notch_widens_the_face() {
    let run = notched_outer_side();

    run.assert_ok("f-rev1");
    let after_revolve = output_body(&run, "f-rev1", "b-1");
    assert_eq!(
        successions(&after_revolve),
        [(
            "f-rev1/lateral/e-2".to_string(),
            NOTCH_FACES
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )],
    );
    assert_volume(
        after_revolve.shape().expect("the ring has a solid"),
        23750.440461,
    );
    run.assert_warnings("f-sh1", &["reference_widened"]);
    assert_eq!(
        message(&run, "f-sh1", "reference_widened")
            .pointer
            .as_deref(),
        Some("/remove_faces/0"),
    );
    let shelled = run.body("b-1");
    for notch_face in NOTCH_FACES {
        assert_no_face(shelled, notch_face);
    }
    assert_volume(run.shape("b-1"), 7363.893180);
    assert_messages_pass_lint(&run, "f-sh1");
}

#[test]
fn t034_the_widened_face_offers_keep_all_use_only_the_first_line_and_pick() {
    let run = notched_outer_side();

    assert_eq!(
        run.warning_fixes("f-sh1", "reference_widened", &Diagnosis::default()),
        [
            Fix::KeepMatch {
                pointer: "/remove_faces/0".to_string()
            },
            Fix::UseReferences {
                pointer: "/remove_faces/0".to_string(),
                references: vec![run.picked_before("f-sh1", "b-1", "f-rev1/lateral/e-9")],
            },
            Fix::Pick {
                pointer: "/remove_faces/0".to_string()
            },
        ],
    );
}

// Tests 28 to 30: an edge axis keeps the side the user picked, turns with the part when its direction is stored, and
// edges that do not lie on one line are not decided by name order (features.md section 3.6).

/// Test 28's example (corpus R38: the wall `LEAN(-5)` in b-2, `SK-RING`, and a quarter turn of it about the wall's
/// edge, with its direction) with the point that `e-12` ends and `e-13` starts at moved to (`top_x`, 30), and the
/// axis's stored `direction` kept or left out.
fn quarter_turn_about_a_leaning_edge(top_x: f64, direction: bool) -> Run {
    let example = load_example("revolve/leaning-edge.ptsm");
    let mut document = with_shared_point_moved(&example, "f-sk5", ["e-12", "e-13"], [top_x, 30.0]);
    if !direction {
        remove_axis_direction(&mut document, "f-rev1", &["axis"]);
    }
    Run::new(document)
}

#[test]
fn t028_an_upright_axis_edge_turns_toward_plus_y() {
    let run = quarter_turn_about_a_leaning_edge(-4.0, true);

    run.assert_all_ok();
    // The axis is +Z through (-4, 0, 0).
    assert_bounding_box(run.shape("b-1"), [-4.0, 0.0, 0.0], [20.0, 24.0, 30.0]);
}

#[test]
fn t028_an_axis_edge_that_leans_keeps_its_side() {
    // The example is `LEAN(-5)` with the direction.
    let runs = [
        (
            "LEAN(-5) with the direction",
            Run::example("revolve/leaning-edge.ptsm"),
        ),
        (
            "LEAN(-3) with the direction",
            quarter_turn_about_a_leaning_edge(-3.0, true),
        ),
        (
            "LEAN(-5) without the direction",
            quarter_turn_about_a_leaning_edge(-5.0, false),
        ),
        (
            "LEAN(-3) without the direction",
            quarter_turn_about_a_leaning_edge(-3.0, false),
        ),
    ];
    for (case, run) in &runs {
        run.assert_all_ok();
        // The axis keeps a positive z, so the turn still goes toward +Y.
        let bounds = run
            .shape("b-1")
            .bounding_box()
            .expect("the kernel bounds a non-empty shape");
        assert_absolute(
            &format!("minimum y with {case}"),
            bounds.min[1],
            0.0,
            LENGTH_TOLERANCE,
        );
        assert!(
            bounds.max[1] > 20.0,
            "with {case} the turn goes toward +Y: {bounds:?}"
        );
    }
}

#[test]
fn t029_a_half_turn_of_the_part_turns_the_axis() {
    // Test 16's document with a quarter turn, b-2 turned by 180 degrees about X before the revolve, and the edge axis
    // with its direction.
    let run = Run::example("revolve/turned-edge-axis.ptsm");

    run.assert_all_ok();
    // The edge still lies at x = -5, y = 0, and its stored direction moves with the part to -Z.
    assert_bounding_box(run.shape("b-1"), [-5.0, -25.0, 0.0], [20.0, 0.0, 30.0]);
}

#[test]
fn t029_without_a_stored_direction_the_fingerprint_keeps_its_side() {
    // As written before format version 2: the fingerprint's +Z, as before this change.
    let mut unstored = load_example("revolve/turned-edge-axis.ptsm");
    remove_axis_direction(&mut unstored, "f-rev1", &["axis"]);

    let run = Run::new(unstored);

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-5.0, 0.0, 0.0], [20.0, 25.0, 30.0]);
}

#[test]
fn t030_axis_edges_not_on_one_line() {
    let [box_1, box_2] = l_shaped_body("b-2");
    let run = Run::records(
        &["b-1", "b-2"],
        &[
            &box_1,
            &box_2,
            RING_SKETCH,
            &revolve(
                RING_PROFILE,
                &edge_axis("b-2", BOTH_FRONT_TOP_EDGES, ""),
                FULL,
                "new",
            ),
        ],
    );

    // Neither edge is chosen by name order.
    assert_choice_between(
        &run,
        "f-rev1",
        "not_collinear",
        ("axis", "edge"),
        "b-2",
        &FRONT_TOP_EDGES_OF_BOTH_BOXES,
        "The edge selected for Axis now matches 2 edges that do not lie on one line. Pick the one you mean.",
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t035_join_fuses_the_ring_with_the_disc() {
    let run = Run::example("revolve/join.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    // 10625 pi: the disc, 2250 pi, and the ring, 9000 pi, less their overlap between radius 10 and 15 over z 0..5.
    assert_volume(shape, 33379.421944);
    assert_face_count(shape, 7);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-20.0, -20.0, -5.0], [20.0, 20.0, 30.0]);
    assert_eq!(body_changes(&run, "f-rev1"), ["b-1 Join"]);
}

#[test]
fn t035_join_keeps_the_names_of_both_features() {
    let run = Run::example("revolve/join.ptsm");

    let body = run.body("b-1");
    assert_face_names(
        body,
        &[
            "f-cyl1/bottom",
            "f-cyl1/side",
            "f-cyl1/top",
            "f-rev1/lateral/e-1",
            "f-rev1/lateral/e-2",
            "f-rev1/lateral/e-3",
            "f-rev1/lateral/e-4",
        ],
    );
    assert_flat_face(body, "f-cyl1/bottom", 2, -5.0, 706.858347);
    let disc_side = assert_round_face(body, "f-cyl1/side", GeometryKind::Cylinder, 15.0);
    assert_face_area(&disc_side, 471.238898);
    // Inside the ring.
    assert_flat_face(body, "f-cyl1/top", 2, 5.0, 314.159265);
    // From radius 15 to 20.
    assert_flat_face(body, "f-rev1/lateral/e-1", 2, 0.0, 549.778714);
    let outer = assert_round_face(body, "f-rev1/lateral/e-2", GeometryKind::Cylinder, 20.0);
    assert_face_area(&outer, 3769.911184);
    assert_flat_face(body, "f-rev1/lateral/e-3", 2, 30.0, 942.477796);
    // From z = 5 to 30.
    let inner = assert_round_face(body, "f-rev1/lateral/e-4", GeometryKind::Cylinder, 10.0);
    assert_face_area(&inner, 1570.796327);
}

#[test]
fn t036_axis_from_a_round_face_matches_the_origin_axis() {
    let run = Run::example("revolve/face-axis.ptsm");
    let reference = Run::example("revolve/quarter.ptsm");

    run.assert_all_quiet();
    assert_quarter_turn(&run);
    assert_same_body(run.body("b-1"), reference.body("b-1"));
}

#[test]
fn t036_axis_from_a_round_face_reads_its_body() {
    let run = Run::example("revolve/face-axis.ptsm");

    assert_unchanged(&run, "f-rev1", "b-2");
    assert_eq!(link_ids(&run.dependencies("f-rev1").bodies_read), ["b-2"]);
}

#[test]
fn t036_the_round_face_axis_is_stored_as_a_pick_stores_it() {
    let run = Run::example("revolve/face-axis.ptsm");

    assert_axis_stored_as_picked(&run, "f-rev1", "/axis", "b-2", "f-cyl1/side");
}
