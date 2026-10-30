//! Acceptance tests of the `sketch` feature (docs/spec/features/sketch.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/sketch/`, from which the tests make their other documents: test 20 runs corpus R32 (a),
//! `examples/references/moved-face-sketch`, tests 25 and 27 `on-split-face` and `on-pattern-copy`,
//! and test 32 test 25's document with another face. The other tests of 21 to 29 (frames on the
//! model, phase 2 of the reference stability plan) expect an error or a warning and write their
//! documents here.
//!
//! Tests 20 to 29 and 32 check what the sketch feature and the features built on it compute: placements,
//! motions, the sketch's own messages and fixes, the messages of later features with their texts and
//! fixes (features.md sections 3.5 and 3.15 and the hole points of features/hole.md), geometry, and
//! the document after a fix as the engine's `apply_fix` would write it. Tests 30 and 31 check the
//! modeling limit of a dimension and the dimension values carried by sketch errors. Tests 34 to 51 load
//! the constraint, drawing tool and record form examples and check the solve against the stored geometry:
//! "as stored" bitwise, "as drawn" within 1e-9 mm.

use crate::feature_support;

use std::f64::consts::FRAC_PI_6;

use feature_support::import::box_step_asset;
use feature_support::{
    AXIS_TOLERANCE, BOTH_TOPS, MOVE_BY_100, Run, TURN_BY_90, assert_absolute, assert_bore_through,
    assert_coordinates, assert_error_field, assert_examples_canonical, assert_face_axis_through,
    assert_face_in_plane, assert_moves, assert_placement, assert_quarter_turns_about_z,
    assert_relative, assert_round_face, assert_sketch_on_top_plane, assert_solid_count,
    assert_top_at, assert_translation, assert_volume, assert_volume_faces_and_bounds, asset_name,
    datum_motion, document, document_text, expression_uses, face, face_motion_before, face_sketch,
    face_sketch_with, feature_record_mut, hole_at_point, id, l_shaped_body, link_ids, load_example,
    message, moved_box_document, one_circle, one_point, parse_document, reference_json, saved,
    sketch_result, solved_point, stored_region, stored_region_match, top_fallback, turn,
};
use partsmith_features::{
    Bound, FallbackCause, FeatureError, FeatureWarning, Fix, Placed, Registry, Repair,
};
use partsmith_model::{FeatureInspector, FieldUnit, Id};
use partsmith_naming::{Follow, Motion, Reference};
use partsmith_occt_sys::GeometryKind;
use partsmith_sketch::{
    Geometry, MatchMethod, Placement, PointRole, RegionMatch, SketchEntity, SketchError,
    SketchParams, SketchStatus,
};
use serde_json::{Value, json};

/// The four lines of `RECT(0, 0, 80, 50)` (features.md section 6.1).
const RECTANGLE_ENTITIES: &str = r#"[{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}]"#;

/// The parameter `width = 100 mm` of tests 7, 9, 10 and 18.
const WIDTH_PARAMETER: &str = r#"{"id": "p-w", "name": "width", "expr": "100 mm"}"#;

/// The message of test 5's `plane_fallback` warning, cause `missing` with a loss (spec section 3.5): the box makes no
/// face named `nothing`, which recompute's diagnosis explains as no longer made by the box.
const FACE_MISSING_TEXT: &str = "The face this sketch is on, a face of 'Box 1', is no longer made by 'Box 1', so the sketch uses the position saved with it.";

/// The message of the `plane_fallback` warning of an imported face that now matches the six faces of a cube, cause
/// `ambiguous` (spec section 3.5).
const FACE_AMBIGUOUS_TEXT: &str = "The face this sketch is on, a face of 'Import 1', now matches 6 faces, so the sketch uses the position saved with it. Pick the one you mean.";

/// The message of test 6's `plane_fallback` warning, cause `not_planar` (spec section 3.5).
const FACE_NOT_FLAT_TEXT: &str = "The face this sketch is on, the side of 'Cylinder 1', is no longer flat, so the sketch uses the position saved with it.";

/// The message of test 19's `plane_fallback` warning, cause `missing` without a loss (spec section 3.5).
const FACE_ON_EMPTY_BODY_TEXT: &str = "The face this sketch is on, a face of a feature that is not in this document, is gone because body 'Body 1' is empty here, so the sketch uses the position saved with it.";

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

