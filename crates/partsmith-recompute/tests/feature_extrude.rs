//! Acceptance tests of the `extrude` feature (docs/spec/features/extrude.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/extrude/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_empty, assert_error_field,
    assert_face_area, assert_face_count, assert_face_in_plane, assert_face_names,
    assert_face_normal, assert_has_face, assert_no_face, assert_unchanged, assert_volume,
    body_update, document_text, element_names, example_paths, face, load_example,
    median_build_time, message, parse_document,
};
use partsmith_features::BodyChange;
use partsmith_naming::ElementKind;

/// `SK-RECT`: `RECT(0, 0, 80, 50)` on XY with id `f-sk1`.
const RECTANGLE_SKETCH: &str = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}]}"#;

/// `P-RECT`: the region of `SK-RECT`.
const RECTANGLE_PROFILE: &str =
    r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}"#;

/// `BOX-BELOW`: spans [-20,-20,-10]-[20,20,0], volume 16000.
const BOX_BELOW: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}"#;

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
    let paths = example_paths("extrude");
    assert!(!paths.is_empty(), "examples/extrude has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
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
    assert_eq!(
        run.messages("f-ext1"),
        ["Distance: Unknown name 'hieght'. Check the spelling, or add a parameter named hieght."],
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
    assert_eq!(
        run.messages("f-ext1"),
        [
            "Through all points away from body 'Body 1', so there is nothing to go through. Turn on Reverse, or use a distance."
        ],
    );
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t023_through_all_with_op_new() {
    let run = run_rectangle(THROUGH_ALL);

    run.assert_error("f-ext1", "through_all_needs_body");
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
        assert_eq!(
            run.messages("f-ext1"),
            ["Body 'Body 1' is empty at this point. Use op new to start it."],
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
            "The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap the body."
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
            "This cut removes all of body 'Body 1'. Make the cut smaller, or change its position or direction."
        ],
    );
    assert_volume(run.shape("b-1"), 1000.0);
}

#[test]
fn t027_cut_that_misses() {
    let run = Run::example("extrude/cut-misses.ptsm");

    run.assert_ok("f-box1");
    run.assert_warnings("f-ext1", &["no_effect"]);
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
        ["Body 'Body 1' is empty at this point. Use op new to start it."],
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
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn t031_region_matched_by_outer_boundary() {
    let run = Run::example("extrude/region-by-outer-boundary.ptsm");

    run.assert_warnings("f-ext1", &["region_remapped"]);
    assert_volume(run.shape("b-1"), 36858.407346);
}

#[test]
fn t032_one_region_missing() {
    let run = Run::example("extrude/region-missing.ptsm");

    run.assert_warnings("f-ext1", &["region_missing"]);
    assert_eq!(
        message(&run, "f-ext1", "region_missing").pointer.as_deref(),
        Some("/profile/regions/1"),
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
        ["None of the selected regions of 'Sketch 1' can be found anymore. Re-pick the profile."],
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
            "'Sketch 1' comes later in the timeline than this feature. Move it above this feature, or pick something earlier for Profile."
        ],
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
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t039_builds_within_budget() {
    let distance = median_build_time(&load_example("extrude/distance.ptsm"), "f-ext1", 21);
    assert!(
        distance < Duration::from_millis(3),
        "test 1 took {distance:?} to build (median of 21 runs), over the 3 ms budget",
    );
    let through_all =
        median_build_time(&load_example("extrude/through-all-cut.ptsm"), "f-ext1", 21);
    assert!(
        through_all < Duration::from_millis(30),
        "test 21 took {through_all:?} to build (median of 21 runs), over the 30 ms budget",
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
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-5", "type": "circle", "center": [0, 0], "radius": 10}, {"id": "e-6", "type": "line", "start": [-10, 0], "end": [10, 0]}]}"#;
    let profile = r#"{"sketch": "f-sk1", "regions": [{"key": "e-5,e-6", "fp": "c=0,4.244132|a=157.079633"}, {"key": "e-5,~e-6", "fp": "c=0,-4.244132|a=157.079633"}]}"#;

    let run = Run::records(
        &["b-1"],
        &[sketch, &extrude(profile, &distance("20"), "new")],
    );

    run.assert_all_ok();
    assert_volume_and_bounds(&run, 6283.185307, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    assert_face_count(run.shape("b-1"), 3);
    let body = run.body("b-1");
    assert_face_area(&face(body, "f-ext1/lateral/e-5"), 1256.637061);
    assert_no_face(body, "f-ext1/lateral/e-6");
}

#[test]
fn t042_pieces_of_one_line_that_stay_separate() {
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}, {"id": "e-5", "type": "circle", "center": [40, 0], "radius": 10}]}"#;
    let profile = r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4,~e-5", "fp": "c=40,25.848398|a=3842.920367"}]}"#;

    let run = Run::records(
        &["b-1"],
        &[sketch, &extrude(profile, &distance("10"), "new")],
    );

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
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}, {"id": "e-5", "type": "circle", "center": [40, 40], "radius": 10}, {"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50]}]}"#;
    let profile = r#"{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,~e-5,~e-6", "fp": "c=61.342937,23.721489|a=1842.920367"}, {"key": "e-1,e-3,e-4,~e-5,e-6", "fp": "c=18.657063,23.721489|a=1842.920367"}]}"#;

    let run = Run::records(
        &["b-1"],
        &[sketch, &extrude(profile, &distance("10"), "new")],
    );

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
    let sketch = r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}, {"id": "e-5", "type": "line", "start": [20, 50], "end": [20, 70]}, {"id": "e-6", "type": "line", "start": [20, 70], "end": [60, 70]}, {"id": "e-7", "type": "line", "start": [60, 70], "end": [60, 50]}]}"#;

    let run = Run::records(
        &["b-1"],
        &[sketch, &extrude(RECTANGLE_PROFILE, &distance("10"), "new")],
    );

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
