//! Acceptance tests of the `extrude` feature (docs/spec/features/extrude.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/extrude/`.

use crate::feature_support;

use feature_support::{
    AXIS_TOLERANCE, BOX_BELOW, Run, absent_faces, assert_bounding_box, assert_coordinates,
    assert_empty, assert_error_field, assert_examples_canonical, assert_face_area,
    assert_face_axis_direction, assert_face_axis_through, assert_face_count, assert_face_in_plane,
    assert_face_kind, assert_face_names, assert_face_normal, assert_face_radius, assert_flat_face,
    assert_has_face, assert_messages_pass_lint, assert_no_face, assert_solid_count,
    assert_unchanged, assert_volume, assert_volume_faces_and_bounds, body_update, document_text,
    element_names, face, face_names_with_prefix, id, keep_and_pick, line, message, output_body,
    parse_document, pick, reference_json, set_field, set_reverse, solved_point,
    stored_region_match, successions, top_fallback, use_reference,
};
use partsmith_features::{
    BodyChange, Bound, Diagnosis, FeatureError, FeatureWarning, Fix, NoEffectHow,
};
use partsmith_naming::{ElementKind, Follow};
use partsmith_occt_sys::GeometryKind;
use partsmith_sketch::{MatchMethod, RegionLoss, RegionMatch};

/// `SK-RECT`: `RECT(0, 0, 80, 50)` on XY with id `f-sk1`.
const RECTANGLE_SKETCH: &str = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}]}"#;

/// `P-RECT`: the region of `SK-RECT`.
const RECTANGLE_PROFILE: &str =
    r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}"#;

/// The extent that goes through the whole target body.
const THROUGH_ALL: &str = r#"{"kind": "through_all"}"#;

/// `SK-CIRCLE(cx, cy, r)`: the circle `e-5` on XY with id `f-sk1`.
fn circle_sketch(center_x: i32, center_y: i32, radius: i32) -> String {
    format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{{"id": "e-5", "type": "circle", "center": [{center_x}, {center_y}], "radius": {radius}}}]}}"#
    )
}

