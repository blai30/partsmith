//! Acceptance tests 126 to 134 of `docs/spec/engine.md` section 6, as far as phase 1 of the reference stability plan
//! builds them: the labels and kinds of fix actions, `apply_fix` of each kind of fix phase 1 offers with its undo
//! label, re-anchoring for `KeepMatch` and `KeepRegions`, timeline fixes, fixes inside a transaction, `stale_fix`, and
//! actions that hold no session data (section 3.22). A test named `tNNN` is section 6 test NNN; a test with several
//! documents is one test per document.
//!
//! Scenarios come from `corpus`: each setup is opened with its references picked, and "after its edit" is the corpus
//! edit applied as one `apply_edits` labeled `Edit`, so the actions it offers carry revision 1.
//!
//! Left for the phases that make the fixes they need: the actions of NC05 (`Use 2.3 mm`, and `Replace r * 2 with` of a
//! too large radius, test 126), NC03 (`Turn on Reverse`), NC10 (`Replace hieght with height`), CUBE10's linear pattern
//! without a target (`Set Operation to Join`), NC04 (`Put it in a new body`, `Attach`), NC07 (`Delete`, `Make ... a
//! reference`) and NC01 (`Connect the ends`) in tests 126, 127 and 131, which phase 3 adds; corpus R26 and R32 (b)
//! (`Attach to`, `Turn the drawing back`) in tests 126 and 135, which phase 2 adds; and test 132 with NC05's fix, which
//! `t132_a_fix_inside_a_transaction_joins_it` checks with a fix phase 1 offers.

mod common;
mod corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, apply, assert_relative, doc_handle, get_document, measure_body,
    number, open_text, revision, saved_text, send, session,
};
use corpus::{
    EDIT_LABEL, action, actions, after_edit, apply_fix, box_record, cube10, datum_offset, document,
    document_with, edited_text, faces_of, features, fix_ref, labels, linear_pattern, message,
    messages, nc11, parameter, picked_before, r03, r06a, r16, r23a, r28, r50, r56b, r66, record,
    redrawn_beside_a_split, repairs, send_fix, statuses, two_areas, volume, without_keys,
};
use partsmith_engine::Session;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The verbs a fix label may start with (the error guidelines, "One-click fixes").
const VERBS: [&str; 21] = [
    "Use",
    "Replace",
    "Turn",
    "Set",
    "Clear",
    "Keep",
    "Remove",
    "Put",
    "Attach",
    "Connect",
    "Delete",
    "Make",
    "Rename",
    "Unsuppress",
    "Suppress",
    "Move",
    "Pick",
    "Show",
    "Restore",
    "Undo",
    "Finish",
];

/// The run kind each label's verb projects to: `pick` for Pick, `show` for Show, `apply` for every fix that changes
/// the document.
fn kind_of(label: &str) -> &'static str {
    match label.split(' ').next() {
        Some("Pick") => "pick",
        Some("Show") => "show",
        _ => "apply",
    }
}

/// Applies an action with `apply_fix` outside a transaction and checks that it is one recipe change recorded as one
/// undo entry labeled with the action's label, emitted like `apply_edits`.
fn apply_as_one_entry(session: &mut Session, doc: u64, action: &Value) -> Sent {
    let before = revision(session, doc);

    let applied = apply_fix(session, doc, action);

    assert_eq!(applied.ok()["revision"], before + 1, "{action}");
    assert_eq!(
        applied.event_types()[..2],
        ["document_changed", "recomputed"],
        "{:?}",
        applied.event_types()
    );
    let changed = applied.event("document_changed");
    assert_eq!(changed["revision"], before + 1);
    assert_eq!(changed["undo"], action["label"]);
    applied
}

/// Undoes the newest entry, checks its label, and checks that the document's text is `before` again.
fn assert_undo_restores(session: &mut Session, doc: u64, label: &Value, before: &str) {
    let undone = send(session, json!({"type": "undo", "doc": doc}));
    assert_eq!(&undone.ok()["label"], label);
    assert_eq!(saved_text(session, doc), before);
}

