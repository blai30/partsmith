//! Acceptance tests of the `shell` feature (docs/spec/features/shell.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/shell/`.

use crate::feature_support;

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::ops::Range;
use std::path::Path;

use feature_support::shell::{ROUNDED_TOP_FACE, rounded_cube_document, shell_record};
use feature_support::{
    CUBE20, LENGTH_TOLERANCE, MOVE_BY_100, Run, assert_bounding_box, assert_coordinates,
    assert_element_repair, assert_error_field, assert_examples_canonical, assert_face_area,
    assert_face_count, assert_face_in_plane, assert_face_kind, assert_face_normal,
    assert_face_position, assert_face_radius, assert_has_face, assert_list_field,
    assert_moved_by_100, assert_no_face, assert_solid_count, assert_unchanged, assert_volume,
    body_update, document, face, face_index, face_names_with_prefix, load_example, parse_document,
    point_components, suggested_limit,
};
use feature_support::{assert_messages_pass_lint, id, message, pick, set_field};
use partsmith_features::{BodyChange, Diagnosis, Explanation, FeatureError, Fix, Limit};
use partsmith_model::{Document, Units};
use partsmith_occt_sys::{GeometryKind, SubShapeKind};

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

/// The top of the taller box of `L_BODY`, at z = 20.
const HIGH_TOP_FACE: &str = r#"{"q": "f-box2/top", "fp": "plane|n=0,0,1|c=10,0,20|a=400"}"#;

/// The rounded join of the inner walls at the concave edge where the step face of `L_BODY` meets
/// the taller box.
const ARC_JOIN: &str = "f-sh1/inner/edge(f-box1/top|f-box2/left)";

/// The box of test 15 (newcomer error corpus NC06): 40 x 40 x 20 mm, spanning
/// [-20,-20,0]-[20,20,20].
const BOX404020: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "20"], "op": "new"}"#;

/// The top face of `BOX404020` as test 15 stores it.
const BOX404020_TOP_FACE: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=1600"}"#;

/// Test 15: `BOX404020` with its four edges parallel to Z rounded by `f-fil1` at `radius`, shelled
/// 2 mm with its top removed.
fn nc06_run(radius: &str) -> Run {
    let rounds = format!(
        r#"{{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-20,-20,10|l=20"}}, {{"q": "edge(f-box1/front|f-box1/right)", "fp": "line|n=0,0,1|c=20,-20,10|l=20"}}, {{"q": "edge(f-box1/back|f-box1/left)", "fp": "line|n=0,0,1|c=-20,20,10|l=20"}}, {{"q": "edge(f-box1/back|f-box1/right)", "fp": "line|n=0,0,1|c=20,20,10|l=20"}}], "radius": "{radius}"}}"#
    );
    Run::records(
        &["b-1"],
        &[
            BOX404020,
            &rounds,
            &shell_record("2", &[BOX404020_TOP_FACE]),
        ],
    )
}

/// The cylinder of tests 9 and 16: radius 20 and height 40, spanning [-20,-20,0]-[20,20,40].
const CYL20: &str = r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "20", "height": "40", "op": "new"}"#;

/// The top face of `CYL20`.
const CYLINDER_TOP_FACE: &str =
    r#"{"q": "f-cyl1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=1256.637061"}"#;

/// Test 5: `BOX805030` shelled with the top removed and a wall of `thickness`.
fn open_top_document(thickness: &str) -> Document {
    document(
        &["b-1"],
        &[BOX805030, &shell_record(thickness, &[TOP_FACE])],
    )
}

/// Test 16: `CYL20` shelled with its top removed and a wall of `thickness`.
fn cylinder_cup_run(thickness: &str) -> Run {
    Run::records(
        &["b-1"],
        &[CYL20, &shell_record(thickness, &[CYLINDER_TOP_FACE])],
    )
}