/// The fallback placement used by the face planes of the tests: origin `origin`, x axis +X,
/// normal +Z.
fn fallback(origin: [f64; 3]) -> Placement {
    Placement {
        origin,
        x_axis: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

/// Asserts that the only warning of `f-sk1` is a `plane_fallback` placed at its saved position, and returns its
/// cause and whether it carries a loss.
fn plane_fallback_cause(run: &Run) -> (FallbackCause, bool) {
    let (placed, cause, has_loss) = plane_fallback_of(run, "f-sk1");
    assert_eq!(placed, Placed::Saved, "where the sketch was placed");
    (cause, has_loss)
}

/// The fixes of the `plane_fallback` message of `f-sk1`.
fn plane_fallback_fixes(run: &Run) -> &[Fix] {
    &message(run, "f-sk1", "plane_fallback").fixes
}

/// `Pick` of the face of the sketch plane.
fn pick_face() -> Fix {
    Fix::Pick {
        pointer: "/plane/face".to_owned(),
    }
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
    assert_eq!(plane_fallback_cause(&run), (FallbackCause::Missing, true));
    assert_eq!(
        message(&run, "f-sk1", "plane_fallback").pointer.as_deref(),
        Some("/plane/face"),
    );
    assert_eq!(run.messages("f-sk1"), [FACE_MISSING_TEXT]);
    assert_eq!(
        sketch_result(&run, "f-sk1").placement,
        fallback([3.0, 4.0, 0.0])
    );
    // The top, the only candidate that faces the way the stored face did, comes first, then the four other faces.
    let fixes = plane_fallback_fixes(&run);
    let attached: Vec<&str> = fixes
        .iter()
        .filter_map(|fix| match fix {
            Fix::Attach { sketch, body, face }
                if sketch.as_str() == "f-sk1" && body.as_str() == "b-1" =>
            {
                Some(face.query.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(attached.len(), 5, "{fixes:?}");
    assert_eq!(attached[0], "f-box1/top", "{fixes:?}");
    assert_eq!(fixes.len(), 6, "{fixes:?}");
    assert_eq!(fixes[5], pick_face());
}

#[test]
fn face_ambiguous() {
    // Section 3.3: a face reference that now matches several faces keeps the fallback placement.
    // Imported faces have fallback names, `f-imp1/face#1` to `#6`, which the resolver confirms by
    // their fingerprint alone (naming.md section 3.9 step 1). The face named here is gone and its
    // partial fingerprint fits all six faces of the cube equally.
    let import = format!(
        r#"{{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "{}", "op": "new"}}"#,
        asset_name('0')
    );
    let sketch = format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "face", "body": "b-1", "face": {{"q": "f-imp1/face#9", "fp": "plane|a=100"}}, "fallback": {{"origin": [3, 4, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}}}, "entities": {RECTANGLE_ENTITIES}}}"#
    );
    let document = parse_document(&document_text("mm", &[], &["b-1"], &[&import, &sketch]));
    let run = Run::with_assets(document, &box_step_asset("Cube", [10.0, 10.0, 10.0]));

    run.assert_ok("f-imp1");
    assert_eq!(plane_fallback_cause(&run), (FallbackCause::Ambiguous, true));
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
    assert_eq!(
        plane_fallback_cause(&run),
        (FallbackCause::NotPlanar, false)
    );
    assert_eq!(run.messages("f-sk1"), [FACE_NOT_FLAT_TEXT]);
    assert_eq!(plane_fallback_fixes(&run), [pick_face()]);
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
        solved_point(&run, "f-sk1", "e-1.end"),
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
        solved_point(&run, "f-sk1", "e-1.end"),
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
        solved_point(&run, "f-sk1", "e-1.end"),
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
fn t018_reference_dimension() {
    let run = Run::example("sketch/reference-dimension.ptsm");

    run.assert_ok("f-sk1");
    assert_coordinates(
        "e-1.end",
        solved_point(&run, "f-sk1", "e-1.end"),
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

    assert_eq!(plane_fallback_cause(&run), (FallbackCause::Missing, false));
    assert_eq!(run.messages("f-sk1"), [FACE_ON_EMPTY_BODY_TEXT]);
    assert_eq!(plane_fallback_fixes(&run), [pick_face()]);
    let sketch = sketch_result(&run, "f-sk1");
    assert_eq!(sketch.placement, fallback([3.0, 4.0, 0.0]));
    assert_eq!(sketch.motion, Motion::IDENTITY);
}

// Tests 20 to 29 and 32: a sketch on a face when the face moves, turns over, splits, spreads over two planes, is
// copied or is lost (spec sections 3.3 and 3.5, ADR 0023).

/// An extrude `id` into `b-1` of the region `key` (with fingerprint `fingerprint`) of `sketch`, by `distance`, with
/// `extra` members (or none) and `op`.
fn extrude(
    id: &str,
    sketch: &str,
    region: (&str, &str),
    distance: &str,
    extra: &str,
    op: &str,
) -> String {
    let (key, fingerprint) = region;
    format!(
        r#"{{"id": "{id}", "type": "extrude", "body": "b-1", "profile": {{"sketch": "{sketch}", "regions": [{{"key": "{key}", "fp": "{fingerprint}"}}]}}, "extent": {{"kind": "distance", "expr": "{distance}"}}{extra}, "op": "{op}"}}"#
    )
}

/// `RECT-11(x0, y0, x1, y1)` of spec section 6 as entities: the lines `e-11` to `e-14`.
fn rectangle_11(corner: [f64; 2], opposite: [f64; 2]) -> String {
    let [x0, y0] = corner;
    let [x1, y1] = opposite;
    format!(
        r#"[{{"id": "e-11", "type": "line", "start": [{x0}, {y0}], "end": [{x1}, {y0}]}}, {{"id": "e-12", "type": "line", "start": [{x1}, {y0}], "end": [{x1}, {y1}]}}, {{"id": "e-13", "type": "line", "start": [{x1}, {y1}], "end": [{x0}, {y1}]}}, {{"id": "e-14", "type": "line", "start": [{x0}, {y1}], "end": [{x0}, {y0}]}}]"#
    )
}

/// The region of `RECT-11`.
const RECTANGLE_11_KEY: &str = "e-11,e-12,e-13,e-14";

/// A sketch `id` on the datum `datum` with these entities.
fn sketch_on_datum(id: &str, datum: &str, entities: &str) -> String {
    format!(
        r#"{{"id": "{id}", "type": "sketch", "plane": {{"kind": "datum", "feature": "{datum}"}}, "entities": {entities}}}"#
    )
}

/// A datum plane `f-dp1` at `offset` above XY.
fn datum_above_xy(offset: &str) -> String {
    format!(
        r#"{{"id": "f-dp1", "type": "datum_plane", "base": {{"kind": "origin", "plane": "XY"}}, "offset": "{offset}"}}"#
    )
}

/// `BLOCK` of spec section 6: `SK-RECT` and its extrude `f-ext1` of 30 into `b-1`, op new, with `extra` members of the
/// extrude (or none).
fn block(extra: &str) -> [String; 2] {
    [
        rectangle_sketch(),
        extrude(
            "f-ext1",
            "f-sk1",
            ("e-1,e-2,e-3,e-4", "c=40,25|a=4000"),
            "30",
            extra,
            "new",
        ),
    ]
}

/// The end cap of `BLOCK` as a pick stored it (its top at z = 30).
const BLOCK_TOP: &str =
    r#"{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}"#;

/// Asserts that `f-hole1` warns `hole_misses_body`. That it is the only warning (one point, so no `no_effect`,
/// features.md section 3.15) and its text and fixes come with phase 3; its root under a drawing turned over is
/// checked by recompute.md test 72 (`diagnosis.rs`).
#[track_caller]
fn assert_hole_misses_the_body(run: &Run, expected_text: &str, expected_fixes: &[Fix]) {
    // One point, so `hole_misses_body` is the only warning, with no `no_effect` (features.md section 3.15).
    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    assert_eq!(run.messages("f-hole1"), [expected_text]);
    assert_eq!(
        message(run, "f-hole1", "hole_misses_body").fixes,
        expected_fixes
    );
}

/// The `Show` fix of sketch `f-sk2` as a whole.
fn show_sketch() -> Fix {
    Fix::Show {
        feature: id("f-sk2"),
        entity: None,
    }
}

/// The text of the `separate_solid` warning of an extrude whose boss floats beside body 'Body 1' in two pieces.
const BOSS_APART_TEXT: &str =
    "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.";

/// The only `plane_fallback` warning of `sketch`: where it placed the sketch, its cause and whether it has a loss.
fn plane_fallback_of(run: &Run, sketch: &str) -> (Placed, FallbackCause, bool) {
    run.assert_warnings(sketch, &["plane_fallback"]);
    let (placed, cause, loss) = run.plane_fallback(sketch);
    (placed.clone(), cause, loss.is_some())
}

/// `Attach` of `sketch` to `face` of `b-1`.
fn attach(sketch: &str, face: Reference) -> Fix {
    Fix::Attach {
        sketch: id(sketch),
        body: id("b-1"),
        face,
    }
}

/// The faces of the `Attach` fixes of a message, by query, in order.
fn attached_faces(fixes: &[Fix]) -> Vec<&str> {
    fixes
        .iter()
        .filter_map(|fix| match fix {
            Fix::Attach { face, .. } => Some(face.query.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn t020_the_part_moves_and_the_sketch_moves_with_its_face() {
    // Corpus R32 (a): the move inserted before the face sketch, whose fallback is in body coordinates.
    let run = Run::example("references/moved-face-sketch.ptsm");

    run.assert_all_ok();
    assert_sketch_on_top_plane(&run, "f-sk2", [100.0, 0.0, 10.0]);
    let sketch = sketch_result(&run, "f-sk2");
    assert_translation(&sketch.motion, [100.0, 0.0, 0.0]);
    assert_coordinates(
        "to_world([15, 15])",
        sketch.to_world([15.0, 15.0]),
        [115.0, 15.0, 10.0],
        AXIS_TOLERANCE,
    );
    assert_bore_through(&run, [115.0, 15.0, 0.0]);
    assert_volume(run.shape("b-1"), 15874.336294);
}

#[test]
fn t020_the_part_turns_and_the_sketch_turns_with_its_face() {
    let mut turned = load_example("references/moved-face-sketch.ptsm");
    *feature_record_mut(&mut turned, "f-mv1") =
        serde_json::from_str(TURN_BY_90).expect("a move body record");

    let run = Run::new(turned);

    run.assert_all_ok();
    let sketch = sketch_result(&run, "f-sk2");
    assert_placement(
        &sketch.placement,
        [0.0, 0.0, 10.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_quarter_turns_about_z(&sketch.motion, 1);
    assert_coordinates(
        "to_world([15, 15])",
        sketch.to_world([15.0, 15.0]),
        [-15.0, 15.0, 10.0],
        AXIS_TOLERANCE,
    );
    assert_bore_through(&run, [-15.0, 15.0, 0.0]);
    assert_volume(run.shape("b-1"), 15874.336294);
}

#[test]
fn t021_a_fallback_in_world_coordinates_rebuilds_as_before() {
    let document = moved_box_document(MOVE_BY_100, None);
    let text = saved(&document);
    let run = Run::new(document);

    run.assert_ok("f-sk2");
    assert_sketch_on_top_plane(&run, "f-sk2", [0.0, 0.0, 10.0]);
    let sketch = sketch_result(&run, "f-sk2");
    assert_eq!(sketch.motion, Motion::IDENTITY);
    assert_coordinates(
        "to_world([15, 15])",
        sketch.to_world([15.0, 15.0]),
        [15.0, 15.0, 10.0],
        AXIS_TOLERANCE,
    );
    // Drilling the other way misses too, so there is no `reverse` fix.
    assert_hole_misses_the_body(
        &run,
        "The hole at (15, 15) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body.",
        &[],
    );
    assert_volume(run.shape("b-1"), 16000.0);
    // Nothing is written back, and no `fallback_frame` is added.
    assert_eq!(saved(&run.document), text);
    assert!(!text.contains("fallback_frame"), "{text}");
}

/// The document of test 22 (corpus R33): the box cut below a sketch attached to its top when it was 10 tall, and a
/// boss on that sketch.
fn cut_top_document() -> partsmith_model::Document {
    document(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "40"], "op": "new"}"#,
            &datum_above_xy("35"),
            &sketch_on_datum(
                "f-sk3",
                "f-dp1",
                &rectangle_11([-15.0, -15.0], [15.0, 15.0]),
            ),
            &extrude(
                "f-ext3",
                "f-sk3",
                (RECTANGLE_11_KEY, "c=0,0|a=900"),
                "10",
                "",
                "cut",
            ),
            &face_sketch(
                r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=400|adj=4,4"}"#,
                [0.0, 0.0, 10.0],
                None,
                &one_circle("e-5", [0.0, 0.0], 3.0),
            ),
            &extrude(
                "f-ext2",
                "f-sk2",
                ("e-5", "c=0,0|a=28.274334"),
                "5",
                "",
                "join",
            ),
        ],
    )
}

#[test]
fn t022_a_face_cut_away_before_the_sketch_keeps_it_where_the_face_was() {
    let run = Run::new(cut_top_document());

    assert_sketch_on_top_plane(&run, "f-sk2", [0.0, 0.0, 40.0]);
    let (placed, cause, has_loss) = plane_fallback_of(&run, "f-sk2");
    assert_eq!(placed, Placed::BeforeRemoval { by: id("f-ext3") });
    assert_eq!(cause, FallbackCause::Missing);
    assert!(has_loss, "the warning carries the loss");
    assert_eq!(
        run.messages("f-sk2"),
        [
            "The face this sketch is on, the top of 'Box 1', was cut away by 'Extrude 1', which comes before this sketch, so the sketch stays where that face was."
        ]
    );
    // The floor of the cut, the only candidate that faces the way the stored face did, comes first.
    let fixes = &message(&run, "f-sk2", "plane_fallback").fixes;
    assert_eq!(
        fixes.first(),
        Some(&attach(
            "f-sk2",
            run.picked_before("f-sk2", "b-1", "f-ext3/start-cap")
        )),
        "{fixes:?}"
    );
    assert_eq!(fixes.last(), Some(&pick_face()), "{fixes:?}");
    assert_eq!(
        attached_faces(fixes).len(),
        fixes.len() - 1,
        "every other fix attaches to a planar candidate: {fixes:?}"
    );
    // The boss stands at z 40..45, above the new top at z = 35, never inside the box.
    run.assert_ok("f-ext3");
    assert_top_at(&run, "b-1", 45.0);
    run.assert_warnings("f-ext2", &["separate_solid"]);
    assert_eq!(run.messages("f-ext2"), [BOSS_APART_TEXT]);
    assert_eq!(
        message(&run, "f-ext2", "separate_solid")
            .root
            .as_ref()
            .map(Id::as_str),
        Some("f-sk2")
    );
}

#[test]
fn t022_the_placement_is_read_from_the_run_and_nothing_is_written() {
    let document = cut_top_document();
    let text = saved(&document);
    let run = Run::new(document);
    assert_eq!(saved(&run.document), text);

    let again = Run::new(parse_document(&text));

    assert_eq!(
        sketch_result(&again, "f-sk2").placement,
        sketch_result(&run, "f-sk2").placement
    );
}

/// The document of test 23 (corpus R26, amended): `BLOCK` after `f-ext1` was deleted and `f-ext3`, 80 tall, added in
/// its place, with the face sketch `f-sk2` given as a whole record.
fn deleted_base_document(face_sketch_record: &str) -> partsmith_model::Document {
    document(
        &["b-1"],
        &[
            &rectangle_sketch(),
            &extrude(
                "f-ext3",
                "f-sk1",
                ("e-1,e-2,e-3,e-4", "c=40,25|a=4000"),
                "80",
                "",
                "new",
            ),
            face_sketch_record,
            &extrude(
                "f-ext2",
                "f-sk2",
                ("e-5", "c=40,25|a=314.159265"),
                "10",
                "",
                "join",
            ),
        ],
    )
}

#[test]
fn t023_a_face_whose_feature_was_deleted_uses_the_saved_position() {
    let run = Run::new(deleted_base_document(&face_sketch(
        BLOCK_TOP,
        [0.0, 0.0, 30.0],
        None,
        &one_circle("e-5", [40.0, 25.0], 10.0),
    )));

    assert_eq!(
        sketch_result(&run, "f-sk2").placement,
        fallback([0.0, 0.0, 30.0])
    );
    assert_eq!(
        plane_fallback_of(&run, "f-sk2"),
        (Placed::Saved, FallbackCause::Missing, true)
    );
    assert_eq!(
        run.messages("f-sk2"),
        [
            "The face this sketch is on, a face of a feature that is not in this document, is gone, so the sketch uses the position saved with it."
        ]
    );
    // The candidate order of naming.md test 133: the top of 'Extrude 1' first, then the nearest faces.
    let fixes = &message(&run, "f-sk2", "plane_fallback").fixes;
    let mut expected: Vec<Fix> = [
        "f-ext3/end-cap",
        "f-ext3/lateral/e-1",
        "f-ext3/lateral/e-3",
        "f-ext3/start-cap",
        "f-ext3/lateral/e-2",
    ]
    .iter()
    .map(|name| attach("f-sk2", run.picked_before("f-sk2", "b-1", name)))
    .collect();
    expected.push(pick_face());
    assert_eq!(fixes, &expected);
    // The boss lies inside the taller block, explained by the sketch that lost its face.
    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_eq!(
        run.messages("f-ext2"),
        [
            "This extrude adds nothing to body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first."
        ]
    );
    let no_effect = message(&run, "f-ext2", "no_effect");
    assert_eq!(no_effect.root.as_ref().map(Id::as_str), Some("f-sk2"));
    assert_eq!(no_effect.fixes, [show_sketch()]);
    assert_volume(run.shape("b-1"), 320000.0);
}

#[test]
fn t023_attaching_to_the_new_top_puts_the_boss_on_it() {
    let before = Run::new(deleted_base_document(&face_sketch(
        BLOCK_TOP,
        [0.0, 0.0, 30.0],
        None,
        &one_circle("e-5", [40.0, 25.0], 10.0),
    )));
    // `Attach` stores the face as picked and the sketch's placement projected onto it, in body coordinates.
    let new_top = reference_json(&before.picked_before("f-sk2", "b-1", "f-ext3/end-cap"));

    let run = Run::new(deleted_base_document(&face_sketch(
        &new_top,
        [0.0, 0.0, 80.0],
        Some("body"),
        &one_circle("e-5", [40.0, 25.0], 10.0),
    )));

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_solid_count(shape, 1);
    assert_volume(shape, 323141.592654);
    assert_top_at(&run, "b-1", 90.0);
}

/// The document of test 24 (corpus R32b): `BLOCK` reversed, a face sketch on its end cap given as a whole record, and
/// a hole 10 deep at its point.
fn turned_over_document(face_sketch_record: &str) -> partsmith_model::Document {
    let [sketch, extrude] = block(r#", "reverse": true"#);
    document(
        &["b-1"],
        &[
            &sketch,
            &extrude,
            face_sketch_record,
            &hole_at_point("5", r#"{"kind": "distance", "expr": "10"}"#),
        ],
    )
}

/// Asserts the outcome of test 24 before `TurnDrawingBack`: the sketch turned over and its hole beside the body.
#[track_caller]
fn assert_drawing_turned_over(run: &Run) {
    let sketch = sketch_result(run, "f-sk2");
    assert_placement(
        &sketch.placement,
        [0.0, 0.0, -30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert_coordinates(
        "y axis",
        sketch.placement.y_axis(),
        [0.0, -1.0, 0.0],
        AXIS_TOLERANCE,
    );
    run.assert_warnings("f-sk2", &["sketch_turned_over"]);
    assert_eq!(
        run.messages("f-sk2"),
        [
            "The face this sketch is on, the top of 'Extrude 1', now faces the other way, so the drawing is mirrored on it."
        ]
    );
    assert_eq!(
        message(run, "f-sk2", "sketch_turned_over").fixes,
        [
            Fix::TurnDrawingBack {
                sketch: id("f-sk2")
            },
            pick_face(),
        ]
    );
    assert_coordinates(
        "to_world([20, 10])",
        sketch.to_world([20.0, 10.0]),
        [20.0, -10.0, -30.0],
        AXIS_TOLERANCE,
    );
    // The point lies off the face, explained by the drawing that lies mirrored.
    assert_hole_misses_the_body(
        run,
        "The hole at (20, 10) in 'Sketch 2' does not touch body 'Body 1' because the drawing of 'Sketch 2' lies mirrored on its face. Fix 'Sketch 2' first.",
        &[show_sketch()],
    );
    assert_eq!(
        message(run, "f-hole1", "hole_misses_body")
            .root
            .as_ref()
            .map(Id::as_str),
        Some("f-sk2")
    );
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t024_a_face_that_turned_over_mirrors_the_drawing_with_a_warning() {
    let run = Run::new(turned_over_document(&face_sketch(
        BLOCK_TOP,
        [0.0, 0.0, 30.0],
        None,
        &one_point([20.0, 10.0]),
    )));

    assert_drawing_turned_over(&run);
}

#[test]
fn t024_a_fallback_in_body_coordinates_turns_over_the_same_way() {
    let run = Run::new(turned_over_document(&face_sketch(
        BLOCK_TOP,
        [0.0, 0.0, 30.0],
        Some("body"),
        &one_point([20.0, 10.0]),
    )));

    assert_drawing_turned_over(&run);
}

#[test]
fn t024_turning_the_drawing_back_drills_the_hole_where_it_was_drawn() {
    // `TurnDrawingBack` as one edit: the point mirrored across the sketch's x axis, the current frame stored as the
    // fallback in body coordinates, the hole's points unchanged.
    let run = Run::new(turned_over_document(&face_sketch_with(
        BLOCK_TOP,
        r#"{"origin": [0, 0, -30], "x_axis": [1, 0, 0], "normal": [0, 0, -1]}"#,
        Some("body"),
        &one_point([20.0, -10.0]),
    )));

    run.assert_all_ok();
    assert_bore_through(&run, [20.0, 10.0, 0.0]);
    // Drilled from z = -30 up to z = -20.
    assert_face_in_plane(&face(run.body("b-1"), "f-hole1/bottom/e-p1"), 2, -20.0);
    assert_volume(run.shape("b-1"), 119803.650459);
}

#[test]
fn t025_a_face_split_into_coplanar_pieces_keeps_the_placement() {
    // `BLOCK`, the slot of naming.md test 84 cut into its top, and a face sketch on the top as a pick stored it.
    let run = Run::example("sketch/on-split-face.ptsm");

    run.assert_quiet("f-sk2");
    assert_sketch_on_top_plane(&run, "f-sk2", [0.0, 0.0, 30.0]);
}

#[test]
fn t026_faces_in_two_planes_use_the_saved_position_and_offer_both() {
    let [box_1, box_2] = l_shaped_body("b-1");
    let run = Run::records(
        &["b-1"],
        &[
            &box_1,
            &box_2,
            &format!(
                r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "face", "body": "b-1", "face": {BOTH_TOPS}, "fallback": {}}}, "entities": {RECTANGLE_ENTITIES}}}"#,
                top_fallback([0.0, 0.0, 10.0])
            ),
        ],
    );

    assert_eq!(
        plane_fallback_cause(&run),
        (FallbackCause::NotCoplanar, true)
    );
    assert_eq!(
        run.messages("f-sk1"),
        [
            "The face this sketch is on, the top of 'Box 1', now matches 2 faces that do not lie in one plane, so the sketch uses the position saved with it. Pick the one you mean."
        ]
    );
    // Neither face is chosen by name order: both are offered, then Pick.
    assert_eq!(
        plane_fallback_fixes(&run),
        [
            attach("f-sk1", run.picked_before("f-sk1", "b-1", "f-box1/top")),
            attach("f-sk1", run.picked_before("f-sk1", "b-1", "f-box2/top")),
            pick_face()
        ]
    );
    assert_eq!(
        sketch_result(&run, "f-sk1").placement,
        fallback([0.0, 0.0, 10.0])
    );
}

#[test]
fn t027_a_sketch_on_a_pattern_copy_follows_the_spacing() {
    let example = Run::example("sketch/on-pattern-copy.ptsm");
    let mut wider = example.document.clone();
    feature_record_mut(&mut wider, "f-lp1").params["direction"]["spacing"] = json!("80");

    for (run, offset) in [(example, 60.0), (Run::new(wider), 80.0)] {
        run.assert_all_ok();
        assert_sketch_on_top_plane(&run, "f-sk2", [offset, 0.0, 10.0]);
        assert_translation(&sketch_result(&run, "f-sk2").motion, [offset, 0.0, 0.0]);
        assert_bore_through(&run, [offset + 5.0, 5.0, 0.0]);
        assert_volume(run.shape("b-1"), 7874.336294);
    }
}

#[test]
fn t028_a_shell_inserted_under_the_sketch_keeps_it_in_the_plane() {
    // Corpus R34.
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["200", "200", "30"], "op": "new"}"#,
            r#"{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=40000|adj=4,4"}]}"#,
            &face_sketch(
                r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=40000|adj=4,4"}"#,
                [0.0, 0.0, 30.0],
                None,
                &one_circle("e-1", [0.0, 0.0], 10.0),
            ),
            &extrude(
                "f-ext2",
                "f-sk2",
                ("e-1", "c=0,0|a=314.159265"),
                "10",
                "",
                "join",
            ),
        ],
    );

    run.assert_followed("f-sk2", Follow::Continued, "/plane/face");
    assert_eq!(
        run.messages("f-sk2"),
        [
            "The top of 'Box 1' selected for Plane was removed by 'Shell 1', which left its rim in the same place, so the rim is used."
        ]
    );
    let placement = &sketch_result(&run, "f-sk2").placement;
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 30.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
    // The boss spans z 30..40, never on the tray floor at z = 2, so it stands apart over the tray's opening.
    assert_top_at(&run, "b-1", 40.0);
    run.assert_warnings("f-ext2", &["separate_solid"]);
    assert_eq!(run.messages("f-ext2"), [BOSS_APART_TEXT]);
}

/// The document of test 29 (corpus R57): a ring revolved with `extent`, a sketch on its start face stored when it
/// turned 270 degrees, and a cut into the ring from it.
fn revolved_ring_document(extent: &str) -> partsmith_model::Document {
    document(
        &["b-1"],
        &[
            r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XZ"}, "entities": [{"id": "e-1", "type": "line", "start": [10, 0], "end": [20, 0]}, {"id": "e-2", "type": "line", "start": [20, 0], "end": [20, 10]}, {"id": "e-3", "type": "line", "start": [20, 10], "end": [10, 10]}, {"id": "e-4", "type": "line", "start": [10, 10], "end": [10, 0]}]}"#,
            &format!(
                r#"{{"id": "f-rev1", "type": "revolve", "body": "b-1", "profile": {{"sketch": "f-sk1", "regions": [{{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,5|a=100"}}]}}, "axis": {{"kind": "origin", "axis": "Z"}}, "extent": {extent}, "op": "new"}}"#
            ),
            &face_sketch_with(
                r#"{"q": "f-rev1/start-cap", "fp": "plane|n=0,-1,0|c=15,0,5|a=100|adj=4,4"}"#,
                r#"{"origin": [15, 0, 5], "x_axis": [1, 0, 0], "normal": [0, -1, 0]}"#,
                None,
                &one_circle("e-11", [0.0, 0.0], 2.0),
            ),
            &extrude(
                "f-ext2",
                "f-sk2",
                ("e-11", "c=0,0|a=12.566371"),
                "3",
                r#", "reverse": true"#,
                "cut",
            ),
        ],
    )
}

/// Asserts the placement of test 29's sketch on the ring's start face: origin (15, 0, 5), normal -Y.
#[track_caller]
fn assert_on_ring_start(run: &Run) {
    let placement = &sketch_result(run, "f-sk2").placement;
    assert_coordinates("origin", placement.origin, [15.0, 0.0, 5.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, -1.0, 0.0], AXIS_TOLERANCE);
}

#[test]
fn t029_a_revolve_that_turns_all_the_way_round_leaves_the_saved_position() {
    let run = Run::new(revolved_ring_document(r#"{"kind": "full"}"#));

    // No state of b-1 has the start face: 'Revolve 1' itself no longer makes it.
    assert_on_ring_start(&run);
    assert_eq!(
        plane_fallback_of(&run, "f-sk2"),
        (Placed::Saved, FallbackCause::Missing, true)
    );
    assert_eq!(
        run.messages("f-sk2"),
        [
            "The face this sketch is on, the front side of 'Revolve 1', is no longer made by 'Revolve 1', which turns all the way round, so the sketch uses the position saved with it."
        ]
    );
    // The round faces come first among the candidates but cannot hold a sketch, so they give no `Attach`.
    assert_eq!(
        message(&run, "f-sk2", "plane_fallback").fixes,
        [
            attach(
                "f-sk2",
                run.picked_before("f-sk2", "b-1", "f-rev1/lateral/e-1")
            ),
            attach(
                "f-sk2",
                run.picked_before("f-sk2", "b-1", "f-rev1/lateral/e-3")
            ),
            pick_face(),
        ]
    );
    // The full ring has no start face to open the cut, so the cut lies inside the material and seals a hollow
    // (features.md section 3.5, `ClosedVoid`; features/sketch.md test 29).
    run.assert_warnings("f-ext2", &["closed_void"]);
}

#[test]
fn t029_the_same_sketch_on_a_partial_turn_is_on_its_face() {
    let run = Run::new(revolved_ring_document(
        r#"{"kind": "angle", "expr": "270 deg"}"#,
    ));

    run.assert_all_ok();
    assert_on_ring_start(&run);
}

#[test]
fn t030_a_dimension_beyond_the_modeling_limit() {
    let run = Run::records(&[], &[&line_dimension("1e200", &[])]);

    let error = run.assert_error("f-sk1", "out_of_range");
    assert_error_field(error, "constraints[2].expr");
    let FeatureError::OutOfRange { bound, .. } = error else {
        panic!("expected OutOfRange, got {error:?}");
    };
    assert_eq!(*bound, Bound::AtMost(1_000_000.0));
    assert_eq!(
        run.messages("f-sk1"),
        ["Dimension must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."]
    );
}

#[test]
fn t030_the_dimensions_own_bound_comes_before_the_modeling_limit() {
    let run = Run::records(&[], &[&line_dimension("-1e200", &[])]);

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
fn t031_values_travel_with_sketch_errors() {
    let sketch = line_dimension(
        "width",
        &[r#"{"id": "c-4", "type": "distance", "line": "e-1", "expr": "50 mm"}"#],
    );
    let run = run_with_width(&[], &[&sketch]);

    let error = run.assert_error("f-sk1", "sketch_conflict");
    let FeatureError::SketchConflict { values, .. } = error else {
        panic!("expected a sketch conflict, got {error:?}");
    };
    assert_eq!(values.get(&id("c-3")), Some(&100.0));
    assert_eq!(values.get(&id("c-4")), Some(&50.0));
    let text = run.messages("f-sk1")[0];
    assert!(
        text.starts_with("In 'Sketch 1', Distance 50 mm cannot hold together with"),
        "{text}"
    );
    assert!(text.contains("Distance 100 mm (width)"), "{text}");
}

#[test]
fn t032_a_face_taken_by_its_position_among_pieces_is_a_guess() {
    // Test 25's document with the left piece's name, without anchors, under the right piece's shape.
    let mut guessed = load_example("sketch/on-split-face.ptsm");
    feature_record_mut(&mut guessed, "f-sk2").params["plane"]["face"] = json!({
        "q": "f-ext1/end-cap#1",
        "fp": "plane|n=0,0,1|c=65,25,30|a=1500|adj=4,4"
    });

    let run = Run::new(guessed);

    run.assert_warnings("f-sk2", &["reference_guessed"]);
    assert_eq!(
        message(&run, "f-sk2", "reference_guessed")
            .pointer
            .as_deref(),
        Some("/plane/face")
    );
    let warning = run.warning("f-sk2", "reference_guessed");
    let FeatureWarning::ReferenceGuessed { alternatives, .. } = warning else {
        panic!("expected a guess, got {warning:?}");
    };
    let alternatives: Vec<&str> = alternatives
        .iter()
        .map(|alternative| alternative.reference.query.as_str())
        .collect();
    assert_eq!(alternatives, ["f-ext1/end-cap#2"]);
    // Every piece of a flat face lies in its plane, so the placement is test 25's.
    assert_sketch_on_top_plane(&run, "f-sk2", [0.0, 0.0, 30.0]);
}

#[test]
fn a_sketch_on_a_datum_takes_the_datums_motion() {
    // Spec section 3.3 step 1: the sketch takes the datum's placement and motion (features/datum_plane.md test 19).
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}"#,
            &turn("b-1", "X", "180 deg"),
            r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}}}"#,
            &sketch_on_datum("f-sk2", "f-dp1", &one_point([1.0, 2.0])),
        ],
    );

    run.assert_all_ok();
    let sketch = sketch_result(&run, "f-sk2");
    // The rotation of 180 degrees about X: (x, y, z) to (x, -y, -z).
    assert_moves(&sketch.motion, &[([1.0, 2.0, 3.0], [1.0, -2.0, -3.0])]);
    assert_moves(
        datum_motion(&run, "f-dp1"),
        &[([1.0, 2.0, 3.0], [1.0, -2.0, -3.0])],
    );
}

#[test]
fn a_face_sketch_in_body_coordinates_takes_the_motion_of_its_face() {
    // Spec section 3.3 step 1: the motion of a face sketch is that of the stored query in the state before it.
    let run = Run::new(moved_box_document(MOVE_BY_100, Some("body")));

    assert_eq!(
        sketch_result(&run, "f-sk2").motion,
        face_motion_before(&run, "f-sk2", "b-1", "f-box1/top")
    );
}

#[test]
fn t033_on_yz_places_the_sketch_on_the_yz_plane() {
    let run = Run::example("sketch/rectangle-yz.ptsm");

    run.assert_all_quiet();
    let sketch = sketch_result(&run, "f-sk1");
    assert_placement(
        &sketch.placement,
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        sketch.placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
    assert_coordinates(
        "to_world([10, 20])",
        sketch.to_world([10.0, 20.0]),
        [0.0, 10.0, 20.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t033_on_yz_has_the_region_of_test_1_and_no_body_changes() {
    let run = Run::example("sketch/rectangle-yz.ptsm");

    let regions = &sketch_result(&run, "f-sk1").profile.regions;
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

// Tests 34 to 51: the constraint, drawing tool and record form examples (spec section 6, before test 34).

/// The stems of the examples of tests 34 to 51 under `examples/sketch/`, in test order.
const CONSTRAINT_AND_TOOL_EXAMPLES: [&str; 18] = [
    "parallel",
    "perpendicular",
    "collinear",
    "concentric",
    "equal",
    "symmetric",
    "distance-points",
    "distance-point-line",
    "horizontal-vertical-distance",
    "radius",
    "tool-rectangle",
    "tool-center-rectangle",
    "tool-circle-point-snap",
    "tool-arc",
    "tool-tangent",
    "tool-angle",
    "tool-diameter",
    "point-and-line-forms",
];

/// Runs the example `examples/sketch/<stem>.ptsm`.
fn run_sketch_example(stem: &str) -> Run {
    Run::example(&format!("sketch/{stem}.ptsm"))
}

/// The params `f-sk1` stores in the run's document.
fn stored_sketch(run: &Run) -> SketchParams {
    let record = run.record("f-sk1");
    serde_json::from_value(Value::Object(record.params.clone()))
        .unwrap_or_else(|error| panic!("the params of f-sk1 are a sketch's: {error}"))
}

/// The entity `entity` of `entities`.
fn entity_of<'a>(entities: &'a [SketchEntity], entity: &str) -> &'a SketchEntity {
    entities
        .iter()
        .find(|candidate| candidate.id.as_str() == entity)
        .unwrap_or_else(|| panic!("the sketch has no entity {entity}"))
}

/// Every number of an entity's geometry as stored: a point's position, a line's start and end, a circle's center
/// and radius, an arc's center, radius and angles.
fn geometry_numbers(entity: &SketchEntity) -> Vec<f64> {
    match &entity.geometry {
        Geometry::Point { position } => position.to_vec(),
        Geometry::Line { start, end } => vec![start[0], start[1], end[0], end[1]],
        Geometry::Circle { center, radius } => vec![center[0], center[1], *radius],
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => vec![center[0], center[1], *radius, *start_angle, *end_angle],
    }
}

/// The radius of a circle or arc.
fn radius_of(entity: &SketchEntity) -> f64 {
    match &entity.geometry {
        Geometry::Circle { radius, .. } | Geometry::Arc { radius, .. } => *radius,
        geometry => panic!("{} has no radius: {geometry:?}", entity.id),
    }
}

/// Every coordinate and radius of an entity: its points (sketch.md section 2.4, in role order) and, for a circle or
/// arc, its radius.
fn coordinates_and_radius(entity: &SketchEntity) -> Vec<f64> {
    let mut numbers: Vec<f64> = [
        PointRole::Point,
        PointRole::Start,
        PointRole::End,
        PointRole::Center,
    ]
    .into_iter()
    .filter_map(|role| entity.point(role))
    .flatten()
    .collect();
    if let Geometry::Circle { radius, .. } | Geometry::Arc { radius, .. } = entity.geometry {
        numbers.push(radius);
    }
    numbers
}

/// Asserts that `solved` and `stored` are the same entity: id, kind and construction flag.
fn assert_same_entity(solved: &SketchEntity, stored: &SketchEntity) {
    assert_eq!(solved.id, stored.id, "entity id");
    assert_eq!(solved.kind(), stored.kind(), "kind of {}", stored.id);
    assert_eq!(
        solved.construction, stored.construction,
        "construction flag of {}",
        stored.id
    );
}

/// Asserts that `solved` is `stored` bitwise ("as stored", spec section 6 before test 34).
fn assert_entity_as_stored(solved: &SketchEntity, stored: &SketchEntity) {
    assert_same_entity(solved, stored);
    let bits = |entity: &SketchEntity| -> Vec<u64> {
        geometry_numbers(entity)
            .into_iter()
            .map(f64::to_bits)
            .collect()
    };
    assert_eq!(
        bits(solved),
        bits(stored),
        "{} is not as stored: solved {:?}, stored {:?}",
        stored.id,
        solved.geometry,
        stored.geometry,
    );
}

/// Asserts that every coordinate and radius of `solved` is within 1e-9 mm of `stored` ("as drawn", spec section 6
/// before test 34).
fn assert_entity_as_drawn(solved: &SketchEntity, stored: &SketchEntity) {
    assert_same_entity(solved, stored);
    let solved_numbers = coordinates_and_radius(solved);
    let stored_numbers = coordinates_and_radius(stored);
    assert_eq!(
        solved_numbers.len(),
        stored_numbers.len(),
        "coordinates of {}",
        stored.id
    );
    for (solved_number, stored_number) in solved_numbers.into_iter().zip(stored_numbers) {
        assert_absolute(
            &format!(
                "{} as drawn: solved {:?}, stored {:?}",
                stored.id, solved.geometry, stored.geometry
            ),
            solved_number,
            stored_number,
            AXIS_TOLERANCE,
        );
    }
}

/// Asserts that these entities of `f-sk1` are solved as stored.
fn assert_as_stored(run: &Run, entities: &[&str]) {
    let solved = &sketch_result(run, "f-sk1").solved.entities;
    let stored = stored_sketch(run).entities;
    for entity in entities {
        assert_entity_as_stored(entity_of(solved, entity), entity_of(&stored, entity));
    }
}

/// Asserts that every entity of `f-sk1` is solved as stored, in the stored order.
fn assert_all_as_stored(run: &Run) {
    let solved = &sketch_result(run, "f-sk1").solved.entities;
    let stored = stored_sketch(run).entities;
    assert_eq!(solved.len(), stored.len(), "entity count");
    for (solved_entity, stored_entity) in solved.iter().zip(&stored) {
        assert_entity_as_stored(solved_entity, stored_entity);
    }
}

/// Asserts that these entities of `f-sk1` are solved as drawn.
fn assert_as_drawn(run: &Run, entities: &[&str]) {
    let solved = &sketch_result(run, "f-sk1").solved.entities;
    let stored = stored_sketch(run).entities;
    for entity in entities {
        assert_entity_as_drawn(entity_of(solved, entity), entity_of(&stored, entity));
    }
}

/// Asserts the status and dof of `f-sk1`'s solve, with no redundant or conflicting constraint and no failed
/// component.
fn assert_solved(run: &Run, status: SketchStatus, dof: u32) {
    let solved = &sketch_result(run, "f-sk1").solved;
    assert_eq!(solved.status, status, "sketch status");
    assert_eq!(solved.dof, dof, "remaining degrees of freedom");
    assert!(
        solved.redundant.is_empty(),
        "redundant constraints {:?}",
        solved.redundant
    );
    assert!(
        solved.conflicting.is_empty(),
        "conflicting constraints {:?}",
        solved.conflicting
    );
    assert!(!solved.failed, "every component converged");
}

/// Asserts solved positions of points of `f-sk1` (within 1e-9 mm).
fn assert_points(run: &Run, expected: &[(&str, [f64; 2])]) {
    for (point, position) in expected {
        assert_coordinates(
            point,
            solved_point(run, "f-sk1", point),
            *position,
            AXIS_TOLERANCE,
        );
    }
}

/// Asserts the solved radius of a circle or arc of `f-sk1` (within 1e-9 mm).
fn assert_radius(run: &Run, entity: &str, expected: f64) {
    let solved = &sketch_result(run, "f-sk1").solved.entities;
    assert_absolute(
        &format!("radius of {entity}"),
        radius_of(entity_of(solved, entity)),
        expected,
        AXIS_TOLERANCE,
    );
}

/// The measured value of the dimension `constraint` of `f-sk1` (mm or rad).
fn measured(run: &Run, constraint: &str) -> f64 {
    *sketch_result(run, "f-sk1")
        .solved
        .measured
        .get(&id(constraint))
        .unwrap_or_else(|| panic!("{constraint} has no measured value"))
}

/// The keys of the regions of `f-sk1`'s profile, in profile order.
fn region_keys(run: &Run) -> Vec<String> {
    sketch_result(run, "f-sk1")
        .profile
        .regions
        .iter()
        .map(|region| region.key.to_string())
        .collect()
}

/// The unit vector of `vector`.
fn unit(vector: [f64; 2]) -> [f64; 2] {
    let length = vector[0].hypot(vector[1]);
    [vector[0] / length, vector[1] / length]
}

/// The cosine of the angle between the line `line` and the radius of the arc `arc` through the point `touch`: a
/// tangent with a touch point holds where it is 0 (sketch.md sections 3.6.4 and 3.6.5).
fn tangent_cosine(run: &Run, line: &str, arc: &str, touch: &str) -> f64 {
    let start = solved_point(run, "f-sk1", &format!("{line}.start"));
    let end = solved_point(run, "f-sk1", &format!("{line}.end"));
    let center = solved_point(run, "f-sk1", &format!("{arc}.center"));
    let touch_point = solved_point(run, "f-sk1", touch);
    let along = unit([end[0] - start[0], end[1] - start[1]]);
    let radial = unit([touch_point[0] - center[0], touch_point[1] - center[1]]);
    along[0] * radial[0] + along[1] * radial[1]
}

#[test]
fn examples_of_tests_34_to_51_store_the_region_as_a_pick_of_the_solved_sketch() {
    // `EXT10` (spec section 6, before test 34): the region as `Region::reference()` writes it.
    for stem in CONSTRAINT_AND_TOOL_EXAMPLES {
        let run = run_sketch_example(stem);

        let RegionMatch::Resolved {
            indices, method, ..
        } = stored_region_match(&run, "f-ext1", "f-sk1")
        else {
            panic!("{stem}: the stored region resolves");
        };
        assert_eq!(method, MatchMethod::Key, "{stem}: match by key");
        let [index] = indices[..] else {
            panic!("{stem}: the stored region is one region, not {indices:?}");
        };
        // Compared as written: a fingerprint's text rounds its numbers (sketch.md section 3.13.6).
        let picked = sketch_result(&run, "f-sk1").profile.regions[index].reference();
        assert_eq!(
            serde_json::to_value(stored_region(&run, "f-ext1")).expect("a region serializes"),
            serde_json::to_value(picked).expect("a region serializes"),
            "{stem}: stored as picked",
        );
    }
}

#[test]
fn t034_parallel_lays_the_top_parallel_to_the_bottom() {
    let run = run_sketch_example("parallel");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_as_drawn(&run, &["e-1", "e-4"]);
    assert_points(
        &run,
        &[("e-2.end", [60.0, 40.0]), ("e-3.start", [60.0, 40.0])],
    );
}

#[test]
fn t034_parallel_extrudes_a_60_by_40_block() {
    let run = run_sketch_example("parallel");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        24000.0,
        6,
        [0.0, 0.0, 0.0],
        [60.0, 40.0, 10.0],
    );
}

#[test]
fn t035_perpendicular_stands_the_side_square_to_the_bottom() {
    let run = run_sketch_example("perpendicular");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_as_drawn(&run, &["e-1", "e-4"]);
    assert_points(
        &run,
        &[("e-2.start", [60.0, 0.0]), ("e-2.end", [60.0, 40.0])],
    );
}

#[test]
fn t035_perpendicular_extrudes_a_60_by_40_block() {
    let run = run_sketch_example("perpendicular");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        24000.0,
        6,
        [0.0, 0.0, 0.0],
        [60.0, 40.0, 10.0],
    );
}

#[test]
fn t036_collinear_with_the_x_axis_moves_only_the_ends_of_the_bottom_down() {
    let run = run_sketch_example("collinear");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 6);
    // The minimum-norm correction moves the two ends of e-1 straight down and changes no other coordinate.
    assert_points(
        &run,
        &[
            ("e-1.start", [0.0, 0.0]),
            ("e-1.end", [60.0, 0.0]),
            ("e-2.start", [60.0, 0.0]),
            ("e-2.end", [60.0, 40.0]),
            ("e-4.start", [0.0, 40.0]),
            ("e-4.end", [0.0, 0.0]),
        ],
    );
    // Every class of e-3's parameters is bitwise equal and in no remaining row (sketch.md section 3.7.3).
    assert_as_stored(&run, &["e-3"]);
}

#[test]
fn t036_collinear_extrudes_a_60_by_40_block() {
    let run = run_sketch_example("collinear");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        24000.0,
        6,
        [0.0, 0.0, 0.0],
        [60.0, 40.0, 10.0],
    );
}

#[test]
fn t037_concentric_on_the_origin_centers_both_circles_exactly_there() {
    let run = run_sketch_example("concentric");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 2);
    // The coincident with the origin freezes e-1's center at 0 and the concentric merges e-2's center into it.
    assert_as_stored(&run, &["e-1"]);
    assert_eq!(solved_point(&run, "f-sk1", "e-2.center"), [0.0, 0.0]);
    assert_radius(&run, "e-1", 20.0);
    assert_radius(&run, "e-2", 8.0);
    assert_eq!(region_keys(&run), ["e-1;~e-2", "e-2"]);
}

#[test]
fn t037_concentric_extrudes_a_ring_around_the_z_axis() {
    let run = run_sketch_example("concentric");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        10555.751316,
        4,
        [-20.0, -20.0, 0.0],
        [20.0, 20.0, 10.0],
    );
    let side = assert_round_face(
        run.body("b-1"),
        "f-ext1/lateral/e-2",
        GeometryKind::Cylinder,
        8.0,
    );
    assert_face_axis_through(&side, [0.0, 0.0, 0.0]);
}

#[test]
fn t038_equal_makes_the_side_as_long_as_the_bottom() {
    let run = run_sketch_example("equal");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_as_drawn(&run, &["e-1"]);
    assert_points(&run, &[("e-2.end", [60.0, 60.0])]);
}

#[test]
fn t038_equal_extrudes_a_square_block() {
    let run = run_sketch_example("equal");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        36000.0,
        6,
        [0.0, 0.0, 0.0],
        [60.0, 60.0, 10.0],
    );
}

