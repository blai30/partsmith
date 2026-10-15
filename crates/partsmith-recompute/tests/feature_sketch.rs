//! Acceptance tests of the `sketch` feature (docs/spec/features/sketch.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/sketch/`.

mod feature_support;

use std::sync::Arc;
use std::time::Duration;

use feature_support::{
    AXIS_TOLERANCE, Assets, Run, assert_coordinates, assert_error_field, assert_examples_canonical,
    assert_median_build_within, assert_placement, assert_relative, document_text, expression_uses,
    link_ids, load_example, message, parse_document, sketch_result,
};
use partsmith_features::{FallbackReason, FeatureError, FeatureWarning, Registry, Repair};
use partsmith_model::{FeatureInspector, FieldUnit};
use partsmith_occt_sys::{Plane, StepBody, make_box, write_step};
use partsmith_sketch::{Placement, PointRef, SketchError};

/// The four lines of `RECT(0, 0, 80, 50)` (features.md section 6.1).
const RECTANGLE_ENTITIES: &str = r#"[{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}]"#;

/// The parameter `width = 100 mm` of tests 7, 9, 10 and 18.
const WIDTH_PARAMETER: &str = r#"{"id": "p-w", "name": "width", "expr": "100 mm"}"#;

/// The message of a `plane_fallback` warning with reason `missing` (spec section 3.5).
const FACE_MISSING_TEXT: &str = "The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel.";

/// The message of a `plane_fallback` warning with reason `ambiguous` (spec section 3.5).
const FACE_AMBIGUOUS_TEXT: &str = "The face this sketch is on now matches several faces, so the sketch stays at its last position. Pick the face in the repair panel.";

/// The message of a `plane_fallback` warning with reason `not_planar` (spec section 3.5).
const FACE_NOT_FLAT_TEXT: &str = "The face this sketch is on is no longer flat, so the sketch stays at its last position. Attach it to a flat face.";

/// `SK-RECT` (spec section 2) with id `f-sk1` on the XY plane.
fn rectangle_sketch() -> String {
    format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": {RECTANGLE_ENTITIES}}}"#
    )
}

/// `LINE-DIM(expr)` (spec section 6), followed by `extra_constraints`.
fn line_dimension(expr: &str, extra_constraints: &[&str]) -> String {
    let mut constraints = vec![
        r#"{"id": "c-1", "type": "fix", "target": "e-1.start"}"#.to_owned(),
        r#"{"id": "c-2", "type": "horizontal", "line": "e-1"}"#.to_owned(),
        format!(r#"{{"id": "c-3", "type": "distance", "line": "e-1", "expr": "{expr}"}}"#),
    ];
    constraints.extend(
        extra_constraints
            .iter()
            .map(|constraint| (*constraint).to_owned()),
    );
    format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}}], "constraints": [{}]}}"#,
        constraints.join(", "),
    )
}

/// Runs a millimeter document with the `width` parameter and these bodies and features.
fn run_with_width(bodies: &[&str], features: &[&str]) -> Run {
    Run::new(parse_document(&document_text(
        "mm",
        &[WIDTH_PARAMETER],
        bodies,
        features,
    )))
}

/// The solved position of `point` in sketch `f-sk1`.
fn solved_point(run: &Run, point: &str) -> [f64; 2] {
    let reference: PointRef = point
        .parse()
        .unwrap_or_else(|error| panic!("{point} is not a point reference: {error}"));
    sketch_result(run, "f-sk1")
        .point(&reference)
        .unwrap_or_else(|| panic!("the sketch has no point {point}"))
}

