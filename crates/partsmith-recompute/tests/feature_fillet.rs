//! Acceptance tests of the `fillet` feature (docs/spec/features/fillet.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/fillet/`.

use crate::feature_support;

use std::collections::BTreeMap;
use std::f64::consts::PI;

use feature_support::fillet::{fillet_record, plate_document};
use feature_support::{
    CUBE20, FRONT_TOP_EDGE, MOVE_BY_100, RISER_EDGE, RISER_SENTENCE, Run, STEP,
    STEP_BOTTOM_LEFT_EDGE, assert_bounding_box, assert_element_repair, assert_error_field,
    assert_examples_canonical, assert_face_area, assert_face_count, assert_face_kind,
    assert_face_radius, assert_limiting_face, assert_list_field, assert_messages_pass_lint,
    assert_moved_by_100, assert_same_body, assert_skipped_as_smooth, assert_unchanged,
    assert_volume, body_update, document_text, face, face_names_with_prefix, id, message,
    output_body, parse_document, remove_entry, round_down_to_two_significant_digits, set_field,
    suggested_limit, use_reference,
};
use partsmith_features::{
    BodyChange, Diagnosis, Explanation, FeatureError, FeatureWarning, Fix, NoEffectHow,
};
use partsmith_model::Units;
use partsmith_occt_sys::GeometryKind;

/// The names of the twelve edges of a box `f-box1`.
const BOX_EDGE_NAMES: [&str; 12] = [
    "edge(f-box1/front|f-box1/top)",
    "edge(f-box1/back|f-box1/top)",
    "edge(f-box1/left|f-box1/top)",
    "edge(f-box1/right|f-box1/top)",
    "edge(f-box1/bottom|f-box1/front)",
    "edge(f-box1/back|f-box1/bottom)",
    "edge(f-box1/bottom|f-box1/left)",
    "edge(f-box1/bottom|f-box1/right)",
    "edge(f-box1/front|f-box1/left)",
    "edge(f-box1/front|f-box1/right)",
    "edge(f-box1/back|f-box1/left)",
    "edge(f-box1/back|f-box1/right)",
];

/// The names of the eight corner vertices of a box `f-box1`.
const BOX_VERTEX_NAMES: [&str; 8] = [
    "vertex(f-box1/back|f-box1/bottom|f-box1/left)",
    "vertex(f-box1/back|f-box1/bottom|f-box1/right)",
    "vertex(f-box1/back|f-box1/left|f-box1/top)",
    "vertex(f-box1/back|f-box1/right|f-box1/top)",
    "vertex(f-box1/bottom|f-box1/front|f-box1/left)",
    "vertex(f-box1/bottom|f-box1/front|f-box1/right)",
    "vertex(f-box1/front|f-box1/left|f-box1/top)",
    "vertex(f-box1/front|f-box1/right|f-box1/top)",
];

/// "Use {value}": `SetField` of the radius to the expression `value`.
fn use_radius(value: &str) -> Fix {
    set_field("radius", "Radius", serde_json::json!(value))
}

/// `names` with `prefix` put in front of each, sorted by byte order.
fn prefixed(prefix: &str, names: &[&str]) -> Vec<String> {
    let mut prefixed: Vec<String> = names.iter().map(|name| format!("{prefix}{name}")).collect();
    prefixed.sort();
    prefixed
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("fillet");
}