/// The timeline of the document, as feature ids.
fn timeline(session: &mut Session, doc: u64) -> Vec<String> {
    features(&get_document(session, doc)["document"])
        .iter()
        .map(|record| record["id"].as_str().expect("records have ids").to_owned())
        .collect()
}

/// The region references of the solution of `sketch` in `sent` whose keys are in `keys`, in the sketch's region order.
fn solved_regions(sent: &Sent, sketch: &str, keys: &[&str]) -> Vec<Value> {
    sent.sketch_solution(sketch)["regions"]
        .as_array()
        .expect("solutions list their regions")
        .iter()
        .map(|region| region["reference"].clone())
        .filter(|reference| {
            keys.iter()
                .any(|key| reference["key"].as_str() == Some(key))
        })
        .collect()
}

/// CUBE10 with a linear pattern along X whose count is `count`, with these parameters.
fn patterned_cube(parameters: &[Value], count: &str) -> String {
    document_with(
        "mm",
        parameters,
        &["b-1"],
        &[cube10(), linear_pattern(count, "20")],
    )
}

/// A box that joins an empty body (features.md section 6.5, empty bodies test 2).
fn box_joining_nothing() -> String {
    document(
        &["b-1"],
        &[box_record(
            "f-box1",
            "b-1",
            ["10", "10", "10"],
            None,
            "join",
        )],
    )
}

// Test 126: labels and kinds.

#[test]
fn t126_r66_a_widened_reference_offers_keep_all_use_only_and_pick() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r66());

    let warning = message(&edited, "f-fil1", "reference_widened");
    assert_eq!(
        labels(&warning),
        [
            "Keep all 5",
            "Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'",
            "Pick an edge again"
        ]
    );
}

#[test]
fn t126_r06a_a_followed_reference_offers_keep_this_match_and_pick() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r06a());

    let notice = message(&edited, "f-fil1", "reference_followed");
    assert_eq!(labels(&notice), ["Keep this match", "Pick an edge again"]);
}

#[test]
fn t126_r03_a_skipped_edge_offers_remove_then_pick_last() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r03());

    let warning = message(&edited, "f-fil1", "reference_missing");
    let offered = labels(&warning);
    assert!(
        offered.ends_with(&[
            "Remove from Edges".to_owned(),
            "Pick an edge again".to_owned()
        ]),
        "{offered:?}"
    );
}

#[test]
fn t126_r50_a_split_region_offers_keep_these_regions_and_pick() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r50());

    let notice = message(&edited, "f-ext1", "region_split");
    assert_eq!(
        labels(&notice),
        ["Keep these regions", "Pick a region again"]
    );
}

#[test]
fn t126_r56b_a_body_a_boolean_empties_offers_the_other_body() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r56b());

    let error = message(&edited, "f-fil1", "body_empty");
    assert_eq!(labels(&error), ["Use body 'Body 2'"]);
}

#[test]
fn t126_r23a_a_suppressed_maker_offers_unsuppress_first() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r23a());

    let error = &messages(&edited, "f-fil1")[0];
    assert_eq!(labels(error)[0], "Unsuppress 'Extrude 1'");
}

#[test]
fn t126_r28_a_maker_that_comes_later_offers_the_move_first() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r28());

    let error = message(&edited, "f-fil1", "references_not_found");
    let offered = labels(&error);
    assert_eq!(offered[0], "Move 'Fillet 1' below 'Extrude 2'");
    assert_eq!(
        offered.last().map(String::as_str),
        Some("Pick an edge again")
    );
}

