//! Acceptance tests 121 to 125 of `docs/spec/engine.md` section 6: `validate` reports feature messages with their own
//! codes, actions and candidates (section 3.19 step 8), element lists carry short references and picks full ones,
//! headless picks name their body, descriptors carry the fallback of a flat face in body coordinates and the axis
//! direction of a line or round element (section 3.9), and drag commits write a face sketch's fallback in body
//! coordinates with `fallback_frame: body` (section 3.6). A test named `tNNN` is section 6 test NNN.

mod common;
mod corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, assert_near, assert_point3, doc_handle, elements, fixture,
    get_document, headless_session, number, open, open_doc, open_text, point3, replace_once,
    revision, saved_text, send, session,
};
use corpus::{
    Scenario, actions, block_then, box_record, datum_offset, datum_plane, distance, document,
    edited_text, extrude, labels, linear_pattern, r03, record, rectangle, rectangle_region,
    repairs, sketch, with, without_keys,
};
use partsmith_engine::{Session, ValidateOptions, ValidationReport};
use partsmith_print::Severity;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Validation with the rebuild step and without the canonical check.
fn rebuild() -> ValidateOptions {
    ValidateOptions {
        rebuild: true,
        canonical: false,
    }
}

/// Every diagnostic as `code at path: message`, for failure output.
fn describe(report: &ValidationReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{} at {:?}: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            )
        })
        .collect()
}

// Test 121: `validate` reports feature messages with their own codes.

#[test]
fn t121_r03_a_lost_edge_is_a_warning_with_the_feature_message() {
    let text = edited_text(&r03());
    let mut session = headless_session();

    let report = session.validate(&text, &[], rebuild());

    let opened = open_text(&mut session, &text);
    let found: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "reference_missing")
        .collect();
    let [diagnostic] = found.as_slice() else {
        panic!("one reference_missing diagnostic: {:?}", describe(&report));
    };
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.path.as_deref(), Some("/features/2/edges/0"));
    let shown = corpus::message(&opened, "f-fil1", "reference_missing");
    assert_eq!(json!(diagnostic.message), shown["message"]);
    let feature = diagnostic
        .feature
        .as_ref()
        .expect("a feature message carries its feature");
    assert_eq!(feature.feature.as_str(), "f-fil1");
    let projected = serde_json::to_value(&feature.diagnostic).expect("diagnostics serialize");
    assert!(
        labels(&projected).ends_with(&[
            "Remove from Edges".to_owned(),
            "Pick an edge again".to_owned()
        ]),
        "{:?}",
        labels(&projected)
    );
    assert_eq!(
        projected["reason"],
        json!({"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"})
    );
    assert!(projected.get("cause").is_none(), "{projected}");
    let shown_repairs = repairs(&opened, "f-fil1");
    assert_eq!(
        serde_json::to_value(&feature.candidates).expect("candidates serialize"),
        shown_repairs[0]["candidates"]
    );
}

#[test]
fn t121_actions_of_validate_carry_revision_0() {
    let text = edited_text(&r03());
    let mut session = headless_session();

    let report = session.validate(&text, &[], rebuild());

    let applies: Vec<Value> = report
        .diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.feature.as_ref())
        .flat_map(|feature| {
            actions(&serde_json::to_value(&feature.diagnostic).expect("diagnostics serialize"))
        })
        .filter(|action| action["run"]["kind"] == "apply")
        .collect();
    assert!(!applies.is_empty(), "{:?}", describe(&report));
    for action in &applies {
        assert_eq!(action["run"]["fix"]["revision"], 0, "{action}");
    }
}

#[test]
fn t121_no_feature_message_is_reported_as_feature_error_or_feature_warning() {
    let mut session = headless_session();
    let negative = replace_once(&fixture("box.ptsm"), r#""20 mm""#, r#""-5 mm""#);

    for text in [edited_text(&r03()), negative] {
        let report = session.validate(&text, &[], rebuild());

        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "feature_error"
                    && diagnostic.code != "feature_warning"),
            "{:?}",
            describe(&report)
        );
    }
}

