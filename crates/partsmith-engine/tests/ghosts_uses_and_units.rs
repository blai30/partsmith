//! Acceptance tests 141 to 145 of `docs/spec/engine.md` section 6: ghosts (section 3.23), `list_uses` (section 3.24)
//! and document units with Keep sizes (section 3.25). Tests 141 and 142 check element, region and cause ghosts on
//! corpus R16 and newcomer error NC11, tests 143 and 144 what `list_uses` reports, and test 145 the units of NC09. A
//! test named `tNNN` is section 6 test NNN. Small tests of the same sections follow each group: ghosts that are not
//! captured (a reference lost since the document was opened, a widened reference), the triangles of a lost face, the
//! typed statuses without ghosts; `list_uses` of names that repeat a feature, of features that are not in the
//! document, in timeline order, of a sketch on a datum, through the typed method, and of a document that is not open;
//! and the label of each unit, the typed units method, the request without `keep_sizes`, `bare_numbers` after edits,
//! parameters used as lengths, and the undo, redo, cancel and dirty state of a units change.
//!
//! Scenarios come from `corpus`, opened with their references picked, in a display session unless a test says
//! otherwise; "after its edit" is the corpus edit applied as one labeled `apply_edits` (`corpus::after_edit`).

use crate::common;
use crate::corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, apply, assert_bounding_box, assert_relative, begin, commit,
    doc_handle, fixture, get_document, headless_session, measure_body, new_doc, open, open_text,
    revision, saved_text, send, session, set_document_units, typed_handle,
};
use corpus::{
    NC11_CAUSE, after_edit, angle, assert_ghost_segments, block_back_top, block_right_side,
    block_then, box_below, box_record, cause, chamfer, circle, cube10, datum_on_face, document,
    document_with, edited_in_a_transaction_focused_on, edited_text, face_plane, fillet,
    ghost_points, hole, labels_and_kinds, linear_pattern, message, nc09, nc11, opened,
    origin_plane, owned_pairs, parameter, picked, r03, r12, r16, r33, r41, r66, record, rectangle,
    rectangle_region, reference, repair_ghost, repairs, revolve, sketch, through_all, volume,
};
use partsmith_engine::Session;
use partsmith_model::Units;
use partsmith_protocol::DocHandle;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

// Test 141: element ghosts (corpus R16).

/// The front top edge of CUBE20 that corpus R16's rabbet cuts away, as a ghost segment.
const FRONT_TOP_EDGE: (&[f64], &[f64]) = (&[-10.0, -10.0, 20.0], &[10.0, -10.0, 20.0]);

/// The `add_parameter` edit of an unrelated parameter `x`.
fn unrelated_parameter() -> Value {
    json!([{"type": "add_parameter", "index": 0, "parameter": parameter("p-x", "x", "1 mm")}])
}

#[test]
fn t141_r16_the_lost_edge_has_its_last_build_as_its_ghost() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r16());

    let ghost = repair_ghost(&edited, "f-fil1", "/edges/0");
    assert_eq!(ghost["frame"], "world", "{ghost}");
    assert_ghost_segments(&edited, &ghost, &[FRONT_TOP_EDGE]);
}

#[test]
fn t141_r16_an_unrelated_edit_keeps_an_equal_ghost() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r16());
    let captured = repair_ghost(&edited, "f-fil1", "/edges/0");

    let added = apply(
        &mut session,
        doc,
        Some("Add parameter"),
        unrelated_parameter(),
    );

    let kept = repair_ghost(&added, "f-fil1", "/edges/0");
    assert_eq!(
        (&kept["kind"], &kept["frame"]),
        (&captured["kind"], &captured["frame"])
    );
    assert_eq!(
        ghost_points(&added, &kept),
        ghost_points(&edited, &captured)
    );
}