#[test]
fn t039_symmetric_about_the_y_axis_centers_the_base() {
    let run = run_sketch_example("symmetric");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 4);
    // The minimum-norm correction moves both ends of the base 1 mm left; the apex stays.
    assert_points(
        &run,
        &[
            ("e-1.start", [-20.0, 0.0]),
            ("e-1.end", [20.0, 0.0]),
            ("e-2.end", [0.0, 30.0]),
        ],
    );
}

#[test]
fn t039_symmetric_extrudes_a_triangle_centered_on_the_y_axis() {
    let run = run_sketch_example("symmetric");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        6000.0,
        5,
        [-20.0, 0.0, 0.0],
        [20.0, 30.0, 10.0],
    );
}

#[test]
fn t040_distance_between_two_points_sets_the_diagonal() {
    let run = run_sketch_example("distance-points");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_as_drawn(&run, &["e-1"]);
    assert_points(&run, &[("e-2.end", [60.0, 45.0])]);
    assert_absolute(
        "measured c-11",
        measured(&run, "c-11"),
        75.0,
        AXIS_TOLERANCE,
    );
}

#[test]
fn t040_distance_between_two_points_extrudes_a_60_by_45_block() {
    let run = run_sketch_example("distance-points");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        27000.0,
        6,
        [0.0, 0.0, 0.0],
        [60.0, 45.0, 10.0],
    );
}

