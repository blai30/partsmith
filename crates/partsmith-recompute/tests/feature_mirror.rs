//! Acceptance tests of the `mirror` feature (docs/spec/features/mirror.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/mirror/`; the document of test 13 (a sketch on the mirror image, ADR 0023) is written
//! here until its example exists.

use crate::feature_support;

use feature_support::{
    AXIS_TOLERANCE, Run, THROUGH_ALL, assert_bore_through, assert_bounding_box, assert_coordinates,
    assert_error_field, assert_examples_canonical, assert_face_area, assert_face_count,
    assert_face_kind, assert_face_normal, assert_face_position, assert_has_face, assert_moves,
    assert_sketch_on_top_plane, assert_solid_count, assert_unchanged, assert_volume,
    assert_volume_faces_and_bounds, body_changes, document, face, face_motion, face_sketch_with,
    hole_at_point, link_ids, message, one_point, pick, set_field, sketch_result,
};
use partsmith_features::{FeatureWarning, NoEffectHow};
use partsmith_occt_sys::GeometryKind;
use serde_json::json;

/// The bodies of every mirror test.
const BODIES: [&str; 2] = ["b-1", "b-2"];

/// The origin plane `YZ` as a plane reference.
const YZ: &str = r#"{"kind": "origin", "plane": "YZ"}"#;

/// The `BOX-R(x)` fixture: a 10 mm cube in b-1 spanning [x-5,-5,0]-[x+5,5,10].
fn box_r(x: &str) -> String {
    format!(
        r#"{{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["{x}", "0", "0"], "op": "new"}}"#
    )
}

