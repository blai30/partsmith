//! Acceptance tests 126 to 135 of `docs/spec/engine.md` section 6, as far as phases 1 and 2 of the reference stability
//! plan build them: the labels and kinds of fix actions, `apply_fix` of each kind of fix these phases offer with its
//! undo label, re-anchoring for `KeepMatch` and `KeepRegions`, timeline fixes, fixes inside a transaction,
//! `stale_fix`, actions that hold no session data, and the frame fixes `Attach` and `TurnDrawingBack` (section 3.22).
//! A test named `tNNN` is section 6 test NNN; a test with several documents is one test per document.
//!
//! Scenarios come from `corpus`: each setup is opened with its references picked, and "after its edit" is the corpus
//! edit applied as one `apply_edits` labeled `Edit`, so the actions it offers carry revision 1. Corpus R26 is made as
//! test 136 makes it, in three `apply_edits` (`r26_recreated`), so its actions carry revision 3.
//!
//! Left for the phases that make the fixes they need: the actions of NC05 (`Use 2.3 mm`, and `Replace r * 2 with` of a
//! too large radius, test 126), NC03 (`Turn on Reverse`), NC10 (`Replace hieght with height`), CUBE10's linear pattern
//! without a target (`Set Operation to Join`), NC04 (`Put it in a new body`, and `Attach 'Sketch 2' to`, the label of
//! an `Attach` on another feature's message), NC07 (`Delete`, `Make ... a reference`) and NC01 (`Connect the ends`) in
//! tests 126, 127 and 131, which phase 3 adds; and test 132 with NC05's fix, which
//! `t132_a_fix_inside_a_transaction_joins_it` checks with a fix phase 1 offers.