#[test]
fn t041_distance_from_a_point_to_a_line_keeps_the_side_it_was_drawn_on() {
    let run = run_sketch_example("distance-point-line");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 2);
    assert_as_drawn(&run, &["e-1", "e-2", "e-3", "e-4"]);
    // 10 mm below the top edge, on the side the hole was drawn on.
    assert_points(&run, &[("e-5.center", [30.0, 30.0])]);
    assert_radius(&run, "e-5", 5.0);
}

#[test]
fn t041_distance_from_a_point_to_a_line_extrudes_a_plate_with_a_hole() {
    let run = run_sketch_example("distance-point-line");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        23214.601837,
        7,
        [0.0, 0.0, 0.0],
        [60.0, 40.0, 10.0],
    );
    let side = assert_round_face(
        run.body("b-1"),
        "f-ext1/lateral/e-5",
        GeometryKind::Cylinder,
        5.0,
    );
    assert_face_axis_through(&side, [30.0, 30.0, 0.0]);
}

#[test]
fn t042_signed_horizontal_and_vertical_distances_place_the_circle_right_of_and_below_the_origin() {
    let run = run_sketch_example("horizontal-vertical-distance");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 1);
    // u(center) - u(origin) = 20 and v(origin) - v(center) = 15.
    assert_points(&run, &[("e-1.center", [20.0, -15.0])]);
    assert_radius(&run, "e-1", 5.0);
}