#[test]
fn t141_r16_undo_removes_the_repair_and_redo_captures_the_ghost_again() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r16());
    let captured = repair_ghost(&edited, "f-fil1", "/edges/0");
    apply(
        &mut session,
        doc,
        Some("Add parameter"),
        unrelated_parameter(),
    )
    .ok();
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(repairs(&undone, "f-fil1"), Vec::<Value>::new());
    let redone = send(&mut session, json!({"type": "redo", "doc": doc}));
    let again = repair_ghost(&redone, "f-fil1", "/edges/0");
    assert_eq!(
        ghost_points(&redone, &again),
        ghost_points(&edited, &captured)
    );
}

#[test]
fn t141_r16_the_document_reopened_has_no_ghost() {
    let mut session = session();

    let reopened = open_text(&mut session, &edited_text(&r16()));

    let lost = repairs(&reopened, "f-fil1");
    assert_eq!(lost.len(), 1, "{lost:?}");
    assert!(lost[0].get("ghost").is_none(), "{}", lost[0]);
}

#[test]
fn t141_r16_a_headless_session_captures_no_ghost() {
    let mut session = headless_session();

    let (_, edited) = after_edit(&mut session, &r16());

    let lost = repairs(&edited, "f-fil1");
    assert_eq!(lost.len(), 1, "{lost:?}");
    assert!(lost[0].get("ghost").is_none(), "{}", lost[0]);
    assert!(edited.buffers.is_empty(), "no buffers in a headless reply");
}

// Test 142: region and cause ghosts (newcomer error NC11).

#[test]
fn t142_nc11_the_lost_region_has_its_outline_as_its_ghost() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc11());

    let ghost = repair_ghost(&edited, "f-ext1", "/profile/regions/0");
    assert_eq!(ghost["frame"], "sketch", "{ghost}");
    assert_eq!(
        ghost_points(&edited, &ghost).len(),
        8,
        "16 values, four segments"
    );
    assert_ghost_segments(
        &edited,
        &ghost,
        &[
            (&[0.0, 0.0], &[80.0, 0.0]),
            (&[80.0, 0.0], &[80.0, 50.0]),
            (&[80.0, 50.0], &[0.0, 50.0]),
            (&[0.0, 50.0], &[0.0, 0.0]),
        ],
    );
}

#[test]
fn t142_nc11_the_cause_has_the_deleted_line_as_its_ghost() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc11());

    let found = cause(&message(&edited, "f-ext1", "profile_not_found"));
    assert_eq!(found["text"], NC11_CAUSE);
    assert_eq!(
        labels_and_kinds(&found),
        owned_pairs(&[("Undo", "undo"), ("Show the line", "show")])
    );
    let ghost = &found["ghost"];
    assert_eq!(ghost["frame"], "sketch", "{found}");
    assert_ghost_segments(&edited, ghost, &[(&[80.0, 50.0], &[0.0, 50.0])]);
}

#[test]
fn t142_nc11_the_empty_body_names_its_root_and_the_same_cause() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc11());

    let empty = message(&edited, "f-fil1", "body_empty");
    assert_eq!(empty["root"], "f-ext1");
    assert_eq!(cause(&empty)["text"], NC11_CAUSE);
}

// Section 3.23: ghosts, rule by rule.

#[test]
fn a_reference_lost_since_the_document_was_opened_gets_no_ghost_when_it_changes() {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &edited_text(&r16())));

    let added = apply(
        &mut session,
        doc,
        Some("Add parameter"),
        unrelated_parameter(),
    );

    let lost = repairs(&added, "f-fil1");
    assert_eq!(lost.len(), 1, "{lost:?}");
    assert!(lost[0].get("ghost").is_none(), "{}", lost[0]);
}

#[test]
fn a_reference_lost_while_its_feature_was_rolled_back_has_its_last_build_as_its_ghost() {
    let mut plain = session();
    let (_, edited) = after_edit(&mut plain, &r41());
    let expected = repair_ghost(&edited, "f-fil1", "/edges/1");
    let mut session = session();

    // Corpus R41's Count lowered in the pattern's inspector: the fillet is rolled back until the commit.
    let committed = edited_in_a_transaction_focused_on(&mut session, &r41(), "f-lp1");

    let ghost = repair_ghost(&committed, "f-fil1", "/edges/1");
    assert_eq!(
        (&ghost["kind"], &ghost["frame"]),
        (&expected["kind"], &expected["frame"])
    );
    assert_eq!(
        ghost_points(&committed, &ghost),
        ghost_points(&edited, &expected)
    );
}