#[test]
fn t126_nc11_one_root_cause_offers_show_and_never_pick_downstream() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc11());

    let lost = message(&edited, "f-ext1", "profile_not_found");
    assert_eq!(labels(&lost), ["Show 'Sketch 1'", "Pick a region again"]);
    let empty = message(&edited, "f-fil1", "body_empty");
    assert_eq!(labels(&empty), ["Show 'Extrude 1'"]);
}

#[test]
fn t126_a_count_that_is_not_whole_offers_the_whole_numbers_around_it() {
    let mut session = session();

    let opened = open_text(&mut session, &patterned_cube(&[], "2.5"));

    let error = message(&opened, "f-lp1", "not_whole_number");
    assert_eq!(labels(&error), ["Use 2", "Use 3"]);
}

#[test]
fn t126_a_field_that_holds_a_formula_is_replaced() {
    let mut session = session();

    let opened = open_text(
        &mut session,
        &patterned_cube(&[parameter("p-n", "n", "5")], "n / 2"),
    );

    let error = message(&opened, "f-lp1", "not_whole_number");
    assert_eq!(
        labels(&error),
        ["Replace n / 2 with 2", "Replace n / 2 with 3"]
    );
}

#[test]
fn t126_an_option_field_is_set_by_its_option_label() {
    let mut session = session();

    let opened = open_text(&mut session, &box_joining_nothing());

    let error = message(&opened, "f-box1", "body_empty");
    assert_eq!(labels(&error), ["Set Operation to New body"]);
}

#[test]
fn t126_every_label_starts_with_a_verb_and_runs_as_its_kind() {
    let mut session = session();
    let mut all = Vec::new();
    for scenario in [r66(), r06a(), r03(), r50(), r56b(), r23a(), r28(), nc11()] {
        let (_, edited) = after_edit(&mut session, &scenario);
        all.extend(statuses(&edited));
    }
    for text in [
        patterned_cube(&[], "2.5"),
        patterned_cube(&[parameter("p-n", "n", "5")], "n / 2"),
        box_joining_nothing(),
    ] {
        all.extend(statuses(&open_text(&mut session, &text)));
    }

    let offered: Vec<Value> = all
        .iter()
        .flat_map(|status| status["messages"].as_array().cloned().unwrap_or_default())
        .flat_map(|message| actions(&message))
        .collect();
    assert!(offered.len() >= 20, "{offered:?}");
    for offer in &offered {
        let label = offer["label"].as_str().expect("actions have labels");
        let verb = label.split(' ').next().unwrap_or_default();
        assert!(VERBS.contains(&verb), "{label:?} starts with a verb");
        assert_eq!(offer["run"]["kind"], kind_of(label), "{offer}");
    }
}

// Test 127: applying fixes.

#[test]
fn t127_r16_using_a_candidate_stores_its_reference() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r16());
    let before = saved_text(&mut session, doc);
    let error = message(&edited, "f-fil1", "references_not_found");
    let first = actions(&error)[0].clone();
    let candidate = repairs(&edited, "f-fil1")[0]["candidates"][0].clone();

    let applied = apply_as_one_entry(&mut session, doc, &first);

    assert_eq!(
        record(&mut session, doc, "f-fil1")["edges"],
        json!([candidate["reference"]])
    );
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
    assert_undo_restores(&mut session, doc, &first["label"], &before);
}

#[test]
fn t127_r03_remove_from_edges_keeps_the_other_edge() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r03());
    let before = saved_text(&mut session, doc);
    let warning = message(&edited, "f-fil1", "reference_missing");
    let remove = action(&warning, "Remove from Edges");

    let applied = apply_as_one_entry(&mut session, doc, &remove);

    let edges = record(&mut session, doc, "f-fil1")["edges"].clone();
    assert_eq!(edges.as_array().map(Vec::len), Some(1), "{edges}");
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        119931.327412,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &remove["label"], &before);
}