mod common;
mod corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, X_AXIS, Z_AXIS, apply, assert_bounding_box, assert_fallback,
    assert_near, assert_placement, assert_point2, assert_point3, assert_relative, doc_handle,
    element_named, get_document, measure_body, number, open_text, point2, point3, revision,
    saved_text, send, session, set_rollback,
};
use corpus::{
    EDIT_LABEL, UPPER_BOSS_WALL, action, actions, after_edit, apply_fix, axis_xy,
    back_line_notched, block_back_side, block_edited, block_right_side, bosses_on_a_moved_box,
    box_record, cube10, datum_offset, datum_on_a_split_side, datum_on_face, document,
    document_with, edited_text, face_plane, faces_of, features, fix_ref, labels, linear_pattern,
    message, messages, moved_by_100, nc11, parameter, picked_before, point, r03, r06a, r16, r23a,
    r26_recreated, r28, r32b, r32b_chamfered_hole_at_an_arc_end, r32b_moved, r32b_pocket, r33, r50,
    r56b, r66, record, redrawn_beside_a_split, redrawn_beside_a_split_with_a_face_sketch,
    reference, repairs, right_side_redrawn_in_two, send_fix, sketch, statuses, two_areas, volume,
    without_keys,
};
use partsmith_engine::Session;
use partsmith_sketch::{HintFrame, RegionRef, SketchParams, SketchPlane};
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
fn t126_a_face_of_a_datum_that_now_spreads_over_several_planes_offers_each_face_then_pick() {
    // features.md section 3.6 (phase 2): the five faces of the notch do not lie in one plane, so the datum's base is
    // `not_coplanar` with each face offered and no Keep all, instead of a widening that the first face decides.
    let mut session = session();
    let scenario = block_edited(
        vec![datum_on_face("f-dp1", "b-1", block_back_side(), "5")],
        back_line_notched(),
    );

    let (_, edited) = after_edit(&mut session, &scenario);

    let error = message(&edited, "f-dp1", "not_coplanar");
    assert_eq!(error["field"], "/base");
    assert_eq!(
        labels(&error),
        [
            "Use the left side of 'Extrude 1'",
            "Use the back side of 'Extrude 1' made from the line from (50, 40) to (30, 40) in 'Sketch 1'",
            "Use the right side of 'Extrude 1'",
            "Use the back side of 'Extrude 1' made from the line from (30, 50) to (0, 50) in 'Sketch 1'",
            "Use the back side of 'Extrude 1' made from the line from (80, 50) to (50, 50) in 'Sketch 1'",
            "Pick a face again"
        ]
    );
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
    for scenario in [
        r66(),
        r06a(),
        r03(),
        r50(),
        r56b(),
        r23a(),
        r28(),
        nc11(),
        r32b(),
        r33(),
    ] {
        let (_, edited) = after_edit(&mut session, &scenario);
        all.extend(statuses(&edited));
    }
    let (_, recreated) = r26_recreated(&mut session);
    all.extend(statuses(&recreated));
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

/// The repair of `feature` in `sent` at `path`.
fn repair_at(sent: &Sent, feature: &str, path: &str) -> Value {
    repairs(sent, feature)
        .into_iter()
        .find(|repair| repair["path"] == path)
        .unwrap_or_else(|| {
            panic!(
                "{feature} has a repair at {path}: {}",
                sent.feature_status(feature)
            )
        })
}

/// The candidates of `feature`'s element repair at `path` in `sent`.
fn candidates_at(sent: &Sent, feature: &str, path: &str) -> Vec<Value> {
    repair_at(sent, feature, path)["candidates"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// The `Attach` labels a sketch's own message offers for its candidates (`attach_label`), those of candidates with
/// `meaning` first (features/sketch.md section 3.5), each group in candidate order.
fn attach_labels(candidates: &[Value]) -> Vec<String> {
    let (with_meaning, without_meaning): (Vec<&Value>, Vec<&Value>) = candidates
        .iter()
        .partition(|candidate| candidate["meaning"] == true);
    with_meaning
        .into_iter()
        .chain(without_meaning)
        .map(attach_label)
        .collect()
}

/// The label of the `Attach` a sketch's own message offers for `candidate` (section 3.22): `Attach to {description}`.
fn attach_label(candidate: &Value) -> String {
    let description = candidate["description"]
        .as_str()
        .expect("candidates have descriptions");
    format!("Attach to {description}")
}

#[test]
fn t126_r32b_a_face_that_turned_over_offers_turn_the_drawing_back_then_pick() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r32b());

    let warning = message(&edited, "f-sk2", "sketch_turned_over");
    assert_eq!(warning["field"], "/plane/face");
    assert_eq!(
        actions(&warning),
        [
            json!({
                "label": "Turn the drawing back",
                "run": {"kind": "apply", "fix": fix_ref("f-sk2", "sketch_turned_over", "/plane/face", 0, 1)}
            }),
            json!({
                "label": "Pick a face again",
                "run": {"kind": "pick", "feature": "f-sk2", "path": "/plane/face"}
            })
        ]
    );
}

#[test]
fn t126_r26_a_sketch_whose_face_is_gone_offers_attach_to_the_new_top_first() {
    let mut session = session();

    let (_, recreated) = r26_recreated(&mut session);

    let warning = message(&recreated, "f-sk2", "plane_fallback");
    let offered = labels(&warning);
    assert_eq!(offered[0], "Attach to the top of 'Extrude 1'");
    let candidates = candidates_at(&recreated, "f-sk2", "/plane/face");
    let mut expected = attach_labels(&candidates);
    expected.push("Pick a face again".to_owned());
    assert_eq!(offered, expected);
    for (index, attach) in actions(&warning).iter().take(candidates.len()).enumerate() {
        let index = u32::try_from(index).expect("few actions");
        assert_eq!(
            attach["run"],
            json!({"kind": "apply", "fix": fix_ref("f-sk2", "plane_fallback", "/plane/face", index, 3)})
        );
    }
}

#[test]
fn t126_r33_a_sketch_placed_where_its_face_was_offers_attach_to_the_new_top_first() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r33());

    let warning = message(&edited, "f-sk2", "plane_fallback");
    let candidates = candidates_at(&edited, "f-sk2", "/plane/face");
    assert_eq!(candidates[0]["reference"]["q"], "f-ext3/start-cap");
    let mut expected = attach_labels(&candidates);
    expected.push("Pick a face again".to_owned());
    assert_eq!(labels(&warning), expected);
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