#[test]
fn a_region_lost_while_its_feature_was_rolled_back_has_its_last_outline_as_its_ghost() {
    let mut session = session();

    // Newcomer error NC11's line deleted in the sketch: the extrude is rolled back until the commit, and the sketch's
    // solution before the commit already lacks the line.
    let committed = edited_in_a_transaction_focused_on(&mut session, &nc11(), "f-sk1");

    let ghost = repair_ghost(&committed, "f-ext1", "/profile/regions/0");
    assert_ghost_segments(
        &committed,
        &ghost,
        &[
            (&[0.0, 0.0], &[80.0, 0.0]),
            (&[80.0, 0.0], &[80.0, 50.0]),
            (&[80.0, 50.0], &[0.0, 50.0]),
            (&[0.0, 50.0], &[0.0, 0.0]),
        ],
    );
}

#[test]
fn a_widened_reference_whose_elements_still_exist_has_no_ghost() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r66());

    let widened = repairs(&edited, "f-fil1");
    assert!(!widened.is_empty(), "{}", edited.feature_status("f-fil1"));
    for repair in widened {
        assert!(repair.get("ghost").is_none(), "{repair}");
    }
}

#[test]
fn a_lost_face_has_its_triangles_as_its_ghost() {
    let mut session = session();

    // Corpus R33: the top of 'Box 1' at z = 40, which the inserted cut removes, is the face the sketch was on.
    let (_, edited) = after_edit(&mut session, &r33());

    let ghost = repair_ghost(&edited, "f-sk2", "/plane/face");
    assert_eq!(
        (&ghost["kind"], &ghost["frame"]),
        (&json!("triangles"), &json!("world"))
    );
    let points = ghost_points(&edited, &ghost);
    assert!(
        points.len() >= 3 && points.len().is_multiple_of(3),
        "whole triangles: {points:?}"
    );
    for point in points {
        assert!((point[2] - 40.0).abs() <= 1e-4, "{point:?} lies on the top");
        assert!(
            point[0].abs() <= 10.0 + 1e-4 && point[1].abs() <= 10.0 + 1e-4,
            "{point:?} lies within the 20 x 20 top"
        );
    }
}

#[test]
fn typed_statuses_leave_out_every_ghost_and_keep_the_cause() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r03());
    let shown = message(&edited, "f-fil1", "reference_missing");
    assert!(shown["cause"]["ghost"].is_object(), "{shown}");
    repair_ghost(&edited, "f-fil1", "/edges/0");

    let statuses = serde_json::to_value(
        session
            .feature_statuses(typed_handle(doc))
            .expect("the document is open"),
    )
    .expect("statuses serialize");

    let fillet = statuses
        .as_array()
        .expect("statuses are a list")
        .iter()
        .find(|status| status["id"] == "f-fil1")
        .expect("the fillet has a status")
        .clone();
    let typed_message = &fillet["messages"][0];
    assert_eq!(typed_message["cause"]["text"], shown["cause"]["text"]);
    assert!(
        typed_message["cause"].get("ghost").is_none(),
        "{typed_message}"
    );
    for repair in fillet["repairs"]
        .as_array()
        .expect("a status lists repairs")
    {
        assert!(repair.get("ghost").is_none(), "{repair}");
    }
}

// Tests 143 and 144: `list_uses`.

/// A use as `list_uses` writes it, without `entity` when it has none.
fn a_use(user: &str, field: &str, feature: &str, entity: Option<&str>) -> Value {
    let mut used = json!({"user": user, "field": field, "feature": feature});
    if let Some(entity) = entity {
        used["entity"] = json!(entity);
    }
    used
}