#[test]
fn t042_horizontal_and_vertical_distance_extrude_the_circle_where_they_put_it() {
    let run = run_sketch_example("horizontal-vertical-distance");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        785.398163,
        3,
        [15.0, -20.0, 0.0],
        [25.0, -10.0, 10.0],
    );
}

#[test]
fn t043_radius_of_an_arc_over_a_40_mm_chord_rises_10_mm() {
    let run = run_sketch_example("radius");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_as_drawn(&run, &["e-1"]);
    assert_points(
        &run,
        &[
            ("e-2.center", [20.0, -15.0]),
            ("e-2.start", [40.0, 0.0]),
            ("e-2.end", [0.0, 0.0]),
        ],
    );
    assert_radius(&run, "e-2", 25.0);
}

#[test]
fn t043_radius_extrudes_the_segment_under_the_arc() {
    let run = run_sketch_example("radius");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        2795.595113,
        4,
        [0.0, 0.0, 0.0],
        [40.0, 10.0, 10.0],
    );
}

#[test]
fn t044_the_2_point_rectangle_tool_output_comes_back_as_stored() {
    let run = run_sketch_example("tool-rectangle");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_all_as_stored(&run);
}

#[test]
fn t044_the_2_point_rectangle_tool_output_extrudes_a_60_by_40_block() {
    let run = run_sketch_example("tool-rectangle");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        24000.0,
        6,
        [0.0, 0.0, 0.0],
        [60.0, 40.0, 10.0],
    );
}