/// The `MIR(plane, extra)` fixture; `extra` holds further members such as `"op": "new"`, or is empty.
fn mirror(plane: &str, extra: &str) -> String {
    let extra = if extra.is_empty() {
        String::new()
    } else {
        format!(", {extra}")
    };
    format!(r#"{{"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {plane}{extra}}}"#)
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("mirror");
}

#[test]
fn t001_join_apart_volume_faces_and_bounds() {
    let run = Run::example("mirror/join-apart.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_face_count(shape, 12);
    assert_solid_count(shape, 2);
    assert_bounding_box(shape, [-15.0, -5.0, 0.0], [15.0, 5.0, 10.0]);
}

#[test]
fn t001_join_apart_image_top_is_mirrored() {
    let run = Run::example("mirror/join-apart.ptsm");

    // Planar at z = 10, spanning x -15..-5: centered at x = -10 with the full 10 by 10 area.
    let top = face(run.body("b-1"), "f-box1/top@f-mir1:1");
    assert_face_kind(&top, GeometryKind::Plane);
    assert_face_position(&top, 2, 10.0);
    assert_face_position(&top, 0, -10.0);
    assert_face_area(&top, 100.0);
}

#[test]
fn t001_join_apart_image_right_faces_outward() {
    let run = Run::example("mirror/join-apart.ptsm");

    let image = face(run.body("b-1"), "f-box1/right@f-mir1:1");
    assert_face_kind(&image, GeometryKind::Plane);
    assert_face_position(&image, 0, -15.0);
    assert_face_normal(&image, [-1.0, 0.0, 0.0]);
}

#[test]
fn t001_join_apart_source_keeps_its_names() {
    let run = Run::example("mirror/join-apart.ptsm");

    let right = face(run.body("b-1"), "f-box1/right");
    assert_face_position(&right, 0, 15.0);
}

#[test]
fn t001_join_apart_changes_the_source_by_join() {
    let run = Run::example("mirror/join-apart.ptsm");

    assert_eq!(body_changes(&run, "f-mir1"), ["b-1 Join"]);
}

#[test]
fn t002_join_touching_fuses() {
    let run = Run::example("mirror/join-touching.ptsm");

    run.assert_all_ok();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        2000.0,
        6,
        [-10.0, -5.0, 0.0],
        [10.0, 5.0, 10.0],
    );
}

#[test]
fn t003_new_body_leaves_the_source() {
    let run = Run::example("mirror/new-body.ptsm");

    run.assert_all_ok();
    assert_unchanged(&run, "f-mir1", "b-1");
    let source = run.shape("b-1");
    assert_volume(source, 1000.0);
    assert_bounding_box(source, [5.0, -5.0, 0.0], [15.0, 5.0, 10.0]);
}

#[test]
fn t003_new_body_holds_the_image() {
    let run = Run::example("mirror/new-body.ptsm");

    assert_volume_faces_and_bounds(
        run.shape("b-2"),
        1000.0,
        6,
        [-15.0, -5.0, 0.0],
        [-5.0, 5.0, 10.0],
    );
    assert_has_face(run.body("b-2"), "f-box1/top@f-mir1:1");
    assert_eq!(body_changes(&run, "f-mir1"), ["b-2 New"]);
}

#[test]
fn t003_new_body_reads_the_source_and_writes_the_target() {
    let run = Run::example("mirror/new-body.ptsm");

    let dependencies = run.dependencies("f-mir1");
    assert_eq!(link_ids(&dependencies.bodies_read), ["b-1"]);
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-2"]);
}

#[test]
fn t004_new_without_target() {
    let run = Run::records(&BODIES, &[&box_r("10"), &mirror(YZ, r#""op": "new""#)]);

    let error = run.assert_error("f-mir1", "target_required");
    assert_error_field(error, "target");
    assert_eq!(
        run.messages("f-mir1"),
        [
            "Operation New body puts the copies into another body, so Target body needs an empty body. Pick one, or set Operation to Join."
        ]
    );
    assert_eq!(
        message(&run, "f-mir1", "target_required").fixes,
        [set_field("op", "Operation", json!("join")), pick("/target")]
    );
}

#[test]
fn t005_target_with_join() {
    let run = Run::records(&BODIES, &[&box_r("10"), &mirror(YZ, r#""target": "b-2""#)]);

    let error = run.assert_error("f-mir1", "target_not_allowed");
    assert_error_field(error, "target");
    assert_eq!(
        run.messages("f-mir1"),
        [
            "Target body is used only when Operation is New body. Clear Target body, or set Operation to New body."
        ]
    );
    assert_eq!(
        message(&run, "f-mir1", "target_not_allowed").fixes,
        [
            set_field("op", "Operation", json!("new")),
            set_field("target", "Target body", serde_json::Value::Null),
        ]
    );
}

#[test]
fn t006_target_with_a_solid() {
    let run = Run::records(
        &BODIES,
        &[
            &box_r("10"),
            r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}"#,
            &mirror(YZ, r#""op": "new", "target": "b-2""#),
        ],
    );

    run.assert_error("f-mir1", "body_not_empty");
    assert_eq!(
        run.messages("f-mir1"),
        [
            "Body 'Body 2' already has a solid here, so this mirror cannot put its result there. Pick an empty body for Target body."
        ]
    );
    assert_eq!(
        message(&run, "f-mir1", "body_not_empty").fixes,
        [pick("/target")]
    );
    assert_volume(run.shape("b-2"), 1.0);
    assert_unchanged(&run, "f-mir1", "b-2");
}

#[test]
fn t007_target_is_the_source() {
    let run = Run::records(
        &BODIES,
        &[&box_r("10"), &mirror(YZ, r#""op": "new", "target": "b-1""#)],
    );

    let error = run.assert_error("f-mir1", "same_body");
    assert_error_field(error, "target");
}

#[test]
fn t008_across_a_datum() {
    let run = Run::example("mirror/across-datum.ptsm");

    run.assert_all_ok();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        2000.0,
        12,
        [5.0, -5.0, 0.0],
        [35.0, 5.0, 10.0],
    );
}

#[test]
fn t009_across_its_own_face() {
    let run = Run::example("mirror/across-own-face.ptsm");

    run.assert_all_ok();
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        2000.0,
        6,
        [5.0, -5.0, 0.0],
        [25.0, 5.0, 10.0],
    );
}

#[test]
fn t010_symmetric_body_has_no_effect() {
    let run = Run::example("mirror/symmetric.ptsm");

    run.assert_warnings("f-mir1", &["no_effect"]);
    assert!(
        matches!(
            run.warning("f-mir1", "no_effect"),
            FeatureWarning::NoEffect {
                how: NoEffectHow::Inside,
                ..
            }
        ),
        "{:?}",
        run.messages("f-mir1")
    );
    assert_eq!(
        run.messages("f-mir1"),
        [
            "This mirror adds nothing to body 'Body 1': its mirror image lies entirely inside the body."
        ]
    );
    assert_unchanged(&run, "f-mir1", "b-1");
}

#[test]
fn t011_empty_source() {
    let run = Run::records(&BODIES, &[&mirror(YZ, "")]);

    run.assert_error("f-mir1", "body_empty");
    assert_eq!(
        run.messages("f-mir1"),
        [
            "Body 'Body 1' has no solid at this point, so this mirror has nothing to work on. Pick a body that has a solid."
        ]
    );
}

// Test 13: the reflection that makes the image is the motion of its faces, so a sketch whose fallback is in body
// coordinates keeps its place and drawing on the image (features.md section 3.6, open issue 9).

/// The document of test 13: a datum plane on YZ at `offset`, `BOX-R(10)`, its mirror image across the datum, a face
/// sketch on the image's top with its fallback stored as its reflection, in the coordinates of 'Box 1', and a hole
/// through all at its point (2, 3).
fn sketch_on_image_document(offset: &str) -> partsmith_model::Document {
    document(
        &BODIES,
        &[
            &format!(
                r#"{{"id": "f-dp1", "type": "datum_plane", "base": {{"kind": "origin", "plane": "YZ"}}, "offset": "{offset}"}}"#
            ),
            &box_r("10"),
            &mirror(r#"{"kind": "datum", "feature": "f-dp1"}"#, ""),
            &face_sketch_with(
                r#"{"q": "f-box1/top@f-mir1:1", "fp": "plane|n=0,0,1|c=-10,0,10|a=100|adj=4,4"}"#,
                r#"{"origin": [10, 0, 10], "x_axis": [-1, 0, 0], "normal": [0, 0, 1]}"#,
                Some("body"),
                &one_point([2.0, 3.0]),
            ),
            &hole_at_point("1", THROUGH_ALL),
        ],
    )
}

#[test]
fn t013_a_sketch_on_the_mirror_image_keeps_its_place() {
    let run = Run::new(sketch_on_image_document("0"));

    // No `sketch_turned_over`: the reflection moves the hint with the face.
    run.assert_all_ok();
    assert_sketch_on_top_plane(&run, "f-sk2", [-10.0, 0.0, 10.0]);
    // Right-handed: the y axis is the normal times the x axis.
    assert_coordinates(
        "y axis",
        sketch_result(&run, "f-sk2").placement.y_axis(),
        [0.0, 1.0, 0.0],
        AXIS_TOLERANCE,
    );
    assert_bore_through(&run, [-8.0, 3.0, 0.0]);
    assert_volume(run.shape("b-1"), 1992.146018);
}

#[test]
fn t013_the_sketch_follows_the_mirror_plane() {
    // The plane moves to x = -10 and the image to x -35..-25.
    let run = Run::new(sketch_on_image_document("-10"));

    run.assert_all_ok();
    assert_sketch_on_top_plane(&run, "f-sk2", [-30.0, 0.0, 10.0]);
    assert_bore_through(&run, [-28.0, 3.0, 0.0]);
    assert_volume(run.shape("b-1"), 1992.146018);
}

#[test]
fn the_image_faces_move_by_the_reflection() {
    // Section 3.3 step 3: the image is named with the reflection, an improper motion, as its transform.
    let run = Run::records(&BODIES, &[&box_r("10"), &mirror(YZ, "")]);

    let motion = face_motion(&run, "f-mir1", "b-1", "f-box1/top@f-mir1:1");
    assert_moves(
        &motion,
        &[
            ([10.0, 0.0, 10.0], [-10.0, 0.0, 10.0]),
            ([12.0, 3.0, 4.0], [-12.0, 3.0, 4.0]),
        ],
    );
    assert!(!motion.is_proper(), "a reflection turns handedness");
}