/// The reply of `list_uses` of a document.
fn list_uses(session: &mut Session, doc: u64) -> Sent {
    send(session, json!({"type": "list_uses", "doc": doc}))
}

/// What `list_uses` reports for `text` opened in a display session, in order.
fn uses_of(text: &str) -> Vec<Value> {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, text));
    list_uses(&mut session, doc).ok()["uses"]
        .as_array()
        .expect("list_uses lists uses")
        .clone()
}

/// The uses of `user` among `uses`, in order.
fn uses_by(uses: &[Value], user: &str) -> Vec<Value> {
    uses.iter()
        .filter(|used| used["user"] == user)
        .cloned()
        .collect()
}

/// BLOCK with `f-fil1` of 2 on its back top edge and a face sketch `f-sk2` on its right side, picked (test 143).
fn block_with_a_fillet_and_a_face_sketch() -> String {
    picked(&document(
        &["b-1"],
        &block_then(vec![
            fillet("f-fil1", "b-1", vec![block_back_top()], "2"),
            sketch(
                "f-sk2",
                face_plane(
                    "b-1",
                    block_right_side(),
                    [80.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0],
                ),
                vec![circle("e-5", [25.0, 15.0], 4.0)],
            ),
        ]),
    ))
}

#[test]
fn t143_list_uses_reports_feature_links_and_the_generators_of_stored_references() {
    let mut session = session();
    let doc = doc_handle(&open_text(
        &mut session,
        &block_with_a_fillet_and_a_face_sketch(),
    ));
    let before = revision(&mut session, doc);

    let listed = list_uses(&mut session, doc);

    assert_eq!(
        listed.ok(),
        &json!({"kind": "ok", "type": "list_uses", "uses": [
            a_use("f-ext1", "/profile/sketch", "f-sk1", None),
            a_use("f-fil1", "/edges/0", "f-ext1", None),
            a_use("f-fil1", "/edges/0", "f-ext1", Some("e-3")),
            a_use("f-sk2", "/plane/face", "f-ext1", Some("e-2"))
        ]})
    );
    assert!(listed.events.is_empty(), "{:?}", listed.event_types());
    assert_eq!(revision(&mut session, doc), before);
}

#[test]
fn t144_r41_a_copy_names_its_source_and_its_pattern_once_each() {
    let uses = uses_of(&r41().setup);

    assert_eq!(
        uses_by(&uses, "f-fil1"),
        [
            a_use("f-fil1", "/edges/0", "f-box1", None),
            a_use("f-fil1", "/edges/0", "f-lp1", None),
            a_use("f-fil1", "/edges/1", "f-box1", None),
            a_use("f-fil1", "/edges/1", "f-lp1", None)
        ]
    );
}

#[test]
fn t144_a_merged_name_of_every_copy_adds_no_other_feature() {
    let top_of_every_copy = json!({
        "q": "f-box1/top",
        "fp": "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4",
        "merged": {"f-box1/top": ["f-box1/top@f-lp1:*"]}
    });
    let text = document(
        &["b-1"],
        &[
            cube10(),
            linear_pattern("3", "20"),
            datum_on_face("f-dp1", "b-1", top_of_every_copy, "0"),
        ],
    );

    let uses = uses_of(&text);

    assert_eq!(
        uses_by(&uses, "f-dp1"),
        [
            a_use("f-dp1", "/base/face", "f-box1", None),
            a_use("f-dp1", "/base/face", "f-lp1", None)
        ]
    );
}

