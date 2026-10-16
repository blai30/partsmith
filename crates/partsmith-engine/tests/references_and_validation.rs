//! Acceptance tests 121 to 123 of `docs/spec/engine.md` section 6: `validate` reports feature messages with their own
//! codes, actions and candidates (section 3.19 step 8), element lists carry short references and picks full ones, and
//! headless picks name their body (section 3.9).
//!
//! Tests 124 (fallbacks in body coordinates and axis directions) and 125 (drag commits in body coordinates) come with
//! phase 2 of the reference stability plan, which fills `fallback` and `axis_direction`.

mod common;
mod corpus;

use common::{
    doc_handle, elements, fixture, headless_session, open, open_doc, open_text, replace_once, send,
    session,
};
use corpus::{
    Scenario, actions, block_then, datum_offset, datum_plane, distance, document, edited_text,
    extrude, labels, r03, rectangle, rectangle_region, repairs, sketch, with,
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
