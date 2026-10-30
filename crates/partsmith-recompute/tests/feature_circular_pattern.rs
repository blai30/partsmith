//! Acceptance tests of the `circular_pattern` feature (docs/spec/features/circular_pattern.md
//! section 6), run as documents through recompute (features.md section 6.1). Geometry documents are
//! the examples in `examples/circular_pattern/`, those of tests 12 to 14 (frames on the model)
//! among them (`sketch-on-copy`, `turned-edge-axis` and `leaning-edge`), from which the tests make
//! their other documents.

use crate::feature_support;

use feature_support::{
    BOTH_FRONT_TOP_EDGES, FRONT_TOP_EDGES_OF_BOTH_BOXES, LENGTH_TOLERANCE, Run, assert_absolute,
    assert_axis_stored_as_picked, assert_bore_through, assert_bounding_box, assert_choice_between,
    assert_coordinates, assert_error_field, assert_examples_canonical, assert_face_area,
    assert_face_centroid, assert_face_count, assert_face_kind, assert_placement,
    assert_quarter_turns_about_z, assert_same_body, assert_solid_count, assert_unchanged,
    assert_volume, body_changes, edge_axis, face, feature_record_mut, l_shaped_body, link_ids,
    load_example, remove_axis_direction, sketch_result, with_shared_point_moved,
};
use partsmith_model::Units;
use partsmith_occt_sys::{ElementGeometry, GeometryKind};
use serde_json::json;

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
    assert_face_centroid(&top, center);
    top
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("circular_pattern");
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
    assert_eq!(
        run.messages("f-cp1"),
        ["This circular pattern makes no copies, because Count is 1."]
    );
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

// Tests 12 to 15: copies carry their rotation as the motion of their faces, and an axis keeps the side the user picked
// (features.md sections 3.6 and 3.9).