#[test]
fn t121_a_negative_size_is_one_out_of_range_error_at_its_field() {
    let mut session = headless_session();
    let negative = replace_once(&fixture("box.ptsm"), r#""20 mm""#, r#""-5 mm""#);

    let report = session.validate(&negative, &[], rebuild());

    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect();
    let [error] = errors.as_slice() else {
        panic!("one error: {:?}", describe(&report));
    };
    assert_eq!(error.code, "out_of_range");
    assert_eq!(error.path.as_deref(), Some("/features/0/size/0"));
}

#[test]
fn t121_an_unknown_type_is_reported_once_from_step_2() {
    let mut session = headless_session();
    let unknown = replace_once(
        &fixture("box.ptsm"),
        r#""type": "box""#,
        r#""type": "wedge_v9""#,
    );

    let report = session.validate(&unknown, &[], rebuild());

    let codes: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.path.as_deref() == Some("/features/0"))
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, ["unknown_type"], "{:?}", describe(&report));
    let unknown_type = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "unknown_type")
        .expect("the type is reported");
    assert!(
        unknown_type.feature.is_none(),
        "step 2 reports the type, not a feature message of the rebuild"
    );
}

// Tests 122 and 123: short and full references, headless picks.

/// The edge test 122 picks: the front edge of the right part of BLOCK's top, which SLOT(30, 50) splits.
const SPLIT_TOP_FRONT_EDGE: &str = "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)";

/// BLOCK with SLOT(30, 50): a 20 mm slot 5 deep through Y from x 30 to 50 (file-format.md example E10).
fn slotted_block() -> String {
    let slot = vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            rectangle(5, 30.0, -1.0, 50.0, 51.0),
        ),
        with(
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(5, 30.0, -1.0, 50.0, 51.0)],
                distance("5"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ];
    Scenario::new(&document(&["b-1"], &block_then(slot)), Vec::new()).setup
}

/// The position of the edge named `name` among the edges of `list_elements`.
fn edge_index(session: &mut Session, doc: u64, name: &str) -> usize {
    elements(session, doc, "b-1", Some("edge"))
        .iter()
        .position(|edge| edge["name"] == name)
        .unwrap_or_else(|| panic!("b-1 has an edge named {name}"))
}

/// The listed descriptor of the element named `name` of `b-1`.
fn listed(session: &mut Session, doc: u64, name: &str) -> Value {
    elements(session, doc, "b-1", None)
        .into_iter()
        .find(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("b-1 has an element named {name}"))
}

/// The anchors of the split top's front edge: the faces its piece of the top touches that the other piece does not.
fn split_edge_anchors() -> Value {
    json!({"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]})
}

#[test]
fn t122_lists_carry_the_short_reference() {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &slotted_block()));

    let edge = listed(&mut session, doc, SPLIT_TOP_FRONT_EDGE);

    assert_eq!(
        edge["reference"],
        json!({"q": SPLIT_TOP_FRONT_EDGE, "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2"})
    );
}

#[test]
fn t122_picks_carry_the_full_reference_with_anchors() {
    let mut session = session();
    let opened = open_text(&mut session, &slotted_block());
    let doc = doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    let index = edge_index(&mut session, doc, SPLIT_TOP_FRONT_EDGE);

    let picked = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": mesh_id, "kind": "edge", "index": index}}),
    );

    let mut expected = listed(&mut session, doc, SPLIT_TOP_FRONT_EDGE);
    expected["reference"] = json!({
        "q": SPLIT_TOP_FRONT_EDGE,
        "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
        "nb": split_edge_anchors()
    });
    assert_eq!(picked.ok()["element"], expected);
}

#[test]
fn t122_on_a_box_every_pick_equals_its_listed_entry() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    let all = elements(&mut session, doc, "b-1", None);

    for kind in ["face", "edge", "vertex"] {
        let of_kind: Vec<&Value> = all
            .iter()
            .filter(|element| element["kind"] == kind)
            .collect();
        for (index, element) in of_kind.iter().enumerate() {
            let picked = send(
                &mut session,
                json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": mesh_id, "kind": kind, "index": index}}),
            );

            assert_eq!(&picked.ok()["element"], *element, "{kind} {index}");
        }
    }
}

#[test]
fn t123_a_headless_pick_names_its_body_and_position() {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, &slotted_block()));
    let index = edge_index(&mut session, doc, SPLIT_TOP_FRONT_EDGE);

    let picked = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": 0, "kind": "edge", "index": index, "body": "b-1"}}),
    );

    let mut expected = listed(&mut session, doc, SPLIT_TOP_FRONT_EDGE);
    expected["reference"]["nb"] = split_edge_anchors();
    assert_eq!(picked.ok()["element"], expected);
}