#[test]
fn t144_a_hole_point_names_the_entity_before_its_role() {
    let rim = reference(
        "edge(f-box1/top|f-hole1/bore/e-1.center)",
        "circle|n=0,0,1|c=0,0,0|l=18.849556|r=3|adj=2,1",
    );
    let text = document(
        &["b-1"],
        &[
            box_below(),
            sketch(
                "f-sk1",
                origin_plane("XY"),
                vec![circle("e-1", [0.0, 0.0], 3.0)],
            ),
            hole(
                "f-hole1",
                "b-1",
                "f-sk1",
                &["e-1.center"],
                "6",
                through_all(),
            ),
            chamfer("f-cha1", "b-1", vec![rim], "0.5"),
        ],
    );

    let uses = uses_of(&text);

    assert_eq!(
        uses_by(&uses, "f-cha1"),
        [
            a_use("f-cha1", "/edges/0", "f-box1", None),
            a_use("f-cha1", "/edges/0", "f-hole1", Some("e-1"))
        ]
    );
}

/// A revolve `f-rev1` of `f-sk1`'s rectangle by `extent` about the front left edge of a box `f-box1` in `b-2`.
fn revolve_about_an_edge_of_another_body(extent: Value) -> String {
    let edge = reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-5,-5,5|l=10|adj=2,2",
    );
    document(
        &["b-1", "b-2"],
        &[
            box_record("f-box1", "b-2", ["10", "10", "10"], None, "new"),
            sketch(
                "f-sk1",
                origin_plane("XZ"),
                rectangle(1, 10.0, 0.0, 20.0, 10.0),
            ),
            revolve(
                "f-rev1",
                "b-1",
                "f-sk1",
                vec![rectangle_region(1, 10.0, 0.0, 20.0, 10.0)],
                json!({"kind": "edge", "body": "b-2", "edge": edge}),
                extent,
                "new",
            ),
        ],
    )
}

#[test]
fn t144_a_feature_whose_params_fail_validation_reports_its_references_and_no_links() {
    let unreadable = json!({"kind": "angle", "expr": {"x": 1}});

    let uses = uses_of(&revolve_about_an_edge_of_another_body(unreadable));

    assert_eq!(
        uses_by(&uses, "f-rev1"),
        [a_use("f-rev1", "/axis/edge", "f-box1", None)]
    );
}

#[test]
fn a_valid_feature_reports_its_links_and_its_references_sorted_by_field() {
    let uses = uses_of(&revolve_about_an_edge_of_another_body(angle("270")));

    assert_eq!(
        uses_by(&uses, "f-rev1"),
        [
            a_use("f-rev1", "/axis/edge", "f-box1", None),
            a_use("f-rev1", "/profile/sketch", "f-sk1", None)
        ]
    );
}

// Section 3.24: `list_uses`, rule by rule.

#[test]
fn a_feature_named_twice_in_one_reference_is_one_use() {
    // The fillet's edge is named by two faces of 'Box 1'.
    let uses = uses_of(&fixture("fillet.ptsm"));

    assert_eq!(uses, [a_use("f-fil1", "/edges/0", "f-box1", None)]);
}

#[test]
fn a_feature_that_is_not_in_the_document_is_reported_as_a_use() {
    let text = document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["20", "20", "20"], None, "new"),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/front|f-gone/x)",
                    "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
                )],
                "2",
            ),
        ],
    );

    let uses = uses_of(&text);

    assert_eq!(
        uses,
        [
            a_use("f-fil1", "/edges/0", "f-box1", None),
            a_use("f-fil1", "/edges/0", "f-gone", None)
        ]
    );
}

#[test]
fn uses_follow_the_timeline_rather_than_the_ids() {
    let edge = |sides: &str, x: f64, y: f64| {
        reference(
            &format!("edge({sides})"),
            &format!("line|n=0,0,1|c={x},{y},10|l=20|adj=2,2"),
        )
    };
    let text = document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["20", "20", "20"], None, "new"),
            fillet(
                "f-fil2",
                "b-1",
                vec![edge("f-box1/front|f-box1/left", -10.0, -10.0)],
                "1",
            ),
            fillet(
                "f-fil1",
                "b-1",
                vec![edge("f-box1/back|f-box1/right", 10.0, 10.0)],
                "1",
            ),
        ],
    );

    let uses = uses_of(&text);

    assert_eq!(
        uses,
        [
            a_use("f-fil2", "/edges/0", "f-box1", None),
            a_use("f-fil1", "/edges/0", "f-box1", None)
        ]
    );
}