#[test]
fn t128_keep_this_match_of_a_face_replaced_by_two_stores_the_one_the_datum_uses() {
    let mut session = session();
    let scenario = block_edited(
        vec![datum_on_face("f-dp1", "b-1", block_right_side(), "5")],
        right_side_redrawn_in_two(),
    );
    let (doc, edited) = after_edit(&mut session, &scenario);
    let before = saved_text(&mut session, doc);
    let notice = message(&edited, "f-dp1", "reference_followed");
    assert_eq!(notice["field"], "/base/face");
    assert_eq!(labels(&notice), ["Keep this match", "Pick a face again"]);
    let expected = picked_before(&before, "f-dp1", "b-1", &["f-ext1/lateral/e-10"]);

    let applied = apply_as_one_entry(&mut session, doc, &action(&notice, "Keep this match"));

    assert_eq!(
        record(&mut session, doc, "f-dp1")["base"]["face"],
        expected[0]
    );
    assert_eq!(applied.feature_status("f-dp1")["status"], "ok");
    assert_eq!(messages(&applied, "f-dp1"), Vec::<Value>::new());
    assert_undo_restores(&mut session, doc, &json!("Keep this match"), &before);
}

#[test]
fn t128_keep_this_match_of_a_datum_on_a_split_side_stores_the_first_half() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &datum_on_a_split_side());
    let before = saved_text(&mut session, doc);
    let notice = message(&edited, "f-dp1", "reference_followed");
    assert_eq!(notice["field"], "/base/face");
    let expected = picked_before(&before, "f-dp1", "b-1", &["f-ext1/lateral/e-2"]);

    let applied = apply_as_one_entry(&mut session, doc, &action(&notice, "Keep this match"));

    assert_eq!(
        record(&mut session, doc, "f-dp1")["base"]["face"],
        expected[0]
    );
    assert_eq!(applied.feature_status("f-dp1")["status"], "ok");
    assert_undo_restores(&mut session, doc, &json!("Keep this match"), &before);
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

/// The full reference of the front right vertical edge after `redrawn_beside_a_split`'s edit, picked just before the
/// fillet in the document `text`.
fn followed_front_right_edge(text: &str) -> Value {
    picked_before(
        text,
        "f-fil1",
        "b-1",
        &["edge(f-ext1/lateral/e-1|f-ext1/lateral/e-9)"],
    )
    .remove(0)
}

/// Applies `Keep these regions` of `f-ext1`'s `region_split` notice in `sent`, then checks that the fillet stores
/// the edge it followed and, once every feature runs, that it is ok with no message.
fn assert_kept_with_the_fillet_edge(session: &mut Session, doc: u64, sent: &Sent, before: &str) {
    let keep = action(
        &message(sent, "f-ext1", "region_split"),
        "Keep these regions",
    );

    apply_fix(session, doc, &keep).ok();

    assert_eq!(
        record(session, doc, "f-fil1")["edges"],
        json!([followed_front_right_edge(before)])
    );
}

#[test]
fn t129_keep_these_regions_rewrites_the_references_of_features_after_the_focus() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &redrawn_beside_a_split());
    let before = saved_text(&mut session, doc);
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Regions"}),
    )
    .ok();
    let focused = send(
        &mut session,
        json!({"type": "set_focus", "doc": doc, "feature": "f-ext1"}),
    );
    assert_eq!(focused.feature_status("f-fil1")["status"], "rolled_back");

    assert_kept_with_the_fillet_edge(&mut session, doc, &focused, &before);

    let committed = send(
        &mut session,
        json!({"type": "commit_transaction", "doc": doc}),
    );
    assert_eq!(committed.feature_status("f-fil1")["status"], "ok");
    assert_eq!(messages(&committed, "f-fil1"), Vec::<Value>::new());
}

#[test]
fn t129_keep_these_regions_rewrites_the_references_of_features_after_the_rollback_marker() {
    let mut session = session();
    let (doc, _) = after_edit(&mut session, &redrawn_beside_a_split());
    let before = saved_text(&mut session, doc);
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

    assert_kept_with_the_fillet_edge(&mut session, doc, &rolled_back, &before);

    let rolled_forward = apply(
        &mut session,
        doc,
        Some("Roll forward"),
        json!([set_rollback(None)]),
    );
    assert_eq!(rolled_forward.feature_status("f-fil1")["status"], "ok");
    assert_eq!(messages(&rolled_forward, "f-fil1"), Vec::<Value>::new());
}

