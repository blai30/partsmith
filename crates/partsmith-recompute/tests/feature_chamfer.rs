//! Acceptance tests of the `chamfer` feature (docs/spec/features/chamfer.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/chamfer/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_element_repair, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_list_field, assert_same_body, assert_volume,
    body_update, document, face, load_example, median_build_time, output_body, suggested_limit,
};
use partsmith_features::{BodyChange, FeatureError};
use partsmith_model::{Document, Units};
use partsmith_occt_sys::GeometryKind;

/// `CUBE20`: a 20 mm cube in `b-1` spanning [-10,-10,0]-[10,10,20].
const CUBE20: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}"#;

/// `E-FT`: the edge between the front and top faces of `CUBE20`.
const FRONT_TOP_EDGE: &str =
    r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#;

/// The 20 x 20 x 2.4 mm plate of test 6.
const PLATE: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}"#;

/// The top loop of `PLATE`: `E-FT`, `E-BT`, `E-LT` and `E-RT` with the plate's fingerprints.
const PLATE_TOP_EDGES: [&str; 4] = [
    r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-10,0,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=10,0,2.4|l=20"}"#,
];

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
fn t002_two_distances_first_on_the_face_that_sorts_first() {
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
    let limit = suggested_limit(max);
    let expected = format!(
        "Chamfer distance 3 mm is too large for the selected edges. The largest distance that works is about {limit} mm. Try {limit} mm or less."
    );
    assert_eq!(run.messages("f-cha1"), [expected.as_str()]);
    assert_eq!(error.suggestion(Units::Mm), Some(format!("{limit} mm")));
    assert_same_body(run.body("b-1"), &output_body(&run, "f-box1", "b-1"));
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
        run.messages("f-cha1"),
        [
            "One of the edges selected for Edges can no longer be found, so it was skipped. Re-pick it in the repair panel."
        ]
    );
    assert_volume(run.shape("b-1"), 7960.0);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds; run with cargo test --release"
)]
fn t011_performance() {
    let top_loop = median_build_time(&load_example("chamfer/top-loop.ptsm"), "f-cha1", 5);
    assert!(
        top_loop < Duration::from_millis(60),
        "chamfering the top loop took {top_loop:?}, the budget is 60 ms"
    );
}
