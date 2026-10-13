//! Acceptance tests of the `shell` feature (docs/spec/features/shell.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/shell/`.

mod feature_support;

use std::f64::consts::PI;
use std::time::Duration;

use feature_support::{
    CUBE20, Run, assert_bounding_box, assert_element_repair, assert_error_field,
    assert_examples_canonical, assert_face_area, assert_face_count, assert_face_kind,
    assert_face_normal, assert_face_position, assert_face_radius, assert_has_face,
    assert_list_field, assert_median_build_within, assert_solid_count, assert_unchanged,
    assert_volume, body_update, document, face, face_index, face_names_with_prefix, load_example,
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

/// The L-shaped body of fillet.md test 4: a 40 x 20 x 10 mm box with a 20 x 20 x 10 mm box
/// joined on the right half of its top, spanning [-20,-10,0]-[20,10,20].
const L_BODY: [&str; 2] = [
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}"#,
    r#"{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "position": ["10", "0", "10"], "op": "join"}"#,
];

/// The step face of `L_BODY`, the left half of the lower box's top, which meets the taller box at
/// a concave edge.
const STEP_FACE: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=-10,0,10|a=400"}"#;

/// `F-ROUND`: `CUBE20`'s four edges parallel to Z rounded at radius 2 by `f-fil1`, which gives the
/// body of ui.md's `cube-features.ptsm` (volume 7680 + 80 pi, 10 faces).
const VERTICAL_ROUNDS: &str = r#"{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-10,-10,10|l=20"}, {"q": "edge(f-box1/front|f-box1/right)", "fp": "line|n=0,0,1|c=10,-10,10|l=20"}, {"q": "edge(f-box1/back|f-box1/left)", "fp": "line|n=0,0,1|c=-10,10,10|l=20"}, {"q": "edge(f-box1/back|f-box1/right)", "fp": "line|n=0,0,1|c=10,10,10|l=20"}], "radius": "2"}"#;

/// The top face of `CUBE20` after `VERTICAL_ROUNDS`: a 20 mm square with corners of radius 2.
const ROUNDED_TOP_FACE: &str =
    r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=396.566371"}"#;

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

/// Tests 19 and 20: `CUBE20` with `VERTICAL_ROUNDS`, shelled with its top removed and a wall of
/// `thickness`.
fn rounded_cube_run(thickness: &str) -> Run {
    Run::records(
        &["b-1"],
        &[
            CUBE20,
            VERTICAL_ROUNDS,
            &shell_record(thickness, &[ROUNDED_TOP_FACE]),
        ],
    )
}

/// The checks of test 19 for every thickness: all ok, one solid, the volume, the face count and
/// the bounding box of the cube; the inner wall of the front face planar at y = -10 + t with the
/// area `front_area`; every face named from its origin.
fn assert_rounded_cube_shell(run: &Run, thickness: f64, volume: f64, faces: u32, front_area: f64) {
    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, volume);
    assert_face_count(shape, faces);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    let inner_front = face(body, "f-sh1/inner/f-box1/front");
    assert_face_kind(&inner_front, GeometryKind::Plane);
    assert_face_position(&inner_front, 1, -10.0 + thickness);
    assert_face_area(&inner_front, front_area);
    assert_eq!(
        face_names_with_prefix(body, "f-fil1/fillet/").len(),
        4,
        "the outer rounds keep their names"
    );
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/face"),
        Vec::<String>::new(),
        "every face has a name from its origin"
    );
}

/// Asserts that no inner face comes from a round: walls at least as thick as the radius close
/// into sharp inner corners.
fn assert_no_inner_rounds(run: &Run) {
    assert_eq!(
        face_names_with_prefix(run.body("b-1"), "f-sh1/inner/f-fil1/"),
        Vec::<String>::new(),
        "the rounds' inner offsets vanish"
    );
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
    assert_examples_canonical("shell");
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
    assert_has_face(body, "f-box1/top");
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
    assert_unchanged(&run, "f-sh1", "b-1");

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
    assert_unchanged(&run, "f-sh1", "b-1");
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

/// shell.md open issue 3: a removed face beside a concave edge. The step face of the L-shaped body
/// borders the concave edge where it meets the taller box, which the kernel's first attempt cannot
/// open; its fallback (kernel.md section 3.4.8 item 3) cuts out the cavity instead.
#[test]
fn removed_face_beside_a_concave_edge() {
    let run = Run::example("shell/step-face-removed.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 5344.0);
    assert_face_count(shape, 16);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-20.0, -10.0, 0.0], [20.0, 10.0, 20.0]);
    let rim = face(body, "f-sh1/rim/f-box1/top");
    assert_face_kind(&rim, GeometryKind::Plane);
    assert_face_position(&rim, 2, 10.0);
    assert_face_area(&rim, 112.0);
    let wall_bottom = face(body, "f-sh1/inner/f-box1/top");
    assert_face_kind(&wall_bottom, GeometryKind::Plane);
    assert_face_position(&wall_bottom, 2, 10.0);
    assert_face_normal(&wall_bottom, [0.0, 0.0, -1.0]);
    assert_face_area(&wall_bottom, 32.0);
    let step_wall = face(body, "f-sh1/inner/f-box2/left");
    assert_face_kind(&step_wall, GeometryKind::Plane);
    assert_face_position(&step_wall, 0, 2.0);
    assert_face_area(&step_wall, 128.0);
    assert_eq!(
        face_index(body, "f-box1/top"),
        None,
        "the removed step face disappears"
    );
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/inner/").len(),
        8,
        "one inner wall per kept face and the bottom of the wall over the opening"
    );
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/face"),
        Vec::<String>::new(),
        "every face has a name from its origin"
    );

    let thin = Run::records(
        &["b-1"],
        &[L_BODY[0], L_BODY[1], &shell_record("0.5", &[STEP_FACE])],
    );
    thin.assert_all_ok();
    assert_volume(thin.shape("b-1"), 1531.0);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t012_performance() {
    assert_median_build_within(
        &load_example("shell/open-top.ptsm"),
        "f-sh1",
        5,
        Duration::from_millis(50),
    );
}