#[test]
fn t129_keep_these_regions_rewrites_a_face_sketch_that_followed_a_replaced_line() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &redrawn_beside_a_split_with_a_face_sketch());
    let before = saved_text(&mut session, doc);
    assert_eq!(
        message(&edited, "f-sk2", "reference_followed")["field"],
        "/plane/face"
    );
    let followed_side = picked_before(&before, "f-sk2", "b-1", &["f-ext1/lateral/e-9"]);
    let keep = action(
        &message(&edited, "f-ext1", "region_split"),
        "Keep these regions",
    );

    let applied = apply_as_one_entry(&mut session, doc, &keep);

    assert_eq!(
        record(&mut session, doc, "f-sk2")["plane"]["face"],
        followed_side[0]
    );
    for feature in ["f-ext1", "f-fil1", "f-sk2"] {
        assert_eq!(applied.feature_status(feature)["status"], "ok");
        assert_eq!(messages(&applied, feature), Vec::<Value>::new());
    }
    assert_undo_restores(&mut session, doc, &keep["label"], &before);
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

#[test]
fn t134_r32b_turn_the_drawing_back_is_offered_alike_in_a_fresh_session() {
    let mut first = session();
    let (doc, edited) = after_edit(&mut first, &r32b());
    let text = saved_text(&mut first, doc);
    let mut second = session();

    let reopened = open_text(&mut second, &text);

    let session_data = ["revision", "mesh_id", "ghost", "cause"];
    assert_eq!(
        without_keys(&json!(messages(&edited, "f-sk2")), &session_data),
        without_keys(&json!(messages(&reopened, "f-sk2")), &session_data)
    );
}

// Test 135: frame fixes.

/// The sketch params a sketch record holds: the record without its own fields.
fn sketch_params(record: &Value) -> SketchParams {
    let mut params = record.clone();
    let fields = params.as_object_mut().expect("records are objects");
    for own in ["id", "type", "name", "suppressed"] {
        fields.remove(own);
    }
    serde_json::from_value(params)
        .unwrap_or_else(|error| panic!("{record} holds sketch params: {error}"))
}

/// The region references a profile record stores.
fn stored_regions(record: &Value) -> Vec<RegionRef> {
    serde_json::from_value(record["profile"]["regions"].clone())
        .unwrap_or_else(|error| panic!("{record} stores region references: {error}"))
}

/// Asserts that `plane` is a face plane on `face` of `b-1` whose fallback has `origin`, x axis (1, 0, 0) and `normal`
/// in `frame` (`assert_fallback`: `Some("body")` for body coordinates, `None` for world coordinates).
fn assert_face_plane(
    plane: &Value,
    face: &Value,
    origin: [f64; 3],
    normal: [f64; 3],
    frame: Option<&str>,
) {
    assert_eq!(plane["kind"], "face", "{plane}");
    assert_eq!(plane["body"], "b-1", "{plane}");
    assert_eq!(&plane["face"], face, "{plane}");
    assert_fallback(plane, origin, normal, frame);
}

/// Asserts that every feature named is ok with no message.
fn assert_ok_without_messages(sent: &Sent, features: &[&str]) {
    for feature in features {
        assert_eq!(sent.feature_status(feature)["status"], "ok", "{feature}");
        assert_eq!(messages(sent, feature), Vec::<Value>::new(), "{feature}");
    }
}

/// The `Attach` action of a sketch's own message for its first candidate, and that candidate's reference.
fn first_attach(sent: &Sent, sketch: &str) -> (Value, Value) {
    let candidates = candidates_at(sent, sketch, "/plane/face");
    let first = &candidates[0];
    let attach = action(
        &message(sent, sketch, "plane_fallback"),
        &attach_label(first),
    );
    (attach, first["reference"].clone())
}

#[test]
fn t135_r32b_turn_the_drawing_back_drills_the_hole_where_it_was_drawn() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r32b());
    let before = saved_text(&mut session, doc);
    let turn = action(
        &message(&edited, "f-sk2", "sketch_turned_over"),
        "Turn the drawing back",
    );

    let applied = apply_as_one_entry(&mut session, doc, &turn);

    assert_ok_without_messages(&applied, &["f-sk2", "f-hole1"]);
    // The bore's axis passes through world (20, 10), drilled from the bottom at z = -30 up to z = -20.
    assert_point2(
        axis_xy(&mut session, doc, "f-hole1/bore/e-p1"),
        [20.0, 10.0],
        GEOMETRY_TOLERANCE,
        "bore axis",
    );
    let bore = element_named(&mut session, doc, "b-1", "f-hole1/bore/e-p1");
    assert_near(
        point3(&bore["axis"]["direction"])[2].abs(),
        1.0,
        GEOMETRY_TOLERANCE,
        "the bore runs along Z",
    );
    assert_point3(
        point3(&bore["centroid"]),
        [20.0, 10.0, -25.0],
        GEOMETRY_TOLERANCE,
        "bore centroid",
    );
    let bottom = element_named(&mut session, doc, "b-1", "f-hole1/bottom/e-p1");
    assert_point3(
        point3(&bottom["centroid"]),
        [20.0, 10.0, -20.0],
        GEOMETRY_TOLERANCE,
        "hole bottom",
    );
    assert_relative(
        volume(&mut session, doc, "b-1"),
        119803.650459,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &turn["label"], &before);
}