#[test]
fn t123_a_headless_pick_without_its_body_or_with_a_mesh_id_is_stale() {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, &slotted_block()));
    let index = edge_index(&mut session, doc, SPLIT_TOP_FRONT_EDGE);

    let without_body = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": 0, "kind": "edge", "index": index}}),
    );
    let with_mesh = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": 5, "kind": "edge", "index": index, "body": "b-1"}}),
    );

    assert_eq!(without_body.error_code(), "stale_pick");
    assert_eq!(with_mesh.error_code(), "stale_pick");
}

#[test]
fn t123_a_headless_pick_past_the_last_edge_or_in_an_unknown_body_fails() {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, &slotted_block()));
    let edges = elements(&mut session, doc, "b-1", Some("edge")).len();

    let past = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": 0, "kind": "edge", "index": edges, "body": "b-1"}}),
    );
    let unknown = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": 0, "kind": "edge", "index": 0, "body": "b-9"}}),
    );

    assert_eq!(past.error_code(), "invalid_pick");
    assert_eq!(unknown.error_code(), "unknown_body");
}

#[test]
fn t123_a_headless_pick_of_a_body_without_geometry_is_body_empty() {
    let mut session = headless_session();
    let doc = open_doc(&mut session, "two-bodies.ptsm");
    send(
        &mut session,
        json!({"type": "apply_edits", "doc": doc, "label": "Roll back", "edits": [{"type": "set_rollback", "rollback": "f-cyl1"}]}),
    )
    .ok();

    let empty = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": 0, "kind": "face", "index": 0, "body": "b-2"}}),
    );

    assert_eq!(empty.error_code(), "body_empty");
}

// Test 124: fallbacks in body coordinates and axis directions.

/// The top of `box.ptsm`.
const BOX_TOP: &str = "f-box1/top";
/// The front top edge of `box.ptsm`, which runs along X before anything moves the box.
const BOX_FRONT_TOP_EDGE: &str = "edge(f-box1/front|f-box1/top)";
/// The unit vector along X.
const X: [f64; 3] = [1.0, 0.0, 0.0];
/// The unit vector along Z.
const Z: [f64; 3] = [0.0, 0.0, 1.0];

/// `box.ptsm` with `more` features after `f-box1`.
fn box_then(more: Vec<Value>) -> String {
    let mut document: Value =
        serde_json::from_str(&fixture("box.ptsm")).expect("fixtures are JSON");
    document["features"]
        .as_array_mut()
        .expect("documents list their features")
        .extend(more);
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// A Move body `feature` of `b-1` by `translation` (features/move_body.md).
fn move_by(feature: &str, translation: [&str; 3]) -> Value {
    json!({"id": feature, "type": "move_body", "body": "b-1", "translation": translation})
}

/// A Move body `feature` of `b-1` that turns it 90 degrees about the origin Z axis.
fn quarter_turn(feature: &str) -> Value {
    json!({
        "id": feature, "type": "move_body", "body": "b-1",
        "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}
    })
}

/// The Move body `f-mv1` of test 124, by (100, 0, 0).
fn move_by_100() -> Value {
    move_by("f-mv1", ["100 mm", "0 mm", "0 mm"])
}

/// The descriptors `list_elements` gives for `b-1` of the document `text`, opened in a fresh display session.
fn descriptors(text: &str) -> Vec<Value> {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, text));
    elements(&mut session, doc, "b-1", None)
}

/// The descriptor named `name` among `descriptors`.
fn named<'a>(descriptors: &'a [Value], name: &str) -> &'a Value {
    descriptors
        .iter()
        .find(|descriptor| descriptor["name"] == name)
        .unwrap_or_else(|| panic!("b-1 has an element named {name}"))
}

/// Asserts that a `Placement` JSON value has this origin, x axis and normal within the geometry tolerance.
fn assert_placement(
    placement: &Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
    what: &str,
) {
    assert!(placement.is_object(), "{what} is a placement: {placement}");
    for (field, expected) in [("origin", origin), ("x_axis", x_axis), ("normal", normal)] {
        assert_point3(
            point3(&placement[field]),
            expected,
            GEOMETRY_TOLERANCE,
            &format!("{what} {field}"),
        );
    }
}

