//! Acceptance tests 136 to 140 of `docs/spec/engine.md` section 6: cause lines (section 3.21) and `apply_fix` of their
//! actions (section 3.22). Test 136 checks the engine twins of the reference stability corpus
//! (`docs/design/reference-stability-corpus.md`, "Where the assertions run"), test 137 attribution per message key,
//! test 138 parameters and headless sessions, test 139 that a message without a clean snapshot has no cause, and test
//! 140 that causes stay out of documents and results. A test named `tNNN` is section 6 test NNN; a test with several
//! scenarios is one test per scenario. The tests after them check the rules of section 3.21 one at a time on small
//! documents: the words of each kind of change, the count of other changes, which actions a cause offers and when,
//! and cause actions that no longer apply.
//!
//! Scenarios come from `corpus`: each setup is opened with its references picked, in a display session unless a test
//! says otherwise, and its edit is applied as one `apply_edits` labeled `Edit`, so the actions it offers carry
//! revision 1. "The named message" of a feature is its first message: its error, else its first warning or notice.

use crate::common;
use crate::corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, Z_AXIS, apply, assert_bounding_box, assert_fallback, assert_relative,
    doc_handle, fixture, headless_session, measure_body, open_text, replace_once, revision,
    saved_text, send, session, set_height, set_rollback,
};
use corpus::{
    Change, EDIT_LABEL, Scenario, actions, after_edit, apply_as_one_entry, apply_fix,
    assert_ghost_segments, assert_undo_restores, block_back_top, block_edited, block_then,
    box_record, cause, circle, circle_region, constrained_sketch, datum_offset, datum_plane,
    distance, document, document_with, edited_text, edits, extrude, fillet, labels,
    labels_and_kinds, line, line_distance, message, messages, nc07, opened, origin_plane,
    parameter, picked, r03, r10a, r12, r12_features, r13, r16, r23b, r26_deleted, r26_recreated,
    r28, r30a, r30b, r33, r41, r48, r50, r54, r57, r63a, record, rectangle, rectangle_region,
    reference, send_fix, sketch, statuses, volume, with, without_keys,
};
use partsmith_engine::Session;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The keys of a status that hold session or display data: the revision of a `FixRef`, a mesh id, a ghost and a cause.
const SESSION_DATA: [&str; 4] = ["revision", "mesh_id", "ghost", "cause"];

/// The first message of `feature` in the only `recomputed` event of `sent`.
fn named_message(sent: &Sent, feature: &str) -> Value {
    messages(sent, feature)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{feature} has a message: {}", sent.feature_status(feature)))
}

/// The cause line a test expects of a message.
struct Expected {
    /// The text of the cause.
    text: &'static str,
    /// The feature the primary change belongs to; `None` for a change of a parameter or of the units.
    feature: Option<&'static str>,
    /// The label and run kind of each action of the cause, in order.
    actions: &'static [(&'static str, &'static str)],
}

/// Asserts the cause of `message`: its text, the feature it names and the label and run kind of each of its actions, in
/// order; returns the cause.
fn assert_cause(message: &Value, expected: &Expected) -> Value {
    let found = cause(message);
    assert_eq!(found["text"], expected.text, "{message}");
    assert_eq!(
        found.get("feature").and_then(Value::as_str),
        expected.feature,
        "{found}"
    );
    let offered: Vec<(String, String)> = expected
        .actions
        .iter()
        .map(|(label, kind)| ((*label).to_owned(), (*kind).to_owned()))
        .collect();
    assert_eq!(labels_and_kinds(&found), offered, "{found}");
    found
}

/// Asserts that no message of `sent` has a cause.
fn assert_no_cause(sent: &Sent) {
    for status in statuses(sent) {
        for message in status["messages"]
            .as_array()
            .expect("statuses list their messages")
        {
            assert!(message.get("cause").is_none(), "{message} has no cause");
        }
    }
}

/// A document after an edit, in a display session, with what its setup gave.
struct Twin {
    /// The session.
    session: Session,
    /// The document.
    doc: u64,
    /// The setup's text, as opened.
    setup: String,
    /// The setup's status of the feature the cause is on.
    clean_status: Value,
    /// The setup's volume of body 'Body 1'.
    clean_volume: f64,
    /// The reply of the edit.
    edited: Sent,
}

impl Twin {
    /// Opens the scenario's setup and applies its edit, keeping what the setup gave for `feature`.
    fn of(scenario: &Scenario, feature: &str) -> Twin {
        Twin::new(&scenario.setup, feature, |session, doc| {
            Value::Array(edits(session, doc, &scenario.changes))
        })
    }

    /// Opens `text` in a display session, keeps its status of `feature` and the volume of body 'Body 1', and applies
    /// the model edits `make` builds as one `apply_edits` labeled `Edit`.
    fn new(text: &str, feature: &str, make: impl FnOnce(&mut Session, u64) -> Value) -> Twin {
        let mut session = session();
        let (doc, sent, setup) = opened(&mut session, text);
        let clean_status = sent.feature_status(feature).clone();
        let clean_volume = volume(&mut session, doc, "b-1");
        let made = make(&mut session, doc);
        let edited = apply(&mut session, doc, Some(EDIT_LABEL), made);
        edited.ok();
        Twin {
            session,
            doc,
            setup,
            clean_status,
            clean_volume,
            edited,
        }
    }

    /// Applies the first action of `cause`, a revert, as one undo entry labeled with its label, and asserts that
    /// `feature` then has the status its setup gave, so the message is gone, and body 'Body 1' the setup's volume.
    fn assert_revert_gives_the_clean_result(&mut self, feature: &str, cause: &Value) {
        let revert = actions(cause)[0].clone();

        let applied = apply_as_one_entry(&mut self.session, self.doc, &revert);

        assert_eq!(
            without_keys(applied.feature_status(feature), &SESSION_DATA),
            without_keys(&self.clean_status, &SESSION_DATA)
        );
        assert_relative(
            volume(&mut self.session, self.doc, "b-1"),
            self.clean_volume,
            GEOMETRY_TOLERANCE,
            "volume of Body 1",
        );
    }

    /// The document's text now.
    fn text(&mut self) -> String {
        saved_text(&mut self.session, self.doc)
    }
}

// Test 136: cause lines of the corpus twins.