#[test]
fn t045_the_center_rectangle_tool_output_comes_back_as_stored() {
    let run = run_sketch_example("tool-center-rectangle");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_all_as_stored(&run);
    // Construction geometry is in no profile.
    assert_eq!(region_keys(&run), ["e-1,e-2,e-3,e-4"]);
}

#[test]
fn t045_the_center_rectangle_tool_output_extrudes_a_block_centered_on_the_origin() {
    let run = run_sketch_example("tool-center-rectangle");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        24000.0,
        6,
        [-30.0, -20.0, 0.0],
        [30.0, 20.0, 10.0],
    );
}

#[test]
fn t046_the_circle_tool_output_with_a_point_snap_comes_back_as_stored() {
    let run = run_sketch_example("tool-circle-point-snap");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_all_as_stored(&run);
    // The point is in no region.
    assert_eq!(region_keys(&run), ["e-2"]);
}

#[test]
fn t046_the_circle_tool_output_extrudes_a_disc_of_radius_13() {
    let run = run_sketch_example("tool-circle-point-snap");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        5309.291585,
        3,
        [-13.0, -13.0, 0.0],
        [13.0, 13.0, 10.0],
    );
}

#[test]
fn t047_the_3_point_arc_tool_output_comes_back_as_stored() {
    let run = run_sketch_example("tool-arc");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 1);
    assert_all_as_stored(&run);
    // The region's counterclockwise boundary follows the line and the arc in their own directions.
    assert_eq!(region_keys(&run), ["e-1,e-2"]);
}

