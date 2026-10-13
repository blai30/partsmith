//! Acceptance tests of the `fillet` feature (docs/spec/features/fillet.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/fillet/`.

mod feature_support;

use std::f64::consts::PI;
use std::time::Duration;

use feature_support::{
    CUBE20, FRONT_TOP_EDGE, PLATE, PLATE_TOP_EDGES, Run, assert_bounding_box,
    assert_element_repair, assert_error_field, assert_examples_canonical, assert_face_area,
    assert_face_count, assert_face_kind, assert_face_radius, assert_list_field,
    assert_median_build_within, assert_same_body, assert_unchanged, assert_volume, body_update,
    document, face, face_names_with_prefix, load_example, median_failed_build_time, output_body,
    suggested_limit,
};
use partsmith_features::{BodyChange, FeatureError, FeatureWarning, IgnoredReason};
use partsmith_model::{Document, Units};
use partsmith_occt_sys::GeometryKind;

/// `E-LOST`: an edge reference that matches nothing.
const LOST_EDGE: &str =
    r#"{"q": "edge(f-box1/nothing|f-box1/top)", "fp": "line|n=0,0,1|c=500,500,500|l=1"}"#;

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

/// `FIL(edges, radius)`: the fillet `f-fil1` of `b-1`.
fn fillet_record(edges: &[&str], radius: &str) -> String {
    format!(
        r#"{{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{}], "radius": "{radius}"}}"#,
        edges.join(", "),
    )
}

/// Test 8: the plate with its four top edges filleted at `radius`.
fn plate_document(radius: &str) -> Document {
    document(&["b-1"], &[PLATE, &fillet_record(&PLATE_TOP_EDGES, radius)])
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
        run.messages("f-fil1"),
        [
            "One of the edges selected for Edges can no longer be found, so it was skipped. Re-pick it in the repair panel."
        ]
    );
    assert_volume(run.shape("b-1"), 7982.831853);
}

#[test]
fn t007_every_edge_lost() {
    let run = Run::records(&["b-1"], &[CUBE20, &fillet_record(&[LOST_EDGE], "2")]);

    run.assert_error("f-fil1", "references_not_found");
    assert_eq!(
        run.messages("f-fil1"),
        ["The edge selected for Edges can no longer be found. Re-pick it in the repair panel."]
    );
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
    let limit = suggested_limit(max);
    let expected = format!(
        "Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about {limit} mm. Try {limit} mm or less."
    );
    assert_eq!(run.messages("f-fil1"), [expected.as_str()]);
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
    run.assert_warnings("f-fil2", &["ignored", "no_effect"]);
    let warning = &run.feature("f-fil2").warnings[0];
    let FeatureWarning::Ignored { reason, .. } = warning else {
        panic!("expected an Ignored warning, got {warning:?}");
    };
    assert!(
        matches!(reason, IgnoredReason::SmoothEdge),
        "the edge is skipped as smooth, got {reason:?}"
    );
    assert_eq!(
        run.messages("f-fil2"),
        [
            "One of the edges selected for Edges is smooth (its faces meet without a corner), so it was skipped.",
            "This feature does not change body 'Body 1'. Check its position, size and direction.",
        ]
    );
    assert_unchanged(&run, "f-fil2", "b-1");
}

/// Step 4 next to a torus: the edge where the side of a cylinder meets the fillet of its top rim
/// (a torus) is smooth, so a second fillet of it is skipped as in test 12 (fillet.md open issue 3).
#[test]
fn smooth_edge_next_to_a_torus_skipped() {
    let run = Run::example("fillet/smooth-edge-next-to-torus.ptsm");

    run.assert_ok("f-cyl1");
    run.assert_ok("f-fil1");
    run.assert_warnings("f-fil2", &["ignored", "no_effect"]);
    let warning = &run.feature("f-fil2").warnings[0];
    let FeatureWarning::Ignored { reason, .. } = warning else {
        panic!("expected an Ignored warning, got {warning:?}");
    };
    assert!(
        matches!(reason, IgnoredReason::SmoothEdge),
        "the edge is skipped as smooth, got {reason:?}"
    );
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
            "Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid."
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
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t015_performance() {
    assert_median_build_within(
        &load_example("fillet/every-edge.ptsm"),
        "f-fil1",
        5,
        Duration::from_millis(100),
    );
    let too_large = median_failed_build_time(&plate_document("3"), "f-fil1", "fillet_too_large", 3);
    assert!(
        too_large < Duration::from_millis(1200),
        "the failing fillet with its search took {too_large:?}, the budget is 1.2 s"
    );
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
    run.assert_warnings("f-fil2", &["ignored", "no_effect"]);
    let warning = &run.feature("f-fil2").warnings[0];
    let FeatureWarning::Ignored { reason, .. } = warning else {
        panic!("expected an Ignored warning, got {warning:?}");
    };
    assert!(
        matches!(reason, IgnoredReason::SmoothEdge),
        "the edge is skipped as smooth, got {reason:?}"
    );
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
    assert_eq!(
        run.messages("f-fil1"),
        ["Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
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