#[test]
fn t012_a_sketch_on_a_copys_face_follows_the_angle() {
    let run = Run::example("circular_pattern/sketch-on-copy.ptsm");

    run.assert_all_ok();
    let sketch = sketch_result(&run, "f-sk2");
    assert_placement(
        &sketch.placement,
        [0.0, 20.0, 5.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_quarter_turns_about_z(&sketch.motion, 1);
    assert_bore_through(&run, [0.0, 20.0, 0.0]);
    assert_volume(run.shape("b-1"), 184.568568);
}

#[test]
fn t012_the_sketch_follows_a_wider_angle() {
    let mut wider = load_example("circular_pattern/sketch-on-copy.ptsm");
    feature_record_mut(&mut wider, "f-cp1").params["angle"] = json!("120 deg");

    let run = Run::new(wider);

    run.assert_all_ok();
    let origin = sketch_result(&run, "f-sk2").placement.origin;
    assert_coordinates(
        "placement origin",
        origin,
        [-10.0, 17.320508, 5.0],
        LENGTH_TOLERANCE,
    );
    // Copy 2 sits at 120 degrees: (20 cos 120, 20 sin 120) = (-10, 10 sqrt 3).
    assert_bore_through(&run, [-10.0, 10.0 * 3.0_f64.sqrt(), 0.0]);
    assert_volume(run.shape("b-1"), 184.568568);
}

#[test]
fn t013_a_half_turn_of_the_part_turns_the_axis() {
    // The axis edge as picked before the half turn of b-2 about X was inserted, with its direction.
    let run = Run::example("circular_pattern/turned-edge-axis.ptsm");

    run.assert_all_ok();
    // The edge still lies on the Z axis and its stored direction turns with the part to -Z.
    assert_bounding_box(run.shape("b-1"), [-2.0, -22.0, 0.0], [22.0, 2.0, 5.0]);
}

#[test]
fn t013_without_a_stored_direction_the_fingerprint_keeps_its_side() {
    let mut unstored = load_example("circular_pattern/turned-edge-axis.ptsm");
    remove_axis_direction(&mut unstored, "f-cp1", &["axis"]);

    let run = Run::new(unstored);

    run.assert_all_ok();
    // The fingerprint's +Z, as test 2.
    assert_bounding_box(run.shape("b-1"), [-2.0, -2.0, 0.0], [22.0, 22.0, 5.0]);
}

#[test]
fn t014_an_axis_edge_that_leans_keeps_its_side() {
    // The example's wall is `LEAN(-5)`; the test moves the point that `e-12` ends and `e-13` starts at.
    let leaning = Run::example("circular_pattern/leaning-edge.ptsm");
    let others = [-4.0, -3.0].map(|top_x| {
        Run::new(with_shared_point_moved(
            &leaning.document,
            "f-sk5",
            ["e-12", "e-13"],
            [top_x, 30.0],
        ))
    });
    let [upright, leaning_right] = &others;

    for (top_x, run) in [(-4, upright), (-5, &leaning), (-3, leaning_right)] {
        run.assert_all_ok();
        // The axis keeps a positive z, so the copy turns toward +Y.
        let bounds = run
            .shape("b-1")
            .bounding_box()
            .expect("the kernel bounds a non-empty shape");
        assert!(
            bounds.max[1] > 20.0,
            "with LEAN({top_x}) the copy turns toward +Y: {bounds:?}"
        );
        assert_absolute(
            &format!("minimum y with LEAN({top_x})"),
            bounds.min[1],
            -2.0,
            LENGTH_TOLERANCE,
        );
    }
}

#[test]
fn t015_axis_edges_not_on_one_line() {
    let [box_1, box_2] = l_shaped_body("b-2");
    let run = Run::records(
        &BODIES,
        &[
            &box_1,
            &box_2,
            PEG,
            &format!(
                r#"{{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {}, "count": "3"}}"#,
                edge_axis("b-2", BOTH_FRONT_TOP_EDGES, "")
            ),
        ],
    );

    assert_choice_between(
        &run,
        "f-cp1",
        "not_collinear",
        ("axis", "edge"),
        "b-2",
        &FRONT_TOP_EDGES_OF_BOTH_BOXES,
        "The edge selected for Axis now matches 2 edges that do not lie on one line. Pick the one you mean.",
    );
    assert_unchanged(&run, "f-cp1", "b-1");
}

/// Asserts that b-1 is identical to test 2's partial angle: three pegs over 90 degrees about +Z.
fn assert_partial_angle(run: &Run) {
    let reference = Run::example("circular_pattern/partial-angle.ptsm");
    let shape = run.shape("b-1");
    assert_volume(shape, 188.495559);
    assert_face_count(shape, 9);
    assert_bounding_box(shape, [-2.0, -2.0, 0.0], [22.0, 22.0, 5.0]);
    assert_top_disc(run, "f-cyl1/top@f-cp1:2", [0.0, 20.0, 5.0]);
    assert_same_body(run.body("b-1"), reference.body("b-1"));
}

#[test]
fn t016_axis_from_a_sketch_line_runs_from_its_start_to_its_end() {
    let run = Run::example("circular_pattern/axis-from-sketch-line.ptsm");

    run.assert_all_quiet();
    assert_partial_angle(&run);
}

#[test]
fn t016_axis_from_a_sketch_line_depends_on_the_sketch() {
    let run = Run::example("circular_pattern/axis-from-sketch-line.ptsm");

    assert_eq!(link_ids(&run.dependencies("f-cp1").features), ["f-sk1"]);
}

#[test]
fn t017_axis_from_a_straight_edge() {
    let run = Run::example("circular_pattern/axis-from-edge.ptsm");

    run.assert_all_quiet();
    assert_partial_angle(&run);
}

#[test]
fn t017_axis_from_a_straight_edge_reads_its_body() {
    let run = Run::example("circular_pattern/axis-from-edge.ptsm");

    assert_unchanged(&run, "f-cp1", "b-2");
    assert_eq!(link_ids(&run.dependencies("f-cp1").bodies_read), ["b-2"]);
}

#[test]
fn t017_the_edge_axis_is_stored_as_a_pick_stores_it() {
    let run = Run::example("circular_pattern/axis-from-edge.ptsm");

    assert_axis_stored_as_picked(
        &run,
        "f-cp1",
        "/axis",
        "b-2",
        "edge(f-box2/front|f-box2/left)",
    );
}

#[test]
fn t018_canceled_build_returns_canceled() {
    let run = Run::example("circular_pattern/six-around.ptsm");

    run.assert_build_canceled("f-cp1");
}
