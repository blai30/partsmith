//! Acceptance tests of the `datum_plane` feature (docs/spec/features/datum_plane.md section 6),
//! run as documents through recompute (features.md section 6.1). Geometry documents are the
//! examples in `examples/datum_plane/`, those of tests 17 to 19 (frames on the model) among them
//! (`on-split-face`, `on-shelled-top` and `on-moved-face`), from which the tests make their other
//! documents; test 16 expects a warning and writes its document here.

use crate::feature_support;

use std::f64::consts::FRAC_1_SQRT_2;

use feature_support::{
    AXIS_TOLERANCE, BOTH_TOPS, Run, assert_choice_between, assert_coordinates,
    assert_element_repair, assert_examples_canonical, assert_messages_pass_lint, assert_moves,
    assert_placement, assert_translation, datum_motion, datum_placement, feature_record_mut, id,
    keep_and_pick, l_shaped_body, link_ids, load_example, turn, use_reference,
};
use partsmith_features::{Diagnosis, Fix};
use partsmith_model::Document;
use partsmith_naming::{Follow, Motion};

/// A box of size 10 at position (3, 4, 0) in `b-1`: x from -2 to 8, y from -1 to 9, z from 0 to 10.
const BOX_AT_3_4: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["3", "4", "0"], "op": "new"}"#;

/// `DP(base, extra)` of the spec: the datum plane `f-dp1` with `extra` members after `base`.
fn datum_plane(base: &str, extra: &str) -> String {
    format!(r#"{{"id": "f-dp1", "type": "datum_plane", "base": {base}{extra}}}"#)
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("datum_plane");
}

#[test]
fn t001_xy_offset() {
    let run = Run::example("datum_plane/xy-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 1.0, 0.0],
        AXIS_TOLERANCE,
    );
    // `datum_placement` requires a datum output, so the feature reports no body updates.
    assert!(run.report.bodies.is_empty(), "the document has no bodies");
}

#[test]
fn t002_xz_offset() {
    let run = Run::example("datum_plane/xz-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, -10.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t003_yz_negative_offset() {
    let run = Run::example("datum_plane/yz-negative-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [-5.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t004_flip() {
    let run = Run::example("datum_plane/flip.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, -1.0, 0.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t005_default_offset() {
    let run = Run::example("datum_plane/default-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 0.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
}

#[test]
fn t006_on_a_face() {
    let run = Run::example("datum_plane/on-face.ptsm");

    run.assert_all_ok();
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, 0.0, 15.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_eq!(link_ids(&run.dependencies("f-dp1").bodies_read), ["b-1"]);
}

#[test]
fn t007_on_a_side_face_x_axis_fallback() {
    let run = Run::example("datum_plane/on-side-face.ptsm");

    run.assert_all_ok();
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(placement, [8.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t008_on_a_front_face() {
    let run = Run::example("datum_plane/on-front-face.ptsm");

    run.assert_all_ok();
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, -1.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t009_on_a_tilted_face() {
    let run = Run::example("datum_plane/on-tilted-face.ptsm");

    run.assert_all_ok();
    // The chamfer face lies in the plane z - y = 28; its normal is (0, -1, 1) / sqrt(2).
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, -14.0, 14.0],
        [1.0, 0.0, 0.0],
        [0.0, -FRAC_1_SQRT_2, FRAC_1_SQRT_2],
    );
}

#[test]
fn t010_chained_datums() {
    let run = Run::example("datum_plane/chained.ptsm");

    run.assert_all_ok();
    let placement = datum_placement(&run, "f-dp2");
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 15.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
    assert_eq!(link_ids(&run.dependencies("f-dp2").features), ["f-dp1"]);
}

#[test]
fn t011_curved_face() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}"#,
            &datum_plane(
                r#"{"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}}"#,
                "",
            ),
        ],
    );

    run.assert_ok("f-cyl1");
    run.assert_error("f-dp1", "not_planar");
    // The face is named in words (features.md section 4.1, `{Element}`).
    assert_eq!(
        run.messages("f-dp1"),
        [
            "The side of 'Cylinder 1' selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane."
        ],
    );
    assert_eq!(
        run.feature("f-dp1").messages[0].fixes,
        [Fix::Pick {
            pointer: "/base/face".to_string()
        }]
    );
    assert_messages_pass_lint(&run, "f-dp1");
}

#[test]
fn t012_base_is_a_sketch() {
    let run = Run::records(
        &[],
        &[
            r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}}"#,
            &datum_plane(r#"{"kind": "datum", "feature": "f-sk1"}"#, ""),
        ],
    );

    run.assert_ok("f-sk1");
    run.assert_error("f-dp1", "wrong_feature_type");
    assert_eq!(
        run.messages("f-dp1"),
        ["'Sketch 1' is not a datum plane. Pick a datum plane for Base plane."],
    );
}

#[test]
fn t013_lost_face() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_AT_3_4,
            &datum_plane(
                r#"{"kind": "face", "body": "b-1", "face": {"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}}"#,
                r#", "offset": "5""#,
            ),
        ],
    );

    run.assert_ok("f-box1");
    let error = run.assert_error("f-dp1", "reference_not_found");
    assert_element_repair(error.repair(), "/base/face", "b-1");
    let fixes = run.error_fixes("f-dp1", &Diagnosis::default());
    // The only face of 'Box 1' that faces the way the stored face did comes first.
    assert_eq!(
        fixes.first(),
        Some(&use_reference(
            "/base/face",
            run.picked_before("f-dp1", "b-1", "f-box1/top")
        ))
    );
    assert_eq!(
        fixes.last(),
        Some(&Fix::Pick {
            pointer: "/base/face".to_string()
        })
    );
    assert_messages_pass_lint(&run, "f-dp1");
}