#[test]
fn t136_r03_a_deleted_bevel_line_is_named_and_shown_where_it_was() {
    let twin = Twin::of(&r03(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
            feature: Some("f-sk1"),
            actions: &[("Undo", "undo"), ("Show the line", "show")],
        },
    );

    assert_eq!(actions(&found)[0]["run"], json!({"kind": "undo"}));
    assert_eq!(
        actions(&found)[1]["run"],
        json!({"kind": "show", "feature": "f-sk1", "entity": "e-5"})
    );
    let ghost = &found["ghost"];
    assert_eq!(ghost["frame"], "sketch", "{found}");
    assert_ghost_segments(&twin.edited, ghost, &[(&[80.0, 40.0], &[70.0, 50.0])]);
}

#[test]
fn t136_r10a_a_moved_slot_line_is_named_and_shown() {
    let twin = Twin::of(&r10a(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you moved the line from (30, -1) to (50, -1) in 'Sketch 2'.",
            feature: Some("f-sk2"),
            actions: &[("Undo", "undo"), ("Show the line", "show")],
        },
    );

    assert_eq!(
        actions(&found)[1]["run"],
        json!({"kind": "show", "feature": "f-sk2", "entity": "e-5"})
    );
}

#[test]
fn t136_r12_a_pocket_made_through_offers_to_set_its_distance_back() {
    let mut twin = Twin::of(&r12(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
            feature: Some("f-ext2"),
            actions: &[
                ("Set Distance of 'Extrude 2' back to 5 mm", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn t136_r13_a_block_made_flush_offers_to_set_its_distance_back() {
    let mut twin = Twin::of(&r13(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Distance of 'Extrude 2' from 20 mm to 30 mm.",
            feature: Some("f-ext2"),
            actions: &[
                ("Set Distance of 'Extrude 2' back to 20 mm", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn t136_r16_an_inserted_cut_is_named_without_its_sketch_and_offers_to_suppress_it() {
    let mut twin = Twin::of(&r16(), "f-fil1");

    // The added sketch reaches the fillet only through the added extrude, so it is not counted.
    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you added 'Extrude 1' before this fillet.",
            feature: Some("f-ext2"),
            actions: &[("Suppress 'Extrude 1'", "apply"), ("Undo", "undo")],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    assert_eq!(
        record(&mut twin.session, twin.doc, "f-ext2")["suppressed"],
        true
    );
}

#[test]
fn t136_r23b_a_deleted_peg_is_named_by_the_label_it_had() {
    let twin = Twin::of(&r23b(), "f-fil1");

    // Peg B's extrude is 'Extrude 1' now.
    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you deleted the former 'Extrude 1'.",
            feature: Some("f-ext1"),
            actions: &[("Undo", "undo")],
        },
    );
}

/// The cause line of 'Sketch 2' in corpus R26 after its delete, and after its add.
const R26_CAUSE: &str = "This started after you deleted the former 'Extrude 1'.";

#[test]
fn t136_r26_after_the_delete_the_sketch_offers_to_keep_where_it_was_then_undo() {
    let mut session = session();

    let (_, deleted, _) = r26_deleted(&mut session);

    // The boss's extrude is 'Extrude 1' now.
    let warning = named_message(&deleted, "f-sk2");
    assert_eq!(warning["code"], "plane_fallback");
    let found = assert_cause(
        &warning,
        &Expected {
            text: R26_CAUSE,
            feature: Some("f-ext1"),
            actions: &[("Keep the sketch where it was", "apply"), ("Undo", "undo")],
        },
    );
    assert_eq!(
        actions(&found)[0]["run"],
        json!({"kind": "apply", "fix": {
            "feature": "f-sk2", "code": "plane_fallback", "field": "/plane/face",
            "cause": true, "index": 0, "revision": 2
        }})
    );
}

#[test]
fn t136_r26_after_the_add_the_same_cause_offers_only_to_keep_where_it_was() {
    let mut session = session();

    let (_, recreated) = r26_recreated(&mut session);

    // The add is the newest undo entry, so Undo would not revert the delete.
    let found = assert_cause(
        &message(&recreated, "f-sk2", "plane_fallback"),
        &Expected {
            text: R26_CAUSE,
            feature: Some("f-ext1"),
            actions: &[("Keep the sketch where it was", "apply")],
        },
    );
    assert_eq!(actions(&found)[0]["run"]["fix"]["revision"], 3);
}

#[test]
fn t136_r26_keeping_the_sketch_where_it_was_puts_the_boss_back_on_top() {
    let mut session = session();
    let (doc, recreated) = r26_recreated(&mut session);
    let before = saved_text(&mut session, doc);
    let keep = actions(&cause(&message(&recreated, "f-sk2", "plane_fallback")))[0].clone();
    assert_eq!(keep["label"], "Keep the sketch where it was");

    let applied = apply_as_one_entry(&mut session, doc, &keep);

    // The placement the sketch was built with before the delete, on the 80 tall block's top.
    assert_fallback(
        &record(&mut session, doc, "f-sk2")["plane"],
        [0.0, 0.0, 80.0],
        Z_AXIS,
        Some("body"),
    );
    assert_relative(
        volume(&mut session, doc, "b-1"),
        323141.592654,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [0.0, 0.0, 0.0],
        [80.0, 50.0, 90.0],
    );
    // Without a `separate_solid` warning, the boss and the block are one solid.
    assert!(
        messages(&applied, "f-ext2")
            .iter()
            .all(|message| message["code"] != "separate_solid"),
        "{}",
        applied.feature_status("f-ext2")
    );
    let still = cause(&message(&applied, "f-sk2", "plane_fallback"));
    assert_eq!(still["text"], R26_CAUSE);
    assert_eq!(actions(&still), Vec::<Value>::new());
    assert_undo_restores(&mut session, doc, &keep["label"], &before);
}

#[test]
fn t136_r30a_a_join_made_a_cut_offers_to_set_the_operation_back() {
    let mut twin = Twin::of(&r30a(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Operation of 'Extrude 2' from Join to Cut.",
            feature: Some("f-ext2"),
            actions: &[
                ("Set Operation of 'Extrude 2' back to Join", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn t136_r30b_a_boolean_join_made_a_cut_offers_to_set_the_operation_back() {
    let mut twin = Twin::of(&r30b(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Operation of 'Boolean 1' from Join to Cut.",
            feature: Some("f-bool1"),
            actions: &[
                ("Set Operation of 'Boolean 1' back to Join", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn t136_r33_a_cut_inserted_before_the_sketch_offers_to_suppress_it_and_not_to_keep_it() {
    let mut twin = Twin::of(&r33(), "f-sk2");

    // The sketch stays at z = 40, where it was built before, so there is nothing to keep.
    let found = assert_cause(
        &named_message(&twin.edited, "f-sk2"),
        &Expected {
            text: "This started after you added 'Extrude 1' before this sketch.",
            feature: Some("f-ext3"),
            actions: &[("Suppress 'Extrude 1'", "apply"), ("Undo", "undo")],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-sk2", &found);
}

#[test]
fn t136_r41_a_lower_count_offers_to_set_it_back() {
    let mut twin = Twin::of(&r41(), "f-fil1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Count of 'Linear pattern 1' from 3 to 2.",
            feature: Some("f-lp1"),
            actions: &[
                ("Set Count of 'Linear pattern 1' back to 3", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn t136_r48_a_units_change_offers_only_undo_since_the_message_offers_the_old_size() {
    let twin = Twin::of(&r48(), "f-fil1");
    let too_large = named_message(&twin.edited, "f-fil1");

    // Its revert, `Use 2 mm`, is the message's own first action.
    assert_eq!(labels(&too_large)[0], "Use 2 mm");
    assert_cause(
        &too_large,
        &Expected {
            text: "This started after you changed the document units from mm to in.",
            feature: None,
            actions: &[("Undo", "undo")],
        },
    );
}

#[test]
fn t136_r54_each_feature_of_the_chain_names_the_deleted_boss_by_the_label_it_had() {
    let twin = Twin::of(&r54(), "f-dp1");

    // 'Extrude 2' labels `f-ext3` now.
    for feature in ["f-dp1", "f-sk3", "f-ext3"] {
        assert_cause(
            &named_message(&twin.edited, feature),
            &Expected {
                text: "This started after you deleted the former 'Extrude 2'.",
                feature: Some("f-ext2"),
                actions: &[("Undo", "undo")],
            },
        );
    }
}

#[test]
fn t136_r57_a_full_turn_offers_to_restore_the_revolve() {
    let mut twin = Twin::of(&r57(), "f-cha1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-cha1"),
        &Expected {
            text: "This started after you changed Extent of 'Revolve 1' from Angle to Full.",
            feature: Some("f-rev1"),
            actions: &[
                ("Restore the settings of 'Revolve 1'", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    // The 270 degree revolve and the chamfer again.
    twin.assert_revert_gives_the_clean_result("f-cha1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn t136_r63a_a_counterbore_offers_to_restore_the_hole() {
    let mut twin = Twin::of(&r63a(), "f-cha1");

    let found = assert_cause(
        &named_message(&twin.edited, "f-cha1"),
        &Expected {
            text: "This started after you changed Type of 'Hole 1' from Simple to Counterbore.",
            feature: Some("f-hole1"),
            actions: &[
                ("Restore the settings of 'Hole 1'", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-cha1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

// Test 137: attribution per message key (corpus R41 with both references).

/// The text of the cause of corpus R41's warning, made by edit (1).
const R41_FIRST_CAUSE: &str =
    "This started after you changed Count of 'Linear pattern 1' from 3 to 2.";

/// Applies one change to the R41 document as one `apply_edits` labeled `label`.
fn change(session: &mut Session, doc: u64, label: &str, edit: Change) -> Sent {
    let made = edits(session, doc, &[edit]);
    let sent = apply(session, doc, Some(label), Value::Array(made));
    sent.ok();
    sent
}

#[test]
fn t137_r41_edit_1_gives_the_warning_of_the_lost_copy_its_cause() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r41());

    let warning = message(&edited, "f-fil1", "reference_missing");
    assert_eq!(warning["field"], "/edges/1");
    assert_cause(
        &warning,
        &Expected {
            text: R41_FIRST_CAUSE,
            feature: Some("f-lp1"),
            actions: &[
                ("Set Count of 'Linear pattern 1' back to 3", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

#[test]
fn t137_r41_edit_2_keeps_the_text_and_makes_the_actions_again_without_undo() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &r41());

    let spaced = change(
        &mut session,
        doc,
        "Spacing",
        Change::Set("f-lp1", "/direction/spacing", json!("25")),
    );

    assert_cause(
        &message(&spaced, "f-fil1", "reference_missing"),
        &Expected {
            text: R41_FIRST_CAUSE,
            feature: Some("f-lp1"),
            actions: &[("Set Count of 'Linear pattern 1' back to 3", "apply")],
        },
    );
}

#[test]
fn t137_r41_edit_3_gives_the_new_error_a_cause_from_its_own_clean_snapshot() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &r41());
    change(
        &mut session,
        doc,
        "Spacing",
        Change::Set("f-lp1", "/direction/spacing", json!("25")),
    );

    let lowered = change(
        &mut session,
        doc,
        "Count",
        Change::Set("f-lp1", "/direction/count", json!("1")),
    );

    let error = message(&lowered, "f-fil1", "references_not_found");
    assert_eq!(error["field"], "/edges");
    assert_cause(
        &error,
        &Expected {
            text: "This started after you changed Count of 'Linear pattern 1' from 2 to 1.",
            feature: Some("f-lp1"),
            actions: &[
                ("Set Count of 'Linear pattern 1' back to 2", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

// Test 138: parameters and headless sessions (python-api.md test 93).

/// Corpus R12's setup with the distance of `f-ext2` the parameter `d` = `5 mm`, picked.
fn r12_with_a_parameter() -> String {
    picked(&document_with(
        "mm",
        &[parameter("p-d", "d", "5 mm")],
        &["b-1"],
        &r12_features("d"),
    ))
}

/// The `update_parameter` edit that gives the parameter `id` named `name` the expression `expr`.
fn set_parameter(id: &str, name: &str, expr: &str) -> Value {
    json!({"type": "update_parameter", "parameter": {"id": id, "name": name, "expr": expr}})
}

/// [`r12_with_a_parameter`] opened in a headless session with `d` changed to `30 mm`: the handle, the text as opened
/// and the edit's reply.
fn r12_with_d_made_30(session: &mut Session) -> (u64, String, Sent) {
    let (doc, _, setup) = opened(session, &r12_with_a_parameter());
    let edited = apply(
        session,
        doc,
        Some(EDIT_LABEL),
        json!([set_parameter("p-d", "d", "30 mm")]),
    );
    edited.ok();
    (doc, setup, edited)
}

#[test]
fn t138_a_parameter_change_is_named_by_the_parameter_in_a_headless_session() {
    let mut session = headless_session();

    let (_, _, edited) = r12_with_d_made_30(&mut session);

    let found = assert_cause(
        &named_message(&edited, "f-fil1"),
        &Expected {
            text: "This started after you changed parameter 'd' from 5 mm to 30 mm.",
            feature: None,
            actions: &[("Set 'd' back to 5 mm", "apply"), ("Undo", "undo")],
        },
    );
    assert!(found.get("ghost").is_none(), "{found}");
}

#[test]
fn t138_setting_the_parameter_back_gives_back_the_text_before_the_change() {
    let mut session = headless_session();
    let (doc, setup, edited) = r12_with_d_made_30(&mut session);
    let revert = actions(&cause(&named_message(&edited, "f-fil1")))[0].clone();

    let applied = apply_as_one_entry(&mut session, doc, &revert);

    assert_eq!(messages(&applied, "f-fil1"), Vec::<Value>::new());
    assert_eq!(saved_text(&mut session, doc), setup);
}

// Test 139: no clean snapshot, no cause.

#[test]
fn t139_r16_a_document_opened_after_its_edit_has_no_cause() {
    let mut session = session();

    let opened = open_text(&mut session, &edited_text(&r16()));

    assert!(!messages(&opened, "f-fil1").is_empty());
    assert_no_cause(&opened);
}

#[test]
fn t139_the_first_error_of_a_new_feature_has_no_cause() {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &fixture("box.ptsm")));
    let gone = fillet(
        "f-fil1",
        "b-1",
        vec![reference(
            "edge(f-box1/front|f-gone/x)",
            "line|n=1,0,0|c=0,-15,40|l=20|adj=2,2",
        )],
        "2 mm",
    );

    let added = apply(
        &mut session,
        doc,
        Some("Add fillet"),
        json!([{"type": "add_feature", "index": 1, "feature": gone}]),
    );

    assert_eq!(added.feature_status("f-fil1")["status"], "error");
    assert_no_cause(&added);
}

#[test]
fn t139_nc07_an_added_constraint_is_named_by_its_title() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc07());

    // A constraint added is no change with a revert, and its feature is the sketch itself, so nothing is shown.
    assert_cause(
        &message(&edited, "f-sk1", "sketch_conflict"),
        &Expected {
            text: "This started after you added Distance 30 mm in 'Sketch 1'.",
            feature: Some("f-sk1"),
            actions: &[("Undo", "undo")],
        },
    );
}

// Test 140: causes stay out of documents and results.

#[test]
fn t140_r12_the_saved_text_is_the_same_as_without_the_session_s_history() {
    let mut display = session();
    let mut headless = headless_session();

    let (shown, edited) = after_edit(&mut display, &r12());
    let (plain, _) = after_edit(&mut headless, &r12());

    assert!(
        named_message(&edited, "f-fil1").get("cause").is_some(),
        "the session that saw the fillet build made a cause"
    );
    assert_eq!(
        saved_text(&mut display, shown),
        saved_text(&mut headless, plain)
    );
}

#[test]
fn t140_r12_statuses_differ_only_in_display_fields_between_two_sessions_that_saw_the_edit() {
    let mut display = session();
    let mut headless = headless_session();

    let (shown, edited) = after_edit(&mut display, &r12());
    let (plain, edited_headless) = after_edit(&mut headless, &r12());

    let display_fields = ["ghost", "element"];
    assert_eq!(
        without_keys(&json!(statuses(&edited)), &display_fields),
        without_keys(&json!(statuses(&edited_headless)), &display_fields)
    );
    assert_relative(
        volume(&mut display, shown, "b-1"),
        volume(&mut headless, plain, "b-1"),
        GEOMETRY_TOLERANCE,
        "volume of Body 1",
    );
}

#[test]
fn t140_r12_a_session_that_never_saw_the_fillet_build_differs_only_in_the_cause() {
    let mut first = session();
    let (doc, edited) = after_edit(&mut first, &r12());
    let text = saved_text(&mut first, doc);
    let mut second = session();

    let reopened = open_text(&mut second, &text);

    assert_no_cause(&reopened);
    assert_eq!(
        without_keys(&json!(statuses(&edited)), &SESSION_DATA),
        without_keys(&json!(statuses(&reopened)), &SESSION_DATA)
    );
}

// Section 3.21, rule by rule.

/// `fillet.ptsm` with `record` inserted at timeline position `index`.
fn fillet_fixture_with(index: usize, record: Value) -> String {
    let mut document: Value =
        serde_json::from_str(&fixture("fillet.ptsm")).expect("fixtures are JSON");
    document["features"]
        .as_array_mut()
        .expect("documents list their features")
        .insert(index, record);
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// A 6 x 6 x 50 box `f-box2` that cuts the corner of 'Box 1' along its front left edge, the edge `fillet.ptsm` rounds.
fn corner_cut() -> Value {
    box_record(
        "f-box2",
        "b-1",
        ["6", "6", "50"],
        Some(["-10", "-15", "-5"]),
        "cut",
    )
}

/// `fillet.ptsm` with its box sized by the parameters `w` (20 mm), `d` (30 mm) and `height` (40 mm), in that order.
fn fillet_sized_by_parameters() -> String {
    let parameters = replace_once(
        &fixture("fillet.ptsm"),
        r#"{"id": "p-h", "name": "height", "expr": "40 mm"}"#,
        r#"{"id": "p-w", "name": "w", "expr": "20 mm"},
    {"id": "p-d", "name": "d", "expr": "30 mm"},
    {"id": "p-h", "name": "height", "expr": "40 mm"}"#,
    );
    replace_once(
        &parameters,
        r#""size": ["20 mm", "30 mm", "height"]"#,
        r#""size": ["w", "d", "height"]"#,
    )
}

/// `fillet.ptsm` after one change of the fillet's radius to `radius`.
fn fillet_with_radius(radius: &'static str) -> Twin {
    Twin::new(&fixture("fillet.ptsm"), "f-fil1", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[Change::Set("f-fil1", "/radius", json!(radius))],
        ))
    })
}

/// The cause of `fillet.ptsm`'s fillet made too large, 30 mm.
const RADIUS_CAUSE: &str = "This started after you changed Radius from 2 mm to 30 mm.";

#[test]
fn a_change_of_the_feature_itself_names_its_field_and_both_values() {
    let twin = fillet_with_radius("30 mm");

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: RADIUS_CAUSE,
            feature: Some("f-fil1"),
            actions: &[("Set Radius back to 2 mm", "apply"), ("Undo", "undo")],
        },
    );
}

#[test]
fn a_revert_of_the_feature_itself_gives_back_the_text_before_the_change() {
    let mut twin = fillet_with_radius("30 mm");
    let changed = twin.text();
    let found = cause(&named_message(&twin.edited, "f-fil1"));

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);

    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
    assert_undo_restores(
        &mut twin.session,
        twin.doc,
        &actions(&found)[0]["label"],
        &changed,
    );
}

#[test]
fn a_revert_carries_a_fix_ref_to_the_cause_at_the_current_revision() {
    let twin = Twin::of(&r12(), "f-fil1");
    let lost = named_message(&twin.edited, "f-fil1");

    let revert = &actions(&cause(&lost))[0];

    assert_eq!(
        revert["run"],
        json!({"kind": "apply", "fix": {
            "feature": "f-fil1", "code": lost["code"], "field": lost["field"],
            "cause": true, "index": 0, "revision": 1
        }})
    );
}

#[test]
fn an_expression_that_is_not_a_plain_value_follows_its_value_and_is_set_back_as_written() {
    let with_parameter = replace_once(
        &fixture("fillet.ptsm"),
        r#"{"id": "p-h", "name": "height", "expr": "40 mm"}"#,
        r#"{"id": "p-h", "name": "height", "expr": "40 mm"},
    {"id": "p-r", "name": "r", "expr": "1 mm"}"#,
    );
    let text = replace_once(
        &with_parameter,
        r#""radius": "2 mm""#,
        r#""radius": "r * 2""#,
    );

    let twin = Twin::new(&text, "f-fil1", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[Change::Set("f-fil1", "/radius", json!("r * 30"))],
        ))
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Radius from 2 mm (r * 2) to 30 mm (r * 30).",
            feature: Some("f-fil1"),
            actions: &[("Set Radius back to r * 2", "apply"), ("Undo", "undo")],
        },
    );
}

#[test]
fn a_parameter_used_upstream_is_named_when_no_feature_changed() {
    let mut twin = Twin::new(&fillet_sized_by_parameters(), "f-fil1", |_, _| {
        json!([set_parameter("p-w", "w", "1.5 mm")])
    });

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed parameter 'w' from 20 mm to 1.5 mm.",
            feature: None,
            actions: &[("Set 'w' back to 20 mm", "apply"), ("Undo", "undo")],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
}

#[test]
fn one_other_change_is_counted_after_the_primary_change() {
    let twin = Twin::new(&fillet_sized_by_parameters(), "f-fil1", |_, _| {
        json!([
            set_parameter("p-w", "w", "1.5 mm"),
            set_parameter("p-h", "height", "50 mm")
        ])
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed parameter 'w' from 20 mm to 1.5 mm, and 1 other change.",
            feature: None,
            actions: &[("Set 'w' back to 20 mm", "apply"), ("Undo", "undo")],
        },
    );
}

#[test]
fn several_other_changes_are_counted_after_the_primary_change() {
    let twin = Twin::new(&fillet_sized_by_parameters(), "f-fil1", |_, _| {
        json!([
            set_parameter("p-w", "w", "1.5 mm"),
            set_parameter("p-d", "d", "31 mm"),
            set_parameter("p-h", "height", "50 mm")
        ])
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed parameter 'w' from 20 mm to 1.5 mm, and 2 other changes.",
            feature: None,
            actions: &[("Set 'w' back to 20 mm", "apply"), ("Undo", "undo")],
        },
    );
}

#[test]
fn unsuppressing_a_feature_offers_to_suppress_it_again() {
    let setup = picked(&fillet_fixture_with(
        1,
        with(corner_cut(), "suppressed", json!(true)),
    ));
    let mut twin = Twin::new(&setup, "f-fil1", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[Change::Set("f-box2", "/suppressed", json!(false))],
        ))
    });

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you unsuppressed 'Box 2'.",
            feature: Some("f-box2"),
            actions: &[("Suppress 'Box 2'", "apply"), ("Undo", "undo")],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn moving_a_feature_above_this_one_offers_to_move_it_back_below_its_former_neighbor() {
    let setup = picked(&fillet_fixture_with(2, corner_cut()));
    let mut twin = Twin::new(&setup, "f-fil1", |session, doc| {
        Value::Array(edits(session, doc, &[Change::Move("f-box2", 1)]))
    });

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you moved 'Box 2' above 'Fillet 1'.",
            feature: Some("f-box2"),
            actions: &[
                ("Move 'Box 2' back below 'Fillet 1'", "apply"),
                ("Undo", "undo"),
            ],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn deleting_a_feature_whose_label_no_feature_has_now_names_it_plainly() {
    // The step of newcomer error NC05 with a fillet of 1 mm on the edge between the top and the riser, which builds.
    let setup = picked(&document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["40", "20", "10"], None, "new"),
            box_record(
                "f-box2",
                "b-1",
                ["20", "30", "5"],
                Some(["10", "0", "7.6"]),
                "cut",
            ),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/top|f-box2/left)",
                    "line|n=0,1,0|c=0,0,10|l=20|adj=2,2",
                )],
                "1",
            ),
        ],
    ));

    let twin = Twin::new(&setup, "f-fil1", |session, doc| {
        Value::Array(edits(session, doc, &[Change::Remove(vec!["f-box2"])]))
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you deleted 'Box 2'.",
            feature: Some("f-box2"),
            actions: &[("Undo", "undo")],
        },
    );
}

#[test]
fn a_units_change_that_reaches_the_feature_through_another_names_the_units() {
    // The box is 1 in wide, which the units change makes 1 mm, narrower than the fillet's 2 mm; the fillet has no
    // number without a unit of its own, so the cause offers no revert.
    let setup = picked(&document_with(
        "in",
        &[],
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["1", "30 mm", "40 mm"], None, "new"),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/front|f-box1/left)",
                    "line|n=0,0,1|c=-12.7,-15,20|l=40|adj=2,2",
                )],
                "2 mm",
            ),
        ],
    ));

    let twin = Twin::new(&setup, "f-fil1", |session, doc| {
        Value::Array(edits(session, doc, &[Change::Units("mm")]))
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed the document units from in to mm.",
            feature: None,
            actions: &[("Undo", "undo")],
        },
    );
}

#[test]
fn a_toggle_reads_on_or_off_and_a_revert_the_message_offers_is_left_out() {
    // Newcomer error NC03's cut, first reversed into the block, then pointed away from it again.
    let setup = picked(&document(
        &["b-1"],
        &block_then(vec![
            datum_offset("f-dp1", "XY", "40"),
            sketch(
                "f-sk2",
                datum_plane("f-dp1"),
                vec![circle("e-5", [40.0, 25.0], 5.0)],
            ),
            with(
                extrude(
                    "f-ext2",
                    "b-1",
                    "f-sk2",
                    vec![circle_region("e-5", [40.0, 25.0], 5.0)],
                    distance("15"),
                    "cut",
                ),
                "reverse",
                json!(true),
            ),
        ]),
    ));

    let twin = Twin::new(&setup, "f-ext2", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[Change::Set("f-ext2", "/reverse", json!(false))],
        ))
    });

    let missed = named_message(&twin.edited, "f-ext2");
    assert!(
        labels(&missed).contains(&"Turn on Reverse".to_owned()),
        "{missed}"
    );
    assert_cause(
        &missed,
        &Expected {
            text: "This started after you changed Reverse from on to off.",
            feature: Some("f-ext2"),
            actions: &[("Undo", "undo")],
        },
    );
}

#[test]
fn a_field_without_a_short_value_is_named_alone_and_restored_by_its_label() {
    let mut twin = Twin::new(&fixture("fillet.ptsm"), "f-fil1", |session, doc| {
        // Named by nothing of the box and far from its edges, so that no candidate of the message
        // restores the edge the setup picked (section 3.21 leaves out a cause action the message
        // offers).
        let gone = reference(
            "edge(f-gone/x|f-gone/y)",
            "line|n=0,0,1|c=500,500,500|l=1|adj=2,2",
        );
        Value::Array(edits(
            session,
            doc,
            &[Change::Set("f-fil1", "/edges", json!([gone]))],
        ))
    });

    let found = assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Edges.",
            feature: Some("f-fil1"),
            actions: &[("Restore Edges of 'Fillet 1'", "apply"), ("Undo", "undo")],
        },
    );

    twin.assert_revert_gives_the_clean_result("f-fil1", &found);
    let setup = twin.setup.clone();
    assert_eq!(twin.text(), setup);
}

#[test]
fn the_cause_is_dropped_when_the_feature_builds_without_the_message() {
    let mut twin = fillet_with_radius("30 mm");
    let fits = edits(
        &mut twin.session,
        twin.doc,
        &[Change::Set("f-fil1", "/radius", json!("3 mm"))],
    );
    let built = apply(
        &mut twin.session,
        twin.doc,
        Some("Radius 3"),
        Value::Array(fits),
    );
    assert_eq!(messages(&built, "f-fil1"), Vec::<Value>::new());
    let too_large = edits(
        &mut twin.session,
        twin.doc,
        &[Change::Set("f-fil1", "/radius", json!("25 mm"))],
    );

    let again = apply(
        &mut twin.session,
        twin.doc,
        Some("Radius 25"),
        Value::Array(too_large),
    );

    // The clean snapshot is the build at 3 mm, not the one before the first problem.
    assert_cause(
        &named_message(&again, "f-fil1"),
        &Expected {
            text: "This started after you changed Radius from 3 mm to 25 mm.",
            feature: Some("f-fil1"),
            actions: &[("Set Radius back to 3 mm", "apply"), ("Undo", "undo")],
        },
    );
}

#[test]
fn undo_is_not_offered_while_a_transaction_is_open() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &fixture("fillet.ptsm"));
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Radius"}),
    )
    .ok();
    let made = edits(
        &mut session,
        doc,
        &[Change::Set("f-fil1", "/radius", json!("30 mm"))],
    );

    let inside = apply(&mut session, doc, None, Value::Array(made));

    assert_cause(
        &named_message(&inside, "f-fil1"),
        &Expected {
            text: RADIUS_CAUSE,
            feature: Some("f-fil1"),
            actions: &[("Set Radius back to 2 mm", "apply")],
        },
    );
}

#[test]
fn redo_makes_the_cause_again_from_the_state_the_undo_left() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &r12());
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(messages(&undone, "f-fil1"), Vec::<Value>::new());

    let redone = send(&mut session, json!({"type": "redo", "doc": doc}));

    assert_cause(
        &named_message(&redone, "f-fil1"),
        &Expected {
            text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
            feature: Some("f-ext2"),
            actions: &[
                ("Set Distance of 'Extrude 2' back to 5 mm", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

#[test]
fn a_rolled_back_feature_keeps_its_cause() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &r12());
    let rolled_back = apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-fil1"))]),
    );
    assert_eq!(
        rolled_back.feature_status("f-fil1")["status"],
        "rolled_back"
    );

    let again = apply(
        &mut session,
        doc,
        Some("Roll forward"),
        json!([set_rollback(None)]),
    );

    // The text stays while the message lasts; the newest undo entry is the rollback, so no Undo.
    assert_cause(
        &named_message(&again, "f-fil1"),
        &Expected {
            text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
            feature: Some("f-ext2"),
            actions: &[("Set Distance of 'Extrude 2' back to 5 mm", "apply")],
        },
    );
}

#[test]
fn an_added_sketch_entity_is_shown_and_has_no_ghost() {
    let twin = Twin::of(&r50(), "f-ext1");

    // A notice gets a cause like any message; the added circle is on screen already.
    let found = assert_cause(
        &message(&twin.edited, "f-ext1", "region_split"),
        &Expected {
            text: "This started after you added the circle at (40, 25) in 'Sketch 1'.",
            feature: Some("f-sk1"),
            actions: &[("Undo", "undo"), ("Show the circle", "show")],
        },
    );
    assert_eq!(
        actions(&found)[1]["run"],
        json!({"kind": "show", "feature": "f-sk1", "entity": "e-5"})
    );
    assert!(found.get("ghost").is_none(), "{found}");
}

#[test]
fn a_moved_sketch_entity_has_its_ghost_where_it_was() {
    let twin = Twin::of(&r10a(), "f-fil1");

    let found = cause(&named_message(&twin.edited, "f-fil1"));

    let ghost = &found["ghost"];
    assert_eq!(ghost["frame"], "sketch", "{found}");
    assert_ghost_segments(&twin.edited, ghost, &[(&[30.0, -1.0], &[50.0, -1.0])]);
}

#[test]
fn a_feature_dragged_past_others_is_the_one_named_and_its_revert_the_message_offers_is_left_out() {
    let twin = Twin::of(&r28(), "f-fil1");
    let lost = message(&twin.edited, "f-fil1", "references_not_found");
    assert!(
        labels(&lost).contains(&"Move 'Fillet 1' below 'Extrude 2'".to_owned()),
        "{lost}"
    );

    // The datum, the sketch and the extrude it passed did not move; its revert is the message's Move.
    assert_cause(
        &lost,
        &Expected {
            text: "This started after you moved 'Fillet 1' above 'Extrude 2'.",
            feature: Some("f-fil1"),
            actions: &[("Undo", "undo")],
        },
    );
}

#[test]
fn an_option_field_the_record_leaves_out_reads_as_its_first_option() {
    // Corpus R41's pattern leaves Operation out, which is Join; it is set to New body into an empty 'Body 2'.
    let mut document: Value = serde_json::from_str(&r41().setup).expect("setups are JSON");
    document["bodies"]
        .as_array_mut()
        .expect("documents list their bodies")
        .push(json!({"id": "b-2", "name": "Body 2"}));
    let twin = Twin::new(&document.to_string(), "f-fil1", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[
                Change::Set("f-lp1", "/op", json!("new")),
                Change::Set("f-lp1", "/target", json!("b-2")),
            ],
        ))
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Operation of 'Linear pattern 1' from Join to New body.",
            feature: Some("f-lp1"),
            actions: &[
                ("Restore the settings of 'Linear pattern 1'", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

#[test]
fn another_change_of_the_feature_of_the_primary_change_is_counted() {
    // 'Box 2' moved above the fillet and made taller in one edit: its record change is named, its move counted.
    let setup = picked(&fillet_fixture_with(2, corner_cut()));
    let twin = Twin::new(&setup, "f-fil1", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[
                Change::Move("f-box2", 1),
                Change::Set("f-box2", "/size/2", json!("60")),
            ],
        ))
    });

    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed Height of 'Box 2' from 50 mm to 60 mm, and 1 other change.",
            feature: Some("f-box2"),
            actions: &[
                ("Set Height of 'Box 2' back to 50 mm", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

#[test]
fn making_an_outline_line_construction_geometry_is_named_and_shown() {
    // Newcomer error NC11 with its back line made construction geometry instead of deleted.
    let outline = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        with(
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            "construction",
            json!(true),
        ),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let scenario = block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "2")],
        outline,
    );
    let twin = Twin::of(&scenario, "f-ext1");

    // The line is still there, so the cause has no ghost.
    let found = assert_cause(
        &message(&twin.edited, "f-ext1", "profile_not_found"),
        &Expected {
            text: "This started after you made the line from (80, 50) to (0, 50) in 'Sketch 1' construction geometry.",
            feature: Some("f-sk1"),
            actions: &[("Undo", "undo"), ("Show the line", "show")],
        },
    );
    assert_eq!(
        actions(&found)[1]["run"],
        json!({"kind": "show", "feature": "f-sk1", "entity": "e-3"})
    );
    assert!(found.get("ghost").is_none(), "{found}");
}

#[test]
fn making_a_reference_dimension_driving_is_named_by_its_title() {
    // Newcomer error NC07 with Distance 30 mm already on the line as a reference, then made driving.
    let setup = picked(&document(
        &["b-1"],
        &[
            constrained_sketch(
                "f-sk1",
                origin_plane("XY"),
                rectangle(1, 0.0, 0.0, 40.0, 20.0),
                vec![
                    line_distance("c-1", "e-1", "40 mm"),
                    with(
                        line_distance("c-2", "e-1", "30 mm"),
                        "driving",
                        json!(false),
                    ),
                ],
            ),
            extrude(
                "f-ext1",
                "b-1",
                "f-sk1",
                vec![rectangle_region(1, 0.0, 0.0, 40.0, 20.0)],
                distance("10"),
                "new",
            ),
        ],
    ));
    let twin = Twin::new(&setup, "f-sk1", |session, doc| {
        Value::Array(edits(
            session,
            doc,
            &[Change::Set("f-sk1", "/constraints/1/driving", json!(true))],
        ))
    });

    assert_cause(
        &message(&twin.edited, "f-sk1", "sketch_conflict"),
        &Expected {
            text: "This started after you made Distance 30 mm in 'Sketch 1' a driving dimension.",
            feature: Some("f-sk1"),
            actions: &[("Undo", "undo")],
        },
    );
}

#[test]
fn a_parameter_without_a_unit_makes_the_units_change_reach_the_feature() {
    // Every length of the box has a unit or is zero; the radius is the parameter `r` = 2, which inches make 2 in, too
    // large.
    let with_parameter = replace_once(
        &fixture("fillet.ptsm"),
        r#"{"id": "p-h", "name": "height", "expr": "40 mm"}"#,
        r#"{"id": "p-h", "name": "height", "expr": "40 mm"},
    {"id": "p-r", "name": "r", "expr": "2"}"#,
    );
    let text = replace_once(&with_parameter, r#""radius": "2 mm""#, r#""radius": "r""#);

    let twin = Twin::new(&text, "f-fil1", |session, doc| {
        Value::Array(edits(session, doc, &[Change::Units("in")]))
    });

    // The fillet's own Radius is no number without a unit, so the units change has no revert here.
    assert_cause(
        &named_message(&twin.edited, "f-fil1"),
        &Expected {
            text: "This started after you changed the document units from mm to in.",
            feature: None,
            actions: &[("Undo", "undo")],
        },
    );
}

/// Begins a transaction labeled `Edit` on `doc`, focused on `focus` when one is given.
fn begin_edit(session: &mut Session, doc: u64, focus: Option<&str>) {
    send(
        session,
        json!({"type": "begin_transaction", "doc": doc, "label": EDIT_LABEL}),
    )
    .ok();
    if let Some(focus) = focus {
        send(
            session,
            json!({"type": "set_focus", "doc": doc, "feature": focus}),
        )
        .ok();
    }
}

/// Commits the open transaction of `doc`; the reply.
fn commit(session: &mut Session, doc: u64) -> Sent {
    let committed = send(session, json!({"type": "commit_transaction", "doc": doc}));
    committed.ok();
    committed
}

#[test]
fn an_edit_made_in_a_transaction_focused_on_its_feature_offers_undo_once_committed() {
    // Corpus R41 the way the inspector edits the pattern: the fillet is rolled back until the commit.
    let scenario = r41();
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &scenario.setup);
    begin_edit(&mut session, doc, Some("f-lp1"));
    let made = edits(&mut session, doc, &scenario.changes);
    apply(&mut session, doc, None, Value::Array(made)).ok();

    let committed = commit(&mut session, doc);

    assert_cause(
        &message(&committed, "f-fil1", "reference_missing"),
        &Expected {
            text: R41_FIRST_CAUSE,
            feature: Some("f-lp1"),
            actions: &[
                ("Set Count of 'Linear pattern 1' back to 3", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

#[test]
fn committing_a_transaction_without_a_focus_sends_its_causes_again_with_undo() {
    let scenario = r12();
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &scenario.setup);
    begin_edit(&mut session, doc, None);
    let made = edits(&mut session, doc, &scenario.changes);
    let inside = apply(&mut session, doc, None, Value::Array(made));
    assert_eq!(
        labels_and_kinds(&cause(&named_message(&inside, "f-fil1"))),
        [(
            "Set Distance of 'Extrude 2' back to 5 mm".to_owned(),
            "apply".to_owned()
        )]
    );

    let committed = commit(&mut session, doc);

    assert_eq!(committed.event_types(), ["document_changed", "recomputed"]);
    assert_cause(
        &named_message(&committed, "f-fil1"),
        &Expected {
            text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
            feature: Some("f-ext2"),
            actions: &[
                ("Set Distance of 'Extrude 2' back to 5 mm", "apply"),
                ("Undo", "undo"),
            ],
        },
    );
}

#[test]
fn committing_a_transaction_that_made_no_cause_sends_no_statuses() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &fixture("fillet.ptsm"));
    begin_edit(&mut session, doc, None);
    apply(&mut session, doc, None, json!([set_height("45 mm")])).ok();

    let committed = commit(&mut session, doc);

    assert_eq!(committed.event_types(), ["document_changed"]);
}

#[test]
fn undo_is_not_offered_when_another_command_changed_the_recipe_after_the_clean_snapshot() {
    // Corpus R12's edit made while the fillet is rolled back: the newest entry moves the marker back, and undoing it
    // would roll the fillet back and leave the distance at 30 mm.
    let scenario = r12();
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &scenario.setup);
    apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-fil1"))]),
    )
    .ok();
    let made = edits(&mut session, doc, &scenario.changes);
    apply(&mut session, doc, Some(EDIT_LABEL), Value::Array(made)).ok();

    let rolled_forward = apply(
        &mut session,
        doc,
        Some("Roll forward"),
        json!([set_rollback(None)]),
    );

    assert_cause(
        &named_message(&rolled_forward, "f-fil1"),
        &Expected {
            text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
            feature: Some("f-ext2"),
            actions: &[("Set Distance of 'Extrude 2' back to 5 mm", "apply")],
        },
    );
}

// Cause actions that no longer apply (section 3.22, `stale_fix`).

/// Asserts that `sent` failed with `stale_fix`, emitted nothing, and left the revision at `at` and the text at `text`.
fn assert_stale(session: &mut Session, doc: u64, sent: &Sent, at: u64, text: &str) {
    assert_eq!(sent.error_code(), "stale_fix");
    assert!(sent.events.is_empty(), "{:?}", sent.event_types());
    assert_eq!(revision(session, doc), at);
    assert_eq!(saved_text(session, doc), text);
}

#[test]
fn a_cause_action_offered_before_another_edit_is_stale() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r12());
    let revert = actions(&cause(&named_message(&edited, "f-fil1")))[0].clone();
    apply(
        &mut session,
        doc,
        Some("Add parameter"),
        json!([{"type": "add_parameter", "index": 0, "parameter": parameter("p-x", "x", "1 mm")}]),
    )
    .ok();
    let text = saved_text(&mut session, doc);

    let sent = apply_fix(&mut session, doc, &revert);

    assert_stale(&mut session, doc, &sent, 2, &text);
}

#[test]
fn the_undo_of_a_cause_and_an_index_past_its_actions_are_stale() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r12());
    let lost = named_message(&edited, "f-fil1");
    let text = saved_text(&mut session, doc);

    for index in [1, 2] {
        let sent = send_fix(
            &mut session,
            doc,
            json!({
                "feature": "f-fil1", "code": lost["code"], "field": lost["field"],
                "cause": true, "index": index, "revision": 1
            }),
        );

        assert_stale(&mut session, doc, &sent, 1, &text);
    }
}

#[test]
fn a_cause_action_of_a_message_without_a_cause_is_stale() {
    let mut session = session();
    let opened = open_text(&mut session, &edited_text(&r12()));
    let doc = doc_handle(&opened);
    let lost = named_message(&opened, "f-fil1");
    let text = saved_text(&mut session, doc);

    let sent = send_fix(
        &mut session,
        doc,
        json!({
            "feature": "f-fil1", "code": lost["code"], "field": lost["field"],
            "cause": true, "index": 0, "revision": 0
        }),
    );

    assert_stale(&mut session, doc, &sent, 0, &text);
}