/// Test 21: `L_BODY` with its vertical edge between the lower box's front and left faces rounded
/// by `f-fil1` at `radius` (none when `None`), shelled 2 mm with `HIGH_TOP_FACE` removed.
fn rounded_l_run(radius: Option<&str>) -> Run {
    let shell = shell_record("2", &[HIGH_TOP_FACE]);
    match radius {
        Some(radius) => {
            let round = format!(
                r#"{{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-20,-10,5|l=10"}}], "radius": "{radius}"}}"#
            );
            Run::records(&["b-1"], &[L_BODY[0], L_BODY[1], &round, &shell])
        }
        None => Run::records(&["b-1"], &[L_BODY[0], L_BODY[1], &shell]),
    }
}

/// The checks of test 21: all ok, one solid, the volume and the face count, and the arc join at
/// the concave edge, a cylinder of radius 2 (the wall).
fn assert_arc_join_kept(run: &Run, volume: f64, faces: u32) {
    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, volume);
    assert_face_count(shape, faces);
    assert_solid_count(shape, 1);
    let join = face(run.body("b-1"), ARC_JOIN);
    assert_face_kind(&join, GeometryKind::Cylinder);
    assert_face_radius(&join, 2.0);
}

/// Tests 19, 20 and 22: `CUBE20` with `VERTICAL_ROUNDS`, shelled with its top removed and a wall of
/// `thickness`.
fn rounded_cube_run(thickness: &str) -> Run {
    Run::new(rounded_cube_document(thickness, &[ROUNDED_TOP_FACE]))
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

/// Round faces a "too large" shell names (features.md section 4.3) that no fillet made: the feature that made them, its
/// label, and their smallest radius in mm.
struct RoundFaces {
    feature: &'static str,
    label: &'static str,
    radius: f64,
}

/// Asserts that `f-sh1` of `run` fails with `shell_too_thick` on `thickness` for the wall `requested`, with the
/// thickest working wall in `max_range`, the round faces `round_faces` as its limit (none when None) and `b-1`
/// unchanged, and returns its limit `{s}`, rounded down to 2 significant digits. The message is the "too large" text of
/// features.md section 4.3, and the only fix is Use {s} mm, since no fillet made the round faces.
fn assert_too_thick(
    run: &Run,
    requested: f64,
    max_range: Range<f64>,
    round_faces: Option<RoundFaces>,
) -> String {
    let error = run.assert_error("f-sh1", "shell_too_thick");
    assert_error_field(error, "thickness");
    let FeatureError::ShellTooThick {
        thickness,
        max,
        limit: found_limit,
        ..
    } = error
    else {
        panic!("expected ShellTooThick, got {error:?}");
    };
    assert_eq!(*thickness, requested, "the requested thickness");
    let max = max.unwrap_or_else(|| panic!("the search finds a working thickness: {error:?}"));
    assert!(
        max_range.contains(&max),
        "the thickest working wall {max} lies in {max_range:?}"
    );
    let limit = suggested_limit(max);
    let mut expected = format!(
        "Shell thickness {requested} mm is too thick for body 'Body 1': the largest that works is about {limit} mm."
    );
    match (&round_faces, found_limit) {
        (None, None) => {}
        (
            Some(round_faces),
            Some(Limit::RoundFaces {
                feature,
                radius,
                fillet,
            }),
        ) => {
            assert_eq!(
                feature,
                &id(round_faces.feature),
                "the feature of the round faces"
            );
            assert!(
                (radius - round_faces.radius).abs() <= 1e-6,
                "round faces of radius {radius}, expected {}",
                round_faces.radius
            );
            assert!(!fillet, "no fillet made them");
            expected.push_str(&format!(
                " Its rounded faces made by '{}' have radius {} mm, smaller than the wall.",
                round_faces.label, round_faces.radius
            ));
        }
        (expected_faces, found) => panic!(
            "expected the round faces {:?}, got the limit {found:?}",
            expected_faces.as_ref().map(|faces| faces.feature)
        ),
    }
    assert_eq!(run.messages("f-sh1"), [expected.as_str()]);
    assert_eq!(
        message(run, "f-sh1", "shell_too_thick").fixes,
        vec![set_field(
            "thickness",
            "Thickness",
            serde_json::Value::String(format!("{limit} mm"))
        )],
        "shell_too_thick offers its limit"
    );
    assert_eq!(error.suggestion(Units::Mm), Some(format!("{limit} mm")));
    assert_unchanged(run, "f-sh1", "b-1");
    limit
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
    // The box has no round faces, so the limit stands alone.
    let limit = assert_too_thick(&run, 30.0, 22.5..25.0, None);

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
    // Recompute's diagnosis: `f-box9` is not in the document, which the face's description already says.
    let diagnosis = Diagnosis {
        losses: BTreeMap::from([(
            0,
            Explanation {
                text: "is gone".to_string(),
                after: None,
                fixes: Vec::new(),
                upstream: false,
            },
        )]),
        ..Diagnosis::default()
    };
    assert_eq!(
        run.render_error("f-sh1", &diagnosis),
        "A face of a feature that is not in this document selected for Faces to remove is gone."
    );
    let fixes = run.error_fixes("f-sh1", &diagnosis);
    let (pick, candidates) = fixes.split_last().expect("the error has fixes");
    assert_eq!(
        pick,
        &Fix::Pick {
            pointer: "/remove_faces/0".to_string()
        }
    );
    assert!(
        !candidates.is_empty()
            && candidates.iter().all(|fix| matches!(
                fix,
                Fix::UseReferences { pointer, references } if pointer == "/remove_faces/0" && references.len() == 1
            )),
        "a UseReferences per candidate face, then Pick: {fixes:?}"
    );
    assert_messages_pass_lint(&run, "f-sh1");
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
        message(&run, "f-sh1", "reference_missing")
            .pointer
            .as_deref(),
        Some("/remove_faces/1")
    );
    assert_messages_pass_lint(&run, "f-sh1");
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

    // The second box does not touch the first, so its join leaves two pieces (features.md section 3.5).
    run.assert_ok("f-box1");
    run.assert_warnings("f-box2", &["separate_solid"]);
    run.assert_ok("f-sh1");
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
            "Body 'Body 1' has no solid at this point, so this shell has nothing to work on. Pick a body that has a solid."
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
fn t013_thickness_below_the_tolerance() {
    let run = Run::new(open_top_document("0.0000001"));

    let error = run.assert_error("f-sh1", "out_of_range");
    assert_error_field(error, "thickness");
    assert_eq!(
        run.messages("f-sh1"),
        [
            "Thickness must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ]
    );
    assert_unchanged(&run, "f-sh1", "b-1");
}

/// The wall end of test 14 along the front edge of the top of `examples/fillet/every-edge`, where
/// the top met its front round tangentially (kernel.md section 3.4.8 item 7).
const FRONT_WALL_END: &str =
    "f-sh1/inner/edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/top))";

/// Asserts that the top of test 14's cube, centered at x = `center_x`, is open: no planar face is
/// left at z = 20 facing up or down, and the wall along its front edge ends in a 16 x 1 mm face
/// normal to it at y = -8, 1 mm under the top.
fn assert_top_open(run: &Run, center_x: f64) {
    let shape = run.shape("b-1");
    let faces = shape.elements(SubShapeKind::Face).expect("faces");
    let left = faces.iter().filter(|element| {
        element.kind == GeometryKind::Plane
            && (element.centroid.z - 20.0).abs() <= 1e-6
            && element
                .direction
                .is_some_and(|direction| (direction.z.abs() - 1.0).abs() <= 1e-9)
    });
    assert_eq!(left.count(), 0, "no face is left where the top was");
    let end = face(run.body("b-1"), FRONT_WALL_END);
    assert_face_kind(&end, GeometryKind::Plane);
    assert_face_area(&end, 16.0);
    assert_face_normal(&end, [0.0, 1.0, 0.0]);
    assert_face_position(&end, 0, center_x);
    assert_face_position(&end, 1, -8.0);
    assert_face_position(&end, 2, 19.5);
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
    // The cube less the cavity of its closed hollow (an 18 mm cube with rounds of radius 1) and
    // the 16 x 16 x 1 mm opening above it.
    let expected = unmoved.shape("b-1");
    assert_volume(expected, 1280.0 + 460.0 * PI / 3.0);
    assert_face_count(expected, 54);
    let shape = moved.shape("b-1");
    assert_volume(shape, expected.volume().expect("a volume"));
    assert_face_count(shape, 54);
    assert_bounding_box(shape, [40.0, -10.0, 0.0], [60.0, 10.0, 20.0]);
    assert_top_open(&unmoved, 0.0);
    assert_top_open(&moved, 50.0);
}

#[test]
fn t015_walls_thicker_than_the_fillets_rounds() {
    let run = nc06_run("1");

    // The 40 mm prism with 1 mm rounds minus an 18 mm deep cavity with a sharp 36 mm square
    // section.
    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_volume(shape, 8592.0 + 20.0 * PI);
    assert_face_count(shape, 15);
    assert_no_inner_rounds(&run);
}

#[test]
fn t015_rounds_thicker_than_the_wall() {
    let run = nc06_run("3 mm");

    // The 40 mm prism with 3 mm rounds minus an 18 mm deep cavity: a 36 mm square with corners of
    // radius 1.
    run.assert_all_ok();
    assert_volume(run.shape("b-1"), 8024.0 + 162.0 * PI);
    let body = run.body("b-1");
    let inner_rounds = face_names_with_prefix(body, "f-sh1/inner/f-fil1/");
    assert_eq!(inner_rounds.len(), 4, "each round has an inner face");
    for name in &inner_rounds {
        let inner_round = face(body, name);
        assert_face_kind(&inner_round, GeometryKind::Cylinder);
        assert_face_radius(&inner_round, 1.0);
    }
}

/// Test 16, the "too large" search where a limit truly exists: the side of the cup is a convex
/// round that no other wall closes, so walls at least as thick as its radius still fail (kernel.md
/// section 3.4.8 item 6) and the search stops just below 20 mm. The `limit`, the round-face
/// sentence and the fixes of test 16 come with features.md section 4.3.
#[test]
fn t016_cylinder_wall_stops_below_the_radius() {
    let run = cylinder_cup_run("30");

    run.assert_ok("f-cyl1");
    // The side of the cylinder, radius 20, is smaller than the wall and larger than what works; it is not a fillet's,
    // so the only fix is the thinner wall.
    let limit = assert_too_thick(
        &run,
        30.0,
        15.0..20.0,
        Some(RoundFaces {
            feature: "f-cyl1",
            label: "Cylinder 1",
            radius: 20.0,
        }),
    );

    let rebuilt = cylinder_cup_run(&limit);
    rebuilt.assert_all_ok();
}

#[test]
fn cylinder_wall_as_thick_as_the_radius_fails() {
    let run = cylinder_cup_run("20");

    run.assert_error("f-sh1", "shell_too_thick");
    assert_unchanged(&run, "f-sh1", "b-1");
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
    // The walls meet in the middle of the cube; the rounds' radius 2 lies below what works, so they are not named.
    let limit = assert_too_thick(&run, 12.0, 9.0..10.0, None);

    let rebuilt = rounded_cube_run(&limit);
    rebuilt.assert_all_ok();
}

#[test]
fn t021_arc_join_without_a_round() {
    let run = rounded_l_run(None);

    // L, 12000, less a cavity 18 mm deep with its arc join at the concave edge, 6592 - 16 pi.
    assert_arc_join_kept(&run, 5408.0 + 16.0 * PI, 16);
}

#[test]
fn t021_arc_join_beside_a_round_thicker_than_the_wall() {
    let run = rounded_l_run(Some("3"));

    // L less the round's material, 10 (9 - 9 pi / 4), less the cavity with its inner corner
    // rounded at radius 1 over its 6 mm in the lower box, 6592 - 16 pi - 6 (1 - pi / 4).
    assert_arc_join_kept(&run, 5324.0 + 37.0 * PI, 18);
}

#[test]
fn t021_arc_join_beside_a_round_thinner_than_the_wall() {
    let run = rounded_l_run(Some("1"));

    // The same cavity, less the round's material, 10 (1 - pi / 4): the round's inner offset
    // vanishes into a sharp corner while the concave edge keeps its arc join.
    assert_arc_join_kept(&run, 5398.0 + 18.5 * PI, 17);
    assert_no_inner_rounds(&run);
}

#[test]
fn t021_arc_join_beside_a_round_as_thick_as_the_wall() {
    let run = rounded_l_run(Some("2"));

    // The same cavity, less the round's material, 10 (4 - pi).
    assert_arc_join_kept(&run, 5368.0 + 26.0 * PI, 17);
    assert_no_inner_rounds(&run);
}

/// The checks of a rounded join of `b-1` in tests 24 and 25: its surface kind, the wall's radius 2
/// and its area.
fn assert_round_join(run: &Run, name: &str, kind: GeometryKind, area: f64) {
    let join = face(run.body("b-1"), name);
    assert_face_kind(&join, kind);
    assert_face_radius(&join, 2.0);
    assert_face_area(&join, area);
}

/// Test 24: the L-shaped body with its bottom removed. The bottom meets the kept faces only at
/// convex edges, so the kernel's first attempt opens it and fills the gap at the concave edge
/// between the step and the taller box with an arc join (naming.md section 3.6.7 rule 4).
#[test]
fn t024_arc_join_at_a_concave_edge() {
    let run = Run::example("shell/concave-edge-join.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    // The L, 12000, less a cavity 16 mm long in y whose section in xz is 452 - pi.
    assert_volume(shape, 4768.0 + 16.0 * PI);
    assert_face_count(shape, 16);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-20.0, -10.0, 0.0], [20.0, 10.0, 20.0]);
    // A quarter turn around the concave edge over its 16 mm between the inner walls.
    assert_round_join(&run, ARC_JOIN, GeometryKind::Cylinder, 16.0 * PI);
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/inner/edge("),
        [ARC_JOIN],
        "the concave edge is the body's only one"
    );
    let step_wall = face(body, "f-sh1/inner/f-box1/top");
    assert_face_in_plane(&step_wall, 2, 8.0);
    assert_face_area(&step_wall, 288.0);
    let riser_wall = face(body, "f-sh1/inner/f-box2/left");
    assert_face_in_plane(&riser_wall, 0, 2.0);
    assert_face_area(&riser_wall, 128.0);
    let rim = face(body, "f-sh1/rim/f-box1/bottom");
    assert_face_in_plane(&rim, 2, 0.0);
    assert_face_area(&rim, 224.0);
    assert_no_face(body, "f-box1/bottom");
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/inner/vertex("),
        Vec::<String>::new(),
        "each end of the concave edge also has two convex edges, so it gets no join of its own"
    );
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/face"),
        Vec::<String>::new(),
        "every face has a name from its origin"
    );
}