#[test]
fn t014_expression_offset() {
    let run = Run::example("datum_plane/expression-offset.ptsm");

    run.assert_ok("f-dp1");
    assert_coordinates(
        "origin",
        datum_placement(&run, "f-dp1").origin,
        [0.0, 0.0, 4.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t018_a_shell_inserted_under_the_datums_face_keeps_it_on_the_rim() {
    // Corpus R58: the shell removes the top, and its rim continues it in the same plane.
    let run = Run::example("datum_plane/on-shelled-top.ptsm");
    // Recompute's diagnosis names the shell whose rim the reference followed (recompute.md section 3.10).
    let diagnosis = Diagnosis {
        by: Some(id("f-sh1")),
        ..Diagnosis::default()
    };

    run.assert_followed("f-dp1", Follow::Continued, "/base/face");
    assert_eq!(
        run.render_warning("f-dp1", "reference_followed", &diagnosis),
        "The top of 'Box 1' selected for Base plane was removed by 'Shell 1', which left its rim in the same place, so the rim is used."
    );
    assert_eq!(
        run.warning_fixes("f-dp1", "reference_followed", &diagnosis),
        keep_and_pick("/base/face")
    );
    let placement = datum_placement(&run, "f-dp1");
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 28.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
}

// Tests 16, 17 and 19: faces in two planes, a face split into coplanar pieces, and the motion a datum passes on
// (section 3.2 steps 1 and 4).

#[test]
fn t016_faces_in_two_planes() {
    let [box_1, box_2] = l_shaped_body("b-1");
    let run = Run::records(
        &["b-1"],
        &[
            &box_1,
            &box_2,
            &datum_plane(
                &format!(r#"{{"kind": "face", "body": "b-1", "face": {BOTH_TOPS}}}"#),
                "",
            ),
        ],
    );

    // Neither face is chosen by name order.
    assert_choice_between(
        &run,
        "f-dp1",
        "not_coplanar",
        ("base", "face"),
        "b-1",
        &["f-box1/top", "f-box2/top"],
        "The face selected for Base plane now matches 2 faces that do not lie in one plane. Pick the one you mean.",
    );
}

#[test]
fn t017_a_face_split_into_coplanar_pieces_keeps_its_plane() {
    // The slot of naming.md test 84 splits the end cap into `f-ext1/end-cap#1` and `#2`.
    let run = Run::example("datum_plane/on-split-face.ptsm");

    run.assert_quiet("f-dp1");
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, 0.0, 35.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
}

/// Test 19's example with its Move body a half turn about the X axis in place of the translation, so the top faces -Z
/// at z = -10.
fn turned_over(example: &Document) -> Document {
    let mut turned = example.clone();
    *feature_record_mut(&mut turned, "f-mv1") =
        serde_json::from_str(&turn("b-1", "X", "180 deg")).expect("a move body record");
    turned
}

/// The rotation of 180 degrees about X: (x, y, z) to (x, -y, -z).
const HALF_TURN_ABOUT_X: [([f64; 3], [f64; 3]); 3] = [
    ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    ([1.0, 2.0, 3.0], [1.0, -2.0, -3.0]),
    ([0.0, 0.0, 10.0], [0.0, 0.0, -10.0]),
];

#[test]
fn t019_a_slide_along_the_face_does_not_move_the_datums_frame() {
    // The box moved by (100, 0, 0) before the datum on its top.
    let run = Run::example("datum_plane/on-moved-face.ptsm");

    run.assert_all_ok();
    // As before this change: the face placement rule takes the origin and x axis from the world.
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    // The identity: the frame P0 of the face mapped back by its motion is the frame P.
    assert_translation(datum_motion(&run, "f-dp1"), [0.0, 0.0, 0.0]);
}

#[test]
fn t019_a_face_turned_over_turns_the_datum_and_its_motion() {
    let run = Run::new(turned_over(&load_example("datum_plane/on-moved-face.ptsm")));

    run.assert_all_ok();
    // The top now lies at z = -10, facing -Z.
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, 0.0, -10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert_moves(datum_motion(&run, "f-dp1"), &HALF_TURN_ABOUT_X);
}

#[test]
fn t019_an_origin_base_and_a_datum_base_pass_on_their_motion() {
    // Test 10's example: test 1's datum on XY passes on the identity, and f-dp2 the motion of f-dp1.
    let run = Run::example("datum_plane/chained.ptsm");

    run.assert_all_ok();
    assert_eq!(*datum_motion(&run, "f-dp1"), Motion::IDENTITY);
    assert_eq!(datum_motion(&run, "f-dp2"), datum_motion(&run, "f-dp1"));
}

#[test]
fn t019_a_datum_on_a_datum_on_a_turned_face_passes_on_the_turn() {
    // Section 3.2 step 4: a datum base passes on that datum's motion. Test 10's f-dp2 is placed after f-dp1.
    let datum_on_datum = load_example("datum_plane/chained.ptsm")
        .feature(&id("f-dp2"))
        .expect("test 10's example has f-dp2")
        .clone();
    let mut document = turned_over(&load_example("datum_plane/on-moved-face.ptsm"));
    document.features.push(datum_on_datum);

    let run = Run::new(document);

    run.assert_all_ok();
    assert_eq!(datum_motion(&run, "f-dp2"), datum_motion(&run, "f-dp1"));
    assert_moves(datum_motion(&run, "f-dp2"), &HALF_TURN_ABOUT_X);
}

#[test]
fn an_offset_beyond_the_modeling_limit_is_out_of_range() {
    let run = Run::records(
        &[],
        &[&datum_plane(
            r#"{"kind": "origin", "plane": "XY"}"#,
            r#", "offset": "1e200""#,
        )],
    );

    run.assert_error("f-dp1", "out_of_range");
    assert_eq!(
        run.messages("f-dp1"),
        ["Offset must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."],
    );
}

#[test]
fn a_negative_offset_beyond_the_modeling_limit_offers_the_lower_limit() {
    // An offset may be negative, so its lower limit is -1000000 mm (features.md section 3.4).
    let run = Run::records(
        &[],
        &[&datum_plane(
            r#"{"kind": "origin", "plane": "XY"}"#,
            r#", "offset": "-2e6""#,
        )],
    );

    run.assert_error("f-dp1", "out_of_range");
    assert_eq!(
        run.messages("f-dp1"),
        ["Offset must be at least -1000000 mm, but it is -2000000 mm. Use -1000000 mm or more."],
    );
    assert_eq!(
        run.feature("f-dp1").messages[0].suggestion.as_deref(),
        Some("-1000000 mm")
    );
}