/// `P-CIRCLE(fp)`: the region of the circle `e-5` with this fingerprint.
fn circle_profile(fingerprint: &str) -> String {
    format!(r#"{{"sketch": "f-sk1", "regions": [{{"key": "e-5", "fp": "{fingerprint}"}}]}}"#)
}

/// A distance extent.
fn distance(expr: &str) -> String {
    format!(r#"{{"kind": "distance", "expr": "{expr}"}}"#)
}

/// A two sides extent.
fn two_sides(expr: &str, opposite_expr: &str) -> String {
    format!(r#"{{"kind": "two_sides", "expr": "{expr}", "opposite_expr": "{opposite_expr}"}}"#)
}

/// `EXT(profile, extent, op)`: the extrude `f-ext1` into `b-1`.
fn extrude(profile: &str, extent: &str, op: &str) -> String {
    format!(
        r#"{{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {profile}, "extent": {extent}, "op": "{op}"}}"#
    )
}

/// Runs `SK-RECT` then `EXT(P-RECT, extent, "new")` in body `b-1`.
fn run_rectangle(extent: &str) -> Run {
    Run::records(
        &["b-1"],
        &[RECTANGLE_SKETCH, &extrude(RECTANGLE_PROFILE, extent, "new")],
    )
}

/// Asserts the volume and the bounding box of `b-1` at the end of the run.
fn assert_volume_and_bounds(run: &Run, volume: f64, min: [f64; 3], max: [f64; 3]) {
    let shape = run.shape("b-1");
    assert_volume(shape, volume);
    assert_bounding_box(shape, min, max);
}

/// Asserts that the cap `name` of `b-1` lies in the plane z = `position`.
fn assert_cap_at(run: &Run, name: &str, position: f64) {
    assert_face_in_plane(&face(run.body("b-1"), name), 2, position);
}

/// The expectations of test 3, shared by test 4.
fn assert_symmetric_extrusion(run: &Run) {
    run.assert_all_ok();
    assert_volume_and_bounds(run, 120000.0, [0.0, 0.0, -15.0], [80.0, 50.0, 15.0]);
    assert_face_count(run.shape("b-1"), 6);
    assert_cap_at(run, "f-ext1/start-cap", -15.0);
    assert_cap_at(run, "f-ext1/end-cap", 15.0);
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("extrude");
}

#[test]
fn t001_distance_new() {
    let run = Run::example("extrude/distance.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 120000.0, [0.0, 0.0, 0.0], [80.0, 50.0, 30.0]);
    assert_face_count(run.shape("b-1"), 6);
    let body = run.body("b-1");
    let start_cap = face(body, "f-ext1/start-cap");
    assert_face_area(&start_cap, 4000.0);
    assert_face_in_plane(&start_cap, 2, 0.0);
    assert_face_normal(&start_cap, [0.0, 0.0, -1.0]);
    let end_cap = face(body, "f-ext1/end-cap");
    assert_face_area(&end_cap, 4000.0);
    assert_face_in_plane(&end_cap, 2, 30.0);
    for (name, area, axis, position) in [
        ("f-ext1/lateral/e-1", 2400.0, 1, 0.0),
        ("f-ext1/lateral/e-2", 1500.0, 0, 80.0),
        ("f-ext1/lateral/e-3", 2400.0, 1, 50.0),
        ("f-ext1/lateral/e-4", 1500.0, 0, 0.0),
    ] {
        let lateral = face(body, name);
        assert_face_area(&lateral, area);
        assert_face_in_plane(&lateral, axis, position);
    }
    let change = &body_update(&run, "f-ext1", "b-1").change;
    assert!(matches!(change, BodyChange::New), "change {change:?}");
}

#[test]
fn t002_reverse() {
    let run = Run::example("extrude/reverse.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 120000.0, [0.0, 0.0, -30.0], [80.0, 50.0, 0.0]);
    assert_face_count(run.shape("b-1"), 6);
    assert_cap_at(&run, "f-ext1/start-cap", 0.0);
    assert_cap_at(&run, "f-ext1/end-cap", -30.0);
}

#[test]
fn t003_symmetric() {
    let run = Run::example("extrude/symmetric.ptsm");

    assert_symmetric_extrusion(&run);
}

#[test]
fn t004_symmetric_ignores_reverse() {
    let run = Run::example("extrude/symmetric-reverse.ptsm");
    let symmetric = Run::example("extrude/symmetric.ptsm");

    assert_symmetric_extrusion(&run);
    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(
            element_names(run.body("b-1"), kind),
            element_names(symmetric.body("b-1"), kind),
            "{kind:?} names of the reversed symmetric extrusion",
        );
    }
}

#[test]
fn t005_two_sides() {
    let run = Run::example("extrude/two-sides.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 120000.0, [0.0, 0.0, -10.0], [80.0, 50.0, 20.0]);
    assert_cap_at(&run, "f-ext1/start-cap", -10.0);
    assert_cap_at(&run, "f-ext1/end-cap", 20.0);
}

#[test]
fn t006_two_sides_reversed() {
    let run = Run::example("extrude/two-sides-reverse.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, -20.0], [80.0, 50.0, 10.0]);
    assert_cap_at(&run, "f-ext1/start-cap", 10.0);
    assert_cap_at(&run, "f-ext1/end-cap", -20.0);
}

#[test]
fn t007_two_sides_one_side_zero() {
    let run = Run::example("extrude/two-sides-one-zero.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 80000.0, [0.0, 0.0, 0.0], [80.0, 50.0, 20.0]);
}

#[test]
fn t008_two_sides_both_zero() {
    let run = run_rectangle(&two_sides("0", "0"));

    run.assert_error("f-ext1", "both_sides_zero");
    assert_empty(&run, "b-1");
}

#[test]
fn t009_zero_distance() {
    let run = run_rectangle(&distance("0"));

    let error = run.assert_error("f-ext1", "out_of_range");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-ext1"),
        ["Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."],
    );
}

#[test]
fn t010_negative_distance() {
    let run = run_rectangle(&distance("-5"));

    run.assert_error("f-ext1", "out_of_range");
    assert_eq!(
        run.messages("f-ext1"),
        ["Distance must be greater than 0 mm, but it is -5 mm. Use a value greater than 0 mm."],
    );
}

#[test]
fn t011_negative_opposite_distance() {
    let run = run_rectangle(&two_sides("5", "-1"));

    let error = run.assert_error("f-ext1", "out_of_range");
    assert_error_field(error, "extent.opposite_expr");
    assert_eq!(
        run.messages("f-ext1"),
        ["Opposite distance must be at least 0 mm, but it is -1 mm. Use 0 mm or more."],
    );
}

#[test]
fn t012_wrong_dimension() {
    let run = run_rectangle(&distance("30 deg"));

    let error = run.assert_error("f-ext1", "expression");
    assert_error_field(error, "extent.expr");
    assert_eq!(
        run.messages("f-ext1"),
        ["Distance: Expected a length, but this expression gives an angle."],
    );
}

#[test]
fn t013_bare_number_in_inches() {
    let run = Run::example("extrude/inches.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 101600.0, [0.0, 0.0, 0.0], [80.0, 50.0, 25.4]);
}

#[test]
fn t014_parameter() {
    let run = Run::example("extrude/parameter.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 96000.0, [0.0, 0.0, 0.0], [80.0, 50.0, 24.0]);
}

#[test]
fn t015_unknown_name() {
    let run = Run::new(parse_document(&document_text(
        "mm",
        &[r#"{"id": "p-h", "name": "height", "expr": "12 mm"}"#],
        &["b-1"],
        &[
            RECTANGLE_SKETCH,
            &extrude(RECTANGLE_PROFILE, &distance("hieght * 2"), "new"),
        ],
    )));

    let error = run.assert_error("f-ext1", "expression");
    assert_error_field(error, "extent.expr");
    // `height` is the one parameter within edit distance 2 (expressions.md section 3.18).
    assert_eq!(
        run.messages("f-ext1"),
        ["Distance: 'hieght' is not a parameter. Did you mean 'height'?"],
    );
    // Section 4.1: the near match is offered as the expression with the name replaced.
    assert_eq!(
        message(&run, "f-ext1", "expression").fixes,
        [set_field(
            "extent.expr",
            "Distance",
            serde_json::json!("height * 2")
        )]
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t016_circle() {
    let run = Run::example("extrude/circle.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 6283.185307, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    assert_face_count(run.shape("b-1"), 3);
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-ext1/lateral/e-5"), 1256.637061);
    assert_face_area(&face(body, "f-ext1/start-cap"), 314.159265);
    assert_face_area(&face(body, "f-ext1/end-cap"), 314.159265);
}

#[test]
fn t017_region_with_a_hole() {
    let run = Run::example("extrude/region-with-hole.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 36858.407346, [0.0, 0.0, 0.0], [80.0, 50.0, 10.0]);
    assert_face_count(run.shape("b-1"), 7);
    assert_face_area(&face(run.body("b-1"), "f-ext1/lateral/e-5"), 628.318531);
}

#[test]
fn t018_union_of_two_regions() {
    let run = Run::example("extrude/union-of-regions.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 40000.0, [0.0, 0.0, 0.0], [80.0, 50.0, 10.0]);
    assert_face_count(run.shape("b-1"), 6);
    assert_no_face(run.body("b-1"), "f-ext1/lateral/e-5");
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-ext1/start-cap"), 4000.0);
    assert_face_area(&face(body, "f-ext1/end-cap"), 4000.0);
}

#[test]
fn t019_join() {
    let run = Run::example("extrude/join.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 16392.699082, [-20.0, -20.0, -10.0], [20.0, 20.0, 5.0]);
    assert_face_count(run.shape("b-1"), 8);
    assert_has_face(run.body("b-1"), "f-box1/top");
    assert_has_face(run.body("b-1"), "f-ext1/lateral/e-5");
    assert_has_face(run.body("b-1"), "f-ext1/end-cap");
    assert_no_face(run.body("b-1"), "f-ext1/start-cap");
    let change = &body_update(&run, "f-ext1", "b-1").change;
    assert!(matches!(change, BodyChange::Join), "change {change:?}");
}

#[test]
fn t020_pocket_cut() {
    let run = Run::example("extrude/pocket-cut.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 15685.840735, [-20.0, -20.0, -10.0], [20.0, 20.0, 0.0]);
    assert_face_count(run.shape("b-1"), 8);
    assert_has_face(run.body("b-1"), "f-ext1/lateral/e-5");
    assert_cap_at(&run, "f-ext1/end-cap", -4.0);
    assert_no_face(run.body("b-1"), "f-ext1/start-cap");
    let change = &body_update(&run, "f-ext1", "b-1").change;
    assert!(matches!(change, BodyChange::Cut), "change {change:?}");
}

#[test]
fn t021_through_all_cut() {
    let run = Run::example("extrude/through-all-cut.ptsm");

    run.assert_all_ok();
    assert_volume(run.shape("b-1"), 15214.601837);
    assert_face_count(run.shape("b-1"), 7);
    assert_has_face(run.body("b-1"), "f-ext1/lateral/e-5");
    assert_no_face(run.body("b-1"), "f-ext1/start-cap");
    assert_no_face(run.body("b-1"), "f-ext1/end-cap");
}

#[test]
fn t022_through_all_pointing_away() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_BELOW,
            &circle_sketch(0, 0, 5),
            &extrude(&circle_profile("c=0,0|a=78.539816"), THROUGH_ALL, "cut"),
        ],
    );

    run.assert_error("f-ext1", "through_all_misses");
    // Reversed, through all reaches the body, so Turn on Reverse is offered and the sentence does not repeat it.
    assert_eq!(
        run.messages("f-ext1"),
        ["Through all points away from body 'Body 1', so there is nothing to go through."],
    );
    assert_eq!(
        message(&run, "f-ext1", "through_all_misses").fixes,
        [set_reverse(true)]
    );
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t022_through_all_that_misses_either_way_asks_to_point_it_at_the_body() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_BELOW,
            &circle_sketch(100, 0, 5),
            &extrude(&circle_profile("c=100,0|a=78.539816"), THROUGH_ALL, "cut"),
        ],
    );

    run.assert_error("f-ext1", "through_all_misses");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Through all points away from body 'Body 1', so there is nothing to go through. Point it at the body, or use a distance."
        ],
    );
    assert_eq!(message(&run, "f-ext1", "through_all_misses").fixes, []);
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t023_through_all_with_op_new() {
    let run = run_rectangle(THROUGH_ALL);

    run.assert_error("f-ext1", "through_all_needs_body");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Through all goes through an existing solid, and body 'Body 1' has none here. Use a distance instead."
        ],
    );
}

#[test]
fn through_all_into_an_empty_body() {
    // Section 3.3 step 3: through all needs the target's solid, so an empty body is an error for
    // every op that can go through all.
    for op in ["join", "cut", "intersect"] {
        let run = Run::records(
            &["b-1"],
            &[
                RECTANGLE_SKETCH,
                &extrude(RECTANGLE_PROFILE, THROUGH_ALL, op),
            ],
        );

        run.assert_error("f-ext1", "body_empty");
        // features.md section 4.1, `body_empty` with `use_new` and no diagnosis.
        assert_eq!(
            run.messages("f-ext1"),
            [
                "Body 'Body 1' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here."
            ],
            "op {op}",
        );
        assert_empty(&run, "b-1");
    }
}

#[test]
fn t024_intersect() {
    let run = Run::example("extrude/intersect.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 3141.592654, [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
    assert_face_count(run.shape("b-1"), 3);
    let change = &body_update(&run, "f-ext1", "b-1").change;
    assert!(matches!(change, BodyChange::Intersect), "change {change:?}");
}

#[test]
fn t025_intersect_without_overlap() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}"#,
            &circle_sketch(100, 0, 5),
            &extrude(
                &circle_profile("c=100,0|a=78.539816"),
                r#"{"kind": "symmetric", "expr": "100"}"#,
                "intersect",
            ),
        ],
    );

    run.assert_error("f-ext1", "empty_result");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap it."
        ],
    );
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t026_cut_everything() {
    let large_rectangle = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [-50, -50], "end": [50, -50]}, {"id": "e-2", "type": "line", "start": [50, -50], "end": [50, 50]}, {"id": "e-3", "type": "line", "start": [50, 50], "end": [-50, 50]}, {"id": "e-4", "type": "line", "start": [-50, 50], "end": [-50, -50]}]}"#;
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}"#,
            large_rectangle,
            &extrude(
                r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=0,0|a=10000"}]}"#,
                r#"{"kind": "symmetric", "expr": "100"}"#,
                "cut",
            ),
        ],
    );

    run.assert_error("f-ext1", "empty_result");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction."
        ],
    );
    assert_volume(run.shape("b-1"), 1000.0);
}