#[test]
fn t001_one_convex_edge() {
    let run = Run::example("fillet/one-edge.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 7982.831853);
    assert_face_count(shape, 7);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    let fillet = face(body, "f-fil1/fillet/edge(f-box1/front|f-box1/top)");
    assert_face_kind(&fillet, GeometryKind::Cylinder);
    assert_face_radius(&fillet, 2.0);
    assert_face_area(&fillet, 62.831853);
    assert_face_area(&face(body, "f-box1/top"), 360.0);
    assert_face_area(&face(body, "f-box1/front"), 360.0);
    let change = &body_update(&run, "f-fil1", "b-1").change;
    assert!(
        matches!(change, BodyChange::Modify),
        "a fillet modifies its body, got {change:?}"
    );
}

#[test]
fn t002_top_loop_of_a_box() {
    let run = Run::example("fillet/top-loop.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 5917.227116);
    assert_face_count(shape, 10);
    assert_bounding_box(shape, [-15.0, -10.0, 0.0], [15.0, 10.0, 10.0]);
    assert_face_area(&face(body, "f-box1/top"), 416.0);
    assert_eq!(
        face_names_with_prefix(body, "f-fil1/fillet/"),
        prefixed("f-fil1/fillet/", &BOX_EDGE_NAMES[..4]),
        "one fillet face per top edge"
    );
    assert_eq!(
        face_names_with_prefix(body, "f-fil1/corner/"),
        Vec::<String>::new(),
        "two fillets meeting at a top corner make no corner blend"
    );
}

#[test]
fn t003_every_edge_of_a_cube() {
    let run = Run::example("fillet/every-edge.ptsm");

    run.assert_all_ok();
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 7804.696111);
    assert_face_count(shape, 26);
    assert_bounding_box(shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0]);
    assert_eq!(
        face_names_with_prefix(body, "f-fil1/fillet/"),
        prefixed("f-fil1/fillet/", &BOX_EDGE_NAMES),
        "one fillet face per edge"
    );
    let corners = face_names_with_prefix(body, "f-fil1/corner/");
    assert_eq!(
        corners,
        prefixed("f-fil1/corner/", &BOX_VERTEX_NAMES),
        "one corner blend per box corner"
    );
    for corner in &corners {
        assert_face_kind(&face(body, corner), GeometryKind::Sphere);
    }
}