/// A direction turned back by a quarter turn about Z: the inverse of the rotation (x, y, z) -> (-y, x, z).
fn turned_back(direction: [f64; 3]) -> [f64; 3] {
    [direction[1], -direction[0], direction[2]]
}

/// The direction a fingerprint text writes after `n=` (naming.md section 3.7): first nonzero component positive.
fn fingerprint_direction(fingerprint: &Value) -> [f64; 3] {
    let text = fingerprint.as_str().expect("fingerprints are text");
    let field = text
        .split('|')
        .find_map(|part| part.strip_prefix("n="))
        .unwrap_or_else(|| panic!("{text} writes a direction"));
    let components: Vec<f64> = field
        .split(',')
        .map(|component| component.parse().expect("fingerprint numbers parse"))
        .collect();
    [components[0], components[1], components[2]]
}

/// Whether a descriptor is of a line edge or of a cylinder, cone or torus face, which have an axis direction.
fn has_an_axis(descriptor: &Value) -> bool {
    descriptor["curve"] == "line"
        || ["cylinder", "cone", "torus"]
            .iter()
            .any(|surface| descriptor["surface"] == *surface)
}

#[test]
fn t124_a_moved_face_has_its_fallback_in_body_coordinates() {
    let listed = descriptors(&box_then(vec![move_by_100()]));

    let top = named(&listed, BOX_TOP);

    // The placement keeps its origin above the model origin (section 3.9); the fallback undoes the move.
    assert_placement(&top["placement"], [0.0, 0.0, 40.0], X, Z, "placement");
    assert_placement(&top["fallback"], [-100.0, 0.0, 40.0], X, Z, "fallback");
}

#[test]
fn t124_a_moved_line_edge_has_the_axis_direction_of_its_fingerprint() {
    let listed = descriptors(&box_then(vec![move_by_100()]));

    let edge = named(&listed, BOX_FRONT_TOP_EDGE);

    assert_point3(
        point3(&edge["axis_direction"]),
        X,
        GEOMETRY_TOLERANCE,
        "axis direction",
    );
}

#[test]
fn t124_a_turned_face_has_its_fallback_turned_back() {
    let listed = descriptors(&box_then(vec![quarter_turn("f-mv1")]));

    let top = named(&listed, BOX_TOP);

    assert_placement(&top["placement"], [0.0, 0.0, 40.0], X, Z, "placement");
    assert_placement(
        &top["fallback"],
        [0.0, 0.0, 40.0],
        [0.0, -1.0, 0.0],
        Z,
        "fallback",
    );
}

#[test]
fn t124_a_turned_line_edge_keeps_the_axis_direction_it_had_before_the_turn() {
    let listed = descriptors(&box_then(vec![quarter_turn("f-mv1")]));

    let edge = named(&listed, BOX_FRONT_TOP_EDGE);

    let direction = point3(&edge["direction"]);
    assert_near(direction[0], 0.0, GEOMETRY_TOLERANCE, "the edge runs along Y");
    assert_near(direction[1].abs(), 1.0, GEOMETRY_TOLERANCE, "the edge runs along Y");
    assert_point3(
        point3(&edge["axis_direction"]),
        X,
        GEOMETRY_TOLERANCE,
        "axis direction",
    );
}

#[test]
fn t124_without_a_motion_the_fallback_equals_the_placement() {
    let listed = descriptors(&fixture("box.ptsm"));

    let faces: Vec<&Value> = listed
        .iter()
        .filter(|descriptor| descriptor["kind"] == "face")
        .collect();

    assert_eq!(faces.len(), 6);
    for face in faces {
        assert!(face["placement"].is_object(), "{face}");
        assert_eq!(face["fallback"], face["placement"], "{}", face["name"]);
    }
}

#[test]
fn t124_round_faces_have_the_axis_direction_of_their_axis() {
    let listed = descriptors(&fixture("circle.ptsm"));

    let round: Vec<&Value> = listed
        .iter()
        .filter(|descriptor| descriptor["surface"] == "cylinder")
        .collect();

    assert!(!round.is_empty(), "circle.ptsm has a round face: {listed:?}");
    for face in round {
        assert_point3(
            point3(&face["axis_direction"]),
            Z,
            GEOMETRY_TOLERANCE,
            &format!("axis direction of {}", face["name"]),
        );
    }
}