#[test]
fn t027_cut_that_misses() {
    let run = Run::example("extrude/cut-misses.ptsm");

    run.assert_ok("f-box1");
    run.assert_warnings("f-ext1", &["no_effect"]);
    // The tool's box and the body's do not overlap across the direction, so the cut lies beside the body; reversed it
    // misses too, so no Reverse is offered (features.md section 3.15).
    let warning = run.warning("f-ext1", "no_effect");
    assert!(
        matches!(
            warning,
            FeatureWarning::NoEffect {
                how: NoEffectHow::Beside,
                reverse_hits: false,
                ..
            }
        ),
        "{warning:?}"
    );
    assert_eq!(
        run.messages("f-ext1"),
        ["This cut does not touch body 'Body 1': it lies beside it."]
    );
    assert_eq!(message(&run, "f-ext1", "no_effect").fixes, []);
    assert_volume(run.shape("b-1"), 16000.0);
    assert_face_count(run.shape("b-1"), 6);
    assert_unchanged(&run, "f-ext1", "b-1");
}

#[test]
fn t028_new_on_a_body_with_a_solid() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_BELOW,
            RECTANGLE_SKETCH,
            &extrude(RECTANGLE_PROFILE, &distance("30"), "new"),
        ],
    );

    run.assert_error("f-ext1", "body_not_empty");
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t029_join_on_an_empty_body() {
    let run = Run::records(
        &["b-1"],
        &[
            RECTANGLE_SKETCH,
            &extrude(RECTANGLE_PROFILE, &distance("30"), "join"),
        ],
    );

    run.assert_error("f-ext1", "body_empty");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Body 'Body 1' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here."
        ],
    );
    let fixes = run.error_fixes("f-ext1", &Diagnosis::default());
    assert!(
        matches!(
            fixes.as_slice(),
            [Fix::SetField { feature: None, field, value }] if field.path == "op" && value == "new"
        ),
        "{fixes:?}"
    );
}

#[test]
fn t030_region_matched_by_fingerprint() {
    let run = Run::example("extrude/region-by-fingerprint.ptsm");

    run.assert_warnings("f-ext1", &["region_remapped"]);
    assert_eq!(
        message(&run, "f-ext1", "region_remapped")
            .pointer
            .as_deref(),
        Some("/profile/regions/0"),
    );
    assert_eq!(
        run.messages("f-ext1"),
        [
            "The outline of the region of 'Sketch 1' selected for Profile was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again."
        ],
    );
    assert_eq!(
        run.warning_fixes("f-ext1", "region_remapped", &Diagnosis::default()),
        [
            Fix::KeepRegions {
                pointer: "/profile/regions/0".to_string()
            },
            Fix::Pick {
                pointer: "/profile/regions/0".to_string()
            },
        ],
    );
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t031_a_hole_circle_drawn_again() {
    // The stored hole circle e-6 is gone and e-5 is drawn in its place: e-5 is new geometry, so its disc fills
    // the hole and the profile is the whole rectangle (features.md open issue 10).
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}, {"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}]}"#;
    let profile = r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4;~e-6", "fp": "c=40,25|a=3685.840735"}]}"#;
    let run = Run::records(
        &["b-1"],
        &[sketch, &extrude(profile, &distance("10"), "new")],
    );

    run.assert_notices("f-ext1", &["region_split"]);
    assert!(
        matches!(
            run.warning("f-ext1", "region_split"),
            FeatureWarning::RegionSplit { count: 2, .. }
        ),
        "{:?}",
        run.feature("f-ext1").warnings
    );
    assert_eq!(
        run.messages("f-ext1"),
        [
            "The region of 'Sketch 1' selected for Profile is now split in 2 parts by other curves of the sketch, and all of them are used. Remove a part from the profile to leave it out."
        ],
    );
    let shape = run.shape("b-1");
    assert_volume(shape, 40000.0);
    assert_face_count(shape, 6);
    assert_no_face(run.body("b-1"), "f-ext1/lateral/e-5");
}

#[test]
fn t032_one_region_missing() {
    let run = Run::example("extrude/region-missing.ptsm");

    run.assert_warnings("f-ext1", &["region_missing"]);
    assert_eq!(
        message(&run, "f-ext1", "region_missing").pointer.as_deref(),
        Some("/profile/regions/1"),
    );
    assert_eq!(
        run.messages("f-ext1"),
        [
            "The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again, so it was skipped."
        ],
    );
    assert_eq!(
        run.warning_fixes("f-ext1", "region_missing", &Diagnosis::default()),
        [
            Fix::Show {
                feature: id("f-sk1"),
                entity: None,
            },
            Fix::Pick {
                pointer: "/profile/regions/1".to_string()
            },
        ],
    );
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t033_all_regions_missing() {
    let run = Run::records(
        &["b-1"],
        &[
            RECTANGLE_SKETCH,
            &extrude(
                r#"{"sketch": "f-sk1", "regions": [{"key": "e-7,e-8", "fp": "c=200,200|a=100"}]}"#,
                &distance("30 mm"),
                "new",
            ),
        ],
    );

    run.assert_error("f-ext1", "profile_not_found");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again."
        ],
    );
}

#[test]
fn t034_profile_from_a_datum() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}}"#,
            &extrude(
                r#"{"sketch": "f-dp1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}"#,
                &distance("30 mm"),
                "new",
            ),
        ],
    );

    run.assert_ok("f-dp1");
    run.assert_error("f-ext1", "wrong_feature_type");
    assert_eq!(
        run.messages("f-ext1"),
        ["'Datum plane 1' is not a sketch. Pick a sketch for Profile."],
    );
}

#[test]
fn t035_sketch_later_in_the_timeline() {
    let run = Run::records(
        &["b-1"],
        &[
            &extrude(RECTANGLE_PROFILE, &distance("30 mm"), "new"),
            RECTANGLE_SKETCH,
        ],
    );

    run.assert_error("f-ext1", "forward_reference");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Profile uses 'Sketch 1', which comes after this extrude. Move this extrude below 'Sketch 1', or pick something earlier."
        ],
    );
    assert_eq!(
        message(&run, "f-ext1", "forward_reference").fixes,
        [
            Fix::Move {
                feature: id("f-ext1"),
                place: partsmith_features::Place::Below(id("f-sk1")),
            },
            pick("/profile/sketch"),
        ]
    );
    run.assert_ok("f-sk1");
}