/// The fallback placement used by the face planes of the tests: origin `origin`, x axis +X,
/// normal +Z.
fn fallback(origin: [f64; 3]) -> Placement {
    Placement {
        origin,
        x_axis: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

/// Asserts that the only warning of `f-sk1` is a `plane_fallback`, and returns its reason.
fn plane_fallback_reason(run: &Run) -> &FallbackReason {
    run.assert_warnings("f-sk1", &["plane_fallback"]);
    let warning = &run.feature("f-sk1").warnings[0];
    let FeatureWarning::PlaneFallback { reason, .. } = warning else {
        panic!("expected a plane fallback warning, got {warning:?}");
    };
    reason
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("sketch");
}

#[test]
fn t001_rectangle_on_xy() {
    let run = Run::example("sketch/rectangle-xy.ptsm");

    run.assert_ok("f-sk1");
    // `sketch_result` requires a sketch output, so the feature reports no body updates.
    let sketch = sketch_result(&run, "f-sk1");
    assert_placement(
        &sketch.placement,
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    let regions = &sketch.profile.regions;
    assert_eq!(regions.len(), 1, "regions of the rectangle");
    assert_eq!(regions[0].key.to_string(), "e-1,e-2,e-3,e-4");
    assert_relative("region area", regions[0].area, 4000.0);
    assert_coordinates(
        "region centroid",
        regions[0].centroid,
        [40.0, 25.0],
        AXIS_TOLERANCE,
    );
    assert!(run.report.bodies.is_empty(), "the document has no bodies");
}

#[test]
fn t002_on_xz() {
    let run = Run::example("sketch/rectangle-xz.ptsm");

    run.assert_ok("f-sk1");
    let sketch = sketch_result(&run, "f-sk1");
    assert_coordinates(
        "normal",
        sketch.placement.normal,
        [0.0, -1.0, 0.0],
        AXIS_TOLERANCE,
    );
    assert_coordinates(
        "x axis",
        sketch.placement.x_axis,
        [1.0, 0.0, 0.0],
        AXIS_TOLERANCE,
    );
    assert_coordinates(
        "to_world([10, 20])",
        sketch.to_world([10.0, 20.0]),
        [10.0, 0.0, 20.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t003_on_a_datum() {
    let run = Run::example("sketch/on-datum.ptsm");

    run.assert_all_ok();
    let placement = &sketch_result(&run, "f-sk1").placement;
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 10.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
    assert_eq!(link_ids(&run.dependencies("f-sk1").features), ["f-dp1"]);
}

#[test]
fn t004_on_a_face() {
    let run = Run::example("sketch/on-face.ptsm");

    run.assert_all_ok();
    assert_placement(
        &sketch_result(&run, "f-sk1").placement,
        [3.0, 4.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_eq!(link_ids(&run.dependencies("f-sk1").bodies_read), ["b-1"]);
}

#[test]
fn t005_face_lost() {
    let run = Run::example("sketch/face-lost.ptsm");

    run.assert_ok("f-box1");
    let reason = plane_fallback_reason(&run);
    assert!(
        matches!(reason, FallbackReason::Missing),
        "fallback reason {reason:?}"
    );
    assert_eq!(
        message(&run, "f-sk1", "plane_fallback").pointer.as_deref(),
        Some("/plane/face"),
    );
    assert_eq!(run.messages("f-sk1"), [FACE_MISSING_TEXT]);
    assert_eq!(
        sketch_result(&run, "f-sk1").placement,
        fallback([3.0, 4.0, 0.0])
    );
}

/// The name of the STEP asset of `face_ambiguous`.
fn cube_asset_name() -> String {
    format!("{}.step", "0".repeat(64))
}

/// A STEP file holding one 10 mm cube with its corner at the origin.
fn cube_step() -> Assets {
    let cube = make_box(&Plane::XY, 10.0, 10.0, 10.0).expect("the kernel makes a valid box");
    let bodies = [StepBody {
        shape: cube.shape,
        name: "Cube".to_owned(),
        color: None,
    }];
    let bytes: Arc<[u8]> = Arc::from(write_step(&bodies).expect("the kernel writes STEP"));
    Assets::from([(cube_asset_name(), bytes)])
}

#[test]
fn face_ambiguous() {
    // Section 3.3: a face reference that now matches several faces keeps the fallback placement.
    // Imported faces have fallback names, `f-imp1/face#1` to `#6`, which the resolver confirms by
    // their fingerprint alone (naming.md section 3.9 step 1). The face named here is gone and its
    // partial fingerprint fits all six faces of the cube equally.
    let import = format!(
        r#"{{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "{}", "op": "new"}}"#,
        cube_asset_name()
    );
    let sketch = format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "face", "body": "b-1", "face": {{"q": "f-imp1/face#9", "fp": "plane|a=100"}}, "fallback": {{"origin": [3, 4, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}}}, "entities": {RECTANGLE_ENTITIES}}}"#
    );
    let document = parse_document(&document_text("mm", &[], &["b-1"], &[&import, &sketch]));
    let run = Run::with_assets(document, &cube_step());

    run.assert_ok("f-imp1");
    let reason = plane_fallback_reason(&run);
    assert!(
        matches!(reason, FallbackReason::Ambiguous),
        "fallback reason {reason:?}"
    );
    assert_eq!(run.messages("f-sk1"), [FACE_AMBIGUOUS_TEXT]);
    // features.md section 4.2: an element repair carries the loss, ambiguous when several faces fit.
    let repair = run.feature("f-sk1").warnings[0].repair();
    assert!(
        matches!(&repair, Some(Repair::Element { loss, .. }) if loss.ambiguous),
        "repair data {repair:?}",
    );
    assert_eq!(
        sketch_result(&run, "f-sk1").placement,
        fallback([3.0, 4.0, 0.0])
    );
}

#[test]
fn t006_face_not_flat() {
    let run = Run::example("sketch/face-not-flat.ptsm");

    run.assert_ok("f-cyl1");
    let reason = plane_fallback_reason(&run);
    assert!(
        matches!(reason, FallbackReason::NotPlanar),
        "fallback reason {reason:?}"
    );
    assert_eq!(run.messages("f-sk1"), [FACE_NOT_FLAT_TEXT]);
    assert_eq!(
        sketch_result(&run, "f-sk1").placement,
        fallback([0.0, 0.0, 0.0])
    );
}

#[test]
fn t007_driving_dimension_from_a_parameter() {
    let run = Run::example("sketch/dimension-parameter.ptsm");

    run.assert_ok("f-sk1");
    assert_coordinates(
        "e-1.end",
        solved_point(&run, "e-1.end"),
        [100.0, 0.0],
        AXIS_TOLERANCE,
    );
    assert_eq!(
        expression_uses(&run.dependencies("f-sk1")),
        [("constraints[2].expr", "width", FieldUnit::Length)],
    );
}

#[test]
fn t008_bare_dimension_in_inches() {
    let run = Run::example("sketch/dimension-inches.ptsm");

    run.assert_ok("f-sk1");
    assert_coordinates(
        "e-1.end",
        solved_point(&run, "e-1.end"),
        [50.8, 0.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t009_conflict() {
    let sketch = line_dimension(
        "width",
        &[r#"{"id": "c-4", "type": "distance", "line": "e-1", "expr": "50 mm"}"#],
    );
    let extrude = r#"{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1", "fp": "c=40,0|a=1"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "new"}"#;
    let run = run_with_width(&["b-1"], &[&sketch, extrude]);

    let error = run.assert_error("f-sk1", "sketch_conflict");
    let FeatureError::SketchConflict { constraints, .. } = error else {
        panic!("expected a sketch conflict, got {error:?}");
    };
    assert!(
        constraints
            .iter()
            .any(|constraint| constraint.as_str() == "c-4"),
        "the conflicting constraints {constraints:?} include c-4",
    );
    run.assert_error("f-ext1", "upstream_failed");
}

#[test]
fn t010_redundant() {
    let run = Run::example("sketch/redundant-dimension.ptsm");

    run.assert_warnings("f-sk1", &["redundant_constraints"]);
    assert_coordinates(
        "e-1.end",
        solved_point(&run, "e-1.end"),
        [100.0, 0.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t011_value_rejected_by_the_solver() {
    let run = Run::records(&[], &[&line_dimension("-5", &[])]);

    let error = run.assert_error("f-sk1", "sketch_invalid");
    assert!(
        matches!(
            error,
            FeatureError::SketchInvalid {
                error: SketchError::ValueNotPositive { .. },
                ..
            }
        ),
        "expected the sketch crate's ValueNotPositive, got {error:?}",
    );
}

#[test]
fn t012_expression_error() {
    let run = Run::records(&[], &[&line_dimension("wdth", &[])]);

    let error = run.assert_error("f-sk1", "expression");
    assert_error_field(error, "constraints[2].expr");
    assert_eq!(
        run.messages("f-sk1"),
        ["Dimension: Unknown name 'wdth'. Check the spelling, or add a parameter named wdth."],
    );
}

#[test]
fn t013_datum_that_is_a_sketch() {
    let run = Run::records(
        &[],
        &[
            &rectangle_sketch(),
            r#"{"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-sk1"}}"#,
        ],
    );

    run.assert_ok("f-sk1");
    run.assert_error("f-sk2", "wrong_feature_type");
    assert_eq!(
        run.messages("f-sk2"),
        ["'Sketch 1' is not a datum plane. Pick a datum plane for Plane."],
    );
}

#[test]
fn t014_construction_only() {
    let run = Run::example("sketch/construction-only.ptsm");

    run.assert_ok("f-sk1");
    assert!(
        sketch_result(&run, "f-sk1").profile.regions.is_empty(),
        "a construction line bounds no region",
    );
}

#[test]
fn t015_declared_ids() {
    let sketch = line_dimension(
        "width",
        &[r#"{"id": "c-4", "type": "distance", "line": "e-1", "expr": "50 mm"}"#],
    );
    let document = parse_document(&document_text("mm", &[WIDTH_PARAMETER], &[], &[&sketch]));

    let dependencies = Registry::with_builtins()
        .dependencies(&document.features[0])
        .expect("the conflicting sketch still has valid params");
    let declared: Vec<&str> = dependencies
        .declared_ids
        .iter()
        .map(|id| id.as_str())
        .collect();
    assert_eq!(declared, ["e-1", "c-1", "c-2", "c-3", "c-4"]);
}

#[test]
fn t016_invalid_params() {
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}], "constraints": [{"id": "c-1", "type": "horizontal", "line": "e-9"}]}"#;
    let run = Run::records(&[], &[sketch]);

    let error = run
        .recompute
        .registry()
        .validate_and_canonicalize(run.record("f-sk1"))
        .expect_err("a constraint on an unknown entity fails validation");
    assert!(
        matches!(
            error,
            FeatureError::SketchInvalid {
                error: SketchError::UnknownEntity { .. },
                ..
            }
        ),
        "expected the sketch crate's UnknownEntity, got {error:?}",
    );
    run.assert_error("f-sk1", "sketch_invalid");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t017_rectangle_builds_within_2_ms() {
    let document = load_example("sketch/rectangle-xy.ptsm");

    // The budget excludes the solve and the profile, which the report does not time separately;
    // for these four unconstrained lines they take a negligible part of the whole build.
    assert_median_build_within(&document, "f-sk1", 21, Duration::from_millis(2));
}

#[test]
fn t018_reference_dimension() {
    let run = Run::example("sketch/reference-dimension.ptsm");

    run.assert_ok("f-sk1");
    assert_coordinates(
        "e-1.end",
        solved_point(&run, "e-1.end"),
        [100.0, 0.0],
        AXIS_TOLERANCE,
    );
    assert_eq!(
        expression_uses(&run.dependencies("f-sk1")),
        [
            ("constraints[2].expr", "width", FieldUnit::Length),
            ("constraints[3].expr", "wdth", FieldUnit::Length),
        ],
    );
    let links = run
        .recompute
        .registry()
        .inspect(run.record("f-sk1"))
        .unwrap_or_else(|error| panic!("the sketch is valid: {error}"));
    let pointers: Vec<(&str, &str)> = links
        .expressions
        .iter()
        .map(|link| (link.pointer.as_str(), link.expr.as_str()))
        .collect();
    assert_eq!(
        pointers,
        [
            ("/constraints/2/expr", "width"),
            ("/constraints/3/expr", "wdth")
        ]
    );
}

#[test]
fn t019_empty_body_under_a_face_plane() {
    let run = Run::example("sketch/face-plane-empty-body.ptsm");

    let reason = plane_fallback_reason(&run);
    assert!(
        matches!(reason, FallbackReason::Missing),
        "fallback reason {reason:?}"
    );
    assert_eq!(
        sketch_result(&run, "f-sk1").placement,
        fallback([3.0, 4.0, 0.0])
    );
}