#[test]
fn t135_r32b_turn_the_drawing_back_mirrors_the_drawing_and_stores_where_it_lies() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r32b());
    let sketch_before = record(&mut session, doc, "f-sk2");
    let hole_before = record(&mut session, doc, "f-hole1");
    let turn = action(
        &message(&edited, "f-sk2", "sketch_turned_over"),
        "Turn the drawing back",
    );

    apply_fix(&mut session, doc, &turn).ok();

    let sketch_after = record(&mut session, doc, "f-sk2");
    let position = point2(&sketch_after["entities"][0]["position"]);
    assert_near(position[0], 20.0, GEOMETRY_TOLERANCE, "e-p1 u");
    assert_near(position[1], -10.0, GEOMETRY_TOLERANCE, "e-p1 v");
    // The placement on the turned-over face: origin (0, 0, -30), normal (0, 0, -1); nothing moved the block, so body
    // coordinates are world coordinates.
    assert_face_plane(
        &sketch_after["plane"],
        &sketch_before["plane"]["face"],
        [0.0, 0.0, -30.0],
        [0.0, 0.0, -1.0],
        Some("body"),
    );
    assert_eq!(
        record(&mut session, doc, "f-hole1"),
        hole_before,
        "a point entity is the same point on the mirrored drawing"
    );
}

#[test]
fn t135_turn_the_drawing_back_stores_the_mirrored_sketch_and_re_anchors_the_regions_it_profiles() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r32b_pocket());
    let before = saved_text(&mut session, doc);
    let sketch_before = sketch_params(&record(&mut session, doc, "f-sk2"));
    let regions_before = stored_regions(&record(&mut session, doc, "f-ext2"));
    let turn = action(
        &message(&edited, "f-sk2", "sketch_turned_over"),
        "Turn the drawing back",
    );

    let applied = apply_as_one_entry(&mut session, doc, &turn);

    let mirrored = sketch_before.mirrored();
    let sketch_after = sketch_params(&record(&mut session, doc, "f-sk2"));
    assert_eq!(sketch_after.entities, mirrored.entities);
    assert_eq!(sketch_after.constraints, mirrored.constraints);
    let SketchPlane::Face { fallback_frame, .. } = &sketch_after.plane else {
        panic!("f-sk2 stays on a face: {:?}", sketch_after.plane);
    };
    assert_eq!(*fallback_frame, HintFrame::Body);
    let expected: Vec<RegionRef> = regions_before
        .iter()
        .map(|region| sketch_before.mirrored_region(region))
        .collect();
    assert_eq!(
        stored_regions(&record(&mut session, doc, "f-ext2")),
        expected
    );
    // The mirrored reference names the region of the turned-back drawing by its key and outline.
    let expected_json = serde_json::to_value(&expected[0]).expect("region references serialize");
    let key = expected_json["key"]
        .as_str()
        .expect("region references have a key");
    let solved = solved_regions(&applied, "f-sk2", &[key]);
    let solved = solved
        .first()
        .unwrap_or_else(|| panic!("the turned-back drawing has the region {expected_json}"));
    assert_eq!(solved["outline"], expected_json["outline"]);
    assert_ok_without_messages(&applied, &["f-sk2", "f-ext2"]);
    assert_relative(
        volume(&mut session, doc, "b-1"),
        119000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_undo_restores(&mut session, doc, &turn["label"], &before);
}