#[test]
fn t047_the_3_point_arc_tool_output_extrudes_a_half_disc() {
    let run = run_sketch_example("tool-arc");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        6283.185307,
        4,
        [0.0, 0.0, 0.0],
        [40.0, 20.0, 10.0],
    );
}

#[test]
fn t048_tangent_inference_output_comes_back_as_stored() {
    let run = run_sketch_example("tool-tangent");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::UnderConstrained, 2);
    assert_all_as_stored(&run);
}

#[test]
fn t048_both_inferred_tangents_hold_at_their_touch_points() {
    let run = run_sketch_example("tool-tangent");

    // c-5 touches at e-1.end, identified with e-2.start; c-8 at e-3.start, identified with e-2.end.
    for (line, touch) in [("e-1", "e-1.end"), ("e-3", "e-3.start")] {
        assert_absolute(
            &format!("cosine between {line} and the radius at {touch}"),
            tangent_cosine(&run, line, "e-2", touch),
            0.0,
            AXIS_TOLERANCE,
        );
    }
}

#[test]
fn t048_tangent_inference_output_extrudes_a_block_with_a_round_end() {
    let run = run_sketch_example("tool-tangent");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        9570.796327,
        6,
        [0.0, 0.0, 0.0],
        [50.0, 20.0, 10.0],
    );
}