#[test]
fn t004_concave_edge_adds_material() {
    let run = Run::example("fillet/concave-edge.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 12017.168147);
    assert_face_count(shape, 9);
    assert_bounding_box(shape, [-20.0, -10.0, 0.0], [20.0, 10.0, 20.0]);
}

#[test]
fn t005_inch_document_radius_is_in_inches() {
    let run = Run::example("fillet/inch-document.ptsm");

    run.assert_all_ok();
    let radius = 2.54;
    assert_volume(
        run.shape("b-1"),
        8000.0 - (1.0 - PI / 4.0) * radius * radius * 20.0,
    );
}

#[test]
fn t006_one_edge_lost() {
    let run = Run::example("fillet/one-edge-lost.ptsm");

    run.assert_ok("f-box1");
    run.assert_warnings("f-fil1", &["reference_missing"]);
    let warning = &run.feature("f-fil1").warnings[0];
    assert_list_field(warning.field(), "edges", 1);
    assert_element_repair(warning.repair(), "/edges/1", "b-1");
    assert_eq!(
        message(&run, "f-fil1", "reference_missing")
            .pointer
            .as_deref(),
        Some("/edges/1")
    );
    assert_messages_pass_lint(&run, "f-fil1");
    assert_volume(run.shape("b-1"), 7982.831853);
}

#[test]
fn t007_every_edge_lost() {
    // An edge of a feature that is not in the document.
    let lost = r#"{"q": "edge(f-box9/front|f-box9/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#;
    let run = Run::records(&["b-1"], &[CUBE20, &fillet_record(&[lost], "2")]);

    run.assert_error("f-fil1", "references_not_found");
    // Recompute's diagnosis: the edge's feature is not in the document, which its description already says.
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
        run.render_error("f-fil1", &diagnosis),
        "An edge of a feature that is not in this document selected for Edges is gone."
    );
    let fixes = run.error_fixes("f-fil1", &diagnosis);
    assert_eq!(
        fixes.first(),
        Some(&use_reference(
            "/edges/0",
            run.picked_before("f-fil1", "b-1", "edge(f-box1/front|f-box1/top)")
        ))
    );
    assert_eq!(
        fixes.last(),
        Some(&Fix::Pick {
            pointer: "/edges/0".to_string()
        })
    );
    assert_messages_pass_lint(&run, "f-fil1");
    assert_unchanged(&run, "f-fil1", "b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 8000.0);
    assert_face_count(shape, 6);
}

#[test]
fn t008_radius_too_large() {
    let run = Run::new(plate_document("3"));

    run.assert_ok("f-box1");
    let error = run.assert_error("f-fil1", "fillet_too_large");
    assert_error_field(error, "radius");
    let FeatureError::FilletTooLarge { radius, max, .. } = error else {
        panic!("expected FilletTooLarge, got {error:?}");
    };
    assert_eq!(*radius, 3.0, "the requested radius");
    let max = max.unwrap_or_else(|| panic!("the search finds a working radius: {error:?}"));
    assert!(
        (2.0..2.4).contains(&max),
        "the largest working radius {max} lies in [2, 2.4)"
    );
    let bisection_steps = max * 1024.0 / 3.0;
    assert_eq!(
        bisection_steps,
        bisection_steps.round(),
        "the largest working radius {max} is a bisection point 3k/1024"
    );
    // The four side faces tie at 2.4 mm across their top edges, and `edge(f-box1/back|f-box1/top)` sorts first;
    // without it the other three edges fail as well, so its removal is not offered.
    // `position` is the entry of the edge's reference in Edges (features.md section 2.8, `Limit::Face`): E-BT is
    // entry 1 of [E-FT, E-BT, E-LT, E-RT].
    assert_limiting_face(
        error,
        1,
        "edge(f-box1/back|f-box1/top)",
        "f-box1/back",
        false,
    );
    let limit = suggested_limit(max);
    let expected = format!(
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about {limit} mm. The back top edge of 'Box 1' lies next to the back side of 'Box 1', which is only 2.4 mm across."
    );
    assert_eq!(run.messages("f-fil1"), [expected.as_str()]);
    assert_eq!(
        message(&run, "f-fil1", "fillet_too_large").fixes,
        [use_radius(&format!("{limit} mm"))]
    );
    assert_eq!(error.suggestion(Units::Mm), Some(format!("{limit} mm")));
    assert_unchanged(&run, "f-fil1", "b-1");
    assert_volume(run.shape("b-1"), 960.0);

    let rebuilt = Run::new(plate_document(&limit));
    rebuilt.assert_all_ok();
}

#[test]
fn t009_zero_radius() {
    let run = Run::records(&["b-1"], &[CUBE20, &fillet_record(&[FRONT_TOP_EDGE], "0")]);

    run.assert_error("f-fil1", "out_of_range");
    assert_eq!(
        run.messages("f-fil1"),
        ["Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
    );
}

#[test]
fn t010_no_edges() {
    let run = Run::records(&["b-1"], &[&fillet_record(&[], "2")]);

    run.assert_error("f-fil1", "empty_selection");
    assert_eq!(
        run.messages("f-fil1"),
        ["Select at least one edge for Edges."]
    );
}

#[test]
fn t011_tangent_chain() {
    let run = Run::example("fillet/tangent-chain.ptsm");

    run.assert_all_ok();
    let after_first = output_body(&run, "f-fil1", "b-1");
    let first_shape = after_first
        .shape()
        .expect("the first fillet leaves a solid");
    assert_volume(first_shape, 5922.743339);
    assert_face_count(first_shape, 10);
    let body = run.body("b-1");
    let shape = run.shape("b-1");
    assert_volume(shape, 5902.689635);
    assert_face_count(shape, 18);
    assert_bounding_box(shape, [-15.0, -10.0, 0.0], [15.0, 10.0, 10.0]);
    assert_eq!(
        face_names_with_prefix(body, "f-fil2/fillet/"),
        prefixed(
            "f-fil2/fillet/",
            &[
                "edge(f-box1/front|f-box1/top)",
                "edge(f-box1/back|f-box1/top)",
                "edge(f-box1/left|f-box1/top)",
                "edge(f-box1/right|f-box1/top)",
                "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
                "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/right))",
                "edge(f-box1/top|f-fil1/fillet/edge(f-box1/back|f-box1/left))",
                "edge(f-box1/top|f-fil1/fillet/edge(f-box1/back|f-box1/right))",
            ],
        ),
        "the fillet follows the whole tangent top loop, one face per edge"
    );
}

#[test]
fn t012_smooth_edge_skipped() {
    let run = Run::example("fillet/smooth-edge.ptsm");

    run.assert_ok("f-box1");
    run.assert_ok("f-fil1");
    // One edge, one message: its own warning already says that nothing changed (features.md section 3.15).
    run.assert_warnings("f-fil2", &["ignored"]);
    assert_skipped_as_smooth(&run.feature("f-fil2").warnings[0]);
    assert_eq!(
        run.messages("f-fil2"),
        [
            "One of the edges selected for Edges is smooth (its faces meet without a corner), so it was skipped."
        ]
    );
    // Its reference resolved to that edge alone, so removing the entry removes only it.
    assert_eq!(
        message(&run, "f-fil2", "ignored").fixes,
        [remove_entry("/edges/0")]
    );
    assert_unchanged(&run, "f-fil2", "b-1");
}

#[test]
fn t012_a_smooth_edge_that_two_entries_select_is_not_removed() {
    // Removing one entry would leave the other selecting the smooth edge, so the warning is on the whole list
    // (features.md section 4.2).
    let smooth = r#"{"q": "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-12,-10,5|l=10"}"#;
    let second_fillet = format!(
        r#"{{"id": "f-fil2", "type": "fillet", "body": "b-1", "edges": [{smooth}, {smooth}], "radius": "1"}}"#
    );
    let run = Run::records(&["b-1"], &[ROUNDED_BOX[0], ROUNDED_BOX[1], &second_fillet]);

    run.assert_warnings("f-fil2", &["ignored", "no_effect"]);
    let ignored = run.warning("f-fil2", "ignored");
    assert_skipped_as_smooth(ignored);
    assert_eq!(
        ignored.field().map(|field| field.path.as_str()),
        Some("edges")
    );
    assert_eq!(message(&run, "f-fil2", "ignored").fixes, []);
    assert_unchanged(&run, "f-fil2", "b-1");
}

/// The 30 x 20 x 10 box of tests 11 and 12 with its four vertical edges rounded at 3 mm by `f-fil1`, so the top loop
/// is one tangent chain.
const ROUNDED_BOX: [&str; 2] = [
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "20", "10"], "op": "new"}"#,
    r#"{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-15,-10,5|l=10"}, {"q": "edge(f-box1/front|f-box1/right)", "fp": "line|n=0,0,1|c=15,-10,5|l=10"}, {"q": "edge(f-box1/back|f-box1/left)", "fp": "line|n=0,0,1|c=-15,10,5|l=10"}, {"q": "edge(f-box1/back|f-box1/right)", "fp": "line|n=0,0,1|c=15,10,5|l=10"}], "radius": "3"}"#,
];

