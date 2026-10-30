//! Acceptance tests of the `linear_pattern` feature (docs/spec/features/linear_pattern.md section
//! 6), run as documents through recompute (features.md section 6.1). Geometry documents are the
//! examples in `examples/linear_pattern/`, those of tests 17 and 18 (frames on the model) among
//! them (`leaning-edge` and `turned-edge-direction`), from which the tests make their other
//! documents; test 16 runs the document of features/sketch.md test 27,
//! `examples/sketch/on-pattern-copy`.

use crate::feature_support;

use feature_support::linear_pattern::{BODIES, CUBE10, X, direction, pattern};
use feature_support::{
    BOTH_FRONT_TOP_EDGES, FRONT_TOP_EDGES_OF_BOTH_BOXES, Run, assert_axis_stored_as_picked,
    assert_bore_through, assert_bounding_box, assert_choice_between, assert_error_field,
    assert_examples_canonical, assert_face_area, assert_face_centroid, assert_face_count,
    assert_face_kind, assert_face_position, assert_face_translated, assert_has_face,
    assert_same_body, assert_solid_count, assert_unchanged, assert_volume, body_changes, edge_axis,
    face, feature_record_mut, l_shaped_body, link_ids, load_example, remove_axis_direction,
    set_field, with_shared_point_moved,
};
use partsmith_model::{Document, Units};
use partsmith_occt_sys::GeometryKind;

/// The origin Y axis as an axis reference.
const Y: &str = r#"{"kind": "origin", "axis": "Y"}"#;

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("linear_pattern");
}

#[test]
fn t001_three_apart_volume_faces_and_bounds() {
    let run = Run::example("linear_pattern/three-apart.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 3000.0);
    assert_face_count(shape, 18);
    assert_solid_count(shape, 3);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [45.0, 5.0, 10.0]);
}

#[test]
fn t001_three_apart_first_copy_top() {
    let run = Run::example("linear_pattern/three-apart.ptsm");

    // Planar at z = 10, spanning x 15..25: centered at x = 20 with the full 10 by 10 area.
    let top = face(run.body("b-1"), "f-box1/top@f-lp1:1");
    assert_face_kind(&top, GeometryKind::Plane);
    assert_face_position(&top, 2, 10.0);
    assert_face_position(&top, 0, 20.0);
    assert_face_area(&top, 100.0);
}

#[test]
fn t001_three_apart_second_copy_right() {
    let run = Run::example("linear_pattern/three-apart.ptsm");

    let right = face(run.body("b-1"), "f-box1/right@f-lp1:2");
    assert_face_kind(&right, GeometryKind::Plane);
    assert_face_position(&right, 0, 45.0);
}

#[test]
fn t001_three_apart_source_keeps_its_names() {
    let run = Run::example("linear_pattern/three-apart.ptsm");

    let right = face(run.body("b-1"), "f-box1/right");
    assert_face_position(&right, 0, 5.0);
    assert_eq!(body_changes(&run, "f-lp1"), ["b-1 Join"]);
}