/// Test 25: `CUBE20` with a 10 mm notch cut from the bottom of its back right quarter, shelled with
/// its bottom removed. The notch's ceiling and two walls meet at (0,0,10), where all three edges
/// are concave, so the three arc joins meet in a sphere join (naming.md section 3.6.7 rule 5).
#[test]
fn t025_sphere_join_at_a_concave_vertex() {
    let run = Run::example("shell/concave-vertex-join.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    // The notched cube, 7000, less the 16 x 16 x 18 mm box inside the walls, 4608, without its
    // part within 2 mm of the notch, 1088 + 82 pi / 3.
    assert_volume(shape, 3480.0 + 82.0 * PI / 3.0);
    assert_face_count(shape, 21);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    let ceiling_joins = [
        "f-sh1/inner/edge(f-box2/front|f-box2/top)",
        "f-sh1/inner/edge(f-box2/left|f-box2/top)",
    ];
    for name in ceiling_joins {
        assert_round_join(&run, name, GeometryKind::Cylinder, 8.0 * PI);
    }
    // The join between the walls runs the full 10 mm down to the opening.
    let wall_join = "f-sh1/inner/edge(f-box2/front|f-box2/left)";
    assert_round_join(&run, wall_join, GeometryKind::Cylinder, 10.0 * PI);
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/inner/edge("),
        [wall_join, ceiling_joins[0], ceiling_joins[1]],
        "one arc join per concave edge of the notch"
    );
    // An eighth of a sphere around the vertex, where the three arc joins meet.
    let corner = "f-sh1/inner/vertex(f-box2/front|f-box2/left|f-box2/top)";
    assert_round_join(&run, corner, GeometryKind::Sphere, 2.0 * PI);
    let center = face(body, corner)
        .location
        .unwrap_or_else(|| panic!("the sphere join {corner} has no center"));
    assert_coordinates(
        "center of the sphere join",
        point_components(center),
        [0.0, 0.0, 10.0],
        LENGTH_TOLERANCE,
    );
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/inner/vertex("),
        [corner],
        "the notch's corner is the body's only concave vertex"
    );
    let ceiling_wall = face(body, "f-sh1/inner/f-box2/top");
    assert_face_in_plane(&ceiling_wall, 2, 12.0);
    assert_face_area(&ceiling_wall, 64.0);
    let rim = face(body, "f-sh1/rim/f-box1/bottom");
    assert_face_in_plane(&rim, 2, 0.0);
    assert_face_area(&rim, 140.0 + PI);
    assert_eq!(
        face_names_with_prefix(body, "f-sh1/face"),
        Vec::<String>::new(),
        "every face has a name from its origin"
    );
}

#[test]
fn t022_wall_just_under_the_rounds() {
    let run = rounded_cube_run("1.9995");

    // The rounds' offsets would be 0.0005 mm thin, so they vanish: the 20 mm prism minus an
    // 18.0005 mm deep cavity with a sharp 16.001 mm square section.
    assert_rounded_cube_shell(
        &run,
        1.9995,
        7680.0 + 80.0 * PI - 16.001 * 16.001 * 18.0005,
        15,
        16.001 * 18.0005,
    );
    assert_no_inner_rounds(&run);
    assert_face_area(
        &face(run.body("b-1"), "f-sh1/rim/f-box1/top"),
        384.0 + 4.0 * PI - 16.001 * 16.001,
    );
}

#[test]
fn t022_closed_hollow_just_under_the_rounds() {
    let run = Run::new(rounded_cube_document("1.9995", &[]));

    // The cube with its rounds minus a sharp cube cavity of side 16.001 mm.
    assert_rounded_cube_shell(
        &run,
        1.9995,
        7680.0 + 80.0 * PI - 16.001 * 16.001 * 16.001,
        16,
        16.001 * 16.001,
    );
    assert_no_inner_rounds(&run);
    assert_has_face(run.body("b-1"), "f-box1/top");
}

/// Test 23: the Playwright fixture `cube-features.ptsm` of ui.md section 6.2 opens with its stated
/// result in the native core (ui.md open issue 17, E-108). It reads the fixture as committed, so
/// a change to it that breaks the shell after the fillet fails here before Playwright runs.
#[test]
fn t023_ui_fixture_with_a_shell_after_a_fillet() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/fixtures/cube-features.ptsm");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read fixture {}: {error}", path.display()));

    let run = Run::new(parse_document(&text));

    assert_eq!(run.report.features.len(), 4, "the fixture has 4 features");
    run.assert_all_quiet();
    let shape = run.shape("b-cube");
    assert_volume(shape, 3072.0 + 80.0 * PI);
    assert_face_count(shape, 15);
    assert_solid_count(shape, 1);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
}

