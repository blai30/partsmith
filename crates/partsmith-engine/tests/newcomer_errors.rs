//! Acceptance test 150 of `docs/spec/engine.md` section 6: the newcomer error corpus
//! (`docs/design/newcomer-error-corpus.md`) through the protocol, in a display session. Each scenario is opened with
//! its references picked, and its edit, when it has one, applied as one labeled `apply_edits` (`corpus::after_edit`).
//! Each gives the codes, messages and action labels the corpus states, in order, and the action its "Then" names,
//! applied with `apply_fix`, gives the corpus's result in one undo entry; NC09 switches the units with
//! `set_document_units`. A test named `t150_ncNN_...` checks scenario NCNN.
//!
//! The cause lines of NC08 and NC11 (engine.md section 3.21), and NC08's "Then", which applies the revert its cause
//! line offers, come with phase 4 of the reference stability plan. The parts only the UI shows (NC01's Extrude disabled
//! reason, NC09's units control) are ui.md's E-110 and E-118.

mod common;
mod corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, assert_bounding_box, assert_relative, count, measure_body,
    saved_text, session, set_document_units,
};
use corpus::{
    above_the_block, actions, after_edit, apply_as_one_entry, assert_ok_without_messages,
    assert_undo_restores, box_record, cube20_front_top, document, face_names, fillet, labels,
    messages, nc01, nc02, nc03, nc04, nc05, nc06, nc07, nc08, nc09, nc10, nc11, nc12, opened,
    picked, record, solution_messages, volume,
};
use partsmith_engine::Session;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The only message of `feature` in the only `recomputed` event of `sent`.
fn only_message(sent: &Sent, feature: &str) -> Value {
    let found = messages(sent, feature);
    assert_eq!(
        found.len(),
        1,
        "{feature} has one message: {}",
        sent.feature_status(feature)
    );
    found[0].clone()
}

/// Asserts a message's severity, code, text and action labels, in order.
fn assert_message(message: &Value, severity: &str, code: &str, text: &str, offered: &[&str]) {
    assert_eq!(message["severity"], severity, "{message}");
    assert_eq!(message["code"], code, "{message}");
    assert_eq!(message["message"], text, "{message}");
    assert_eq!(labels(message), offered, "{message}");
}

/// Asserts that `feature` has status `status` and one message, with this severity, code and text and the actions
/// labeled `offered`.
fn assert_only_message(
    sent: &Sent,
    feature: &str,
    status: &str,
    (severity, code, text): (&str, &str, &str),
    offered: &[&str],
) {
    assert_eq!(sent.feature_status(feature)["status"], status, "{feature}");
    assert_message(&only_message(sent, feature), severity, code, text, offered);
}

/// The action of the only message of `feature` at position `index`.
fn offered_action(sent: &Sent, feature: &str, index: usize) -> Value {
    actions(&only_message(sent, feature))[index].clone()
}

/// Asserts the volume of `b-1` at the display state.
fn assert_block_volume(session: &mut Session, doc: u64, expected: f64) {
    assert_relative(
        volume(session, doc, "b-1"),
        expected,
        GEOMETRY_TOLERANCE,
        "volume of Body 1",
    );
}

// NC01 Open outline.

#[test]
fn t150_nc01_an_open_outline_has_no_region_and_one_finding_that_connects_the_ends() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc01());

    assert_eq!(sent.sketch_solution("f-sk1")["regions"], json!([]));
    let findings = solution_messages(sent.sketch_solution("f-sk1"));
    assert_eq!(
        findings.len(),
        1,
        "both open ends are one finding: {findings:?}"
    );
    assert_message(
        &findings[0],
        "warning",
        "open_endpoint",
        "The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.",
        &["Connect the ends (0.3 mm apart)"],
    );
}

#[test]
fn t150_nc01_connecting_the_ends_gives_the_sketch_one_region() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc01());
    let connect = actions(&solution_messages(sent.sketch_solution("f-sk1"))[0])[0].clone();

    let applied = apply_as_one_entry(&mut session, doc, &connect);

    let solution = applied.sketch_solution("f-sk1");
    assert_eq!(
        solution["regions"].as_array().map(Vec::len),
        Some(1),
        "{solution}"
    );
    assert_eq!(solution["messages"], json!([]));
    assert_undo_restores(&mut session, doc, &connect["label"], &before);
}

