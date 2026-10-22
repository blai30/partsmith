//! Acceptance tests of the `linear_pattern` feature (docs/spec/features/linear_pattern.md section
//! 6), run as documents through recompute (features.md section 6.1). Geometry documents are the
//! examples in `examples/linear_pattern/`; the documents of tests 16 to 19 (frames on the model) are
//! written here until their examples exist.

use crate::feature_support;

use std::collections::BTreeMap;
use std::sync::Arc;

use feature_support::linear_pattern::{BODIES, CUBE10, X, direction, pattern};
use feature_support::{
    BOTH_FRONT_TOP_EDGES, FRONT_TOP_EDGES_OF_BOTH_BOXES, Run, assert_bore_through,
    assert_bounding_box, assert_choice_between, assert_error_field, assert_examples_canonical,
    assert_face_area, assert_face_count, assert_face_kind, assert_face_position,
    assert_face_translated, assert_has_face, assert_solid_count, assert_unchanged, assert_volume,
    body_changes, edge_axis, face, l_shaped_body, leaning_wall_pattern, link_ids, set_field,
    sketch_on_pattern_copy_document, turn,
};
use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, FeatureError, FeatureOutput, Values,
};
use partsmith_model::{EmptyContext, Id, Scope, Units};
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
    let registry = run.recompute.registry();
    let record = registry
        .validate_and_canonicalize(run.record("f-lp1"))
        .expect("the grid pattern record is valid")
        .record;
    let dependencies = run.dependencies("f-lp1");
    let manifest = registry
        .get("linear_pattern")
        .expect("linear_pattern is a built-in type")
        .manifest();
    let parameters = run.document.evaluate_parameters(&EmptyContext);
    let scope = Scope {
        units: run.document.units,
        parameters: &parameters,
        context: &EmptyContext,
    };
    let values = Values::evaluate(&dependencies.expressions, &scope, manifest);
    let bodies: BTreeMap<Id, BodyState> =
        run.feature("f-lp1").bodies_before.iter().cloned().collect();
    assert!(
        bodies.keys().any(|body| body.as_str() == "b-1"),
        "the run recorded the state of b-1 before the pattern"
    );
    // features.md section 2.6: the context also takes the bodies' writer chains, which this build does not read.
    let history: BTreeMap<Id, Vec<(Id, BodyState)>> = BTreeMap::new();
    let upstream: BTreeMap<Id, Arc<FeatureOutput>> = BTreeMap::new();
    let assets: BTreeMap<String, Arc<[u8]>> = BTreeMap::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let context = BuildContext::new(BuildInputs {
        feature: &record.id,
        manifest,
        units: run.document.units,
        values: &values,
        bodies: &bodies,
        history: &history,
        upstream: &upstream,
        assets: &assets,
        cancel: &cancel,
    });

    let result = registry.build(&record, &context);

    assert!(
        matches!(result, Err(FeatureError::Canceled)),
        "a canceled build returns Canceled, not {result:?}",
    );
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
    for (spacing, offset) in [("60", 60.0), ("80", 80.0)] {
        let run = Run::new(sketch_on_pattern_copy_document(spacing));

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

/// The R38 setup with its left wall leaning by `lean`, and the pattern of test 17 along its bottom edge, with the
/// axis's `extra` members.
fn pattern_along_leaning_wall(lean: i32, extra: &str) -> Run {
    Run::new(leaning_wall_pattern(lean, extra))
}

#[test]
fn t017_the_pattern_of_the_upright_wall_goes_along_the_edge() {
    let run = pattern_along_leaning_wall(0, "");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, 0.0], [40.0, 50.0, 10.0]);
}

#[test]
fn t017_an_edge_direction_that_leans_keeps_its_side() {
    for extra in ["", r#", "direction": [0, 1, 0]"#] {
        let run = pattern_along_leaning_wall(-1, extra);
        run.assert_all_ok();
        // The copy is offset by (-1.498129, 29.962570, 0), along (-0.049938, 0.998752, 0).
        assert_bounding_box(
            run.shape("b-1"),
            [-2.498129, 0.0, 0.0],
            [40.0, 49.962570, 10.0],
        );

        let run = pattern_along_leaning_wall(1, extra);
        run.assert_all_ok();
        // Offset (1.498129, 29.962570, 0).
        assert_bounding_box(
            run.shape("b-1"),
            [0.0, 0.0, 0.0],
            [41.498129, 49.962570, 10.0],
        );
    }
}

/// `CUBE10`, a half turn of it about Z inserted before the pattern, and the pattern of test 18 along its front top
/// edge as picked before the turn, with the axis's `extra` members.
fn pattern_after_a_half_turn(extra: &str) -> Run {
    Run::records(
        &BODIES,
        &[
            CUBE10,
            &turn("b-1", "Z", "180 deg"),
            &pattern(
                &direction(
                    &edge_axis(
                        "b-1",
                        r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}"#,
                        extra,
                    ),
                    "2",
                    "20",
                ),
                "",
            ),
        ],
    )
}

#[test]
fn t018_a_half_turn_of_the_part_turns_a_stored_direction() {
    let run = pattern_after_a_half_turn(r#", "direction": [1, 0, 0]"#);

    run.assert_all_ok();
    // The stored direction turns with the part to -X.
    assert_bounding_box(run.shape("b-1"), [-25.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
}

#[test]
fn t018_without_a_stored_direction_the_fingerprint_keeps_its_side() {
    // As written before format version 2: the fingerprint's +X.
    let run = pattern_after_a_half_turn("");

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