#[test]
fn t013_thickness_below_the_tolerance() {
    let run = Run::new(open_top_document("0.0000001"));

    let error = run.assert_error("f-sh1", "out_of_range");
    assert_error_field(error, "thickness");
    assert_eq!(
        run.messages("f-sh1"),
        ["Thickness must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
    );
    assert_unchanged(&run, "f-sh1", "b-1");
}

#[test]
fn t014_moved_body() {
    let every_edge = load_example("fillet/every-edge.ptsm");
    let records: Vec<String> = every_edge
        .features
        .iter()
        .map(|record| serde_json::to_string(record).expect("records serialize"))
        .collect();
    let records: Vec<&str> = records.iter().map(String::as_str).collect();
    let moved_top = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,0,20|a=256"}"#;
    let top = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=256"}"#;
    let move_record =
        r#"{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["50", "0", "0"]}"#;
    let moved_shell = shell_record("1", &[moved_top]);
    let shell = shell_record("1", &[top]);

    let moved = Run::new(document(
        &["b-1"],
        &[records[0], records[1], move_record, &moved_shell],
    ));
    let unmoved = Run::new(document(&["b-1"], &[records[0], records[1], &shell]));

    moved.assert_all_ok();
    unmoved.assert_all_ok();
    let shape = moved.shape("b-1");
    let expected = unmoved.shape("b-1");
    assert_volume(shape, expected.volume().expect("a volume"));
    assert_eq!(
        shape.count(partsmith_occt_sys::SubShapeKind::Face),
        expected.count(partsmith_occt_sys::SubShapeKind::Face)
    );
    assert_bounding_box(shape, [40.0, -10.0, 0.0], [60.0, 10.0, 20.0]);
}

#[test]
fn t019_wall_thinner_than_the_rounds() {
    let run = rounded_cube_run("1");

    // The 20 mm prism minus a 19 mm deep cavity: an 18 mm square with corners of radius 1.
    assert_rounded_cube_shell(&run, 1.0, 1600.0 + 61.0 * PI, 19, 304.0);
    let inner_round = face(
        run.body("b-1"),
        "f-sh1/inner/f-fil1/fillet/edge(f-box1/front|f-box1/right)",
    );
    assert_face_kind(&inner_round, GeometryKind::Cylinder);
    assert_face_radius(&inner_round, 1.0);
}

#[test]
fn t019_wall_as_thick_as_the_rounds() {
    let run = rounded_cube_run("2");

    // The 20 mm prism minus a 18 mm deep cavity with a sharp 16 mm square section.
    assert_rounded_cube_shell(&run, 2.0, 3072.0 + 80.0 * PI, 15, 288.0);
    assert_no_inner_rounds(&run);
    let rim = face(run.body("b-1"), "f-sh1/rim/f-box1/top");
    assert_face_kind(&rim, GeometryKind::Plane);
    assert_face_position(&rim, 2, 20.0);
    assert_face_area(&rim, 128.0 + 4.0 * PI);
}

#[test]
fn t019_wall_thicker_than_the_rounds() {
    let run = rounded_cube_run("3");

    // The 20 mm prism minus a 17 mm deep cavity with a sharp 14 mm square section.
    assert_rounded_cube_shell(&run, 3.0, 4348.0 + 80.0 * PI, 15, 238.0);
    assert_no_inner_rounds(&run);
}

#[test]
fn t020_too_thick_past_the_rounds() {
    let run = rounded_cube_run("12");

    run.assert_ok("f-fil1");
    let error = run.assert_error("f-sh1", "shell_too_thick");
    assert_error_field(error, "thickness");
    let FeatureError::ShellTooThick { thickness, max, .. } = error else {
        panic!("expected ShellTooThick, got {error:?}");
    };
    assert_eq!(*thickness, 12.0, "the requested thickness");
    let max = max.unwrap_or_else(|| panic!("the search finds a working thickness: {error:?}"));
    assert!(
        (9.0..10.0).contains(&max),
        "the thickest working wall {max} lies in [9, 10), where the walls meet in the middle"
    );
    let limit = suggested_limit(max);
    assert_eq!(error.suggestion(Units::Mm), Some(format!("{limit} mm")));
    assert_unchanged(&run, "f-sh1", "b-1");

    let rebuilt = rounded_cube_run(&limit);
    rebuilt.assert_all_ok();
}