#[test]
fn a_sketch_on_a_datum_uses_the_datum() {
    let uses = uses_of(&r12().setup);

    assert_eq!(
        uses_by(&uses, "f-sk2"),
        [a_use("f-sk2", "/plane/feature", "f-dp1", None)]
    );
}

#[test]
fn the_typed_list_uses_returns_the_uses_of_the_protocol() {
    let mut session = session();
    let doc = doc_handle(&open_text(
        &mut session,
        &block_with_a_fillet_and_a_face_sketch(),
    ));
    let listed = list_uses(&mut session, doc).ok()["uses"].clone();

    let typed_uses = session
        .list_uses(typed_handle(doc))
        .expect("the document is open");

    assert_eq!(
        serde_json::to_value(typed_uses).expect("uses serialize"),
        listed
    );
}

#[test]
fn list_uses_of_a_document_that_is_not_open_fails() {
    let mut session = session();

    let listed = list_uses(&mut session, 7);

    assert_eq!(listed.error_code(), "unknown_document");
    assert!(listed.events.is_empty());
}

/// The document units in `document_changed` of `sent`.
fn units_of(sent: &Sent) -> Value {
    sent.event("document_changed")["document"]["units"].clone()
}

/// A millimeter document with the parameter `w` = `30`, used as the X size of a box sized `w`, `20 mm` and `10 mm`.
fn box_sized_by_a_bare_parameter() -> String {
    document_with(
        "mm",
        &[parameter("p-w", "w", "30")],
        &["b-1"],
        &[box_record(
            "f-box1",
            "b-1",
            ["w", "20 mm", "10 mm"],
            None,
            "new",
        )],
    )
}

// Test 145: document units (NC09).

#[test]
fn t145_opening_counts_the_four_numbers_without_a_unit() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc09());

    assert_eq!(sent.event("document_changed")["bare_numbers"], 4);
}

#[test]
fn t145_keep_sizes_is_one_change_that_keeps_four_numbers() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(
        switched.ok(),
        &json!({"kind": "ok", "type": "set_document_units", "revision": 1, "kept": 4})
    );
    assert_eq!(
        switched.event_types()[..2],
        ["document_changed", "recomputed"],
        "{:?}",
        switched.event_types()
    );
    let changed = switched.event("document_changed");
    assert_eq!(changed["revision"], 1);
    assert_eq!(changed["document"]["units"], "in");
    assert_eq!(changed["bare_numbers"], 0);
    assert_eq!(changed["undo"], "Change units to in");
}

#[test]
fn t145_keep_sizes_writes_the_old_unit_after_each_number() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    set_document_units(&mut session, doc, "in", true).ok();

    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["40 mm", "20 mm", "10 mm"])
    );
    assert_eq!(record(&mut session, doc, "f-fil1")["radius"], "2 mm");
}

#[test]
fn t145_keep_sizes_keeps_the_volume() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());
    let before = volume(&mut session, doc, "b-1");

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.feature_status("f-fil1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        before,
        GEOMETRY_TOLERANCE,
        "volume",
    );
}

#[test]
fn t145_undo_restores_the_text_exactly() {
    let mut session = session();
    let (doc, _, before) = opened(&mut session, &nc09());
    set_document_units(&mut session, doc, "in", true).ok();

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(undone.ok()["label"], "Change units to in");
    assert_eq!(undone.event("document_changed")["bare_numbers"], 4);
    assert_eq!(saved_text(&mut session, doc), before);
}

#[test]
fn t145_without_keep_sizes_nothing_is_kept_and_the_numbers_mean_inches() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", false);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 0);
    let changed = switched.event("document_changed");
    assert_eq!(changed["document"]["units"], "in");
    assert_eq!(changed["bare_numbers"], 4);
    assert_eq!(changed["undo"], "Change units to in");
    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["40", "20", "10"])
    );
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [-508.0, -254.0, 0.0],
        [508.0, 254.0, 254.0],
    );
}

