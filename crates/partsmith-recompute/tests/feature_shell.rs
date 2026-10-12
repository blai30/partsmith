//! Acceptance tests of the `shell` feature (docs/spec/features/shell.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/shell/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_element_repair, assert_error_field,
    assert_face_area, assert_face_count, assert_face_kind, assert_face_position, assert_list_field,
    assert_same_body, assert_solid_count, assert_volume, body_update, document, example_paths,
    face, face_index, face_names_with_prefix, load_example, median_build_time, output_body,
    suggested_limit,
};
use partsmith_features::{BodyChange, FeatureError};
use partsmith_model::{Document, Units};
use partsmith_occt_sys::GeometryKind;

/// `BOX805030`: an 80 x 50 x 30 mm box in `b-1` spanning [-40,-25,0]-[40,25,30], volume 120000.
const BOX805030: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["80", "50", "30"], "op": "new"}"#;

/// `F-TOP`: the top face of `BOX805030`.
const TOP_FACE: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000"}"#;

/// `F-LOST`: a face reference that matches nothing.
const LOST_FACE: &str = r#"{"q": "f-box9/top", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}"#;

/// `SHELL(thickness, faces)`: the shell `f-sh1` of `b-1`, without `remove_faces` when `faces` is
/// empty.
fn shell_record(thickness: &str, faces: &[&str]) -> String {
    if faces.is_empty() {
        format!(r#"{{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "{thickness}"}}"#)
    } else {
        format!(
            r#"{{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "{thickness}", "remove_faces": [{}]}}"#,
            faces.join(", "),
        )
    }
}

/// Test 5: `BOX805030` shelled with the top removed and a wall of `thickness`.
fn open_top_document(thickness: &str) -> Document {
    document(
        &["b-1"],
        &[BOX805030, &shell_record(thickness, &[TOP_FACE])],
    )
}

/// The geometry and names of test 1: `BOX805030` shelled 2 mm with the top removed.
fn assert_open_top_geometry(run: &Run) {
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 22112.0);
    assert_face_count(shape, 11);
    assert_bounding_box(shape, [-40.0, -25.0, 0.0], [40.0, 25.0, 30.0]);
    let inner_bottom = face(body, "f-sh1/inner/f-box1/bottom");
    assert_face_kind(&inner_bottom, GeometryKind::Plane);
    assert_face_position(&inner_bottom, 2, 2.0);
    assert_face_area(&inner_bottom, 3496.0);
    let inner_front = face(body, "f-sh1/inner/f-box1/front");
    assert_face_kind(&inner_front, GeometryKind::Plane);
    assert_face_position(&inner_front, 1, -23.0);
    assert_face_area(&inner_front, 2128.0);
    let rim = face(body, "f-sh1/rim/f-box1/top");
    assert_face_kind(&rim, GeometryKind::Plane);
    assert_face_position(&rim, 2, 30.0);
    assert_face_area(&rim, 504.0);
    assert_eq!(
        face_index(body, "f-box1/top"),
        None,
        "the removed top face disappears"
    );
    assert_face_area(&face(body, "f-box1/bottom"), 4000.0);
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("shell");
    assert!(!paths.is_empty(), "examples/shell has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_open_top() {
    let run = Run::example("shell/open-top.ptsm");

    run.assert_all_ok();
    assert_open_top_geometry(&run);
    let change = &body_update(&run, "f-sh1", "b-1").change;
    assert!(
        matches!(change, BodyChange::Modify),
        "a shell modifies its body, got {change:?}"
    );
}

#[test]
fn t002_two_faces_removed() {
    let run = Run::example("shell/two-faces-removed.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 17856.0);
    assert_face_count(shape, 10);
    assert_face_area(&face(body, "f-sh1/rim/f-box1/top"), 352.0);
    assert_face_area(&face(body, "f-sh1/rim/f-box1/front"), 272.0);
}

#[test]
fn t003_closed_hollow() {
    let run = Run::example("shell/closed-hollow.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 29104.0);
    assert_face_count(shape, 12);
    assert_bounding_box(shape, [-40.0, -25.0, 0.0], [40.0, 25.0, 30.0]);
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/inner/"),
        [
            "f-sh1/inner/f-box1/back",
            "f-sh1/inner/f-box1/bottom",
            "f-sh1/inner/f-box1/front",
            "f-sh1/inner/f-box1/left",
            "f-sh1/inner/f-box1/right",
            "f-sh1/inner/f-box1/top",
        ],
        "one inner wall per outer face"
    );
    assert!(
        face_index(body, "f-box1/top").is_some(),
        "a closed hollow keeps the top face"
    );
    assert_solid_count(shape, 1);
}

#[test]
fn t004_parameter_thickness() {
    let run = Run::example("shell/parameter-thickness.ptsm");

    run.assert_all_ok();
    assert_open_top_geometry(&run);
    let change = &body_update(&run, "f-sh1", "b-1").change;
    assert!(
        matches!(change, BodyChange::Modify),
        "a shell modifies its body, got {change:?}"
    );
}

#[test]
fn t005_too_thick() {
    let run = Run::new(open_top_document("30"));

    run.assert_ok("f-box1");
    let error = run.assert_error("f-sh1", "shell_too_thick");
    assert_error_field(error, "thickness");
    let FeatureError::ShellTooThick { thickness, max, .. } = error else {
        panic!("expected ShellTooThick, got {error:?}");
    };
    assert_eq!(*thickness, 30.0, "the requested thickness");
    let max = max.unwrap_or_else(|| panic!("the search finds a working thickness: {error:?}"));
    assert!(
        (22.5..25.0).contains(&max),
        "the thickest working wall {max} lies in [22.5, 25)"
    );
    let limit = suggested_limit(max);
    let expected = format!(
        "Shell thickness 30 mm is too thick for body 'Body 1'. The thickest wall that works is about {limit} mm. Try {limit} mm or less."
    );
    assert_eq!(run.messages("f-sh1"), [expected.as_str()]);
    assert_eq!(error.suggestion(Units::Mm), Some(format!("{limit} mm")));
    assert_same_body(run.body("b-1"), &output_body(&run, "f-box1", "b-1"));

    let rebuilt = Run::new(open_top_document(&limit));
    rebuilt.assert_all_ok();
}

#[test]
fn t006_zero_thickness() {
    let run = Run::new(open_top_document("0"));

    run.assert_error("f-sh1", "out_of_range");
    assert_eq!(
        run.messages("f-sh1"),
        ["Thickness must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
    );
}

#[test]
fn t007_removed_face_lost() {
    let run = Run::records(&["b-1"], &[BOX805030, &shell_record("2", &[LOST_FACE])]);

    run.assert_error("f-sh1", "references_not_found");
    assert_eq!(
        run.messages("f-sh1"),
        [
            "The face selected for Faces to remove can no longer be found. Re-pick it in the repair panel."
        ]
    );
    assert_same_body(run.body("b-1"), &output_body(&run, "f-box1", "b-1"));
}

#[test]
fn t008_one_of_two_removed_faces_lost() {
    let run = Run::example("shell/one-face-lost.ptsm");

    run.assert_ok("f-box1");
    run.assert_warnings("f-sh1", &["reference_missing"]);
    let warning = &run.feature("f-sh1").warnings[0];
    assert_list_field(warning.field(), "remove_faces", 1);
    assert_element_repair(warning.repair(), "/remove_faces/1", "b-1");
    assert_eq!(
        run.messages("f-sh1"),
        [
            "One of the faces selected for Faces to remove can no longer be found, so it was skipped. Re-pick it in the repair panel."
        ]
    );
    assert_open_top_geometry(&run);
}

#[test]
fn t009_cylinder_cup() {
    let run = Run::example("shell/cylinder-cup.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 11586.193706);
    assert_face_count(shape, 5);
    assert_bounding_box(shape, [-20.0, -20.0, 0.0], [20.0, 20.0, 40.0]);
}

#[test]
fn t010_two_solids() {
    let run = Run::example("shell/two-solids.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 2.0 * (8000.0 - 16.0 * 16.0 * 18.0));
    assert_volume(shape, 6784.0);
    assert_face_count(shape, 22);
    assert_solid_count(shape, 2);
}

#[test]
fn t011_empty_body() {
    let run = Run::records(&["b-1"], &[&shell_record("2", &[])]);

    run.assert_error("f-sh1", "body_empty");
    assert_eq!(
        run.messages("f-sh1"),
        [
            "Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid."
        ]
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds; run with cargo test --release"
)]
fn t012_performance() {
    let open_top = median_build_time(&load_example("shell/open-top.ptsm"), "f-sh1", 5);
    assert!(
        open_top < Duration::from_millis(50),
        "shelling the open-top box took {open_top:?}, the budget is 50 ms"
    );
}