// NC02 Join buried in the body.

#[test]
fn t150_nc02_a_join_buried_in_the_body_offers_to_attach_its_sketch_to_the_top() {
    let mut session = session();

    let (doc, sent, _) = opened(&mut session, &nc02());

    assert_only_message(
        &sent,
        "f-ext2",
        "warning",
        (
            "warning",
            "no_effect",
            "This extrude adds nothing to body 'Body 1': it lies entirely inside the body.",
        ),
        &["Attach 'Sketch 2' to the top of 'Extrude 1'"],
    );
    assert_ok_without_messages(&sent, &["f-sk1", "f-ext1", "f-sk2"]);
    assert_block_volume(&mut session, doc, 120000.0);
}

#[test]
fn t150_nc02_attaching_stands_the_boss_on_the_top() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc02());
    let attach = offered_action(&sent, "f-ext2", 0);

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    assert_ok_without_messages(&applied, &["f-sk1", "f-ext1", "f-sk2", "f-ext2"]);
    assert_block_volume(&mut session, doc, 120785.398163);
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [0.0, 0.0, 0.0],
        [80.0, 50.0, 40.0],
    );
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

// NC03 Cut that points away.

/// The `no_effect` sentence of NC03, at distance 15 and at distance 5.
const POINTS_AWAY: &str =
    "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.";

#[test]
fn t150_nc03_a_cut_that_points_away_offers_turn_on_reverse() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc03());

    assert_only_message(
        &sent,
        "f-ext2",
        "warning",
        ("warning", "no_effect", POINTS_AWAY),
        &["Turn on Reverse"],
    );
    assert_ok_without_messages(&sent, &["f-sk1", "f-ext1", "f-dp1", "f-sk2"]);
}

#[test]
fn t150_nc03_turning_on_reverse_cuts_a_pocket() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc03());
    let reverse = offered_action(&sent, "f-ext2", 0);

    let applied = apply_as_one_entry(&mut session, doc, &reverse);

    assert_ok_without_messages(&applied, &["f-ext2"]);
    // The pocket spans z 25..30.
    assert_block_volume(&mut session, doc, 119607.300918);
    assert_undo_restores(&mut session, doc, &reverse["label"], &before);
}

#[test]
fn t150_nc03_a_cut_that_misses_both_ways_keeps_its_sentence_without_reverse() {
    // Reversed, the cut spans z 35..40 and still misses.
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &above_the_block("5", "cut"));

    assert_only_message(
        &sent,
        "f-ext2",
        "warning",
        ("warning", "no_effect", POINTS_AWAY),
        &[],
    );
}

// NC04 Floating join.

#[test]
fn t150_nc04_a_floating_join_offers_a_new_body_or_to_attach_its_sketch() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc04());

    assert_only_message(
        &sent,
        "f-ext2",
        "warning",
        (
            "warning",
            "separate_solid",
            "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.",
        ),
        &[
            "Put it in a new body",
            "Attach 'Sketch 2' to the top of 'Extrude 1'",
        ],
    );
}

#[test]
fn t150_nc04_attaching_joins_the_boss_as_one_solid() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc04());
    let attach = offered_action(&sent, "f-ext2", 1);

    let applied = apply_as_one_entry(&mut session, doc, &attach);

    // Without a `separate_solid` warning the body is one solid; the boss spans z 30..45.
    assert_ok_without_messages(&applied, &["f-sk2", "f-ext2"]);
    assert_block_volume(&mut session, doc, 121178.097245);
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [0.0, 0.0, 0.0],
        [80.0, 50.0, 45.0],
    );
    assert_undo_restores(&mut session, doc, &attach["label"], &before);
}

// NC05 Fillet too large.

#[test]
fn t150_nc05_a_fillet_too_large_names_the_wall_that_limits_it() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc05());

    assert_only_message(
        &sent,
        "f-fil1",
        "error",
        (
            "error",
            "fillet_too_large",
            "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.",
        ),
        &["Use 2.3 mm"],
    );
    assert_ok_without_messages(&sent, &["f-box1", "f-box2"]);
}

