//! Acceptance tests of the `chamfer` feature (docs/spec/features/chamfer.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/chamfer/`; the documents of tests 15 and 16 (corpus R22) are written here until their
//! examples exist (features.md open issue 22).

use crate::feature_support;

use std::f64::consts::FRAC_1_SQRT_2;

use feature_support::{
    BOX_EDGE_NAMES, BOX_VERTEX_NAMES, CUBE20, FRONT_TOP_EDGE, LENGTH_TOLERANCE, MOVE_BY_100, PLATE,
    PLATE_TOP_EDGES, RISER_EDGE, RISER_SENTENCE, Run, STEP, STEP_BOTTOM_LEFT_EDGE, assert_absolute,
    assert_bounding_box, assert_coordinates, assert_edge_queries, assert_element_repair,
    assert_error_field, assert_examples_canonical, assert_face_area, assert_face_centroid,
    assert_face_count, assert_face_kind, assert_face_names, assert_face_normal,
    assert_limiting_face, assert_list_field, assert_messages_pass_lint, assert_moved_by_100,
    assert_placement, assert_relative, assert_relative_within, assert_skipped_as_smooth,
    assert_unchanged, assert_volume, body_update, document, edge, face, face_index, face_names,
    face_sketch, message, one_point, point_components, prefixed, remove_entry, set_field,
    sketch_result, suggested_limit, with_edges_reversed,
};
use partsmith_features::{BodyChange, FeatureError, Fix};
use partsmith_model::{Document, Units};
use partsmith_occt_sys::GeometryKind;

/// `CHA(edges, distance)`: the chamfer `f-cha1` of `b-1`, with `distance` a JSON object.
fn chamfer_record(edges: &[&str], distance: &str) -> String {
    format!(
        r#"{{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{}], "distance": {distance}}}"#,
        edges.join(", "),
    )
}