#[test]
fn t012_two_smooth_edges_skipped_change_nothing() {
    // Both tangent edges of the front left fillet face.
    let second_fillet = r#"{"id": "f-fil2", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-12,-10,5|l=10"}, {"q": "edge(f-box1/left|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-15,-7,5|l=10"}], "radius": "1"}"#;
    let run = Run::records(&["b-1"], &[ROUNDED_BOX[0], ROUNDED_BOX[1], second_fillet]);

    run.assert_ok("f-fil1");
    run.assert_warnings("f-fil2", &["ignored", "ignored", "no_effect"]);
    let warning = run.warning("f-fil2", "no_effect");
    assert!(
        matches!(
            warning,
            FeatureWarning::NoEffect {
                how: NoEffectHow::Skipped { field },
                ..
            } if field.path == "edges"
        ),
        "{warning:?}"
    );
    assert_eq!(
        message(&run, "f-fil2", "no_effect").text,
        "This fillet changes nothing in body 'Body 1', because every edge selected for Edges was skipped."
    );
    assert_unchanged(&run, "f-fil2", "b-1");
    assert_messages_pass_lint(&run, "f-fil2");
}

/// Step 4 next to a torus: the edge where the side of a cylinder meets the fillet of its top rim
/// (a torus) is smooth, so a second fillet of it is skipped as in test 12 (fillet.md open issue 3).
#[test]
fn smooth_edge_next_to_a_torus_skipped() {
    let run = Run::example("fillet/smooth-edge-next-to-torus.ptsm");

    run.assert_ok("f-cyl1");
    run.assert_ok("f-fil1");
    run.assert_warnings("f-fil2", &["ignored"]);
    assert_skipped_as_smooth(&run.feature("f-fil2").warnings[0]);
    assert_unchanged(&run, "f-fil2", "b-1");
    assert_face_count(run.shape("b-1"), 4);
}