#[test]
fn t127_using_a_region_stores_the_region_it_names() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &two_areas());
    let before = saved_text(&mut session, doc);
    let expected = solved_regions(&edited, "f-sk1", &["e-1,e-3,e-4,e-9"]);
    let use_region = action(
        &message(&edited, "f-ext1", "profile_not_found"),
        "Use the region of 'Sketch 1' around (40, 25)",
    );

    let applied = apply_as_one_entry(&mut session, doc, &use_region);

    assert_eq!(
        record(&mut session, doc, "f-ext1")["profile"]["regions"],
        json!(expected)
    );
    assert_eq!(applied.feature_status("f-ext1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        120000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &use_region["label"], &before);
}

#[test]
fn t127_r56b_use_body_resolves_the_edge_in_the_other_body() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r56b());
    let before = saved_text(&mut session, doc);
    let use_body = action(
        &message(&edited, "f-fil1", "body_empty"),
        "Use body 'Body 2'",
    );

    let applied = apply_as_one_entry(&mut session, doc, &use_body);

    assert_eq!(record(&mut session, doc, "f-fil1")["body"], "b-2");
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
    assert_eq!(messages(&applied, "f-fil1"), Vec::<Value>::new());
    assert_relative(
        volume(&mut session, doc, "b-2"),
        8997.853982,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &use_body["label"], &before);
}

#[test]
fn t127_use_3_sets_a_whole_count() {
    let mut session = session();
    let opened = open_text(&mut session, &patterned_cube(&[], "2.5"));
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    let use_three = action(&message(&opened, "f-lp1", "not_whole_number"), "Use 3");

    let applied = apply_as_one_entry(&mut session, doc, &use_three);

    assert_eq!(
        record(&mut session, doc, "f-lp1")["direction"]["count"],
        "3"
    );
    assert_eq!(applied.feature_status("f-lp1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        3000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &use_three["label"], &before);
}

#[test]
fn t127_replacing_a_formula_writes_the_value() {
    let text = patterned_cube(&[parameter("p-n", "n", "5")], "n / 2");
    let mut session = session();
    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    let replace = action(
        &message(&opened, "f-lp1", "not_whole_number"),
        "Replace n / 2 with 3",
    );

    let applied = apply_as_one_entry(&mut session, doc, &replace);

    assert_eq!(
        record(&mut session, doc, "f-lp1")["direction"]["count"],
        "3"
    );
    assert_eq!(applied.feature_status("f-lp1")["status"], "ok");
    assert_undo_restores(&mut session, doc, &replace["label"], &before);
}

#[test]
fn t127_set_operation_to_new_body_starts_the_body() {
    let mut session = session();
    let opened = open_text(&mut session, &box_joining_nothing());
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    let set_new = action(
        &message(&opened, "f-box1", "body_empty"),
        "Set Operation to New body",
    );

    let applied = apply_as_one_entry(&mut session, doc, &set_new);

    assert_eq!(record(&mut session, doc, "f-box1")["op"], "new");
    assert_eq!(applied.feature_status("f-box1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        1000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &set_new["label"], &before);
}

// Test 128: re-anchoring.

#[test]
fn t128_r06a_keep_this_match_stores_the_matched_edge() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r06a());
    let before = saved_text(&mut session, doc);
    let expected = picked_before(
        &before,
        "f-fil1",
        "b-1",
        &["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"],
    );
    let keep = action(
        &message(&edited, "f-fil1", "reference_followed"),
        "Keep this match",
    );

    let applied = apply_as_one_entry(&mut session, doc, &keep);

    assert_eq!(
        record(&mut session, doc, "f-fil1")["edges"],
        json!(expected)
    );
    assert_eq!(messages(&applied, "f-fil1"), Vec::<Value>::new());
    assert_undo_restores(&mut session, doc, &keep["label"], &before);
}

#[test]
fn t128_r66_keep_all_stores_every_edge_in_name_order() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r66());
    let before = saved_text(&mut session, doc);
    let expected = picked_before(
        &before,
        "f-fil1",
        "b-1",
        &[
            "edge(f-ext1/end-cap|f-ext1/lateral/e-10)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-11)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-12)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-13)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
        ],
    );
    let keep_all = action(
        &message(&edited, "f-fil1", "reference_widened"),
        "Keep all 5",
    );

    let applied = apply_as_one_entry(&mut session, doc, &keep_all);

    assert_eq!(
        record(&mut session, doc, "f-fil1")["edges"],
        json!(expected)
    );
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
    assert_eq!(messages(&applied, "f-fil1"), Vec::<Value>::new());
    assert_eq!(faces_of(&mut session, doc, "b-1", "f-fil1"), 5);
    assert_undo_restores(&mut session, doc, &keep_all["label"], &before);
}