/// An equal-distance chamfer distance.
fn equal_distance(expr: &str) -> String {
    format!(r#"{{"kind": "equal", "expr": "{expr}"}}"#)
}

/// "Use {value}": `SetField` of the equal distance to the expression `value`.
fn use_distance(value: &str) -> Fix {
    set_field("distance.expr", "Distance", serde_json::json!(value))
}

/// Test 6: the plate with its four top edges chamfered at the equal distance `distance`.
fn plate_document(distance: &str) -> Document {
    document(
        &["b-1"],
        &[
            PLATE,
            &chamfer_record(&PLATE_TOP_EDGES, &equal_distance(distance)),
        ],
    )
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("chamfer");
}

#[test]
fn t001_equal_distance() {
    let run = Run::example("chamfer/equal-distance.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 7960.0);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    let chamfer = face(body, "f-cha1/chamfer/edge(f-box1/front|f-box1/top)");
    assert_face_kind(&chamfer, GeometryKind::Plane);
    assert_face_area(&chamfer, 56.568542);
    assert_face_area(&face(body, "f-box1/top"), 360.0);
    assert_face_area(&face(body, "f-box1/front"), 360.0);
    let change = &body_update(&run, "f-cha1", "b-1").change;
    assert!(
        matches!(change, BodyChange::Modify),
        "a chamfer modifies its body, got {change:?}"
    );
}

#[test]
fn t002_two_distances_first_on_the_first_face_of_the_stored_reference() {
    let run = Run::example("chamfer/two-distances.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 7970.0);
    assert_face_count(shape, 7);
    assert_face_area(&face(body, "f-box1/front"), 380.0);
    assert_face_area(&face(body, "f-box1/top"), 340.0);
    assert_face_area(
        &face(body, "f-cha1/chamfer/edge(f-box1/front|f-box1/top)"),
        63.245553,
    );
}

#[test]
fn t003_two_distances_flipped() {
    let run = Run::example("chamfer/two-distances-flipped.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    assert_volume(run.shape("b-1"), 7970.0);
    assert_face_area(&face(body, "f-box1/front"), 340.0);
    assert_face_area(&face(body, "f-box1/top"), 380.0);
}

#[test]
fn t004_top_loop() {
    let run = Run::example("chamfer/top-loop.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 3850.666667);
    assert_face_count(shape, 10);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
    assert_face_area(&face(run.body("b-1"), "f-box1/top"), 256.0);
}

#[test]
fn t005_concave_edge_adds_material() {
    let run = Run::example("chamfer/concave-edge.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 12040.0);
    assert_face_count(shape, 9);
}

#[test]
fn t006_too_large() {
    let run = Run::new(plate_document("3"));

    run.assert_ok("f-box1");
    let error = run.assert_error("f-cha1", "chamfer_too_large");
    assert_error_field(error, "distance.expr");
    let FeatureError::ChamferTooLarge { distance, max, .. } = error else {
        panic!("expected ChamferTooLarge, got {error:?}");
    };
    assert_eq!(*distance, 3.0, "the requested distance");
    let max = max.unwrap_or_else(|| panic!("the search finds a working distance: {error:?}"));
    assert!(
        (2.0..2.4).contains(&max),
        "the largest working distance {max} lies in [2, 2.4)"
    );
    // The four side faces tie at 2.4 mm, and `edge(f-box1/back|f-box1/top)` sorts first; the other three edges fail
    // without it, so its removal is not offered.
    // `position` is the entry of the edge's reference in Edges (features.md section 2.8, `Limit::Face`): E-BT is
    // entry 1 of [E-FT, E-BT, E-LT, E-RT].
    assert_limiting_face(
        error,
        1,
        "edge(f-box1/back|f-box1/top)",
        "f-box1/back",
        false,
    );
    let limit = suggested_limit(max);
    let expected = format!(
        "Chamfer distance 3 mm is too large for the selected edges: the largest that works is about {limit} mm. The back top edge of 'Box 1' lies next to the back side of 'Box 1', which is only 2.4 mm across."
    );
    assert_eq!(run.messages("f-cha1"), [expected.as_str()]);
    assert_eq!(
        message(&run, "f-cha1", "chamfer_too_large").fixes,
        [use_distance(&format!("{limit} mm"))]
    );
    assert_eq!(error.suggestion(Units::Mm), Some(format!("{limit} mm")));
    assert_unchanged(&run, "f-cha1", "b-1");
    assert_volume(run.shape("b-1"), 960.0);

    let rebuilt = Run::new(plate_document(&limit));
    rebuilt.assert_all_ok();
}

#[test]
fn t007_zero_distance() {
    let run = Run::records(
        &["b-1"],
        &[
            CUBE20,
            &chamfer_record(&[FRONT_TOP_EDGE], &equal_distance("0")),
        ],
    );

    let error = run.assert_error("f-cha1", "out_of_range");
    assert_error_field(error, "distance.expr");
    assert_eq!(
        run.messages("f-cha1"),
        ["Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
    );
}

#[test]
fn t008_negative_second_distance() {
    let distance = r#"{"kind": "two_distances", "expr": "1", "second_expr": "-1"}"#;
    let run = Run::records(
        &["b-1"],
        &[CUBE20, &chamfer_record(&[FRONT_TOP_EDGE], distance)],
    );

    let error = run.assert_error("f-cha1", "out_of_range");
    assert_error_field(error, "distance.second_expr");
    assert_eq!(
        run.messages("f-cha1"),
        [
            "Second distance must be greater than 0 mm, but it is -1 mm. Use a value greater than 0 mm."
        ]
    );
}

#[test]
fn t009_no_edges() {
    let run = Run::records(&["b-1"], &[&chamfer_record(&[], &equal_distance("1"))]);

    run.assert_error("f-cha1", "empty_selection");
    assert_eq!(
        run.messages("f-cha1"),
        ["Select at least one edge for Edges."]
    );
}

#[test]
fn t010_edge_lost() {
    let run = Run::example("chamfer/edge-lost.ptsm");

    run.assert_ok("f-box1");
    run.assert_warnings("f-cha1", &["reference_missing"]);
    let warning = &run.feature("f-cha1").warnings[0];
    assert_list_field(warning.field(), "edges", 1);
    assert_element_repair(warning.repair(), "/edges/1", "b-1");
    assert_eq!(
        message(&run, "f-cha1", "reference_missing")
            .pointer
            .as_deref(),
        Some("/edges/1")
    );
    assert_messages_pass_lint(&run, "f-cha1");
    assert_volume(run.shape("b-1"), 7960.0);
}

/// Step 5 for two distances: the plate of test 6 with 3 mm on each wall (whose name sorts before
/// `f-box1/top`) and 4 mm on the top. The search scales both distances and reports the larger
/// one; a working wall distance lies in [2, 2.4) as in test 6, so the larger scaled distance, 4/3
/// of it, lies in [8/3, 3.2).
#[test]
fn two_distances_too_large() {
    let distance = r#"{"kind": "two_distances", "expr": "3", "second_expr": "4"}"#;
    let run = Run::records(
        &["b-1"],
        &[PLATE, &chamfer_record(&PLATE_TOP_EDGES, distance)],
    );

    run.assert_ok("f-box1");
    let error = run.assert_error("f-cha1", "chamfer_too_large");
    assert_error_field(error, "distance");
    let FeatureError::ChamferTooLarge { distance, max, .. } = error else {
        panic!("expected ChamferTooLarge, got {error:?}");
    };
    assert_eq!(*distance, 4.0, "the larger requested distance");
    let max = max.unwrap_or_else(|| panic!("the search finds working distances: {error:?}"));
    assert!(
        (8.0 / 3.0..3.2).contains(&max),
        "the larger working distance {max} lies in [8/3, 3.2)"
    );
    let bisection_steps = max * 1024.0 / 4.0;
    assert_eq!(
        bisection_steps,
        bisection_steps.round(),
        "the larger working distance {max} is a bisection point 4k/1024"
    );
    let limit = suggested_limit(max);
    let expected = format!(
        "Chamfer distance 4 mm is too large for the selected edges: the largest that works is about {limit} mm. The back top edge of 'Box 1' lies next to the back side of 'Box 1', which is only 2.4 mm across."
    );
    assert_eq!(run.messages("f-cha1"), [expected.as_str()]);
    assert_eq!(
        error.suggestion(Units::Mm),
        None,
        "a two-distance limit scales both distances, so it has no one-step fix"
    );
    // No SetField for two distances, and the other edges fail without the limiting one.
    assert_eq!(message(&run, "f-cha1", "chamfer_too_large").fixes, []);
    assert_unchanged(&run, "f-cha1", "b-1");
}

/// Step 3: an edge whose faces meet without a corner is skipped as in fillet.md test 12, here the
/// edge where the front face meets the fillet of a vertical box edge.
#[test]
fn smooth_edge_skipped() {
    let run = Run::example("chamfer/smooth-edge.ptsm");

    run.assert_ok("f-box1");
    run.assert_ok("f-fil1");
    // One edge, one message (features.md section 3.15).
    run.assert_warnings("f-cha1", &["ignored"]);
    assert_skipped_as_smooth(&run.feature("f-cha1").warnings[0]);
    assert_eq!(
        run.messages("f-cha1"),
        [
            "One of the edges selected for Edges is smooth (its faces meet without a corner), so it was skipped."
        ]
    );
    assert_eq!(
        message(&run, "f-cha1", "ignored").fixes,
        [remove_entry("/edges/0")]
    );
    assert_unchanged(&run, "f-cha1", "b-1");
}

#[test]
fn t012_edge_order_does_not_matter() {
    let listed = Run::example("chamfer/edge-order.ptsm");
    let reversed = Run::new(with_edges_reversed(&listed.document, "f-cha1"));

    // The front top edge and the top arc of the front left fillet, then the other order.
    let edges = [
        "edge(f-box1/front|f-box1/top)",
        "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
    ];
    assert_edge_queries(&listed, "f-cha1", &edges);
    assert_edge_queries(&reversed, "f-cha1", &[edges[1], edges[0]]);
    let runs = [listed, reversed];
    for run in &runs {
        run.assert_all_ok();
    }
    let [listed, reversed] = runs.each_ref().map(|run| {
        run.shape("b-1")
            .volume()
            .expect("the chamfered body has a volume")
    });
    assert_relative_within(
        "volume with the edges in reversed order against listed order",
        reversed,
        listed,
        1e-9,
    );
    assert_eq!(
        face_names(runs[0].body("b-1")),
        face_names(runs[1].body("b-1")),
        "both orders name the same faces"
    );
}

#[test]
fn t013_chamfer_beside_a_fillet_on_a_pipe_tee() {
    let run = Run::example("chamfer/pipe-tee.ptsm");

    run.assert_ok("f-cyl1");
    run.assert_ok("f-cyl2");
    run.assert_ok("f-fil1");
    // One edge, so no `no_effect` (features.md section 3.15).
    run.assert_warnings("f-cha1", &["ignored"]);
    assert_skipped_as_smooth(&run.feature("f-cha1").warnings[0]);
    assert_unchanged(&run, "f-cha1", "b-1");
}

#[test]
fn t014_distance_below_the_tolerance() {
    let distance = r#"{"kind": "two_distances", "expr": "1", "second_expr": "0.0000001"}"#;
    let run = Run::records(
        &["b-1"],
        &[CUBE20, &chamfer_record(&[FRONT_TOP_EDGE], distance)],
    );

    let error = run.assert_error("f-cha1", "out_of_range");
    assert_error_field(error, "distance.second_expr");
    assert_eq!(
        run.messages("f-cha1"),
        [
            "Second distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ]
    );
    assert_unchanged(&run, "f-cha1", "b-1");
}

// Tests 15 and 16 (corpus R22): the side of a two-distance chamfer follows the face the edge was picked from, whatever
// names a join inserted before it gives the faces (section 3.3 step 4), with the join's id sorting before and after
// `f-box1`.

/// The chamfer of tests 15 and 16: 1 mm on the front and 3 mm on the top of `CUBE20`'s front top edge, as picked
/// before the join was inserted.
fn picked_side_chamfer() -> String {
    chamfer_record(
        &[
            r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"}"#,
        ],
        r#"{"kind": "two_distances", "expr": "1", "second_expr": "3"}"#,
    )
}

/// `CUBE20`, the join `join` inserted before the chamfer, then the chamfer of tests 15 and 16.
fn joined_then_chamfered(join: &str) -> Run {
    Run::records(&["b-1"], &[CUBE20, join, &picked_side_chamfer()])
}

/// The box of test 15 with id `join_id`, joined beside `CUBE20` (x 10..30, y -20..20): the tops merge, the fronts do
/// not.
fn base_beside(join_id: &str) -> String {
    format!(
        r#"{{"id": "{join_id}", "type": "box", "body": "b-1", "size": ["20", "40", "20"], "position": ["20", "0", "0"], "op": "join"}}"#
    )
}

/// The box of test 16 with id `join_id`, joined behind `CUBE20` (x -10..10, y 10..30): the tops merge.
fn box_behind(join_id: &str) -> String {
    format!(
        r#"{{"id": "{join_id}", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "position": ["0", "20", "0"], "op": "join"}}"#
    )
}

/// The name of the edge between two faces (naming.md section 3.6: adjacent face names, sorted).
fn edge_between(first: &str, second: &str) -> String {
    let (low, high) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    format!("edge({low}|{high})")
}

/// Asserts test 15's sides: the chamfer face meets `f-box1/front` along z = 19 (1 mm down the front) and the merged
/// top `top` along y = -7 (3 mm into the top), where the front top edge now has the name `edge_name`.
#[track_caller]
fn assert_picked_sides(run: &Run, top: &str, edge_name: &str) {
    let body = run.body("b-1");
    let chamfer = format!("f-cha1/chamfer/{edge_name}");
    let on_front = edge(body, &edge_between("f-box1/front", &chamfer));
    assert_absolute(
        "z of the chamfer's edge on the front",
        on_front.centroid.z,
        19.0,
        LENGTH_TOLERANCE,
    );
    assert_absolute(
        "y of the chamfer's edge on the front",
        on_front.centroid.y,
        -10.0,
        LENGTH_TOLERANCE,
    );
    let on_top = edge(body, &edge_between(top, &chamfer));
    assert_absolute(
        "y of the chamfer's edge on the top",
        on_top.centroid.y,
        -7.0,
        LENGTH_TOLERANCE,
    );
    assert_absolute(
        "z of the chamfer's edge on the top",
        on_top.centroid.z,
        20.0,
        LENGTH_TOLERANCE,
    );
}

#[test]
fn t015_the_sides_survive_a_join_that_renames_the_top() {
    // `f-base1` sorts before `f-box1`, so the merged top is `f-base1/top` and the edge's name now sorts the top first.
    let run = joined_then_chamfered(&base_beside("f-base1"));

    run.assert_ok("f-cha1");
    assert_picked_sides(&run, "f-base1/top", "edge(f-base1/top|f-box1/front)");
}

#[test]
fn t015_the_sides_do_not_depend_on_the_joins_id() {
    let before = joined_then_chamfered(&base_beside("f-base1"));
    // `f-side1` sorts after `f-box1`, so the merged top keeps the name `f-box1/top`.
    let after = joined_then_chamfered(&base_beside("f-side1"));

    after.assert_ok("f-cha1");
    assert_picked_sides(&after, "f-box1/top", "edge(f-box1/front|f-box1/top)");
    let renamed = face(
        before.body("b-1"),
        "f-cha1/chamfer/edge(f-base1/top|f-box1/front)",
    );
    let kept = face(
        after.body("b-1"),
        "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
    );
    assert_relative("chamfer face area", kept.measure, renamed.measure);
    assert_coordinates(
        "chamfer face centroid",
        point_components(kept.centroid),
        point_components(renamed.centroid),
        LENGTH_TOLERANCE,
    );
    assert_relative(
        "volume",
        after
            .shape("b-1")
            .volume()
            .expect("the kernel measures the volume"),
        before
            .shape("b-1")
            .volume()
            .expect("the kernel measures the volume"),
    );
}

#[test]
fn t016_the_sides_survive_a_join_behind() {
    // `f-a1` sorts before `f-box1`, so the merged top is `f-a1/top`.
    let run = joined_then_chamfered(&box_behind("f-a1"));

    run.assert_ok("f-cha1");
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-box1/front"), 380.0);
    assert_face_area(&face(body, "f-a1/top"), 740.0);
    assert_volume(run.shape("b-1"), 15970.0);
}

#[test]
fn t016_the_side_never_depends_on_the_inserted_features_id() {
    // `f-rear1` sorts after `f-box1`, so the merged top keeps the name `f-box1/top`.
    let run = joined_then_chamfered(&box_behind("f-rear1"));

    run.assert_ok("f-cha1");
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-box1/front"), 380.0);
    assert_face_area(&face(body, "f-box1/top"), 740.0);
    assert_volume(run.shape("b-1"), 15970.0);
}

/// `CUBE20` moved by `MOVE_BY_100`, then its front top edge chamfered at 2 mm, and `extra` records after it.
fn chamfered_after_a_move(extra: &[&str]) -> Run {
    let chamfer = chamfer_record(&[FRONT_TOP_EDGE], &equal_distance("2"));
    let mut records = vec![CUBE20, MOVE_BY_100, &chamfer];
    records.extend_from_slice(extra);
    Run::records(&["b-1"], &records)
}

#[test]
fn a_beveled_face_moves_with_the_part() {
    let run = chamfered_after_a_move(&[]);

    run.assert_all_ok();
    assert_moved_by_100(
        &run,
        "f-cha1",
        "b-1",
        "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
    );
}

#[test]
fn a_sketch_on_the_beveled_face_stays_on_it_after_the_move() {
    // The bevel runs from (y, z) = (-10, 18) to (-8, 20) in body coordinates; the fallback at its middle in body
    // coordinates is moved by the beveled face's motion (features.md section 3.6).
    let bevel = r#"{"q": "f-cha1/chamfer/edge(f-box1/front|f-box1/top)", "fp": "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542"}"#;
    let sketch = face_sketch(
        bevel,
        [0.0, -9.0, 19.0],
        Some("body"),
        &one_point([0.0, 0.0]),
    );

    let run = chamfered_after_a_move(&[&sketch]);

    run.assert_all_ok();
    assert_placement(
        &sketch_result(&run, "f-sk2").placement,
        [100.0, -9.0, 19.0],
        [1.0, 0.0, 0.0],
        [0.0, -FRAC_1_SQRT_2, FRAC_1_SQRT_2],
    );
}

/// `STEP` with `CHA(edges, {"kind": "equal", "expr": distance})`.
fn step_run(edges: &[&str], distance: &str) -> Run {
    let chamfer = chamfer_record(edges, &equal_distance(distance));
    Run::records(&["b-1"], &[STEP[0], STEP[1], &chamfer])
}

/// Asserts the error of test 17, also the error of test 18: `chamfer_too_large` with a largest working distance in
/// [2, 2.4) and the riser as the limit, rendered with its limit `{s}`, which this returns.
#[track_caller]
fn assert_riser_limits(run: &Run, removable: bool) -> String {
    let error = run.assert_error("f-cha1", "chamfer_too_large");
    let FeatureError::ChamferTooLarge { max, .. } = error else {
        panic!("expected ChamferTooLarge, got {error:?}");
    };
    let max = max.unwrap_or_else(|| panic!("the search finds a working distance: {error:?}"));
    assert!(
        (2.0..2.4).contains(&max),
        "the largest working distance {max} lies in [2, 2.4)"
    );
    assert_limiting_face(
        error,
        0,
        "edge(f-box1/top|f-box2/left)",
        "f-box2/left",
        removable,
    );
    let limit = suggested_limit(max);
    assert_eq!(
        run.messages("f-cha1"),
        [format!(
            "Chamfer distance 3 mm is too large for the selected edges: the largest that works is about {limit} mm. {RISER_SENTENCE}"
        )
        .as_str()]
    );
    assert_messages_pass_lint(run, "f-cha1");
    limit
}

#[test]
fn t017_the_limiting_face_is_named() {
    let run = step_run(&[RISER_EDGE], "3");

    let limit = assert_riser_limits(&run, false);

    assert_eq!(
        message(&run, "f-cha1", "chamfer_too_large").fixes,
        [use_distance(&format!("{limit} mm"))]
    );
    assert_unchanged(&run, "f-cha1", "b-1");
    step_run(&[RISER_EDGE], &limit).assert_all_ok();
}

#[test]
fn t018_the_edge_that_limits_it_can_be_removed_when_the_others_work() {
    let run = step_run(&[RISER_EDGE, STEP_BOTTOM_LEFT_EDGE], "3");

    let limit = assert_riser_limits(&run, true);

    assert_eq!(
        message(&run, "f-cha1", "chamfer_too_large").fixes,
        [
            use_distance(&format!("{limit} mm")),
            remove_entry("/edges/0")
        ]
    );
}

#[test]
fn t018_removing_the_limiting_edge_chamfers_the_other_at_the_requested_distance() {
    let run = step_run(&[STEP_BOTTOM_LEFT_EDGE], "3");

    run.assert_all_ok();
    // The stepped body's 7040 less 3 x 3 / 2 x 20.
    assert_volume(run.shape("b-1"), 6950.0);
}

/// The center of `CUBE20`, which spans [-10,-10,0]-[10,10,20].
const CUBE20_CENTER: [f64; 3] = [0.0, 0.0, 10.0];

/// The outward normal of the face `f-box1/<side>` of `CUBE20`.
fn cube_face_normal(face: &str) -> [f64; 3] {
    match face {
        "f-box1/front" => [0.0, -1.0, 0.0],
        "f-box1/back" => [0.0, 1.0, 0.0],
        "f-box1/left" => [-1.0, 0.0, 0.0],
        "f-box1/right" => [1.0, 0.0, 0.0],
        "f-box1/bottom" => [0.0, 0.0, -1.0],
        "f-box1/top" => [0.0, 0.0, 1.0],
        other => panic!("{other} is not a face of CUBE20"),
    }
}

/// The sum of the outward normals of the faces that an edge or vertex name lists, such as
/// `edge(f-box1/front|f-box1/top)`.
fn summed_normals(element: &str) -> [f64; 3] {
    let faces = element
        .split_once('(')
        .and_then(|(_, faces)| faces.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{element} is not an edge or vertex name"));
    faces
        .split('|')
        .map(cube_face_normal)
        .fold([0.0; 3], |sum, normal| {
            [sum[0] + normal[0], sum[1] + normal[1], sum[2] + normal[2]]
        })
}

/// The point `distance` from the center of `CUBE20` along each of the outward normals that sum to
/// `normals`: an edge's midpoint or a corner at 10 mm.
fn from_cube_center(normals: [f64; 3], distance: f64) -> [f64; 3] {
    [0, 1, 2].map(|axis| CUBE20_CENTER[axis] + distance * normals[axis])
}

#[test]
fn t019_every_edge_of_a_cube() {
    let run = Run::example("chamfer/every-edge.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    // 8000 less a prism of section 2 along each edge without its last 2 mm at either end (384 in
    // all) and 20/3 at each corner.
    assert_volume(shape, 7562.666667);
    assert_face_count(shape, 26);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    let top = face(body, "f-box1/top");
    assert_face_kind(&top, GeometryKind::Plane);
    assert_face_area(&top, 256.0);
    // The six faces of the box, one chamfer face per edge and one corner face per corner of the
    // cube, and no other faces.
    let names: Vec<String> = ["back", "bottom", "front", "left", "right", "top"]
        .map(|side| format!("f-box1/{side}"))
        .into_iter()
        .chain(prefixed("f-cha1/chamfer/", &BOX_EDGE_NAMES))
        .chain(prefixed("f-cha1/corner/", &BOX_VERTEX_NAMES))
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_face_names(body, &names);
}

#[test]
fn t019_each_chamfer_face_lies_along_its_edge() {
    let run = Run::example("chamfer/every-edge.ptsm");

    let body = run.body("b-1");
    for edge in BOX_EDGE_NAMES {
        let chamfer = face(body, &format!("f-cha1/chamfer/{edge}"));
        assert_face_kind(&chamfer, GeometryKind::Plane);
        // A 16 mm strip, 2 sqrt(2) mm wide.
        assert_face_area(&chamfer, 32.0 * 2.0_f64.sqrt());
        let normals = summed_normals(edge);
        assert_face_normal(&chamfer, normals.map(|component| component * FRAC_1_SQRT_2));
        // The edge's midpoint, 10 mm from the center along each normal, moved 1 mm against each.
        assert_face_centroid(&chamfer, from_cube_center(normals, 10.0 - 1.0));
    }
    let front_top = face(body, "f-cha1/chamfer/edge(f-box1/front|f-box1/top)");
    assert_face_normal(&front_top, [0.0, -FRAC_1_SQRT_2, FRAC_1_SQRT_2]);
    assert_face_centroid(&front_top, [0.0, -9.0, 19.0]);
}

#[test]
fn t019_each_corner_face_lies_at_its_corner() {
    let run = Run::example("chamfer/every-edge.ptsm");

    let body = run.body("b-1");
    let topology = run.shape("b-1").topology().expect("topology of the body");
    for vertex in BOX_VERTEX_NAMES {
        let name = format!("f-cha1/corner/{vertex}");
        let corner = face(body, &name);
        assert_face_kind(&corner, GeometryKind::Plane);
        let index = face_index(body, &name).unwrap_or_else(|| panic!("{name} is a face"));
        let bounds = &topology.faces[index as usize - 1];
        assert_eq!(bounds.edges.len(), 3, "edges of {name}");
        assert_eq!(bounds.vertices.len(), 3, "vertices of {name}");
        assert_face_area(&corner, 2.0 * 3.0_f64.sqrt());
        // The corner, 10 mm from the center along each normal, moved 4/3 mm against each.
        assert_face_centroid(
            &corner,
            from_cube_center(summed_normals(vertex), 10.0 - 4.0 / 3.0),
        );
    }
    let front_left_top = face(
        body,
        "f-cha1/corner/vertex(f-box1/front|f-box1/left|f-box1/top)",
    );
    assert_face_centroid(&front_left_top, [-26.0 / 3.0, -26.0 / 3.0, 56.0 / 3.0]);
}
