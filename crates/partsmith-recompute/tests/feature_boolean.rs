//! Acceptance tests of the `boolean` feature (docs/spec/features/boolean.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/boolean/`.

use crate::feature_support;

use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_error_field, assert_examples_canonical,
    assert_face_count, assert_face_kind, assert_face_position, assert_has_face, assert_solid_count,
    assert_unchanged, assert_volume, body_changes, face, id, link_ids,
};
use partsmith_features::{Because, Diagnosis, Fix};
use partsmith_occt_sys::GeometryKind;

/// The bodies of every boolean test.
const BODIES: [&str; 3] = ["b-1", "b-2", "b-3"];

/// The `BOX1` fixture: b-1 spanning [-10,-10,0]-[10,10,20].
const BOX1: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}"#;

/// The `BOX2` fixture: b-2 spanning [0,-10,0]-[20,10,20].
const BOX2: &str = r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "position": ["10", "0", "0"], "op": "new"}"#;

/// The `BOOL(tools, op)` fixture, with `tools` given as a JSON array.
fn boolean(tools: &str, op: &str) -> String {
    format!(
        r#"{{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": {tools}, "op": "{op}"}}"#
    )
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("boolean");
}

#[test]
fn t001_join_volume_faces_and_bounds() {
    let run = Run::example("boolean/join.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 12000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [20.0, 10.0, 20.0]);
}

#[test]
fn t001_join_keeps_target_and_tool_face_names() {
    let run = Run::example("boolean/join.ptsm");

    let target = run.body("b-1");
    assert_has_face(target, "f-box1/left");
    assert_has_face(target, "f-box2/right");
}

#[test]
fn t001_join_consumes_the_tool() {
    let run = Run::example("boolean/join.ptsm");

    assert_empty(&run, "b-2");
    assert_eq!(body_changes(&run, "f-bool1"), ["b-1 Join", "b-2 Clear"]);
}

#[test]
fn t002_join_keeping_tools_leaves_the_tool() {
    let run = Run::example("boolean/join-keep-tools.ptsm");

    run.assert_all_ok();
    assert_volume(run.shape("b-1"), 12000.0);
    assert_volume(run.shape("b-2"), 8000.0);
    assert_unchanged(&run, "f-bool1", "b-2");
    assert_eq!(body_changes(&run, "f-bool1"), ["b-1 Join"]);
}

#[test]
fn t002_join_keeping_tools_reads_the_tool() {
    let run = Run::example("boolean/join-keep-tools.ptsm");

    let dependencies = run.dependencies("f-bool1");
    assert_eq!(link_ids(&dependencies.bodies_read), ["b-2"]);
    assert_eq!(link_ids(&dependencies.bodies_written), ["b-1"]);
}

#[test]
fn t003_cut_volume_faces_and_bounds() {
    let run = Run::example("boolean/cut.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 4000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [0.0, 10.0, 20.0]);
    assert_empty(&run, "b-2");
}

#[test]
fn t003_cut_names_the_new_wall_after_the_tool_face() {
    let run = Run::example("boolean/cut.ptsm");

    let wall = face(run.body("b-1"), "f-box2/left");
    assert_face_kind(&wall, GeometryKind::Plane);
    assert_face_position(&wall, 0, 0.0);
}

#[test]
fn t004_intersect() {
    let run = Run::example("boolean/intersect.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 4000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
}

#[test]
fn t005_cut_with_two_tools() {
    let run = Run::example("boolean/cut-two-tools.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 3750.0);
    assert_face_count(shape, 10);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [0.0, 10.0, 20.0]);
    assert_empty(&run, "b-2");
    assert_empty(&run, "b-3");
}

#[test]
fn t006_intersect_with_two_tools_keeps_what_is_inside_every_tool() {
    let run = Run::example("boolean/intersect-two-tools.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 250.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, -5.0, 15.0], [5.0, 5.0, 20.0]);
}

#[test]
fn t007_tool_is_the_target() {
    let run = Run::records(&BODIES, &[BOX1, &boolean(r#"["b-1"]"#, "join")]);

    let error = run.assert_error("f-bool1", "same_body");
    assert_error_field(error, "tools[0]");
    assert_eq!(
        run.messages("f-bool1"),
        ["Body 'Body 1' cannot be used twice here. Pick a different body for Tool bodies."]
    );
}

#[test]
fn t008_repeated_tool() {
    let run = Run::records(
        &BODIES,
        &[BOX1, BOX2, &boolean(r#"["b-2", "b-2"]"#, "join")],
    );

    let error = run.assert_error("f-bool1", "same_body");
    assert_error_field(error, "tools[1]");
}

#[test]
fn t009_empty_tool() {
    let run = Run::records(&BODIES, &[BOX1, &boolean(r#"["b-2"]"#, "join")]);

    let error = run.assert_error("f-bool1", "tool_empty");
    assert_error_field(error, "tools[0]");
    assert_eq!(
        run.messages("f-bool1"),
        ["Body 'Body 2' is empty here, so it cannot be used as a tool."]
    );
    assert_eq!(
        run.error_fixes("f-bool1", &Diagnosis::default()),
        [Fix::Pick {
            pointer: "/tools/0".to_string()
        }]
    );
    assert_unchanged(&run, "f-bool1", "b-1");
}

#[test]
fn t010_empty_target() {
    let run = Run::records(&BODIES, &[BOX2, &boolean(r#"["b-2"]"#, "join")]);

    run.assert_error("f-bool1", "body_empty");
    assert_eq!(
        run.messages("f-bool1"),
        [
            "Body 'Body 1' has no solid at this point, so this boolean has nothing to work on. Pick a body that has a solid."
        ]
    );
    assert_unchanged(&run, "f-bool1", "b-2");
}

#[test]
fn t011_disjoint_join() {
    let run = Run::example("boolean/join-disjoint.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 16000.0);
    assert_face_count(shape, 12);
    assert_solid_count(shape, 2);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [60.0, 10.0, 20.0]);
}

#[test]
fn t012_cut_that_misses() {
    let run = Run::example("boolean/cut-misses.ptsm");

    run.assert_warnings("f-bool1", &["no_effect"]);
    // A boolean has no direction, so its cut lies beside the body, with nothing to offer.
    assert_eq!(
        run.messages("f-bool1"),
        ["This cut does not touch body 'Body 1': it lies beside it."]
    );
    assert_eq!(run.feature("f-bool1").messages[0].fixes, []);
    assert_unchanged(&run, "f-bool1", "b-1");
    assert_volume(run.shape("b-1"), 8000.0);
    assert_empty(&run, "b-2");
}

#[test]
fn t013_cut_everything() {
    let run = Run::records(
        &BODIES,
        &[
            BOX1,
            r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["30", "30", "30"], "position": ["0", "0", "-5"], "op": "new"}"#,
            &boolean(r#"["b-2"]"#, "cut"),
        ],
    );

    run.assert_error("f-bool1", "empty_result");
    assert_eq!(
        run.messages("f-bool1"),
        [
            "This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction."
        ]
    );
    assert_unchanged(&run, "f-bool1", "b-1");
    assert_unchanged(&run, "f-bool1", "b-2");
}

#[test]
fn t014_no_tools() {
    let run = Run::records(&BODIES, &[BOX1, &boolean("[]", "join")]);

    run.assert_error("f-bool1", "empty_selection");
    assert_eq!(
        run.messages("f-bool1"),
        ["Select at least one body for Tool bodies."]
    );
}

#[test]
fn t015_tool_names_survive_for_a_later_fillet() {
    let run = Run::example("boolean/cut-then-fillet.ptsm");

    run.assert_ok("f-fil1");
    let shape = run.shape("b-1");
    assert_volume(shape, 3982.831853);
    assert_face_count(shape, 7);
}

/// `BOX2` suppressed.
const BOX2_SUPPRESSED: &str = r#"{"id": "f-box2", "type": "box", "suppressed": true, "body": "b-2", "size": ["20", "20", "20"], "position": ["10", "0", "0"], "op": "new"}"#;

#[test]
fn t017_a_tool_whose_feature_is_suppressed() {
    let run = Run::records(
        &BODIES,
        &[BOX1, BOX2_SUPPRESSED, &boolean(r#"["b-2"]"#, "join")],
    );
    // Recompute's diagnosis finds the suppressed writer of b-2 (recompute.md section 3.10, `because`).
    let diagnosis = Diagnosis {
        because: Some(Because::Suppressed(id("f-box2"))),
        ..Diagnosis::default()
    };

    run.assert_error("f-bool1", "tool_empty");
    assert_eq!(
        run.render_error("f-bool1", &diagnosis),
        "Body 'Body 2' is empty here because 'Box 2', which makes it, is suppressed, so it cannot be used as a tool."
    );
    assert_eq!(
        run.error_fixes("f-bool1", &diagnosis),
        [
            Fix::Unsuppress {
                feature: id("f-box2")
            },
            Fix::Pick {
                pointer: "/tools/0".to_string()
            },
        ]
    );
    assert_unchanged(&run, "f-bool1", "b-1");
}

#[test]
fn t017_unsuppressing_the_tool_gives_test_1() {
    // Test 1's document.
    let run = Run::example("boolean/join.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 12000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [20.0, 10.0, 20.0]);
}