#[test]
fn t124_only_flat_faces_have_a_fallback_and_only_lines_and_round_faces_an_axis_direction() {
    for fixture_name in ["box.ptsm", "circle.ptsm"] {
        let listed = descriptors(&fixture(fixture_name));

        for descriptor in &listed {
            let flat = descriptor["surface"] == "plane";
            assert_eq!(
                descriptor.get("fallback").is_some(),
                flat,
                "{fixture_name}: {descriptor}"
            );
            assert_eq!(
                descriptor.get("axis_direction").is_some(),
                has_an_axis(descriptor),
                "{fixture_name}: {descriptor}"
            );
        }
    }
}

#[test]
fn t124_fallbacks_and_axis_directions_undo_the_motion_of_the_face() {
    // A quarter turn about Z, then a move by (100, 0, 0): every face moves by p -> R p + t.
    let translation = [100.0, 0.0, 0.0];
    let listed = descriptors(&box_then(vec![
        quarter_turn("f-mv1"),
        move_by("f-mv2", ["100 mm", "0 mm", "0 mm"]),
    ]));
    let to_body = |point: [f64; 3]| {
        turned_back([
            point[0] - translation[0],
            point[1] - translation[1],
            point[2] - translation[2],
        ])
    };

    let mut flat = 0;
    let mut with_axis = 0;
    for descriptor in &listed {
        let name = &descriptor["name"];
        let placement = &descriptor["placement"];
        if placement.is_object() {
            flat += 1;
            let fallback = &descriptor["fallback"];
            assert_placement(
                fallback,
                to_body(point3(&placement["origin"])),
                turned_back(point3(&placement["x_axis"])),
                turned_back(point3(&placement["normal"])),
                &format!("fallback of {name}"),
            );
        }
        if has_an_axis(descriptor) {
            with_axis += 1;
            assert_point3(
                point3(&descriptor["axis_direction"]),
                turned_back(fingerprint_direction(&descriptor["reference"]["fp"])),
                GEOMETRY_TOLERANCE,
                &format!("axis direction of {name}"),
            );
        }
    }
    assert_eq!((flat, with_axis), (6, 12), "the box's faces and edges");
}

#[test]
fn t124_an_edge_along_y_before_the_turn_keeps_the_side_its_fingerprint_had_there() {
    // The left top edge runs along Y before the turn and along X after it. Its fingerprint there writes (1, 0, 0)
    // (first nonzero component positive), which turns back to (0, -1, 0): the axis direction keeps the side of the
    // fingerprint where the element is picked, not the side the fingerprint had before the turn.
    let listed = descriptors(&box_then(vec![quarter_turn("f-mv1")]));

    let edge = named(&listed, "edge(f-box1/left|f-box1/top)");

    assert_point3(
        fingerprint_direction(&edge["reference"]["fp"]),
        X,
        GEOMETRY_TOLERANCE,
        "fingerprint direction after the turn",
    );
    assert_point3(
        point3(&edge["axis_direction"]),
        [0.0, -1.0, 0.0],
        GEOMETRY_TOLERANCE,
        "axis direction",
    );
}

#[test]
fn t124_a_face_of_a_pattern_copy_has_its_fallback_in_the_coordinates_of_the_original() {
    // The copy of a 20 x 20 x 10 box at spacing 60 spans x 50..70; its motion is the copy's transform.
    let text = document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["20", "20", "10"], None, "new"),
            linear_pattern("2", "60"),
        ],
    );
    let listed = descriptors(&text);

    let copy_top = named(&listed, "f-box1/top@f-lp1:1");

    assert_placement(&copy_top["placement"], [0.0, 0.0, 10.0], X, Z, "placement");
    assert_placement(&copy_top["fallback"], [-60.0, 0.0, 10.0], X, Z, "fallback");
}

#[test]
fn t124_a_pick_of_a_moved_element_equals_its_listed_entry() {
    let mut session = session();
    let opened = open_text(&mut session, &box_then(vec![quarter_turn("f-mv1"), move_by_100()]));
    let doc = doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    let all = elements(&mut session, doc, "b-1", None);

    for kind in ["face", "edge"] {
        let of_kind: Vec<&Value> = all
            .iter()
            .filter(|element| element["kind"] == kind)
            .collect();
        for (index, element) in of_kind.iter().enumerate() {
            let picked = send(
                &mut session,
                json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": mesh_id, "kind": kind, "index": index}}),
            );

            assert_eq!(&picked.ok()["element"], *element, "{kind} {index}");
        }
    }
}