#[test]
fn t013_empty_body() {
    let run = Run::records(&["b-1"], &[&fillet_record(&[FRONT_TOP_EDGE], "2")]);

    run.assert_error("f-fil1", "body_empty");
    assert_eq!(
        run.messages("f-fil1"),
        [
            "Body 'Body 1' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid."
        ]
    );
}

#[test]
fn t014_edges_of_another_body() {
    let other_box = r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["50", "0", "0"], "op": "new"}"#;
    let other_edge =
        r#"{"q": "edge(f-box2/front|f-box2/top)", "fp": "line|n=1,0,0|c=50,-5,10|l=10"}"#;
    let run = Run::records(
        &["b-1", "b-2"],
        &[CUBE20, other_box, &fillet_record(&[other_edge], "1")],
    );

    run.assert_ok("f-box1");
    run.assert_ok("f-box2");
    run.assert_error("f-fil1", "references_not_found");
    assert_unchanged(&run, "f-fil1", "b-1");
    // The fillet declares only b-1, so b-2 is compared with the box that made it.
    assert_same_body(run.body("b-2"), &output_body(&run, "f-box2", "b-2"));
}

#[test]
fn t016_fillet_beside_a_fillet_on_a_pipe_tee() {
    let run = Run::example("fillet/pipe-tee.ptsm");

    run.assert_ok("f-cyl1");
    run.assert_ok("f-cyl2");
    run.assert_ok("f-fil1");
    let tee = output_body(&run, "f-cyl2", "b-1");
    let tee_shape = tee.shape().expect("the tee is a solid");
    assert_volume(tee_shape, 20599.513715);
    assert_face_count(tee_shape, 5);
    let joint = output_body(&run, "f-fil1", "b-1");
    let joint_shape = joint.shape().expect("the filleted tee is a solid");
    assert_volume(joint_shape, 20616.732472);
    assert_face_count(joint_shape, 6);
    face(&joint, "f-fil1/fillet/edge(f-cyl1/side|f-cyl2/side)");
    // One edge, so its `ignored` warning is the only one (features.md section 3.15).
    run.assert_warnings("f-fil2", &["ignored"]);
    assert_skipped_as_smooth(&run.feature("f-fil2").warnings[0]);
    assert_unchanged(&run, "f-fil2", "b-1");
}

#[test]
fn t017_radius_below_the_tolerance() {
    let run = Run::records(
        &["b-1"],
        &[CUBE20, &fillet_record(&[FRONT_TOP_EDGE], "0.0000001")],
    );

    let error = run.assert_error("f-fil1", "out_of_range");
    assert_error_field(error, "radius");
    // The value is written with the digits that tell it apart from the bound (features.md section 3.12).
    assert_eq!(
        run.messages("f-fil1"),
        [
            "Radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
        ]
    );
    assert_unchanged(&run, "f-fil1", "b-1");
}

