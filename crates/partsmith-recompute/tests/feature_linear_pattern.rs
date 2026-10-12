//! Acceptance tests of the `linear_pattern` feature (docs/spec/features/linear_pattern.md section
//! 6), run as documents through recompute (features.md section 6.1). Geometry documents are the
//! examples in `examples/linear_pattern/`.

mod feature_support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_position, assert_has_face, assert_solid_count,
    assert_unchanged, assert_volume, body_changes, document, example_paths, face, link_ids,
    median_build_time,
};
use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, FeatureError, FeatureOutput, Values,
};
use partsmith_model::{EmptyContext, Id, Scope, Units};
use partsmith_occt_sys::GeometryKind;

/// The bodies of every linear pattern test.
const BODIES: [&str; 2] = ["b-1", "b-2"];

/// The `CUBE10` fixture: b-1 spanning [-5,-5,0]-[5,5,10].
const CUBE10: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}"#;

/// The origin X axis as an axis reference.
const X: &str = r#"{"kind": "origin", "axis": "X"}"#;

/// The origin Y axis as an axis reference.
const Y: &str = r#"{"kind": "origin", "axis": "Y"}"#;

/// The `DIR(axis, count, spacing)` fixture.
fn direction(axis: &str, count: &str, spacing: &str) -> String {
    format!(r#"{{"axis": {axis}, "count": "{count}", "spacing": "{spacing}"}}"#)
}

/// The `LP(direction, extra)` fixture; `extra` holds further members, or is empty.
fn pattern(direction: &str, extra: &str) -> String {
    let extra = if extra.is_empty() {
        String::new()
    } else {
        format!(", {extra}")
    };
    format!(
        r#"{{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {direction}{extra}}}"#
    )
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("linear_pattern");
    assert!(!paths.is_empty(), "examples/linear_pattern has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
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
    assert_unchanged(&run, "f-lp1", "b-1");
}

#[test]
fn t006_fractional_count() {
    let run = Run::records(&BODIES, &[CUBE10, &pattern(&direction(X, "2.5", "20"), "")]);

    let error = run.assert_error("f-lp1", "not_whole_number");
    assert_error_field(error, "direction.count");
    assert_eq!(
        run.messages("f-lp1"),
        ["Count must be a whole number, but it is 2.5. Use 2 or 3."]
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
        ["This pattern would make 20000 instances; the limit is 10000. Use fewer instances."]
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
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t014_one_hundred_instances_build_within_500_ms() {
    let document = document(&BODIES, &[CUBE10, &pattern(&direction(X, "100", "20"), "")]);

    let duration = median_build_time(&document, "f-lp1", 5);
    assert!(
        duration < Duration::from_millis(500),
        "100 instances took {duration:?} to build (median of 5 runs), over the 500 ms budget",
    );
}