#[test]
fn t128_r66_use_only_stores_the_first_edge() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r66());
    let before = saved_text(&mut session, doc);
    let expected = picked_before(
        &before,
        "f-fil1",
        "b-1",
        &["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"],
    );
    let use_only = action(
        &message(&edited, "f-fil1", "reference_widened"),
        "Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'",
    );

    let applied = apply_as_one_entry(&mut session, doc, &use_only);

    assert_eq!(
        record(&mut session, doc, "f-fil1")["edges"],
        json!(expected)
    );
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
    assert_eq!(faces_of(&mut session, doc, "b-1", "f-fil1"), 1);
    assert_undo_restores(&mut session, doc, &use_only["label"], &before);
}

// Test 129: keeping regions.

#[test]
fn t129_r50_keep_these_regions_stores_the_ring_and_the_disc() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r50());
    let before = saved_text(&mut session, doc);
    let expected = solved_regions(&edited, "f-sk1", &["e-1,e-2,e-3,e-4;~e-5", "e-5"]);
    assert_eq!(expected.len(), 2, "the ring and the disc: {expected:?}");
    let keep = action(
        &message(&edited, "f-ext1", "region_split"),
        "Keep these regions",
    );

    let applied = apply_as_one_entry(&mut session, doc, &keep);

    assert_eq!(
        record(&mut session, doc, "f-ext1")["profile"]["regions"],
        json!(expected)
    );
    assert_eq!(messages(&applied, "f-ext1"), Vec::<Value>::new());
    assert_relative(
        volume(&mut session, doc, "b-1"),
        120000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &keep["label"], &before);
}

#[test]
fn t129_keep_these_regions_rewrites_the_references_that_followed_a_replaced_line() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &redrawn_beside_a_split());
    let before = saved_text(&mut session, doc);
    let split = message(&edited, "f-ext1", "region_split");
    assert_eq!(split["field"], "/profile/regions/1");
    assert_eq!(
        message(&edited, "f-fil1", "reference_followed")["field"],
        "/edges/0"
    );
    let every_region = solved_regions(
        &edited,
        "f-sk1",
        &["e-1,e-3,e-4,e-9", "e-5,e-6,e-7,e-8;~e-10", "e-10"],
    );
    assert_eq!(every_region.len(), 3, "{every_region:?}");
    let followed_edge = picked_before(
        &before,
        "f-fil1",
        "b-1",
        &["edge(f-ext1/lateral/e-1|f-ext1/lateral/e-9)"],
    );
    let measured_before = measure_body(&mut session, doc, "b-1");

    let applied = apply_as_one_entry(&mut session, doc, &action(&split, "Keep these regions"));

    assert_eq!(
        record(&mut session, doc, "f-ext1")["profile"]["regions"],
        json!(every_region)
    );
    assert_eq!(
        record(&mut session, doc, "f-fil1")["edges"],
        json!(followed_edge)
    );
    for feature in ["f-ext1", "f-fil1"] {
        assert_eq!(applied.feature_status(feature)["status"], "ok");
        assert_eq!(messages(&applied, feature), Vec::<Value>::new());
    }
    let measured_after = measure_body(&mut session, doc, "b-1");
    for field in ["volume", "area"] {
        assert_relative(
            number(&measured_after[field]),
            number(&measured_before[field]),
            GEOMETRY_TOLERANCE,
            field,
        );
    }
    assert_eq!(measured_after["face_count"], measured_before["face_count"]);
    assert_undo_restores(&mut session, doc, &json!("Keep these regions"), &before);
}