#[test]
fn t150_nc05_the_largest_radius_builds() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc05());
    let use_largest = offered_action(&sent, "f-fil1", 0);

    let applied = apply_as_one_entry(&mut session, doc, &use_largest);

    assert_ok_without_messages(&applied, &["f-box1", "f-box2", "f-fil1"]);
    assert_undo_restores(&mut session, doc, &use_largest["label"], &before);
}

// NC06 2 mm walls after a 1 mm fillet.

#[test]
fn t150_nc06_two_mm_walls_after_a_one_mm_fillet_build_with_sharp_inner_corners() {
    let mut session = session();

    let (doc, sent, _) = opened(&mut session, &nc06());

    assert_ok_without_messages(&sent, &["f-box1", "f-fil1", "f-sh1"]);
    assert_block_volume(&mut session, doc, 8654.831853);
    assert_eq!(
        count(&measure_body(&mut session, doc, "b-1")["face_count"]),
        15
    );
    let rounded_inside: Vec<String> = face_names(&mut session, doc, "b-1")
        .into_iter()
        .filter(|name| name.starts_with("f-sh1/inner/f-fil1/"))
        .collect();
    assert_eq!(
        rounded_inside,
        Vec::<String>::new(),
        "the inner offsets of the rounds vanish"
    );
}

// NC07 Constraint conflict.

#[test]
fn t150_nc07_a_conflict_names_both_dimensions_and_the_extrude_names_the_sketch_as_its_root() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc07());

    assert_only_message(
        &edited,
        "f-sk1",
        "error",
        (
            "error",
            "sketch_conflict",
            "In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.",
        ),
        &[
            "Delete Distance 30 mm",
            "Make Distance 30 mm a reference",
            "Delete Distance 40 mm",
            "Make Distance 40 mm a reference",
        ],
    );
    assert_only_message(
        &edited,
        "f-ext1",
        "error",
        (
            "error",
            "upstream_failed",
            "'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first.",
        ),
        &["Show 'Sketch 1'"],
    );
    assert_eq!(only_message(&edited, "f-ext1")["root"], "f-sk1");
}

#[test]
fn t150_nc07_deleting_the_new_dimension_builds_the_plate() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &nc07());
    let before = saved_text(&mut session, doc);
    let delete = offered_action(&edited, "f-sk1", 0);

    let applied = apply_as_one_entry(&mut session, doc, &delete);

    assert_ok_without_messages(&applied, &["f-sk1", "f-ext1"]);
    assert_block_volume(&mut session, doc, 8000.0);
    assert_undo_restores(&mut session, doc, &delete["label"], &before);
}

// NC08 A dimension value the sketch cannot take.

#[test]
fn t150_nc08_a_dimension_value_the_sketch_cannot_take_is_not_solved() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc08());

    // The message offers nothing itself: its cause line offers the revert and Undo (phase 4).
    assert_only_message(
        &edited,
        "f-sk1",
        "error",
        (
            "error",
            "sketch_not_solved",
            "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.",
        ),
        &[],
    );
}

// NC09 Units switch with bare numbers.

#[test]
fn t150_nc09_keep_sizes_adds_the_old_unit_to_the_four_numbers_as_one_undo_entry() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc09());
    assert_eq!(sent.event("document_changed")["bare_numbers"], 4);
    let volume_before = volume(&mut session, doc, "b-1");

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 4);
    let changed = switched.event("document_changed");
    assert_eq!(changed["document"]["units"], "in");
    assert_eq!(changed["undo"], "Change units to in");
    assert_eq!(changed["bare_numbers"], 0);
    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["40 mm", "20 mm", "10 mm"])
    );
    assert_eq!(record(&mut session, doc, "f-fil1")["radius"], "2 mm");
    assert_ok_without_messages(&switched, &["f-box1", "f-fil1"]);
    assert_relative(
        volume(&mut session, doc, "b-1"),
        volume_before,
        GEOMETRY_TOLERANCE,
        "the volume is kept",
    );
    assert_undo_restores(&mut session, doc, &json!("Change units to in"), &before);
}