#[test]
fn t145_the_current_units_change_nothing() {
    for keep_sizes in [true, false] {
        let mut session = session();
        let (doc, _, before) = opened(&mut session, &nc09());

        let same = set_document_units(&mut session, doc, "mm", keep_sizes);

        assert_eq!(
            same.ok(),
            &json!({"kind": "ok", "type": "set_document_units", "revision": 0, "kept": 0}),
            "keep_sizes {keep_sizes}"
        );
        assert!(same.events.is_empty(), "no event: {:?}", same.event_types());
        assert_eq!(revision(&mut session, doc), 0);
        assert_eq!(saved_text(&mut session, doc), before);
        let nothing = send(&mut session, json!({"type": "undo", "doc": doc}));
        assert_eq!(nothing.error_code(), "nothing_to_undo", "no undo entry");
    }
}

#[test]
fn t145_inside_a_transaction_the_edits_join_it() {
    let mut session = session();
    let (doc, _, before) = opened(&mut session, &nc09());
    begin(&mut session, doc, "Units").ok();

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 4);
    let changed = switched.event("document_changed");
    assert!(
        changed.get("undo").is_none(),
        "no undo entry of its own: {changed}"
    );
    assert_eq!(changed["transaction"], "Units");
    let committed = commit(&mut session, doc);
    assert_eq!(committed.ok()["recorded"], true);
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undone.ok()["label"], "Units");
    assert_eq!(saved_text(&mut session, doc), before);
}

// Section 3.25: the change of units as a command.

#[test]
fn a_request_without_keep_sizes_does_not_keep_sizes() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = send(
        &mut session,
        json!({"type": "set_document_units", "doc": doc, "units": "in"}),
    );

    assert_eq!(switched.ok()["kept"], 0);
    assert_eq!(switched.event("document_changed")["bare_numbers"], 4);
}

#[test]
fn the_label_names_the_new_units_by_their_symbol() {
    let mut session = session();
    let doc = new_doc(&mut session);

    for (units, label) in [
        ("cm", "Change units to cm"),
        ("m", "Change units to m"),
        ("in", "Change units to in"),
        ("mm", "Change units to mm"),
    ] {
        let switched = set_document_units(&mut session, doc, units, true);

        assert_eq!(switched.ok()["kept"], 0, "{units}");
        assert_eq!(units_of(&switched), units);
        assert_eq!(switched.event("document_changed")["undo"], label);
    }
    assert_eq!(revision(&mut session, doc), 4);
}

#[test]
fn keep_sizes_from_inches_writes_inches() {
    let text = document_with(
        "in",
        &[],
        &["b-1"],
        &[box_record("f-box1", "b-1", ["2", "1", "1"], None, "new")],
    );
    let mut session = session();
    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);
    assert_eq!(opened.event("document_changed")["bare_numbers"], 3);
    let before = volume(&mut session, doc, "b-1");

    let switched = set_document_units(&mut session, doc, "mm", true);

    assert_eq!(switched.ok()["kept"], 3);
    assert_eq!(
        switched.event("document_changed")["undo"],
        "Change units to mm"
    );
    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["2 in", "1 in", "1 in"])
    );
    assert_relative(
        volume(&mut session, doc, "b-1"),
        before,
        GEOMETRY_TOLERANCE,
        "volume",
    );
}

#[test]
fn keep_sizes_without_bare_numbers_only_changes_the_units() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let features_before = get_document(&mut session, doc)["document"]["features"].clone();

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 0);
    assert_eq!(units_of(&switched), "in");
    assert_eq!(
        get_document(&mut session, doc)["document"]["features"],
        features_before
    );
}