#[test]
fn the_rim_and_inside_faces_move_with_the_part() {
    let top = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"}"#;
    let run = Run::records(&["b-1"], &[CUBE20, MOVE_BY_100, &shell_record("2", &[top])]);

    run.assert_all_ok();
    assert_moved_by_100(&run, "f-sh1", "b-1", "f-sh1/rim/f-box1/top");
    assert_moved_by_100(&run, "f-sh1", "b-1", "f-sh1/inner/f-box1/bottom");
}

#[test]
fn t017_every_face_removed() {
    let faces = [
        r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"}"#,
        r#"{"q": "f-box1/bottom", "fp": "plane|n=0,0,-1|c=0,0,0|a=400"}"#,
        r#"{"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=0,-10,10|a=400"}"#,
        r#"{"q": "f-box1/back", "fp": "plane|n=0,1,0|c=0,10,10|a=400"}"#,
        r#"{"q": "f-box1/left", "fp": "plane|n=-1,0,0|c=-10,0,10|a=400"}"#,
        r#"{"q": "f-box1/right", "fp": "plane|n=1,0,0|c=10,0,10|a=400"}"#,
    ];
    let run = Run::records(&["b-1"], &[CUBE20, &shell_record("2", &faces)]);

    let error = run.assert_error("f-sh1", "shell_removes_all");
    assert!(
        matches!(error, FeatureError::ShellRemovesAll { body, count: 6 } if *body == id("b-1")),
        "{error:?}"
    );
    assert_eq!(
        run.messages("f-sh1"),
        [
            "This shell would remove all 6 faces of body 'Body 1', which leaves nothing to hollow. Remove at most 5 faces, or leave Faces to remove empty for a closed hollow."
        ]
    );
    assert_eq!(
        message(&run, "f-sh1", "shell_removes_all").fixes,
        [pick("/remove_faces")]
    );
    // Checked before any kernel work, so no kernel text appears and nothing reads as a wall that is too thick.
    assert_messages_pass_lint(&run, "f-sh1");
    assert_unchanged(&run, "f-sh1", "b-1");
}