// Test 130: timeline fixes.

#[test]
fn t130_r23a_unsuppress_gives_back_the_text_before_the_suppress() {
    let scenario = r23a();
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &scenario);
    let unsuppress = actions(&messages(&edited, "f-fil1")[0])[0].clone();
    assert_eq!(unsuppress["label"], "Unsuppress 'Extrude 1'");

    let applied = apply_as_one_entry(&mut session, doc, &unsuppress);

    assert_eq!(saved_text(&mut session, doc), scenario.setup);
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
}

#[test]
fn t130_r28_move_below_gives_back_the_original_text() {
    let scenario = r28();
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &scenario);
    let move_below = action(
        &message(&edited, "f-fil1", "references_not_found"),
        "Move 'Fillet 1' below 'Extrude 2'",
    );

    let applied = apply_as_one_entry(&mut session, doc, &move_below);

    assert_eq!(saved_text(&mut session, doc), scenario.setup);
    assert_eq!(applied.feature_status("f-fil1")["status"], "ok");
}

#[test]
fn t130_r28_move_below_lands_below_its_neighbor_after_an_insertion_above() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &r28());
    let inserted = apply(
        &mut session,
        doc,
        Some("Add datum"),
        json!([{"type": "add_feature", "index": 5, "feature": datum_offset("f-dp2", "XY", "50")}]),
    );
    assert_eq!(
        timeline(&mut session, doc),
        [
            "f-sk1", "f-ext1", "f-fil1", "f-dp1", "f-sk2", "f-dp2", "f-ext2"
        ]
    );
    let move_below = action(
        &message(&inserted, "f-fil1", "references_not_found"),
        "Move 'Fillet 1' below 'Extrude 2'",
    );
    assert_eq!(move_below["run"]["fix"]["revision"], 2);

    apply_as_one_entry(&mut session, doc, &move_below);

    assert_eq!(
        timeline(&mut session, doc),
        [
            "f-sk1", "f-ext1", "f-dp1", "f-sk2", "f-dp2", "f-ext2", "f-fil1"
        ]
    );
}

// Test 132: fixes in transactions.

#[test]
fn t132_a_fix_inside_a_transaction_joins_it() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r03());
    let before = saved_text(&mut session, doc);
    let remove = action(
        &message(&edited, "f-fil1", "reference_missing"),
        "Remove from Edges",
    );
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Tidy"}),
    )
    .ok();

    let applied = apply_fix(&mut session, doc, &remove);

    assert_eq!(applied.ok()["revision"], 2);
    let changed = applied.event("document_changed");
    assert_eq!(changed["undo"], EDIT_LABEL, "no undo entry of its own");
    assert_eq!(changed["transaction"], "Tidy");
    let committed = send(
        &mut session,
        json!({"type": "commit_transaction", "doc": doc}),
    );
    assert_eq!(committed.ok()["recorded"], true);
    assert_undo_restores(&mut session, doc, &json!("Tidy"), &before);
}

// Test 133: `stale_fix`.

/// Asserts that a reply is the `stale_fix` error with no events, and that the document is still at `revision` with
/// `text`.
fn assert_stale(session: &mut Session, doc: u64, sent: &Sent, revision_before: u64, text: &str) {
    assert_eq!(sent.error_code(), "stale_fix");
    assert_eq!(
        sent.error_message(),
        "This fix no longer applies because the document changed after it was offered. Look at the problem again."
    );
    assert!(sent.events.is_empty(), "{:?}", sent.event_types());
    assert_eq!(revision(session, doc), revision_before);
    assert_eq!(saved_text(session, doc), text);
}