#[test]
fn a_parameter_used_as_a_length_is_counted_and_kept() {
    let mut session = session();
    let opened = open_text(&mut session, &box_sized_by_a_bare_parameter());
    let doc = doc_handle(&opened);
    assert_eq!(opened.event("document_changed")["bare_numbers"], 1);

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["kept"], 1);
    assert_eq!(
        switched.event("document_changed")["document"]["parameters"],
        json!([{"id": "p-w", "name": "w", "expr": "30 mm"}])
    );
    assert_eq!(switched.parameter_value("p-w")["value"], 30.0);
}

#[test]
fn redo_changes_the_units_again() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());
    set_document_units(&mut session, doc, "in", true).ok();
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();

    let redone = send(&mut session, json!({"type": "redo", "doc": doc}));

    assert_eq!(redone.ok()["label"], "Change units to in");
    assert_eq!(units_of(&redone), "in");
    assert_eq!(redone.event("document_changed")["bare_numbers"], 0);
}

#[test]
fn a_units_change_makes_the_document_dirty_and_undo_makes_it_clean() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", true);
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(switched.event("document_changed")["dirty"], true);
    assert_eq!(undone.event("document_changed")["dirty"], false);
}

#[test]
fn cancelling_the_transaction_reverts_the_units() {
    let mut session = session();
    let (doc, _, before) = opened(&mut session, &nc09());
    begin(&mut session, doc, "Units").ok();
    set_document_units(&mut session, doc, "in", true).ok();

    let cancelled = send(
        &mut session,
        json!({"type": "cancel_transaction", "doc": doc}),
    );

    assert_eq!(units_of(&cancelled), "mm");
    assert_eq!(cancelled.event("document_changed")["bare_numbers"], 4);
    assert_eq!(saved_text(&mut session, doc), before);
}

#[test]
fn an_unknown_document_fails_before_anything_changes() {
    let mut session = session();

    let sent = set_document_units(&mut session, 9, "in", true);

    assert_eq!(sent.error_code(), "unknown_document");
    assert!(sent.events.is_empty(), "{:?}", sent.event_types());
}

#[test]
fn the_typed_method_returns_the_revision_and_the_kept_count() {
    let mut session = session();
    let opened = session
        .open_document(&nc09(), Vec::new())
        .expect("the NC09 document opens");

    let changed = session
        .set_document_units(opened.doc, Units::In, true)
        .expect("the units change");
    let unchanged = session
        .set_document_units(opened.doc, Units::In, true)
        .expect("the same units change nothing");

    assert_eq!((changed.revision, changed.kept), (1, 4));
    assert_eq!((unchanged.revision, unchanged.kept), (1, 0));
    let document = session.document(opened.doc).expect("the document is open");
    assert_eq!(document.units, Units::In);
    assert_eq!(document.bare_numbers(session.registry()), 0);
}

#[test]
fn the_typed_method_fails_for_a_document_that_is_not_open() {
    let mut session = session();

    let failed = session.set_document_units(DocHandle(3), Units::In, false);

    assert!(
        matches!(
            failed,
            Err(partsmith_engine::EngineError::UnknownDocument { .. })
        ),
        "{failed:?}"
    );
}

// Section 3.25: `bare_numbers` in `document_changed`.

#[test]
fn documents_without_bare_numbers_report_none() {
    let mut session = session();

    let created = send(&mut session, json!({"type": "new_document"}));
    let opened = open(&mut session, "box.ptsm");

    assert_eq!(created.event("document_changed")["bare_numbers"], 0);
    assert_eq!(opened.event("document_changed")["bare_numbers"], 0);
}

#[test]
fn bare_numbers_follow_each_edit() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let mut sized = get_document(&mut session, doc)["document"]["features"][0].clone();
    sized["size"] = json!(["20", "30 mm", "height"]);

    let edited = apply(
        &mut session,
        doc,
        Some("Size"),
        json!([{"type": "update_feature", "feature": sized}]),
    );
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(edited.event("document_changed")["bare_numbers"], 1);
    assert_eq!(undone.event("document_changed")["bare_numbers"], 0);
    assert_eq!(saved_text(&mut session, doc), fixture("box.ptsm"));
}