#[test]
fn t002_overlapping_copies_fuse() {
    let run = Run::example("linear_pattern/overlapping.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_face_count(shape, 6);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [15.0, 5.0, 10.0]);
}

#[test]
fn t003_grid_volume_faces_and_bounds() {
    let run = Run::example("linear_pattern/grid.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 36);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [45.0, 20.0, 10.0]);
}

#[test]
fn t003_grid_instance_names() {
    let run = Run::example("linear_pattern/grid.ptsm");

    let body = run.body("b-1");
    for name in [
        "f-box1/top@f-lp1:1",
        "f-box1/top@f-lp1:0.1",
        "f-box1/top@f-lp1:2.1",
    ] {
        assert_has_face(body, name);
    }
}

#[test]
fn t004_reverse() {
    let run = Run::example("linear_pattern/reverse.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-45.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
}

#[test]
fn t005_count_1_has_no_effect() {
    let run = Run::example("linear_pattern/count-one.ptsm");

    run.assert_warnings("f-lp1", &["no_effect"]);
    assert_eq!(
        run.messages("f-lp1"),
        ["This linear pattern makes no copies, because Count is 1."]
    );
    assert_unchanged(&run, "f-lp1", "b-1");
}

#[test]
fn t005_count_1_in_both_directions_names_both_counts() {
    let run = Run::records(
        &BODIES,
        &[
            CUBE10,
            &pattern(
                &direction(X, "1", "20"),
                &format!(r#""second_direction": {}"#, direction(Y, "1", "15")),
            ),
        ],
    );

    run.assert_warnings("f-lp1", &["no_effect"]);
    assert_eq!(
        run.messages("f-lp1"),
        ["This linear pattern makes no copies, because Count and Second count are 1."]
    );
    assert_unchanged(&run, "f-lp1", "b-1");
}

#[test]
fn t006_fractional_count() {
    let run = Run::records(&BODIES, &[CUBE10, &pattern(&direction(X, "2.5", "20"), "")]);

    let error = run.assert_error("f-lp1", "not_whole_number");
    assert_error_field(error, "direction.count");
    // The floor and the ceiling are the fixes, so the sentence does not repeat them (features.md section 4.1).
    assert_eq!(
        run.messages("f-lp1"),
        ["Count must be a whole number, but it is 2.5."]
    );
    let use_count = |value: &str| set_field("direction.count", "Count", serde_json::json!(value));
    assert_eq!(
        run.feature("f-lp1").messages[0].fixes,
        [use_count("2"), use_count("3")]
    );
}

#[test]
fn t007_zero_count() {
    let run = Run::records(&BODIES, &[CUBE10, &pattern(&direction(X, "0", "20"), "")]);

    let error = run.assert_error("f-lp1", "out_of_range");
    assert_error_field(error, "direction.count");
    assert_eq!(
        run.messages("f-lp1"),
        ["Count must be at least 1, but it is 0. Use 1 or more."]
    );
}

// Regression: evaluation rejects a negative count against 0, which suggested "0" and then failed
// again against the pattern's own minimum of 1.
#[test]
fn negative_count_suggests_a_count_that_works() {
    let run = Run::records(&BODIES, &[CUBE10, &pattern(&direction(X, "-1", "20"), "")]);

    let error = run.assert_error("f-lp1", "out_of_range");
    assert_error_field(error, "direction.count");
    assert_eq!(
        run.messages("f-lp1"),
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
            CUBE10,
            r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}"#,
            &pattern(&direction(X, "1", "20"), r#""op": "new", "target": "b-2""#),
        ],
    );

    run.assert_error("f-lp1", "body_not_empty");
    assert_unchanged(&run, "f-lp1", "b-2");
    assert_volume(run.shape("b-2"), 1.0);
}

#[test]
fn t008_zero_spacing() {
    let run = Run::records(&BODIES, &[CUBE10, &pattern(&direction(X, "3", "0"), "")]);

    let error = run.assert_error("f-lp1", "out_of_range");
    assert_error_field(error, "direction.spacing");
}

#[test]
fn t009_new_body() {
    let run = Run::example("linear_pattern/new-body.ptsm");

    run.assert_all_ok();
    assert_unchanged(&run, "f-lp1", "b-1");
    let copies = run.shape("b-2");
    assert_volume(copies, 2000.0);
    assert_face_count(copies, 12);
    assert_bounding_box(copies, [15.0, -5.0, 0.0], [45.0, 5.0, 10.0]);
    assert_eq!(body_changes(&run, "f-lp1"), ["b-2 New"]);
}

#[test]
fn t010_direction_from_a_sketch_line() {
    let run = Run::example("linear_pattern/sketch-line-direction.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [35.0, 45.0, 10.0]);
    assert_eq!(link_ids(&run.dependencies("f-lp1").features), ["f-sk1"]);
}

#[test]
fn t011_direction_from_an_edge() {
    let run = Run::example("linear_pattern/edge-direction.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-5.0, -5.0, 0.0], [25.0, 5.0, 10.0]);
}

#[test]
fn t012_too_many_instances() {
    let run = Run::records(
        &BODIES,
        &[
            CUBE10,
            &pattern(
                &direction(X, "200", "20"),
                &format!(r#""second_direction": {}"#, direction(Y, "100", "20")),
            ),
        ],
    );

    run.assert_error("f-lp1", "too_many_instances");
    assert_eq!(
        run.messages("f-lp1"),
        ["This linear pattern would make 20000 copies, more than the limit of 10000. Lower Count."]
    );
    assert_unchanged(&run, "f-lp1", "b-1");
}

#[test]
fn t013_canceled_build_returns_canceled() {
    let run = Run::example("linear_pattern/grid.ptsm");

    run.assert_build_canceled("f-lp1");
}

#[test]
fn t015_fractional_count_below_1() {
    let run = Run::records(&BODIES, &[CUBE10, &pattern(&direction(X, "0.5", "20"), "")]);

    let error = run.assert_error("f-lp1", "out_of_range");
    assert_error_field(error, "direction.count");
    assert_eq!(
        run.messages("f-lp1"),
        ["Count must be at least 1, but it is 0.5. Use 1 or more."]
    );
    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
    assert_unchanged(&run, "f-lp1", "b-1");
}

// Tests 16 to 19: copies carry their transform as the motion of their faces, and an edge direction keeps the side the
// user picked (features.md sections 3.6 and 3.9).

#[test]
fn t016_a_sketch_on_a_copys_face_follows_the_spacing() {
    // The document of features/sketch.md test 27, at its spacing of 60 and at 80.
    let example = Run::example("sketch/on-pattern-copy.ptsm");
    let mut wider = example.document.clone();
    feature_record_mut(&mut wider, "f-lp1").params["direction"]["spacing"] =
        serde_json::json!("80");

    for (run, offset) in [(example, 60.0), (Run::new(wider), 80.0)] {
        run.assert_all_ok();
        assert_face_translated(
            &run,
            "f-lp1",
            "b-1",
            "f-box1/top@f-lp1:1",
            [offset, 0.0, 0.0],
        );
        assert_bore_through(&run, [offset + 5.0, 5.0, 0.0]);
    }
}

/// The example of test 17 (corpus R38) with its left wall's top corner at (`lean_x`, 20), the shared point of `e-3`'s
/// end and `e-4`'s start, and the axis's stored `direction` kept or left out.
fn leaning_wall_document(lean_x: f64, direction: bool) -> Document {
    let example = load_example("linear_pattern/leaning-edge.ptsm");
    let mut document = with_shared_point_moved(&example, "f-sk1", ["e-3", "e-4"], [lean_x, 20.0]);
    if !direction {
        remove_axis_direction(&mut document, "f-lp1", &["direction", "axis"]);
    }
    document
}

#[test]
fn t017_the_pattern_of_the_upright_wall_goes_along_the_edge() {
    // Before the edit.
    let run = Run::new(leaning_wall_document(0.0, false));

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, 0.0], [40.0, 50.0, 10.0]);
}

#[test]
fn t017_an_edge_direction_that_leans_keeps_its_side() {
    // The example leans to (-1, 20) and stores the direction.
    let leaning_left = [
        Run::new(leaning_wall_document(-1.0, false)),
        Run::example("linear_pattern/leaning-edge.ptsm"),
    ];
    for run in &leaning_left {
        run.assert_all_ok();
        // The copy is offset by (-1.498129, 29.962570, 0), along (-0.049938, 0.998752, 0).
        assert_bounding_box(
            run.shape("b-1"),
            [-2.498129, 0.0, 0.0],
            [40.0, 49.962570, 10.0],
        );
    }

    for direction in [false, true] {
        let run = Run::new(leaning_wall_document(1.0, direction));
        run.assert_all_ok();
        // Offset (1.498129, 29.962570, 0).
        assert_bounding_box(
            run.shape("b-1"),
            [0.0, 0.0, 0.0],
            [41.498129, 49.962570, 10.0],
        );
    }
}

#[test]
fn t018_a_half_turn_of_the_part_turns_a_stored_direction() {
    // The front top edge of `CUBE10` as picked before the half turn about Z was inserted, with its direction.
    let run = Run::example("linear_pattern/turned-edge-direction.ptsm");

    run.assert_all_ok();
    // The stored direction turns with the part to -X.
    assert_bounding_box(run.shape("b-1"), [-25.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
}

#[test]
fn t018_without_a_stored_direction_the_fingerprint_keeps_its_side() {
    // As written before format version 2: the fingerprint's +X.
    let mut unstored = load_example("linear_pattern/turned-edge-direction.ptsm");
    remove_axis_direction(&mut unstored, "f-lp1", &["direction", "axis"]);

    let run = Run::new(unstored);

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [-5.0, -5.0, 0.0], [25.0, 5.0, 10.0]);
}

#[test]
fn t019_direction_edges_not_on_one_line() {
    let [box_1, box_2] = l_shaped_body("b-1");
    let run = Run::records(
        &BODIES,
        &[
            &box_1,
            &box_2,
            &pattern(
                &direction(&edge_axis("b-1", BOTH_FRONT_TOP_EDGES, ""), "2", "50"),
                "",
            ),
        ],
    );

    assert_choice_between(
        &run,
        "f-lp1",
        "not_collinear",
        ("direction.axis", "edge"),
        "b-1",
        &FRONT_TOP_EDGES_OF_BOTH_BOXES,
        "The edge selected for Direction now matches 2 edges that do not lie on one line. Pick the one you mean.",
    );
    assert_unchanged(&run, "f-lp1", "b-1");
}

#[test]
fn a_spacing_beyond_the_modeling_limit_is_out_of_range() {
    let run = Run::records(
        &BODIES,
        &[CUBE10, &pattern(&direction(X, "3", "1e200"), "")],
    );

    let error = run.assert_error("f-lp1", "out_of_range");
    assert_error_field(error, "direction.spacing");
    assert_eq!(
        run.messages("f-lp1"),
        ["Spacing must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."]
    );
    assert_unchanged(&run, "f-lp1", "b-1");
}

#[test]
fn t020_direction_from_a_round_face_matches_the_origin_axis() {
    let run = Run::example("linear_pattern/face-direction.ptsm");
    let reference = Run::example("linear_pattern/three-apart.ptsm");

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 3000.0);
    assert_face_count(shape, 18);
    assert_solid_count(shape, 3);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [45.0, 5.0, 10.0]);
    assert_face_position(&face(run.body("b-1"), "f-box1/right@f-lp1:2"), 0, 45.0);
    assert_same_body(run.body("b-1"), reference.body("b-1"));
}

#[test]
fn t020_direction_from_a_round_face_reads_its_body() {
    let run = Run::example("linear_pattern/face-direction.ptsm");

    assert_unchanged(&run, "f-lp1", "b-2");
    assert_eq!(link_ids(&run.dependencies("f-lp1").bodies_read), ["b-2"]);
}

#[test]
fn t020_the_round_face_direction_is_stored_as_a_pick_stores_it() {
    let run = Run::example("linear_pattern/face-direction.ptsm");

    assert_axis_stored_as_picked(&run, "f-lp1", "/direction/axis", "b-2", "f-cyl1/side");
}

#[test]
fn t021_second_direction_reversed_volume_faces_and_bounds() {
    let run = Run::example("linear_pattern/second-reverse.ptsm");

    run.assert_all_quiet();
    assert_grid_shape(&run, -15.0);
}

#[test]
fn t021_second_direction_reversed_puts_the_second_row_toward_minus_y() {
    let run = Run::example("linear_pattern/second-reverse.ptsm");

    assert_grid_tops(&run, -15.0);
}

/// Asserts test 3's grid in `run`'s b-1, its second row of 10 mm cubes centered at y =
/// `second_row_y` (15 toward +Y, -15 toward -Y): volume, faces, solids and bounds.
fn assert_grid_shape(run: &Run, second_row_y: f64) {
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 36);
    assert_solid_count(shape, 6);
    // The cubes reach 5 mm past their rows, the first centered at y = 0.
    let low_y = second_row_y.min(0.0) - 5.0;
    let high_y = second_row_y.max(0.0) + 5.0;
    assert_bounding_box(shape, [-5.0, low_y, 0.0], [45.0, high_y, 10.0]);
}

/// Asserts the planar tops of three cubes of test 3's grid in `run`'s b-1, its second row centered
/// at y = `second_row_y`.
fn assert_grid_tops(run: &Run, second_row_y: f64) {
    let body = run.body("b-1");
    for (name, center) in [
        ("f-box1/top@f-lp1:1", [20.0, 0.0, 10.0]),
        ("f-box1/top@f-lp1:0.1", [0.0, second_row_y, 10.0]),
        ("f-box1/top@f-lp1:2.1", [40.0, second_row_y, 10.0]),
    ] {
        let top = face(body, name);
        assert_face_kind(&top, GeometryKind::Plane);
        assert_face_centroid(&top, center);
    }
}

/// Asserts the grid of tests 22 to 24 in `run`'s b-1: test 3's grid, its second row toward +Y.
fn assert_grid_with_second_row_toward_plus_y(run: &Run) {
    assert_grid_shape(run, 15.0);
    assert_grid_tops(run, 15.0);
}

#[test]
fn t022_second_direction_from_a_sketch_line_runs_from_its_start_to_its_end() {
    let run = Run::example("linear_pattern/second-sketch-line-direction.ptsm");

    run.assert_all_quiet();
    assert_grid_with_second_row_toward_plus_y(&run);
}

#[test]
fn t023_second_direction_from_an_edge_follows_its_stored_direction() {
    let run = Run::example("linear_pattern/second-edge-direction.ptsm");

    run.assert_all_quiet();
    assert_grid_with_second_row_toward_plus_y(&run);
}

#[test]
fn t023_the_second_edge_direction_is_stored_as_a_pick_stores_it() {
    let run = Run::example("linear_pattern/second-edge-direction.ptsm");

    assert_axis_stored_as_picked(
        &run,
        "f-lp1",
        "/second_direction/axis",
        "b-1",
        "edge(f-box1/left|f-box1/top)",
    );
}

#[test]
fn t024_second_direction_from_a_round_face_follows_its_stored_direction() {
    let run = Run::example("linear_pattern/second-face-direction.ptsm");

    run.assert_all_quiet();
    assert_grid_with_second_row_toward_plus_y(&run);
}

#[test]
fn t024_the_rod_of_the_second_direction_is_unchanged() {
    let run = Run::example("linear_pattern/second-face-direction.ptsm");

    assert_unchanged(&run, "f-lp1", "b-2");
    let rod = run.shape("b-2");
    assert_volume(rod, 125.663706);
    assert_face_count(rod, 3);
    assert_bounding_box(rod, [18.0, -10.0, 18.0], [22.0, 0.0, 22.0]);
}

#[test]
fn t024_the_second_round_face_direction_is_stored_as_a_pick_stores_it() {
    let run = Run::example("linear_pattern/second-face-direction.ptsm");

    assert_axis_stored_as_picked(
        &run,
        "f-lp1",
        "/second_direction/axis",
        "b-2",
        "f-cyl1/side",
    );
}