#[test]
fn removing_every_face_of_one_of_two_solids_is_counted_for_that_solid() {
    // Shell test 10's two cubes; every face of the first is removed, so nothing of it would be left to hollow.
    let second = r#"{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "position": ["50", "0", "0"], "op": "join"}"#;
    let faces = [
        r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"}"#,
        r#"{"q": "f-box1/bottom", "fp": "plane|n=0,0,-1|c=0,0,0|a=400"}"#,
        r#"{"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=0,-10,10|a=400"}"#,
        r#"{"q": "f-box1/back", "fp": "plane|n=0,1,0|c=0,10,10|a=400"}"#,
        r#"{"q": "f-box1/left", "fp": "plane|n=-1,0,0|c=-10,0,10|a=400"}"#,
        r#"{"q": "f-box1/right", "fp": "plane|n=1,0,0|c=10,0,10|a=400"}"#,
    ];
    let run = Run::records(&["b-1"], &[CUBE20, second, &shell_record("2", &faces)]);

    let error = run.assert_error("f-sh1", "shell_removes_all");
    assert!(
        matches!(error, FeatureError::ShellRemovesAll { count: 6, .. }),
        "{error:?}"
    );
    assert_unchanged(&run, "f-sh1", "b-1");
}

#[test]
fn t018_a_closed_hollow_is_intended() {
    let run = Run::example("shell/closed-hollow.ptsm");

    // Shell does not use `apply_op`, so its closed hollow gives no `closed_void` (features.md section 3.5).
    run.assert_all_quiet();
    let shape = run.shape("b-1");
    assert_solid_count(shape, 1);
    assert_eq!(shape.count(SubShapeKind::Shell), 2);
}