#[test]
fn t124_a_headless_session_lists_the_same_fallbacks_and_axis_directions() {
    let text = box_then(vec![quarter_turn("f-mv1"), move_by_100()]);
    let mut headless = headless_session();
    let doc = doc_handle(&open_text(&mut headless, &text));

    let listed = elements(&mut headless, doc, "b-1", None);

    assert_eq!(listed, descriptors(&text));
}

// Test 125: drag commits write body coordinates.

/// The reference of `box.ptsm`'s top after the Move body of test 124, as a pick stores it.
fn moved_top() -> Value {
    json!({"q": BOX_TOP, "fp": "plane|n=0,0,1|c=100,0,40|a=600|adj=4,4"})
}

/// A face sketch `f-sk1` on `face` of `b-1` holding one line `e-1` (0, 0)-(10, 0), whose fallback has `origin`, x
/// axis (1, 0, 0) and normal (0, 0, 1), in `frame` (`None` writes no `fallback_frame`: world coordinates).
fn face_sketch(face: Value, origin: [f64; 3], frame: Option<&str>) -> Value {
    let mut plane = json!({
        "kind": "face", "body": "b-1", "face": face,
        "fallback": {"origin": origin, "x_axis": X, "normal": Z}
    });
    if let Some(frame) = frame {
        plane["fallback_frame"] = json!(frame);
    }
    json!({
        "id": "f-sk1", "type": "sketch", "plane": plane,
        "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
        "constraints": []
    })
}

/// The document of test 125: `box.ptsm` moved by (100, 0, 0), then the face sketch `f-sk1` on its top with its
/// fallback at (0, 0, 40) in `frame`.
fn sketch_on_moved_top(frame: Option<&str>) -> String {
    box_then(vec![
        move_by_100(),
        face_sketch(moved_top(), [0.0, 0.0, 40.0], frame),
    ])
}

/// Drags `e-1.end` of `f-sk1` to `to` in one frame, then ends the drag with `commit`; returns the end's reply.
fn drag_and_end(session: &mut Session, doc: u64, to: [f64; 2], commit: bool) -> Sent {
    send(
        session,
        json!({"type": "drag_sketch", "doc": doc, "feature": "f-sk1", "point": "e-1.end", "to": to}),
    )
    .ok();
    send(
        session,
        json!({"type": "end_sketch_drag", "doc": doc, "feature": "f-sk1", "commit": commit}),
    )
}

/// Opens `text` in a fresh display session, drags `e-1.end` to (20, 5) and commits; returns the session, the handle
/// and the commit's reply.
fn committed(text: &str) -> (Session, u64, Sent) {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, text));
    let ended = drag_and_end(&mut session, doc, [20.0, 5.0], true);
    assert_eq!(ended.ok()["revision"], 1, "the commit is one edit");
    (session, doc, ended)
}

#[test]
fn t125_a_drag_commit_on_a_face_writes_the_resolved_placement_with_the_body_marker() {
    // Nothing moves the box, so body coordinates are world coordinates: the fallback becomes the placement on the
    // face, the world fallback (3, 4, 0) projected onto the top.
    let top = json!({"q": BOX_TOP, "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"});
    let text = box_then(vec![face_sketch(top, [3.0, 4.0, 0.0], None)]);

    let (mut session, doc, _) = committed(&text);

    let plane = record(&mut session, doc, "f-sk1")["plane"].clone();
    assert_placement(&plane["fallback"], [3.0, 4.0, 40.0], X, Z, "fallback");
    assert_eq!(plane["fallback_frame"], "body");
}

#[test]
fn t125_a_drag_commit_on_a_moved_face_keeps_its_body_fallback() {
    let (mut session, doc, _) = committed(&sketch_on_moved_top(Some("body")));

    let plane = record(&mut session, doc, "f-sk1")["plane"].clone();
    assert_placement(&plane["fallback"], [0.0, 0.0, 40.0], X, Z, "fallback");
    assert_eq!(plane["fallback_frame"], "body");
}