/// Opens corpus R16's edited text in a fresh display session; returns the session, the handle and the first action's
/// `FixRef` (revision 0).
fn opened_r16() -> (Session, u64, Value) {
    let mut session = session();
    let opened = open_text(&mut session, &edited_text(&r16()));
    let doc = doc_handle(&opened);
    let first =
        actions(&message(&opened, "f-fil1", "references_not_found"))[0]["run"]["fix"].clone();
    assert_eq!(first["revision"], 0);
    (session, doc, first)
}

/// Adds a parameter nothing uses: a recipe change unrelated to any message.
fn unrelated_edit(session: &mut Session, doc: u64) -> Sent {
    apply(
        session,
        doc,
        Some("Add parameter"),
        json!([{"type": "add_parameter", "index": 0, "parameter": {"id": "p-w", "name": "w", "expr": "5 mm"}}]),
    )
}

#[test]
fn t133_a_fix_offered_before_another_edit_is_stale() {
    let (mut session, doc, first) = opened_r16();
    unrelated_edit(&mut session, doc).ok();
    let text = saved_text(&mut session, doc);

    let sent = send_fix(&mut session, doc, first);

    assert_stale(&mut session, doc, &sent, 1, &text);
}

#[test]
fn t133_a_fix_of_a_rolled_back_feature_is_stale_at_the_current_revision() {
    let (mut session, doc, mut first) = opened_r16();
    unrelated_edit(&mut session, doc).ok();
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Look"}),
    )
    .ok();
    let focused = send(
        &mut session,
        json!({"type": "set_focus", "doc": doc, "feature": "f-box1"}),
    );
    assert_eq!(focused.feature_status("f-fil1")["status"], "rolled_back");
    first["revision"] = json!(1);
    let text = get_document(&mut session, doc)["document"].clone();

    let sent = send_fix(&mut session, doc, first);

    assert_eq!(sent.error_code(), "stale_fix");
    assert!(sent.events.is_empty(), "{:?}", sent.event_types());
    assert_eq!(revision(&mut session, doc), 1);
    assert_eq!(get_document(&mut session, doc)["document"], text);
}

#[test]
fn t133_a_pick_action_an_index_past_the_actions_and_another_code_are_stale() {
    let (mut session, doc, _) = opened_r16();
    let text = saved_text(&mut session, doc);
    let stale = [
        fix_ref("f-fil1", "references_not_found", "/edges", 5, 0),
        fix_ref("f-fil1", "references_not_found", "/edges", 9, 0),
        fix_ref("f-fil1", "fillet_too_large", "/edges", 0, 0),
    ];

    for fix in stale {
        let sent = send_fix(&mut session, doc, fix.clone());

        assert_stale(&mut session, doc, &sent, 0, &text);
    }
    let applied = send_fix(
        &mut session,
        doc,
        fix_ref("f-fil1", "references_not_found", "/edges", 0, 0),
    );
    assert_eq!(
        applied.ok()["revision"],
        1,
        "the fix at index 0 still applies"
    );
}

// Test 134: actions hold no session data.

#[test]
fn t134_actions_and_repairs_are_equal_in_a_fresh_session() {
    let mut first = session();
    let (doc, edited) = after_edit(&mut first, &r16());
    let text = saved_text(&mut first, doc);
    let mut second = session();

    let reopened = open_text(&mut second, &text);

    let session_data = ["revision", "mesh_id", "ghost", "cause"];
    let fresh = Value::Array(statuses(&reopened));
    for status in statuses(&reopened) {
        for message in status["messages"]
            .as_array()
            .expect("statuses list messages")
        {
            assert!(
                message.get("cause").is_none(),
                "a fresh document has no cause: {message}"
            );
        }
    }
    assert_eq!(
        without_keys(&Value::Array(statuses(&edited)), &session_data),
        without_keys(&fresh, &session_data)
    );
}