#[test]
fn t036_sketch_on_xz() {
    let run = Run::example("extrude/sketch-on-xz.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 120000.0, [0.0, -30.0, 0.0], [80.0, 0.0, 50.0]);
}

#[test]
fn t037_sketch_on_a_datum() {
    let run = Run::example("extrude/sketch-on-datum.ptsm");

    run.assert_all_ok();
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, 10.0], [80.0, 50.0, 40.0]);
}

#[test]
fn t038_empty_profile() {
    let run = Run::records(
        &["b-1"],
        &[
            RECTANGLE_SKETCH,
            &extrude(
                r#"{"sketch": "f-sk1", "regions": []}"#,
                &distance("30 mm"),
                "new",
            ),
        ],
    );

    run.assert_error("f-ext1", "empty_selection");
    assert_eq!(
        run.messages("f-ext1"),
        ["Select at least one region for Profile."]
    );
}

#[test]
fn t040_union_of_regions_split_by_a_line() {
    // Features.md sections 3.7 and 3.10: regions merged in 2D give the same solid as the one region
    // they cover, with one lateral face per entity.
    let run = Run::example("extrude/union-split-by-line.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 40000.0, [0.0, 0.0, 0.0], [80.0, 50.0, 10.0]);
    let body = run.body("b-1");
    assert_face_names(
        body,
        &[
            "f-ext1/end-cap",
            "f-ext1/lateral/e-1",
            "f-ext1/lateral/e-2",
            "f-ext1/lateral/e-3",
            "f-ext1/lateral/e-4",
            "f-ext1/start-cap",
        ],
    );
    assert_face_area(&face(body, "f-ext1/lateral/e-1"), 800.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-3"), 800.0);
    assert_no_face(body, "f-ext1/lateral/e-5");
}

#[test]
fn t041_union_of_circle_halves() {
    let run = Run::example("extrude/union-of-circle-halves.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 6283.185307, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    assert_face_count(run.shape("b-1"), 3);
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-ext1/lateral/e-5"), 1256.637061);
    assert_no_face(body, "f-ext1/lateral/e-6");
}

#[test]
fn t042_pieces_of_one_line_that_stay_separate() {
    let run = Run::example("extrude/pieces-stay-separate.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 38429.203673);
    assert_face_count(shape, 8);
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-ext1/lateral/e-1#1"), 300.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-1#2"), 300.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-5"), 314.159265);
    assert_no_face(body, "f-ext1/lateral/e-1");
}

#[test]
fn t043_union_touching_itself_on_a_circle() {
    // The circle touches e-3 at (40, 50), so the union's boundary runs round it between two visits
    // to that point; its two arcs stay separate instead of closing on themselves.
    let run = Run::example("extrude/union-touching-itself.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 36858.407346, [0.0, 0.0, 0.0], [80.0, 50.0, 10.0]);
    let body = run.body("b-1");
    assert_face_names(
        body,
        &[
            "f-ext1/end-cap",
            "f-ext1/lateral/e-1",
            "f-ext1/lateral/e-2",
            "f-ext1/lateral/e-3#1",
            "f-ext1/lateral/e-3#2",
            "f-ext1/lateral/e-4",
            "f-ext1/lateral/e-5#1",
            "f-ext1/lateral/e-5#2",
            "f-ext1/start-cap",
        ],
    );
    assert_face_area(&face(body, "f-ext1/lateral/e-1"), 800.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-3#1"), 400.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-3#2"), 400.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-5#1"), 314.159265);
    assert_face_area(&face(body, "f-ext1/lateral/e-5#2"), 314.159265);
}

#[test]
fn t044_pieces_of_one_line_in_a_single_region_merge() {
    // e-5 and e-7 end on e-3 and split it into three pieces that follow each other along the lower
    // region's boundary, so they merge although no selected region shares them.
    let run = Run::example("extrude/pieces-in-one-region.ptsm");

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 40000.0, [0.0, 0.0, 0.0], [80.0, 50.0, 10.0]);
    let body = run.body("b-1");
    assert_face_names(
        body,
        &[
            "f-ext1/end-cap",
            "f-ext1/lateral/e-1",
            "f-ext1/lateral/e-2",
            "f-ext1/lateral/e-3",
            "f-ext1/lateral/e-4",
            "f-ext1/start-cap",
        ],
    );
    assert_face_area(&face(body, "f-ext1/lateral/e-3"), 800.0);
}

#[test]
fn t045_distance_below_the_tolerance() {
    let run = run_rectangle(&distance("0.0000001"));

    let error = run.assert_error("f-ext1", "out_of_range");
    assert_error_field(error, "extent.expr");
    // The value is written with the digits that tell it apart from the bound (features.md section 3.12).
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ],
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t046_two_sides_below_the_tolerance() {
    let run = run_rectangle(&two_sides("0.0000005", "0.0000004"));

    run.assert_error("f-ext1", "both_sides_zero");
    assert_empty(&run, "b-1");
}

// Tests 47 to 57 use `BLOCK`: `SK-RECT` and the extrude of `P-RECT-V2`, the rectangle region as a pick stores it
// since file format version 2, distance 30, op new (x 0..80, y 0..50, z 0..30). Sketch edits are applied to the
// document as it stands; the documents below are the documents after the edit.

/// `P-RECT-V2`: the region of `SK-RECT` with its outline.
const RECTANGLE_PROFILE_V2: &str = r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}]}"#;

/// The right top edge of `BLOCK`, as stored before the edit (corpus R06).
const RIGHT_TOP_EDGE: &str = r#"{"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-2)", "fp": "line|n=0,1,0|c=80,25,30|l=50|adj=2,2"}"#;

/// The back top edge of `BLOCK`, as stored before the edit (corpus R07 and R66).
const BACK_TOP_EDGE: &str = r#"{"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"}"#;

/// The lines of `SK-RECT` without `removed`, followed by `added`.
fn rectangle_lines(removed: &[&str], added: &[String]) -> Vec<String> {
    [
        ("e-1", [0, 0], [80, 0]),
        ("e-2", [80, 0], [80, 50]),
        ("e-3", [80, 50], [0, 50]),
        ("e-4", [0, 50], [0, 0]),
    ]
    .into_iter()
    .filter(|(entity, _, _)| !removed.contains(entity))
    .map(|(entity, start, end)| line(entity, start, end))
    .chain(added.iter().cloned())
    .collect()
}

/// The sketch `f-sk1` on XY with these entities.
fn rectangle_sketch_with(entities: &[String]) -> String {
    format!(
        r#"{{"id": "f-sk1", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{}]}}"#,
        entities.join(", ")
    )
}

/// `FIL(edge, radius)`: the fillet `f-fil1` of one edge in `b-1`.
fn fillet(edge: &str, radius: &str) -> String {
    format!(
        r#"{{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{edge}], "radius": "{radius}"}}"#
    )
}

/// `BLOCK` with the sketch lines `lines` and the extrude distance `distance`, followed by `others`.
fn block_run(lines: &[String], distance_expr: &str, others: &[&str]) -> Run {
    let sketch = rectangle_sketch_with(lines);
    let extrude = extrude(RECTANGLE_PROFILE_V2, &distance(distance_expr), "new");
    let mut features: Vec<&str> = vec![&sketch, &extrude];
    features.extend_from_slice(others);
    Run::records(&["b-1"], &features)
}

/// A diagnosis that names the sketch a followed or widened reference followed (recompute.md section 3.10).
fn followed_in_the_sketch() -> Diagnosis {
    Diagnosis {
        by: Some(id("f-sk1")),
        ..Diagnosis::default()
    }
}

/// `BLOCK` after deleting `e-2` and drawing `e-9` in its place (corpus R06a), with the fillet of radius 2 on the right
/// top edge as stored before the edit, or `stored` in its place.
fn redrawn_right_side(stored: &str) -> Run {
    let lines = rectangle_lines(&["e-2"], &[line("e-9", [80, 0], [80, 50])]);
    block_run(&lines, "30", &[&fillet(stored, "2")])
}

#[test]
fn t047_a_side_line_deleted_and_drawn_again_is_followed() {
    let run = redrawn_right_side(RIGHT_TOP_EDGE);

    run.assert_ok("f-ext1");
    let after_extrude = output_body(&run, "f-ext1", "b-1");
    assert_has_face(&after_extrude, "f-ext1/lateral/e-9");
    assert_eq!(
        successions(&after_extrude),
        [(
            "f-ext1/lateral/e-2".to_string(),
            vec!["f-ext1/lateral/e-9".to_string()]
        )],
    );
    assert!(
        after_extrude.naming().aliases().is_empty(),
        "a succession is never a merge alias"
    );
    run.assert_followed("f-fil1", Follow::Replaced, "/edges/0");
    assert_eq!(
        run.warning_fixes("f-fil1", "reference_followed", &Diagnosis::default()),
        keep_and_pick("/edges/0"),
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t047_the_fillet_stays_on_the_right_top_edge() {
    let run = redrawn_right_side(RIGHT_TOP_EDGE);

    let rounded = face(
        run.body("b-1"),
        "f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
    );
    assert_face_axis_direction(&rounded, [0.0, 1.0, 0.0]);
    assert_face_axis_through(&rounded, [78.0, 25.0, 28.0]);
    assert_volume(run.shape("b-1"), 119957.079633);
}

#[test]
fn t047_keeping_the_match_stores_the_new_edge_and_ends_the_notice() {
    let run = redrawn_right_side(RIGHT_TOP_EDGE);
    let kept = run.picked_before("f-fil1", "b-1", "edge(f-ext1/end-cap|f-ext1/lateral/e-9)");
    let kept_json = serde_json::to_string(&kept).expect("references serialize");

    let after = redrawn_right_side(&kept_json);

    assert_eq!(
        kept.query.to_string(),
        "edge(f-ext1/end-cap|f-ext1/lateral/e-9)"
    );
    after.assert_ok("f-fil1");
    assert_volume(after.shape("b-1"), 119957.079633);
}

/// `BLOCK` after deleting `e-3` and drawing `e-9` in its place, at the extrude distance `distance_expr`, with a fillet
/// of radius 2 on the back top edge as stored before the edit (corpus R07).
fn redrawn_back_side(distance_expr: &str) -> Run {
    let lines = rectangle_lines(&["e-3"], &[line("e-9", [80, 50], [0, 50])]);
    block_run(&lines, distance_expr, &[&fillet(BACK_TOP_EDGE, "2")])
}

#[test]
fn t048_the_follow_lasts_through_the_next_edit() {
    let after_first_edit = redrawn_back_side("30");
    after_first_edit.assert_notices("f-fil1", &["reference_followed"]);

    let run = redrawn_back_side("80");

    run.assert_ok("f-ext1");
    run.assert_notices("f-fil1", &["reference_followed"]);
    assert_eq!(
        run.messages("f-fil1"),
        after_first_edit.messages("f-fil1"),
        "the same notice as after the first edit"
    );
    let rounded = face(
        run.body("b-1"),
        "f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
    );
    assert_face_axis_direction(&rounded, [1.0, 0.0, 0.0]);
    assert_face_axis_through(&rounded, [40.0, 48.0, 78.0]);
    assert_face_area(&rounded, 251.327412);
    assert_eq!(
        face_names_with_prefix(run.body("b-1"), "f-fil1/"),
        ["f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-9)"],
        "the bottom back edge stays sharp"
    );
    assert_volume(run.shape("b-1"), 319931.327412);
}

/// `BLOCK` with `e-2` ending at (80, 25) and `e-6` continuing it (corpus R49), and a shell of thickness 2 opening
/// the right side as stored before the edit.
fn split_right_side() -> Run {
    let lines = rectangle_lines(
        &["e-2"],
        &[
            line("e-2", [80, 0], [80, 25]),
            line("e-6", [80, 25], [80, 50]),
        ],
    );
    let shell = r#"{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [{"q": "f-ext1/lateral/e-2", "fp": "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4"}]}"#;
    block_run(&lines, "30", &[shell])
}

#[test]
fn t049_a_side_line_split_by_a_collinear_line_keeps_the_whole_opening() {
    let run = split_right_side();

    run.assert_ok("f-ext1");
    let after_extrude = output_body(&run, "f-ext1", "b-1");
    assert_eq!(
        successions(&after_extrude),
        [(
            "f-ext1/lateral/e-2".to_string(),
            vec![
                "f-ext1/lateral/e-2".to_string(),
                "f-ext1/lateral/e-6".to_string()
            ]
        )],
    );
    assert_face_in_plane(&face(&after_extrude, "f-ext1/lateral/e-2"), 0, 80.0);
    assert_face_area(&face(&after_extrude, "f-ext1/lateral/e-2"), 750.0);
    assert_face_area(&face(&after_extrude, "f-ext1/lateral/e-6"), 750.0);
    run.assert_followed("f-sh1", Follow::Extended, "/remove_faces/0");
    let shelled = run.body("b-1");
    assert_no_face(shelled, "f-ext1/lateral/e-2");
    assert_no_face(shelled, "f-ext1/lateral/e-6");
    assert_volume(run.shape("b-1"), 26712.0);
    assert_messages_pass_lint(&run, "f-sh1");
}

/// The lines of the notch of corpus R66, drawn in place of `e-3`.
fn notch_lines() -> Vec<String> {
    vec![
        line("e-9", [80, 50], [50, 50]),
        line("e-10", [50, 50], [50, 40]),
        line("e-11", [50, 40], [30, 40]),
        line("e-12", [30, 40], [30, 50]),
        line("e-13", [30, 50], [0, 50]),
    ]
}

/// `BLOCK` with `e-3` replaced by the notch, and a fillet of radius 1 on the back top edge as stored before the edit.
fn notched_back() -> Run {
    block_run(
        &rectangle_lines(&["e-3"], &notch_lines()),
        "30",
        &[&fillet(BACK_TOP_EDGE, "1")],
    )
}

#[test]
fn t050_a_side_line_replaced_by_a_notch_widens_the_edge() {
    let run = notched_back();

    run.assert_ok("f-ext1");
    let after_extrude = output_body(&run, "f-ext1", "b-1");
    assert_eq!(
        successions(&after_extrude),
        [(
            "f-ext1/lateral/e-3".to_string(),
            ["e-9", "e-10", "e-11", "e-12", "e-13"]
                .iter()
                .map(|entity| format!("f-ext1/lateral/{entity}"))
                .collect::<Vec<_>>()
        )],
    );
    let notched = after_extrude.shape().expect("the block has a solid");
    assert_volume(notched, 114000.0);
    assert_face_count(notched, 10);
    run.assert_warnings("f-fil1", &["reference_widened"]);
    assert_eq!(
        message(&run, "f-fil1", "reference_widened")
            .pointer
            .as_deref(),
        Some("/edges/0"),
    );
    assert_eq!(
        face_names_with_prefix(run.body("b-1"), "f-fil1/fillet/").len(),
        5,
        "the fillet is built on all five top edges until the user chooses"
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t050_the_widened_edge_says_what_replaced_its_line() {
    let run = notched_back();

    assert_eq!(
        run.render_warning("f-fil1", "reference_widened", &followed_in_the_sketch()),
        "The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one."
    );
}

#[test]
fn t050_without_a_diagnosis_the_widened_edge_names_its_sketch_in_words() {
    let run = notched_back();

    assert_eq!(
        run.render_warning("f-fil1", "reference_widened", &Diagnosis::default()),
        "The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in its sketch. Keep all of them, or use only one."
    );
}

#[test]
fn t050_the_widened_edge_offers_keep_all_use_only_the_first_line_and_pick() {
    let run = notched_back();

    assert_eq!(
        run.warning_fixes("f-fil1", "reference_widened", &followed_in_the_sketch()),
        [
            Fix::KeepMatch {
                pointer: "/edges/0".to_string()
            },
            use_reference(
                "/edges/0",
                run.picked_before("f-fil1", "b-1", "edge(f-ext1/end-cap|f-ext1/lateral/e-9)"),
            ),
            Fix::Pick {
                pointer: "/edges/0".to_string()
            },
        ],
    );
}

/// `BLOCK`, `f-dp1` datum XY offset 30, `f-sk2` on it with the circle `e-5` center (40, 25) radius 10, and `f-ext2`,
/// a cut of that circle with `extent` (corpus R12 and R30 (a)).
fn round_cut(extent: &str) -> Run {
    let datum = r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}"#;
    let sketch = r#"{"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"}, "entities": [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}]}"#;
    let cut = format!(
        r#"{{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {{"sketch": "f-sk2", "regions": [{{"key": "e-5", "fp": "c=40,25|a=314.159265"}}]}}, "extent": {extent}, "op": "cut"}}"#
    );
    block_run(&rectangle_lines(&[], &[]), "30", &[datum, sketch, &cut])
}

/// A reversed distance extent.
fn reversed_distance(expr: &str) -> String {
    format!(r#"{{"kind": "distance", "expr": "{expr}"}}, "reverse": true"#)
}

#[test]
fn t051_a_cut_that_goes_all_the_way_through_records_its_end_cap_absent() {
    for depth in ["30", "40"] {
        let run = round_cut(&reversed_distance(depth));

        run.assert_ok("f-ext2");
        assert_eq!(
            absent_faces(&run, "f-ext2"),
            [("f-ext2/end-cap".to_string(), "Through".to_string())],
            "distance {depth}"
        );
        let shape = run.shape("b-1");
        assert_volume(shape, 110575.222039);
        assert_face_count(shape, 7);
    }
}

#[test]
fn t051_a_pocket_records_nothing_absent() {
    let run = round_cut(&reversed_distance("5"));

    run.assert_ok("f-ext2");
    assert!(absent_faces(&run, "f-ext2").is_empty());
    assert_face_in_plane(&face(run.body("b-1"), "f-ext2/end-cap"), 2, 25.0);
}

#[test]
fn t056_a_region_whose_lines_survive_matches_by_boundary() {
    let run = Run::records(
        &["b-1"],
        &[
            RECTANGLE_SKETCH,
            &extrude(
                r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-9", "fp": "c=40,25|a=4000"}]}"#,
                &distance("30"),
                "new",
            ),
        ],
    );

    run.assert_ok("f-ext1");
    assert!(run.messages("f-ext1").is_empty());
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t057_a_cut_whose_end_reaches_the_outside_records_end_outside() {
    let run = round_cut(r#"{"kind": "symmetric", "expr": "40"}"#);

    run.assert_ok("f-ext2");
    assert_face_in_plane(&face(run.body("b-1"), "f-ext2/start-cap"), 2, 10.0);
    assert_eq!(
        absent_faces(&run, "f-ext2"),
        [("f-ext2/end-cap".to_string(), "EndOutside".to_string())],
    );
    assert_volume(run.shape("b-1"), 113716.814693);
}

#[test]
fn t057_the_same_cut_from_the_top_goes_through() {
    let run = round_cut(&reversed_distance("30"));

    assert_eq!(
        absent_faces(&run, "f-ext2"),
        [("f-ext2/end-cap".to_string(), "Through".to_string())],
    );
}

#[test]
fn a_new_extrude_records_nothing_absent() {
    let run = run_rectangle(&distance("30"));

    assert!(absent_faces(&run, "f-ext1").is_empty());
}

/// The circle `e-5` at (40, 25) with radius 5, the boss or cut of tests 52 to 54.
const SMALL_CIRCLE: &str = r#"{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}"#;

/// The profile of `SMALL_CIRCLE` in `f-sk2`.
const SMALL_CIRCLE_PROFILE: &str =
    r#"{"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=40,25|a=78.539816"}]}"#;

/// `f-sk2` with `SMALL_CIRCLE` on `plane` (sketch.md plane JSON).
fn small_circle_sketch(plane: &str) -> String {
    format!(
        r#"{{"id": "f-sk2", "type": "sketch", "plane": {plane}, "entities": [{SMALL_CIRCLE}]}}"#
    )
}

/// `f-ext2`, the extrude of `SMALL_CIRCLE_PROFILE` with `extent` (an extent JSON, optionally followed by more members)
/// and `op`.
fn small_circle_extrude(extent: &str, op: &str) -> String {
    format!(
        r#"{{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {SMALL_CIRCLE_PROFILE}, "extent": {extent}, "op": "{op}"}}"#
    )
}

/// `BLOCK` with `others` after it.
fn on_block(others: &[&str]) -> Run {
    block_run(&rectangle_lines(&[], &[]), "30", others)
}

/// The datum `f-dp1` at z = 40, above `BLOCK` (newcomer error corpus NC03).
const DATUM_ABOVE_THE_BLOCK: &str = r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "40"}"#;

/// The plane of a sketch on the datum `f-dp1`.
const ON_THE_DATUM: &str = r#"{"kind": "datum", "feature": "f-dp1"}"#;

/// The face plane of `f-sk2` attached to `top`, a stored reference to the top of `BLOCK`, with the fallback its
/// placement projected onto the top gives, in body coordinates: what the `Attach` fix stores (features.md section
/// 3.15, engine.md section 3.22).
fn attached_to_the_top(top: &str) -> String {
    format!(
        r#"{{"kind": "face", "body": "b-1", "face": {top}, "fallback": {}, "fallback_frame": "body"}}"#,
        top_fallback([0.0, 0.0, 30.0])
    )
}

/// `Attach 'Sketch 2' to the top of 'Extrude 1'`, with the top as a pick just before `f-ext2` stores it.
fn attach_to_the_top(run: &Run) -> Fix {
    Fix::Attach {
        sketch: id("f-sk2"),
        body: id("b-1"),
        face: run.picked_before("f-ext2", "b-1", "f-ext1/end-cap"),
    }
}

#[test]
fn t052_a_join_buried_in_the_body_offers_to_attach_its_sketch_to_the_top() {
    let sketch = small_circle_sketch(r#"{"kind": "origin", "plane": "XY"}"#);
    let boss = small_circle_extrude(&distance("10"), "join");
    let run = on_block(&[&sketch, &boss]);

    run.assert_warnings("f-ext2", &["no_effect"]);
    let warning = run.warning("f-ext2", "no_effect");
    assert!(
        matches!(
            warning,
            FeatureWarning::NoEffect {
                how: NoEffectHow::Inside,
                ..
            }
        ),
        "{warning:?}"
    );
    assert_eq!(
        run.messages("f-ext2"),
        ["This extrude adds nothing to body 'Body 1': it lies entirely inside the body."]
    );
    // A join inside the body gets no Reverse: reversed, it adds nothing either.
    assert_eq!(
        message(&run, "f-ext2", "no_effect").fixes,
        [attach_to_the_top(&run)]
    );
    assert_volume(run.shape("b-1"), 120000.0);
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t052_attaching_the_sketch_to_the_top_stands_the_boss_on_it() {
    let before = on_block(&[
        &small_circle_sketch(r#"{"kind": "origin", "plane": "XY"}"#),
        &small_circle_extrude(&distance("10"), "join"),
    ]);
    let top = reference_json(&before.picked_before("f-ext2", "b-1", "f-ext1/end-cap"));

    let run = on_block(&[
        &small_circle_sketch(&attached_to_the_top(&top)),
        &small_circle_extrude(&distance("10"), "join"),
    ]);

    run.assert_all_quiet();
    assert_volume(run.shape("b-1"), 120785.398163);
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, 0.0], [80.0, 50.0, 40.0]);
}

/// Newcomer error corpus NC03: `BLOCK`, the datum at z = 40, the circle on it and `f-ext2` with `extent` and `op`.
fn above_the_block(extent: &str, op: &str) -> Run {
    let sketch = small_circle_sketch(ON_THE_DATUM);
    let tool = small_circle_extrude(extent, op);
    on_block(&[DATUM_ABOVE_THE_BLOCK, &sketch, &tool])
}

/// The `how` of the only `no_effect` warning of `f-ext2`.
fn no_effect_how(run: &Run) -> NoEffectHow {
    match run.warning("f-ext2", "no_effect") {
        FeatureWarning::NoEffect { how, .. } => how.clone(),
        other => panic!("expected a no_effect warning, got {other:?}"),
    }
}

/// Asserts that `how` is `Away` (`short` false) or `Short` (`short` true) with `gap`.
#[track_caller]
fn assert_gap(how: &NoEffectHow, short: bool, gap: f64) {
    let found = match (how, short) {
        (NoEffectHow::Away { gap }, false) | (NoEffectHow::Short { gap }, true) => *gap,
        _ => panic!(
            "expected {} with gap {gap}, got {how:?}",
            if short { "Short" } else { "Away" }
        ),
    };
    assert!((found - gap).abs() <= 1e-5, "gap {found}, expected {gap}");
}

#[test]
fn t053_a_cut_that_points_away_offers_reverse_when_reversed_it_reaches_the_body() {
    let run = above_the_block(&distance("15"), "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_gap(&no_effect_how(&run), false, 10.0);
    assert_eq!(
        run.messages("f-ext2"),
        [
            "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it."
        ]
    );
    assert_eq!(
        message(&run, "f-ext2", "no_effect").fixes,
        [set_reverse(true)]
    );
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t053_turning_on_reverse_cuts_the_pocket() {
    let run = above_the_block(&reversed_distance("15"), "cut");

    run.assert_all_quiet();
    // The pocket runs from the datum at z = 40 down to z = 25.
    assert_volume(run.shape("b-1"), 119607.300918);
}

#[test]
fn t053_a_cut_that_misses_either_way_offers_no_reverse() {
    let run = above_the_block(&distance("5"), "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_gap(&no_effect_how(&run), false, 10.0);
    // Reversed, the cut ends at z = 35 and misses too.
    assert_eq!(
        run.messages("f-ext2"),
        [
            "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it."
        ]
    );
    assert_eq!(message(&run, "f-ext2", "no_effect").fixes, []);
}

#[test]
fn t053_a_reversed_cut_that_ends_short_says_by_how_much() {
    let run = above_the_block(&reversed_distance("5"), "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_gap(&no_effect_how(&run), true, 5.0);
    assert_eq!(
        run.messages("f-ext2"),
        ["This cut does not reach body 'Body 1': it ends 5 mm short of it."]
    );
    // The other way it misses too.
    assert_eq!(message(&run, "f-ext2", "no_effect").fixes, []);
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t054_a_join_that_floats_leaves_the_body_in_two_pieces() {
    let run = above_the_block(&distance("15"), "join");

    run.assert_warnings("f-ext2", &["separate_solid"]);
    assert_eq!(
        run.messages("f-ext2"),
        ["This extrude does not touch body 'Body 1', so the body now has 2 separate pieces."]
    );
    // Put it in a new body, then Attach 'Sketch 2' to the top of 'Extrude 1'.
    assert_eq!(
        message(&run, "f-ext2", "separate_solid").fixes,
        [Fix::NewBody, attach_to_the_top(&run)]
    );
    assert_solid_count(run.shape("b-1"), 2);
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t054_attaching_the_floating_sketch_to_the_top_joins_the_boss() {
    let before = above_the_block(&distance("15"), "join");
    let top = reference_json(&before.picked_before("f-ext2", "b-1", "f-ext1/end-cap"));

    let run = on_block(&[
        DATUM_ABOVE_THE_BLOCK,
        &small_circle_sketch(&attached_to_the_top(&top)),
        &small_circle_extrude(&distance("15"), "join"),
    ]);

    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_solid_count(shape, 1);
    assert_volume(shape, 121178.097245);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [80.0, 50.0, 45.0]);
}

/// The datum `f-dp1` at z = 30, in the plane of the top of `BLOCK`.
const DATUM_ON_THE_TOP: &str = r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}"#;

#[test]
fn t058_a_reversed_join_from_the_top_offers_to_turn_reverse_off() {
    let sketch = small_circle_sketch(ON_THE_DATUM);
    let boss = small_circle_extrude(&reversed_distance("10"), "join");
    let run = on_block(&[DATUM_ON_THE_TOP, &sketch, &boss]);

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_eq!(no_effect_how(&run), NoEffectHow::Inside);
    assert_eq!(
        run.messages("f-ext2"),
        ["This extrude adds nothing to body 'Body 1': it lies entirely inside the body."]
    );
    // Attached to the bottom, the sketch would face down and the reversed sweep would run up into the body again, so
    // only turning Reverse off helps (features.md section 3.15).
    assert_eq!(
        message(&run, "f-ext2", "no_effect").fixes,
        [set_reverse(false)]
    );
    assert_volume(run.shape("b-1"), 120000.0);
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t058_turning_reverse_off_stands_the_boss_on_the_top() {
    let sketch = small_circle_sketch(ON_THE_DATUM);
    let boss = small_circle_extrude(&distance("10"), "join");
    let run = on_block(&[DATUM_ON_THE_TOP, &sketch, &boss]);

    run.assert_all_quiet();
    assert_volume(run.shape("b-1"), 120785.398163);
    assert_bounding_box(run.shape("b-1"), [0.0, 0.0, 0.0], [80.0, 50.0, 40.0]);
}

#[test]
fn t058_a_reversed_join_inside_either_way_offers_nothing() {
    // The datum at z = 20: reversed the boss spans z 10..20, the other way z 20..30, inside the body both times.
    let datum = r#"{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "20"}"#;
    let sketch = small_circle_sketch(ON_THE_DATUM);
    let boss = small_circle_extrude(&reversed_distance("10"), "join");
    let run = on_block(&[datum, &sketch, &boss]);

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_eq!(no_effect_how(&run), NoEffectHow::Inside);
    assert_eq!(message(&run, "f-ext2", "no_effect").fixes, []);
}

#[test]
fn t059_a_join_beside_the_body_in_the_plane_of_its_top_offers_only_a_new_body() {
    // The sketch lies in the plane of the top already, so attaching it there would change nothing.
    let sketch = r#"{"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"}, "entities": [{"id": "e-5", "type": "circle", "center": [100, 25], "radius": 5}]}"#;
    let boss = r#"{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=100,25|a=78.539816"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "join"}"#;
    let run = on_block(&[DATUM_ON_THE_TOP, sketch, boss]);

    run.assert_warnings("f-ext2", &["separate_solid"]);
    assert_eq!(
        run.messages("f-ext2"),
        ["This extrude does not touch body 'Body 1', so the body now has 2 separate pieces."]
    );
    assert_eq!(
        message(&run, "f-ext2", "separate_solid").fixes,
        [Fix::NewBody]
    );
    assert_solid_count(run.shape("b-1"), 2);
}

#[test]
fn t060_a_join_of_which_only_part_floats_says_so_and_offers_no_fix() {
    // The boss at (40, 25) stands on the top and the one at (100, 25) beside the body: a new body would take the
    // boss that joined along (features.md section 3.15).
    let sketch = format!(
        r#"{{"id": "f-sk2", "type": "sketch", "plane": {ON_THE_DATUM}, "entities": [{SMALL_CIRCLE}, {{"id": "e-6", "type": "circle", "center": [100, 25], "radius": 5}}]}}"#
    );
    let bosses = r#"{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=40,25|a=78.539816"}, {"key": "e-6", "fp": "c=100,25|a=78.539816"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "join"}"#;
    let run = on_block(&[DATUM_ON_THE_TOP, &sketch, bosses]);

    run.assert_warnings("f-ext2", &["separate_solid"]);
    let warning = run.warning("f-ext2", "separate_solid");
    assert!(
        matches!(
            warning,
            FeatureWarning::SeparateSolid {
                pieces: 2,
                part_joined: true,
                attach: None,
                ..
            }
        ),
        "{warning:?}"
    );
    assert_eq!(
        run.messages("f-ext2"),
        [
            "Part of this extrude does not touch body 'Body 1', so the body now has 2 separate pieces."
        ]
    );
    assert_eq!(message(&run, "f-ext2", "separate_solid").fixes, []);
    assert_solid_count(run.shape("b-1"), 2);
    assert_volume(run.shape("b-1"), 121570.796327);
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t061_through_all_with_op_new_on_a_body_that_holds_a_solid() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_BELOW,
            RECTANGLE_SKETCH,
            &extrude(RECTANGLE_PROFILE, THROUGH_ALL, "new"),
        ],
    );

    run.assert_error("f-ext1", "body_not_empty");
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Body 'Body 1' already has a solid here, so Operation New body cannot start it. Set Operation to Join."
        ]
    );
    assert_eq!(
        message(&run, "f-ext1", "body_not_empty").fixes,
        [set_field("op", "Operation", serde_json::json!("join"))]
    );
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t055_distance_beyond_the_modeling_limit() {
    let run = run_rectangle(&distance("1e200"));

    let error = run.assert_error("f-ext1", "out_of_range");
    assert_error_field(error, "extent.expr");
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
    // The range check, not the kernel, reports it: no kernel text about the profile appears.
    assert_eq!(
        run.messages("f-ext1"),
        ["Distance must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."]
    );
    assert_eq!(
        message(&run, "f-ext1", "out_of_range").fixes,
        [set_field(
            "extent.expr",
            "Distance",
            serde_json::json!("1000000 mm")
        )]
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t062_a_profile_bounded_by_an_arc_builds_a_block_with_a_round_end() {
    let run = Run::example("extrude/arc-bounded.ptsm");

    run.assert_all_quiet();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        149452.431127,
        6,
        [0.0, 0.0, 0.0],
        [105.0, 50.0, 30.0],
    );
    assert_face_names(
        run.body("b-1"),
        &[
            "f-ext1/end-cap",
            "f-ext1/lateral/e-1",
            "f-ext1/lateral/e-2",
            "f-ext1/lateral/e-3",
            "f-ext1/lateral/e-4",
            "f-ext1/start-cap",
        ],
    );
}

#[test]
fn t062_the_side_swept_from_the_arc_is_a_cylinder_named_after_the_arc() {
    let run = Run::example("extrude/arc-bounded.ptsm");

    let side = face(run.body("b-1"), "f-ext1/lateral/e-2");
    assert_face_kind(&side, GeometryKind::Cylinder);
    assert_face_radius(&side, 25.0);
    assert_face_area(&side, 2356.19449);
    assert_face_axis_direction(&side, [0.0, 0.0, 1.0]);
    assert_face_axis_through(&side, [80.0, 25.0, 0.0]);
}

#[test]
fn t062_the_flat_sides_and_the_caps_of_the_arc_bounded_block() {
    let run = Run::example("extrude/arc-bounded.ptsm");

    let body = run.body("b-1");
    // The lines keep the areas of test 1.
    assert_face_area(&face(body, "f-ext1/lateral/e-1"), 2400.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-3"), 2400.0);
    assert_face_area(&face(body, "f-ext1/lateral/e-4"), 1500.0);
    assert_flat_face(body, "f-ext1/start-cap", 2, 0.0, 4981.747704);
    assert_flat_face(body, "f-ext1/end-cap", 2, 30.0, 4981.747704);
}

/// The message of test 63's `profile_not_found`.
const OPEN_OUTLINE_TEXT: &str = "The region of 'Sketch 1' selected for Profile is gone because its outline is no longer closed.";

#[test]
fn t063_an_open_outline_loses_the_region_picked_before_the_gap() {
    let run = Run::example("extrude/open-outline.ptsm");

    // The open ends are a profile finding, not a warning of the sketch feature.
    run.assert_quiet("f-sk1");
    let error = run.assert_error("f-ext1", "profile_not_found");
    let FeatureError::ProfileNotFound { regions, .. } = error else {
        panic!("expected profile_not_found, got {error:?}");
    };
    let losses: Vec<Option<RegionLoss>> =
        regions.iter().map(|region| region.loss.clone()).collect();
    // Every entity of the stored key is still in the sketch, so the outline is open.
    assert_eq!(losses, [Some(RegionLoss::Open)]);
    assert_empty(&run, "b-1");
}

#[test]
fn t063_the_open_outline_offers_show_then_pick_and_no_regions_to_use() {
    let run = Run::example("extrude/open-outline.ptsm");

    let message = message(&run, "f-ext1", "profile_not_found");
    assert_eq!(message.pointer.as_deref(), Some("/profile"));
    assert_eq!(message.text, OPEN_OUTLINE_TEXT);
    assert_eq!(
        message.fixes,
        [
            Fix::Show {
                feature: id("f-sk1"),
                entity: None,
            },
            pick("/profile/regions/0"),
        ],
    );
    assert_messages_pass_lint(&run, "f-ext1");
}

#[test]
fn t063_connecting_the_ends_matches_the_stored_region_by_its_key() {
    let run = Run::example("extrude/open-outline-connected.ptsm");

    run.assert_all_quiet();
    assert!(
        matches!(
            stored_region_match(&run, "f-ext1", "f-sk1"),
            RegionMatch::Resolved {
                method: MatchMethod::Key,
                ..
            }
        ),
        "the stored region matches by its key",
    );
    // The coincident merges the two ends at their mean (sketch.md section 3.7.3).
    for point in ["e-1.start", "e-4.end"] {
        assert_coordinates(
            point,
            solved_point(&run, "f-sk1", point),
            [0.0, 0.15],
            AXIS_TOLERANCE,
        );
    }
}

#[test]
fn t063_connecting_the_ends_extrudes_the_closed_outline() {
    let run = Run::example("extrude/open-outline-connected.ptsm");

    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        119820.0,
        6,
        [0.0, 0.0, 0.0],
        [80.0, 50.0, 30.0],
    );
}