#[test]
fn t125_a_drag_commit_turns_a_world_fallback_into_body_coordinates() {
    let (mut session, doc, _) = committed(&sketch_on_moved_top(None));

    let plane = record(&mut session, doc, "f-sk1")["plane"].clone();
    assert_placement(&plane["fallback"], [-100.0, 0.0, 40.0], X, Z, "fallback");
    assert_eq!(plane["fallback_frame"], "body");
}

#[test]
fn t125_a_world_fallback_rebuilds_with_its_placement_unchanged_after_the_commit() {
    let mut session = session();
    let opened = open_text(&mut session, &sketch_on_moved_top(None));
    let doc = doc_handle(&opened);
    let before = opened.sketch_solution("f-sk1")["placement"].clone();
    assert_placement(&before, [0.0, 0.0, 40.0], X, Z, "placement before the commit");

    let ended = drag_and_end(&mut session, doc, [20.0, 5.0], true);

    assert_eq!(ended.feature_status("f-sk1")["status"], "ok");
    assert_placement(
        &ended.sketch_solution("f-sk1")["placement"],
        point3(&before["origin"]),
        point3(&before["x_axis"]),
        point3(&before["normal"]),
        "placement after the commit",
    );
}

#[test]
fn t125_a_body_fallback_places_the_sketch_on_the_moved_face_before_and_after_the_commit() {
    let mut session = session();
    let opened = open_text(&mut session, &sketch_on_moved_top(Some("body")));
    let doc = doc_handle(&opened);
    let solved = send(
        &mut session,
        json!({"type": "solve_sketch", "doc": doc, "feature": "f-sk1"}),
    );

    let ended = drag_and_end(&mut session, doc, [20.0, 5.0], true);

    for (placement, what) in [
        (&opened.sketch_solution("f-sk1")["placement"], "sketch_solved on open"),
        (&solved.ok()["solution"]["placement"], "solve_sketch"),
        (&ended.sketch_solution("f-sk1")["placement"], "sketch_solved after the commit"),
    ] {
        assert_placement(placement, [100.0, 0.0, 40.0], X, Z, what);
    }
}

#[test]
fn t125_a_drag_commit_changes_only_the_geometry_and_the_fallback() {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &sketch_on_moved_top(None)));
    let before = record(&mut session, doc, "f-sk1");

    drag_and_end(&mut session, doc, [20.0, 5.0], true).ok();

    let after = record(&mut session, doc, "f-sk1");
    let geometry_and_plane = ["entities", "plane"];
    assert_eq!(
        without_keys(&after, &geometry_and_plane),
        without_keys(&before, &geometry_and_plane)
    );
    for field in ["kind", "body", "face"] {
        assert_eq!(after["plane"][field], before["plane"][field], "plane {field}");
    }
    let end = &after["entities"][0]["end"];
    assert_ne!(
        [number(&end[0]), number(&end[1])],
        [10.0, 0.0],
        "the dragged end is written"
    );
}

#[test]
fn t125_undoing_a_drag_commit_restores_the_world_fallback_exactly() {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &sketch_on_moved_top(None)));
    let before = saved_text(&mut session, doc);
    drag_and_end(&mut session, doc, [20.0, 5.0], true).ok();

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(undone.ok()["label"], "Drag");
    assert_eq!(saved_text(&mut session, doc), before);
    let plane = record(&mut session, doc, "f-sk1")["plane"].clone();
    assert!(plane.get("fallback_frame").is_none(), "{plane}");
}

#[test]
fn t125_a_drag_that_ends_where_it_started_writes_nothing() {
    // Nothing rewrites a fallback on its own: a commit whose geometry equals the stored geometry is no edit, so the
    // world fallback keeps its coordinates and gets no marker.
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &sketch_on_moved_top(None)));
    let before = get_document(&mut session, doc)["document"].clone();

    let ended = drag_and_end(&mut session, doc, [10.0, 0.0], true);

    assert_eq!(ended.ok()["revision"], 0);
    assert_eq!(revision(&mut session, doc), 0);
    assert_eq!(get_document(&mut session, doc)["document"], before);
}

#[test]
fn t125_a_drag_commit_on_an_origin_plane_keeps_its_plane() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");

    drag_and_end(&mut session, doc, [20.0, 5.0], true).ok();

    assert_eq!(
        record(&mut session, doc, "f-sk1")["plane"],
        json!({"kind": "origin", "plane": "XY"})
    );
}