#[test]
fn t018_edge_order_does_not_matter() {
    // `E-FT`, `E-BT` and `E-LT` of `CUBE20`.
    let edges = [
        FRONT_TOP_EDGE,
        r#"{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,20|l=20"}"#,
        r#"{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-10,0,20|l=20"}"#,
    ];
    let reversed = [edges[2], edges[1], edges[0]];

    let listed = Run::records(&["b-1"], &[CUBE20, &fillet_record(&edges, "1")]);
    let reversed = Run::records(&["b-1"], &[CUBE20, &fillet_record(&reversed, "1")]);

    listed.assert_all_ok();
    reversed.assert_all_ok();
    let (first, second) = (listed.shape("b-1"), reversed.shape("b-1"));
    assert!(
        first.to_brep().expect("BRep bytes") == second.to_brep().expect("BRep bytes"),
        "the results differ"
    );
    assert_eq!(
        first.volume().expect("a volume").to_bits(),
        second.volume().expect("a volume").to_bits()
    );
    assert_eq!(listed.body("b-1").naming(), reversed.body("b-1").naming());
}

#[test]
fn a_rounded_face_moves_with_the_part() {
    let run = Run::records(
        &["b-1"],
        &[CUBE20, MOVE_BY_100, &fillet_record(&[FRONT_TOP_EDGE], "2")],
    );

    run.assert_all_ok();
    assert_moved_by_100(
        &run,
        "f-fil1",
        "b-1",
        "f-fil1/fillet/edge(f-box1/front|f-box1/top)",
    );
}

/// `STEP` with `FIL(edges, radius)`.
fn step_run(edges: &[&str], radius: &str) -> Run {
    let fillet = fillet_record(edges, radius);
    Run::records(&["b-1"], &[STEP[0], STEP[1], &fillet])
}

#[test]
fn t019_the_limiting_face_is_named() {
    let run = step_run(&[RISER_EDGE], "3");

    let error = run.assert_error("f-fil1", "fillet_too_large");
    // There is no other reference, so the removal of the riser's edge cannot be verified.
    assert_limiting_face(
        error,
        0,
        "edge(f-box1/top|f-box2/left)",
        "f-box2/left",
        false,
    );
    assert_eq!(
        run.messages("f-fil1"),
        [format!(
            "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. {RISER_SENTENCE}"
        )
        .as_str()]
    );
    assert_eq!(
        message(&run, "f-fil1", "fillet_too_large").fixes,
        [use_radius("2.3 mm")]
    );
    assert_messages_pass_lint(&run, "f-fil1");
    assert_unchanged(&run, "f-fil1", "b-1");
}

#[test]
fn t019_the_suggested_radius_builds() {
    step_run(&[RISER_EDGE], "2.3").assert_all_ok();
}

#[test]
fn t020_the_edge_that_limits_it_can_be_removed_when_the_others_work() {
    let run = step_run(&[RISER_EDGE, STEP_BOTTOM_LEFT_EDGE], "3");

    let error = run.assert_error("f-fil1", "fillet_too_large");
    // One more attempt at 3 mm without `edges[0]` works.
    assert_limiting_face(
        error,
        0,
        "edge(f-box1/top|f-box2/left)",
        "f-box2/left",
        true,
    );
    assert_eq!(
        run.messages("f-fil1"),
        [format!(
            "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. {RISER_SENTENCE}"
        )
        .as_str()]
    );
    assert_eq!(
        message(&run, "f-fil1", "fillet_too_large").fixes,
        [use_radius("2.3 mm"), remove_entry("/edges/0")]
    );
}