#[test]
fn t135_turn_the_drawing_back_renames_the_references_to_a_hole_at_an_arc_end() {
    // The mirrored drawing swaps the ends of the arc, so the hole drills at `e-5.end` and names its faces after it
    // (features/hole.md section 3.4); the chamfer's reference to the rim is renamed in the same batch, so the chamfer
    // keeps its edge.
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r32b_chamfered_hole_at_an_arc_end());
    assert_eq!(edited.feature_status("f-cha1")["status"], "ok");
    let before = saved_text(&mut session, doc);
    let turn = action(
        &message(&edited, "f-sk2", "sketch_turned_over"),
        "Turn the drawing back",
    );

    let applied = apply_as_one_entry(&mut session, doc, &turn);

    assert_eq!(
        record(&mut session, doc, "f-hole1")["points"],
        json!(["e-5.end"])
    );
    assert_eq!(
        record(&mut session, doc, "f-cha1")["edges"][0]["q"],
        "edge(f-ext1/end-cap|f-hole1/bore/e-5.end)"
    );
    assert_ok_without_messages(&applied, &["f-sk2", "f-hole1", "f-cha1"]);
    assert_point2(
        axis_xy(&mut session, doc, "f-hole1/bore/e-5.end"),
        [50.0, 30.0],
        GEOMETRY_TOLERANCE,
        "bore axis",
    );
    assert_undo_restores(&mut session, doc, &turn["label"], &before);
}

#[test]
fn t135_r26_attach_puts_the_sketch_on_the_new_top_in_body_coordinates() {
    let mut session = session();
    let (doc, recreated) = r26_recreated(&mut session);
    let before = saved_text(&mut session, doc);
    let (attach, top) = first_attach(&recreated, "f-sk2");
    assert_eq!(attach["label"], "Attach to the top of 'Extrude 1'");
    assert_eq!(top["q"], "f-ext3/end-cap");

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    // The sketch stood at its saved position at z = 30; projected onto the new top, it stands at z = 80.
    assert_face_plane(
        &record(&mut session, doc, "f-sk2")["plane"],
        &top,
        [0.0, 0.0, 80.0],
        Z_AXIS,
        Some("body"),
    );
    assert_ok_without_messages(&applied, &["f-ext3", "f-sk2", "f-ext2"]);
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
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

#[test]
fn t135_r33_attach_projects_where_the_sketch_stands_onto_the_new_top() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r33());
    let before = saved_text(&mut session, doc);
    let (attach, new_top) = first_attach(&edited, "f-sk2");
    assert_eq!(new_top["q"], "f-ext3/start-cap");

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    // The sketch stood where the top was before 'Extrude 1' cut it away, at z = 40; attached, it stands on the floor of
    // the cut at z = 35, where the boss now joins the box.
    assert_face_plane(
        &record(&mut session, doc, "f-sk2")["plane"],
        &new_top,
        [0.0, 0.0, 35.0],
        Z_AXIS,
        Some("body"),
    );
    assert_ok_without_messages(&applied, &["f-sk2", "f-ext2"]);
    assert_relative(
        volume(&mut session, doc, "b-1"),
        14141.371669,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [-10.0, -10.0, 0.0],
        [10.0, 10.0, 40.0],
    );
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

/// features/sketch.md test 5 after a Move body: a sketch `f-sk1` on a face CUBE10 does not have, with the point `e-p1`,
/// its fallback at (3, 4, 0) in `frame` (`None` writes no `fallback_frame`: world coordinates), with the cube moved by
/// (100, 0, 0) before it.
fn sketch_on_a_lost_face_of_the_moved_cube(frame: Option<&str>) -> String {
    let mut plane = face_plane(
        "b-1",
        reference("f-box1/nothing", "plane|n=0,0,1|c=500,500,500|a=1"),
        [3.0, 4.0, 0.0],
        X_AXIS,
        Z_AXIS,
    );
    if let Some(frame) = frame {
        plane["fallback_frame"] = json!(frame);
    }
    document(
        &["b-1"],
        &[
            cube10(),
            moved_by_100(),
            sketch("f-sk1", plane, vec![point("e-p1", [0.0, 0.0])]),
        ],
    )
}