#[test]
fn t150_nc09_without_keep_sizes_the_four_numbers_mean_inches() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", false);

    assert_eq!(switched.ok()["kept"], 0);
    assert_eq!(switched.event("document_changed")["bare_numbers"], 4);
    // The box is 1016 x 508 x 254 mm and the radius 50.8 mm, which still fits.
    assert_ok_without_messages(&switched, &["f-box1", "f-fil1"]);
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [-508.0, -254.0, 0.0],
        [508.0, 254.0, 254.0],
    );
}

#[test]
fn t150_nc09_a_fillet_whose_bare_number_now_fails_says_what_it_means_and_offers_millimeters() {
    // A 20 mm cube whose sizes carry their unit and a fillet of radius `2`: in inches the radius is 50.8 mm (corpus
    // R48's sentence, features.md section 4.3).
    let cube = picked(&document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["20 mm", "20 mm", "20 mm"], None, "new"),
            fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"),
        ],
    ));
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &cube);
    let switched = set_document_units(&mut session, doc, "in", false);
    let before = saved_text(&mut session, doc);

    let error = only_message(&switched, "f-fil1");
    assert_eq!(error["code"], "fillet_too_large");
    let sentence = error["message"].as_str().expect("messages are text");
    assert!(
        sentence.starts_with(
            "Fillet radius 2 in is too large for the selected edges: the largest that works is about "
        ),
        "{sentence}"
    );
    assert!(
        sentence.ends_with(" in. '2' has no unit, so it means 2 in in this document."),
        "{sentence}"
    );
    let offered = labels(&error);
    assert_eq!(offered[0], "Use 2 mm", "{offered:?}");
    let use_millimeters = actions(&error)[0].clone();

    let applied = apply_as_one_entry(&mut session, doc, &use_millimeters);

    assert_eq!(record(&mut session, doc, "f-fil1")["radius"], "2 mm");
    assert_ok_without_messages(&applied, &["f-fil1"]);
    assert_block_volume(&mut session, doc, 7982.831853);
    assert_undo_restores(&mut session, doc, &use_millimeters["label"], &before);
}

// NC10 Misspelled parameter.

#[test]
fn t150_nc10_a_misspelled_parameter_offers_the_near_match() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc10());

    assert_only_message(
        &sent,
        "f-ext1",
        "error",
        (
            "error",
            "expression",
            "Distance: 'hieght' is not a parameter. Did you mean 'height'?",
        ),
        &["Replace hieght with height"],
    );
}

#[test]
fn t150_nc10_replacing_the_name_builds_the_block() {
    let mut session = session();
    let (doc, sent, before) = opened(&mut session, &nc10());
    let replace = offered_action(&sent, "f-ext1", 0);

    let applied = apply_as_one_entry(&mut session, doc, &replace);

    assert_ok_without_messages(&applied, &["f-sk1", "f-ext1"]);
    assert_block_volume(&mut session, doc, 120000.0);
    assert_undo_restores(&mut session, doc, &replace["label"], &before);
}

// NC11 A deleted sketch line.

#[test]
fn t150_nc11_a_deleted_line_is_one_problem_whose_downstream_feature_names_its_root() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc11());

    assert_only_message(
        &edited,
        "f-ext1",
        "error",
        (
            "error",
            "profile_not_found",
            "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted.",
        ),
        &["Show 'Sketch 1'", "Pick a region again"],
    );
    assert_only_message(
        &edited,
        "f-fil1",
        "error",
        (
            "error",
            "body_empty",
            "Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.",
        ),
        &["Show 'Extrude 1'"],
    );
    assert_eq!(only_message(&edited, "f-fil1")["root"], "f-ext1");
}

// NC12 Hole beside the body.

#[test]
fn t150_nc12_a_hole_beside_the_body_says_where_it_is_and_offers_no_reverse() {
    let mut session = session();

    let (doc, sent, _) = opened(&mut session, &nc12());

    assert_only_message(
        &sent,
        "f-hole1",
        "warning",
        (
            "warning",
            "hole_misses_body",
            "The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body.",
        ),
        &[],
    );
    assert_ok_without_messages(&sent, &["f-sk1", "f-ext1", "f-sk2"]);
    assert_block_volume(&mut session, doc, 120000.0);
}