#[test]
fn t020_an_edge_that_two_entries_select_is_not_removable() {
    // Removing `edges[0]` would leave `edges[1]` selecting the riser's edge, so the fillet would fail again
    // (features.md section 4.3).
    let run = step_run(&[RISER_EDGE, RISER_EDGE, STEP_BOTTOM_LEFT_EDGE], "3");

    let error = run.assert_error("f-fil1", "fillet_too_large");
    assert_limiting_face(
        error,
        0,
        "edge(f-box1/top|f-box2/left)",
        "f-box2/left",
        false,
    );
    assert_eq!(
        message(&run, "f-fil1", "fillet_too_large").fixes,
        [use_radius("2.3 mm")]
    );
}

#[test]
fn t020_removing_the_limiting_edge_rounds_the_other_at_the_requested_radius() {
    let run = step_run(&[STEP_BOTTOM_LEFT_EDGE], "3");

    run.assert_all_ok();
    assert_volume(run.shape("b-1"), 7001.371669);
}

#[test]
fn t021_a_limit_far_below_the_request_counts_as_none() {
    let run = step_run(&[RISER_EDGE], "30");

    let error = run.assert_error("f-fil1", "fillet_too_large");
    let FeatureError::FilletTooLarge { max, .. } = error else {
        panic!("expected FilletTooLarge, got {error:?}");
    };
    // About 2.3 mm works, below 10 percent of 30 mm, which is no useful fix (features.md section 4.3).
    assert_eq!(*max, None);
    assert_limiting_face(
        error,
        0,
        "edge(f-box1/top|f-box2/left)",
        "f-box2/left",
        false,
    );
    assert_eq!(error.suggestion(Units::Mm), None);
    assert_eq!(
        message(&run, "f-fil1", "fillet_too_large").fixes,
        [Fix::Suppress {
            feature: id("f-fil1")
        }]
    );
    let text = run.messages("f-fil1")[0];
    assert!(text.ends_with(RISER_SENTENCE), "{text:?}");
    assert_messages_pass_lint(&run, "f-fil1");
}

/// Test 22: a 20 mm cube in an inch document with its front top edge filleted at `radius`.
fn inch_cube(radius: &str) -> Run {
    let cube = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "20 mm", "20 mm"], "op": "new"}"#;
    let fillet = fillet_record(&[FRONT_TOP_EDGE], radius);
    Run::new(parse_document(&document_text(
        "in",
        &[],
        &["b-1"],
        &[cube, &fillet],
    )))
}

#[test]
fn t022_a_bare_number_in_an_inch_document() {
    // Corpus R48: '2' is 2 in, 50.8 mm, on a 20 mm cube.
    let run = inch_cube("2");

    let error = run.assert_error("f-fil1", "fillet_too_large");
    let FeatureError::FilletTooLarge { max, .. } = error else {
        panic!("expected FilletTooLarge, got {error:?}");
    };
    let max = max.unwrap_or_else(|| panic!("the search finds a working radius: {error:?}"));
    assert!(
        (12.7..20.0).contains(&max),
        "the largest working radius {max} mm lies in [12.7, 20)"
    );
    let limit = round_down_to_two_significant_digits(max / 25.4);
    // The bare-number sentence takes the place of the limit sentence.
    assert_eq!(
        run.messages("f-fil1"),
        [format!(
            "Fillet radius 2 in is too large for the selected edges: the largest that works is about {limit} in. '2' has no unit, so it means 2 in in this document."
        )
        .as_str()]
    );
    assert_eq!(
        message(&run, "f-fil1", "fillet_too_large").fixes,
        [use_radius("2 mm"), use_radius(&format!("{limit} in"))]
    );
    assert_eq!(error.suggestion(Units::In).as_deref(), Some("2 mm"));
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t022_the_bare_number_fix_keeps_the_size_that_was_meant() {
    let run = inch_cube("2 mm");

    run.assert_all_quiet();
    assert_volume(run.shape("b-1"), 7982.831853);
}