#[test]
fn t135_attach_to_a_face_with_the_sketch_s_motion_stores_the_fallback_in_body_coordinates() {
    // The lost face moved with the cube, as the top did: the saved position (3, 4, 0) in body coordinates is world
    // (103, 4, 0), which projects onto the moved top at (103, 4, 10), (3, 4, 10) before the move.
    let mut session = session();
    let opened = open_text(
        &mut session,
        &sketch_on_a_lost_face_of_the_moved_cube(Some("body")),
    );
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    let (attach, top) = first_attach(&opened, "f-sk1");
    assert_eq!(top["q"], "f-box1/top");

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    assert_face_plane(
        &record(&mut session, doc, "f-sk1")["plane"],
        &top,
        [3.0, 4.0, 10.0],
        Z_AXIS,
        Some("body"),
    );
    assert_placement(
        &applied.sketch_solution("f-sk1")["placement"],
        [103.0, 4.0, 10.0],
        X_AXIS,
        Z_AXIS,
        "placement",
    );
    assert_ok_without_messages(&applied, &["f-sk1"]);
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

#[test]
fn t135_attach_of_a_world_fallback_to_a_moved_face_keeps_world_coordinates() {
    // The sketch's fallback is in world coordinates, so it has no motion, while the moved top has the Move body's:
    // body coordinates would give the sketch, and every face built from it, that motion. Projected onto the moved top,
    // the sketch stands at world (3, 4, 10), stored as it is.
    let mut session = session();
    let opened = open_text(&mut session, &sketch_on_a_lost_face_of_the_moved_cube(None));
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    let (attach, top) = first_attach(&opened, "f-sk1");
    assert_eq!(top["q"], "f-box1/top");

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    assert_face_plane(
        &record(&mut session, doc, "f-sk1")["plane"],
        &top,
        [3.0, 4.0, 10.0],
        Z_AXIS,
        None,
    );
    assert_placement(
        &applied.sketch_solution("f-sk1")["placement"],
        [3.0, 4.0, 10.0],
        X_AXIS,
        Z_AXIS,
        "placement",
    );
    assert_ok_without_messages(&applied, &["f-sk1"]);
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

#[test]
fn t135_attach_is_offered_only_for_faces_that_keep_the_sketch_s_motion() {
    // A taller box `f-box2` joined after the Move body has no motion, while the sketch's fallback is in body
    // coordinates of the moved cube. Attaching to a face of `f-box2` would take the cube's motion from the sketch and
    // from every face built from it, so only the faces of the cube are offered; every face stays a candidate.
    let mut text: Value =
        serde_json::from_str(&sketch_on_a_lost_face_of_the_moved_cube(Some("body")))
            .expect("documents are JSON");
    text["features"]
        .as_array_mut()
        .expect("documents list their features")
        .insert(
            2,
            box_record(
                "f-box2",
                "b-1",
                ["10", "10", "20"],
                Some(["100", "8", "0"]),
                "join",
            ),
        );
    let mut session = session();
    let opened = open_text(&mut session, &text.to_string());

    let candidates = candidates_at(&opened, "f-sk1", "/plane/face");

    let offered = labels(&message(&opened, "f-sk1", "plane_fallback"));
    let queries: Vec<&str> = candidates
        .iter()
        .map(|candidate| {
            candidate["reference"]["q"]
                .as_str()
                .expect("queries are text")
        })
        .collect();
    assert!(
        queries.iter().any(|query| query.starts_with("f-box1/")),
        "{queries:?}"
    );
    assert!(
        queries.iter().any(|query| query.starts_with("f-box2/")),
        "{queries:?}"
    );
    for (candidate, query) in candidates.iter().zip(&queries) {
        let label = attach_label(candidate);
        assert_eq!(
            offered.contains(&label),
            query.starts_with("f-box1/"),
            "{label} for {query} in {offered:?}"
        );
    }
}

#[test]
fn t135_attach_of_a_world_fallback_to_a_moved_face_moves_no_later_sketch() {
    // `f-sk1` stands at its saved position in world coordinates; `f-sk3` stores its fallback in the body coordinates of
    // the cap `f-ext1` builds from `f-sk1`, whose motion is the identity. Attached to the moved top in body coordinates,
    // `f-sk1` would give that cap the Move body's motion, and the upper boss would land at x = 200, off the body.
    let text = bosses_on_a_moved_box(
        vec![moved_by_100()],
        face_plane(
            "b-1",
            reference("f-box1/nothing", "plane|n=0,0,1|c=100,0,40|a=600"),
            [0.0, 0.0, 40.0],
            X_AXIS,
            Z_AXIS,
        ),
        [0.0, 0.0, 50.0],
        Z_AXIS,
    );
    let mut session = session();
    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    assert_point2(
        axis_xy(&mut session, doc, UPPER_BOSS_WALL),
        [100.0, 0.0],
        GEOMETRY_TOLERANCE,
        "boss axis before",
    );
    let (attach, top) = first_attach(&opened, "f-sk1");
    assert_eq!(top["q"], "f-box1/top");

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    assert_face_plane(
        &record(&mut session, doc, "f-sk1")["plane"],
        &top,
        [0.0, 0.0, 40.0],
        Z_AXIS,
        None,
    );
    assert_ok_without_messages(&applied, &["f-sk1", "f-ext1", "f-sk3", "f-ext3"]);
    assert_point2(
        axis_xy(&mut session, doc, UPPER_BOSS_WALL),
        [100.0, 0.0],
        GEOMETRY_TOLERANCE,
        "boss axis after",
    );
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [90.0, -15.0, 0.0],
        [110.0, 15.0, 55.0],
    );
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

#[test]
fn t135_turning_back_a_world_fallback_on_a_moved_face_moves_no_later_sketch() {
    // A half turn about X, then (100, 0, 0), turn the box's top over to z = -40, facing down, while `f-sk1`'s fallback
    // in world coordinates faces up, so its drawing lies mirrored there. `f-sk3` stores its fallback in the body
    // coordinates of the cap `f-ext1` builds from `f-sk1`, whose motion is the identity. Turned back in body
    // coordinates, `f-sk1` would give that cap the motion of both moves, and the upper boss would land at x = 200.
    let half_turn = json!({
        "id": "f-mv0", "type": "move_body", "body": "b-1",
        "rotation": {"axis": {"kind": "origin", "axis": "X"}, "angle": "180 deg"}
    });
    let text = bosses_on_a_moved_box(
        vec![half_turn, moved_by_100()],
        face_plane(
            "b-1",
            reference("f-box1/top", "plane|n=0,0,-1|c=100,0,-40|a=600|adj=4,4"),
            [0.0, 0.0, 40.0],
            X_AXIS,
            Z_AXIS,
        ),
        [0.0, 0.0, -50.0],
        [0.0, 0.0, -1.0],
    );
    let mut session = session();
    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);
    let before = saved_text(&mut session, doc);
    assert_point2(
        axis_xy(&mut session, doc, UPPER_BOSS_WALL),
        [100.0, 0.0],
        GEOMETRY_TOLERANCE,
        "boss axis before",
    );
    let face = record(&mut session, doc, "f-sk1")["plane"]["face"].clone();
    let turn = action(
        &message(&opened, "f-sk1", "sketch_turned_over"),
        "Turn the drawing back",
    );

    let applied = apply_as_one_entry(&mut session, doc, &turn);

    assert_face_plane(
        &record(&mut session, doc, "f-sk1")["plane"],
        &face,
        [0.0, 0.0, -40.0],
        [0.0, 0.0, -1.0],
        None,
    );
    assert_ok_without_messages(&applied, &["f-sk1", "f-ext1", "f-sk3", "f-ext3"]);
    assert_point2(
        axis_xy(&mut session, doc, UPPER_BOSS_WALL),
        [100.0, 0.0],
        GEOMETRY_TOLERANCE,
        "boss axis after",
    );
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [90.0, -15.0, -55.0],
        [110.0, 15.0, 0.0],
    );
    assert_undo_restores(&mut session, doc, &turn["label"], &before);
}

#[test]
fn t135_r32b_moved_turn_the_drawing_back_stores_the_placement_without_the_move() {
    // Corpus R32 (b) with BLOCK moved by (100, 0, 0) before the sketch, whose fallback is in body coordinates: the
    // placement on the turned-over face, origin (100, 0, -30) and normal (0, 0, -1), is stored with the Move body
    // undone (features/sketch.md section 3.5), so the hole is drilled at world (120, 10).
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &r32b_moved());
    let face = record(&mut session, doc, "f-sk2")["plane"]["face"].clone();
    let turn = action(
        &message(&edited, "f-sk2", "sketch_turned_over"),
        "Turn the drawing back",
    );

    let applied = apply_as_one_entry(&mut session, doc, &turn);

    assert_face_plane(
        &record(&mut session, doc, "f-sk2")["plane"],
        &face,
        [0.0, 0.0, -30.0],
        [0.0, 0.0, -1.0],
        Some("body"),
    );
    assert_ok_without_messages(&applied, &["f-sk2", "f-hole1"]);
    assert_point2(
        axis_xy(&mut session, doc, "f-hole1/bore/e-p1"),
        [120.0, 10.0],
        GEOMETRY_TOLERANCE,
        "bore axis",
    );
}
