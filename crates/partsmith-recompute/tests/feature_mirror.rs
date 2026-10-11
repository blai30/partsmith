//! Acceptance tests of the `mirror` feature (docs/spec/features/mirror.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/mirror/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_error_field, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_normal, assert_face_position,
    assert_solid_count, assert_unchanged, assert_volume, body_changes, example_paths, face,
    face_index, link_ids, load_example, median_build_time,
};
use partsmith_occt_sys::GeometryKind;

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
    let paths = example_paths("mirror");
    assert!(!paths.is_empty(), "examples/mirror has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
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
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [-10.0, -5.0, 0.0], [10.0, 5.0, 10.0]);
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

    let image = run.shape("b-2");
    assert_volume(image, 1000.0);
    assert_face_count(image, 6);
    assert_bounding_box(image, [-15.0, -5.0, 0.0], [-5.0, 5.0, 10.0]);
    assert!(
        face_index(run.body("b-2"), "f-box1/top@f-mir1:1").is_some(),
        "f-box1/top@f-mir1:1 exists in b-2",
    );
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
        ["Op new puts the copies into another body. Pick an empty body for Target body."]
    );
}

#[test]
fn t005_target_with_join() {
    let run = Run::records(&BODIES, &[&box_r("10"), &mirror(YZ, r#""target": "b-2""#)]);

    let error = run.assert_error("f-mir1", "target_not_allowed");
    assert_error_field(error, "target");
    assert_eq!(
        run.messages("f-mir1"),
        ["Target body is only used with op new. Remove it, or set the op to new."]
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
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_face_count(shape, 12);
    assert_bounding_box(shape, [5.0, -5.0, 0.0], [35.0, 5.0, 10.0]);
}

#[test]
fn t009_across_its_own_face() {
    let run = Run::example("mirror/across-own-face.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [5.0, -5.0, 0.0], [25.0, 5.0, 10.0]);
}

#[test]
fn t010_symmetric_body_has_no_effect() {
    let run = Run::records(&BODIES, &[&box_r("0"), &mirror(YZ, "")]);

    run.assert_warnings("f-mir1", &["no_effect"]);
    assert_unchanged(&run, "f-mir1", "b-1");
}

#[test]
fn t011_empty_source() {
    let run = Run::records(&BODIES, &[&mirror(YZ, "")]);

    run.assert_error("f-mir1", "body_empty");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t012_join_apart_builds_within_15_ms() {
    let document = load_example("mirror/join-apart.ptsm");

    let duration = median_build_time(&document, "f-mir1", 21);
    assert!(
        duration < Duration::from_millis(15),
        "the mirror took {duration:?} to build (median of 21 runs), over the 15 ms budget",
    );
}