#[test]
fn t049_the_typed_angle_output_comes_back_as_stored() {
    let run = run_sketch_example("tool-angle");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_all_as_stored(&run);
    assert_absolute("measured c-2", measured(&run, "c-2"), 60.0, AXIS_TOLERANCE);
    assert_absolute(
        "measured c-3",
        measured(&run, "c-3"),
        FRAC_PI_6,
        AXIS_TOLERANCE,
    );
}

#[test]
fn t049_the_typed_angle_output_extrudes_a_right_triangle() {
    let run = run_sketch_example("tool-angle");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        7794.228634,
        5,
        [0.0, 0.0, 0.0],
        [51.961524, 30.0, 10.0],
    );
}

#[test]
fn t050_the_typed_diameter_output_comes_back_as_stored() {
    let run = run_sketch_example("tool-diameter");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    assert_all_as_stored(&run);
    assert_absolute("measured c-2", measured(&run, "c-2"), 25.0, AXIS_TOLERANCE);
}

#[test]
fn t050_the_typed_diameter_output_extrudes_a_25_mm_disc() {
    let run = run_sketch_example("tool-diameter");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        4908.738521,
        3,
        [-12.5, -12.5, 0.0],
        [12.5, 12.5, 10.0],
    );
}

#[test]
fn t051_points_forms_and_line_forms_of_distances_move_every_line_onto_a_40_by_25_rectangle() {
    let run = run_sketch_example("point-and-line-forms");

    run.assert_all_quiet();
    assert_solved(&run, SketchStatus::FullyConstrained, 0);
    // Each line keeps the direction it was drawn in: its start and end land on these corners.
    assert_points(
        &run,
        &[
            ("e-1.start", [0.0, 0.0]),
            ("e-1.end", [40.0, 0.0]),
            ("e-2.start", [40.0, 0.0]),
            ("e-2.end", [40.0, 25.0]),
            ("e-3.start", [40.0, 25.0]),
            ("e-3.end", [0.0, 25.0]),
            ("e-4.start", [0.0, 25.0]),
            ("e-4.end", [0.0, 0.0]),
        ],
    );
}

#[test]
fn t051_points_forms_and_line_forms_of_distances_extrude_a_40_by_25_block() {
    let run = run_sketch_example("point-and-line-forms");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        10000.0,
        6,
        [0.0, 0.0, 0.0],
        [40.0, 25.0, 10.0],
    );
}
