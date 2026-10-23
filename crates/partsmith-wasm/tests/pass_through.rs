//! The protocol passes through `ps_call` unchanged (protocol.md section 2.10; the reference stability plan, sections 10.3 item 9, 10.4, 10.5 and 10.6). The bridge answers the diagnostics, fixes and errors of ADR 0024, the documents of format version 2 (ADR 0025), the frames on the model of ADR 0023 (body-coordinate fallbacks and axis directions in descriptors, drag commits, `Attach` and `Turn the drawing back`), the newcomer errors of phase 3 (sketch finding messages with `Connect the ends`, the conflict's Delete and Make a reference, `Put it in a new body`, renamed settings and misspelled parameter names, the limits that the kernel's solid counts and widths across edges find, `set_document_units` and `bare_numbers`) and the causes, ghosts and uses of phase 4 (cause lines with their reverts, `Keep the sketch where it was`, Undo and Show, the ghosts of lost elements, lost regions and edited sketch entities with their buffers, and `list_uses`) with the response frame that `Session::call` gives a session of the module's options (display on, random ids): nothing is added, dropped or reordered on the way through.
//!
//! Each test sends its requests through `ps_call` (the `Block` helper) and to a reference `Session`, and compares the decoded frames. Document handles and mesh ids count per session, and on the Emscripten target every test shares the module's one session, so the comparison leaves those two numbers out, and a command that names a document or a mesh, such as a pick, names each side's own. A fix whose edits take a new id from the session's id generator (`Put it in a new body`, `Connect the ends`) gets a random id on each side, so the comparison of that frame reads each side's new ids as the same placeholders. Everything else, buffers included, must be equal. Each test also checks the new shape it is about in the bridge's frame, so an empty answer on both sides cannot pass.
//!
//! A test named `engine_tNNN` sends the documents and requests of engine.md section 6 test NNN, one named `engine_tNNN_ncMM` those of newcomer error corpus scenario NCMM in that test, one named `engine_tNNN_rMM` those of reference stability corpus scenario RMM in that test, and one named `file_format_tNNN` those of file-format.md section 6 test NNN; the bridge's own tests are protocol.md's (`tNNN` in the other test files).

use crate::common::{error_info, ok_result, outcome_json};
use crate::pass_through_support::*;
use partsmith_model::FORMAT_VERSION;
use partsmith_protocol::{ErrorCode, ResponseFrame};
use serde_json::{Value, json};

// Diagnostics, fixes, errors and format version 2 (the reference stability plan, section 10.3 item 9).

#[test]
fn a_message_with_fix_actions_passes_through_unchanged() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, FRACTIONAL_PATTERN_TEXT);

    let message = feature_message(&opened, "f-lp1", "not_whole_number");
    assert_eq!(labels(&message), ["Use 2", "Use 3"]);
    assert_eq!(
        message["actions"][1]["run"],
        json!({"kind": "apply", "fix": use_three()})
    );
}

/// BLOCK's region picked before its right side `e-2` was deleted and its bottom `e-1` was drawn on to x = 200 under a second outline: the stored region now matches two separate areas, so `f-ext1` fails with `profile_not_found` and a region repair with both areas as candidates (engine.md test 119; sketch.md section 3.14 matches regions from the sketch alone, so the text gives the same repair opened fresh).
const TWO_AREAS_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {"kind": "origin", "plane": "XY"},
      "entities": [
        {"id": "e-1", "type": "line", "start": [0, 0], "end": [200, 0]},
        {"id": "e-9", "type": "line", "start": [80, 0], "end": [80, 50]},
        {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
        {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]},
        {"id": "e-5", "type": "line", "start": [200, 0], "end": [200, 20]},
        {"id": "e-6", "type": "line", "start": [200, 20], "end": [150, 20]},
        {"id": "e-7", "type": "line", "start": [150, 20], "end": [150, 0]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext1",
      "type": "extrude",
      "body": "b-1",
      "profile": {
        "sketch": "f-sk1",
        "regions": [
          {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}
        ]
      },
      "extent": {"kind": "distance", "expr": "30"},
      "op": "new"
    }
  ]
}
"#;

#[test]
fn a_region_repair_with_its_candidates_passes_through_unchanged() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, TWO_AREAS_TEXT);

    feature_message(&opened, "f-ext1", "profile_not_found");
    let repair = feature_repair(&opened, "f-ext1", "/profile/regions/0");
    assert_eq!(repair["kind"], "region", "{repair}");
    assert_eq!(
        repair["description"],
        "the region of 'Sketch 1' around (40, 25)"
    );
    let descriptions: Vec<&Value> = candidates(&repair)
        .iter()
        .map(|candidate| &candidate["description"])
        .collect();
    assert_eq!(
        descriptions,
        [
            "the region of 'Sketch 1' around (40, 25)",
            "the region of 'Sketch 1' around (175, 10)"
        ]
    );
}

#[test]
fn an_element_repair_with_its_candidates_and_relations_passes_through_unchanged() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, CUT_AWAY_EDGE_TEXT);

    feature_message(&opened, "f-fil1", "references_not_found");
    let repair = feature_repair(&opened, "f-fil1", "/edges/0");
    assert_eq!(repair["kind"], "element", "{repair}");
    assert_eq!(repair["description"], "the front top edge of 'Box 1'");
    let first = &candidates(&repair)[0];
    assert_eq!(
        first["reference"],
        json!({"q": "edge(f-box1/front|f-ext2/lateral/e-1)", "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"})
    );
    assert_eq!(
        first["relation"],
        "on the front side of 'Box 1', 5 mm from where it was"
    );
}

/// The edge of E10's fillet: the front edge of the right part of the block's top, which the slot splits (engine.md test 122).
const SPLIT_TOP_FRONT_EDGE: &str = "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)";

#[test]
fn a_pick_with_its_full_reference_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, E10_TEXT);
    sides.send(2, |side| begin_transaction(side, "Edit Fillet 1"));
    sides.send(
        3,
        |side| json!({"type": "set_focus", "doc": side.doc, "feature": "f-fil1"}),
    );
    let edges = sides.send(4, list_edges);
    let index = listed_index(&edges, SPLIT_TOP_FRONT_EDGE);

    let picked = sides.send(5, |side| pick(side, "edge", index));

    assert_eq!(
        ok_result(&picked, "pick_reference")["element"]["reference"],
        json!({
            "q": SPLIT_TOP_FRONT_EDGE,
            "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
            "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}
        })
    );
}

#[test]
fn apply_fix_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, FRACTIONAL_PATTERN_TEXT);

    let applied = sides.send(2, |side| apply_fix(side, &use_three()));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_eq!(feature_status(&applied, "f-lp1")["status"], "ok");
}

#[test]
fn a_stale_fix_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, FRACTIONAL_PATTERN_TEXT);
    sides.send(2, |side| apply_fix(side, &use_three()));

    let stale = sides.send(3, |side| apply_fix(side, &use_three()));

    assert_eq!(stale.response.id, 3);
    assert_eq!(error_info(&stale).code, ErrorCode::StaleFix);
    assert_eq!(
        error_info(&stale).message,
        "This fix no longer applies because the document changed after it was offered. Look at the problem again."
    );
    assert!(stale.events.is_empty(), "a stale fix changes nothing");
}

#[test]
fn a_fix_reference_with_an_unknown_field_passes_through_as_malformed_request() {
    let mut sides = SideBySide::new();
    sides.open(1, FRACTIONAL_PATTERN_TEXT);
    let mut fix = use_three();
    fix["extra"] = json!(1);

    let rejected = sides.send(2, |side| apply_fix(side, &fix));

    assert_eq!(rejected.response.id, 2);
    assert_eq!(error_info(&rejected).code, ErrorCode::MalformedRequest);
}

#[test]
fn a_version_2_document_with_every_new_field_passes_through_byte_for_byte() {
    let mut sides = SideBySide::new();
    sides.open(1, E10_TEXT);

    let saved = sides.send(2, save_document);

    assert_eq!(ok_result(&saved, "save_document")["text"], E10_TEXT);
}

#[test]
fn a_version_1_document_passes_through_saved_at_version_2() {
    let mut sides = SideBySide::new();
    sides.open(1, &e10_at_version(1));

    let saved = sides.send(2, save_document);

    assert_eq!(ok_result(&saved, "save_document")["text"], E10_TEXT);
}

#[test]
fn a_document_newer_than_version_2_passes_through_as_unsupported() {
    let mut sides = SideBySide::new();
    let newer = FORMAT_VERSION + 1;

    let rejected = sides.open(1, &e10_at_version(newer));

    assert_eq!(
        error_info(&rejected).code,
        ErrorCode::UnsupportedFormatVersion
    );
    assert_eq!(
        error_info(&rejected).message,
        format!(
            "This file uses format version {newer}, which is newer than this version of Partsmith supports (up to {FORMAT_VERSION}). Update Partsmith to open it."
        )
    );
}

#[test]
fn short_references_in_element_lists_pass_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, E10_TEXT);

    let elements = listed(&sides.send(2, list_edges));

    assert!(!elements.is_empty(), "E10's block has edges");
    for element in &elements {
        let mut keys: Vec<&str> = element["reference"]
            .as_object()
            .unwrap_or_else(|| panic!("each element has a reference: {element}"))
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["fp", "q"], "a listed reference is short: {element}");
    }
}

#[test]
fn an_error_with_a_detail_passes_through_unchanged() {
    let mut sides = SideBySide::new();

    let rejected = sides.open(1, r#"{"format": "partsmith","#);

    let info = error_info(&rejected);
    assert_eq!(info.code, ErrorCode::ParseError);
    assert_eq!(
        info.message,
        "The file is not valid JSON at line 1, column 23."
    );
    assert!(
        info.detail
            .as_deref()
            .is_some_and(|detail| !detail.is_empty()),
        "the parser's text travels in the detail: {info:?}"
    );
}

#[test]
fn an_error_with_an_action_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, FRACTIONAL_PATTERN_TEXT);
    sides.send(2, |side| begin_transaction(side, "Resize"));

    let refused = sides.send(3, save_document);

    assert_eq!(error_info(&refused).code, ErrorCode::TransactionOpen);
    assert_eq!(
        outcome_json(&refused)["actions"],
        json!([{"label": "Finish \"Resize\" and save", "run": {"kind": "finish_transaction"}}])
    );
}

// Frames on the model (ADR 0023; the reference stability plan, section 10.4).

#[test]
fn engine_t124_a_moved_face_passes_through_with_its_fallback_in_body_coordinates() {
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());

    let faces = sides.send(2, list_faces);

    let top = listed_named(&faces, BOX_TOP);
    assert_level_placement(&top["placement"], [0.0, 0.0, 40.0], "placement");
    assert_level_placement(&top["fallback"], [-100.0, 0.0, 40.0], "fallback");
}

#[test]
fn engine_t124_a_moved_line_edge_passes_through_with_its_axis_direction() {
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());

    let edges = sides.send(2, list_edges);

    let edge = listed_named(&edges, BOX_FRONT_TOP_EDGE);
    assert_vector(&edge["axis_direction"], X_AXIS, "axis direction");
}

/// The Move body `f-mv1` of `b-1` that turns it 90 degrees about the origin Z axis (engine.md test 124).
fn quarter_turn() -> Value {
    json!({
        "id": "f-mv1", "type": "move_body", "body": "b-1",
        "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}
    })
}

#[test]
fn engine_t124_a_turned_line_edge_passes_through_with_the_axis_direction_it_had_before_the_turn() {
    let mut sides = SideBySide::new();
    sides.open(1, &box_then(vec![quarter_turn()]));

    let edges = sides.send(2, list_edges);

    let edge = listed_named(&edges, BOX_FRONT_TOP_EDGE);
    let [x, y, _] = vector(&edge["direction"]);
    assert!(
        x.abs() <= GEOMETRY_TOLERANCE && (y.abs() - 1.0).abs() <= GEOMETRY_TOLERANCE,
        "the edge runs along Y after the turn: {edge}"
    );
    assert_vector(&edge["axis_direction"], X_AXIS, "axis direction");
}

/// The keys of the descriptor named `name` that a `list_elements` response frame writes, in wire order.
fn wire_keys(frame: &[u8], name: &str) -> Vec<String> {
    let response = wire_response(frame);
    let elements = response["result"]["elements"]
        .as_array()
        .expect("list_elements lists elements");
    keys_of(named(elements, name))
}

#[test]
fn engine_t124_descriptors_keep_their_key_order_through_ps_call() {
    // Protocol.md test 79: `fallback` after `placement`, `axis_direction` after it, both before `adjacent`.
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());

    let face_keys = wire_keys(&sides.send_raw(2, list_faces), BOX_TOP);
    let edge_keys = wire_keys(&sides.send_raw(3, list_edges), BOX_FRONT_TOP_EDGE);

    let order = ["placement", "fallback", "axis_direction", "adjacent"];
    assert_eq!(
        in_wire_order(&face_keys, &order),
        ["placement", "fallback", "adjacent"],
        "{face_keys:?}"
    );
    assert_eq!(
        in_wire_order(&edge_keys, &order),
        ["axis_direction", "adjacent"],
        "{edge_keys:?}"
    );
}

#[test]
fn engine_t124_a_pick_of_a_moved_face_passes_through_with_its_fallback() {
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());
    let faces = sides.send(2, list_faces);
    let index = listed_index(&faces, BOX_TOP);

    let picked = sides.send(3, |side| pick(side, "face", index));

    let element = &ok_result(&picked, "pick_reference")["element"];
    assert_eq!(element["name"], BOX_TOP);
    assert_level_placement(&element["fallback"], [-100.0, 0.0, 40.0], "fallback");
}

#[test]
fn engine_t125_a_drag_commit_on_a_moved_face_passes_through_with_its_body_fallback() {
    let plane = plane_after_a_drag_commit(Some("body"));

    assert_level_placement(&plane["fallback"], [0.0, 0.0, 40.0], "fallback");
    assert_eq!(plane["fallback_frame"], "body", "{plane}");
}

#[test]
fn engine_t125_a_drag_commit_of_a_world_fallback_passes_through_in_world_coordinates() {
    let plane = plane_after_a_drag_commit(None);

    assert_level_placement(&plane["fallback"], [0.0, 0.0, 40.0], "fallback");
    assert!(plane.get("fallback_frame").is_none(), "{plane}");
}

#[test]
fn engine_t125_solve_sketch_of_a_body_fallback_passes_through_placed_on_the_moved_face() {
    let mut sides = SideBySide::new();
    sides.open(1, &sketch_on_moved_top(Some("body")));

    let solved = sides.send(2, |side| solve_sketch(side, "f-sk1"));

    assert_level_placement(
        &ok_result(&solved, "solve_sketch")["solution"]["placement"],
        [100.0, 0.0, 40.0],
        "placement",
    );
}

#[test]
fn engine_t135_an_attach_action_passes_through_unchanged() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, LOST_FACE_ON_A_MOVED_CUBE_TEXT);

    let message = feature_message(&opened, "f-sk1", "plane_fallback");
    let attach = action_labeled(&message, "Attach to the top of 'Box 1'");
    assert_eq!(attach["run"]["kind"], "apply", "{attach}");
    assert_eq!(attach["run"]["fix"]["feature"], "f-sk1", "{attach}");
    assert_eq!(attach["run"]["fix"]["code"], "plane_fallback", "{attach}");
    assert_eq!(attach["run"]["fix"]["revision"], 0, "{attach}");
}

#[test]
fn engine_t135_apply_fix_of_attach_passes_through_with_the_fallback_in_body_coordinates() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, LOST_FACE_ON_A_MOVED_CUBE_TEXT);
    let message = feature_message(&opened, "f-sk1", "plane_fallback");

    let applied = sides.send(2, |side| {
        apply_action(side, &message, "Attach to the top of 'Box 1'")
    });

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sk1");
    assert_level_placement(
        &sketch_solution(&applied, "f-sk1")["placement"],
        [103.0, 4.0, 10.0],
        "placement",
    );
    let plane = record(&mut sides, 3, "f-sk1")["plane"].clone();
    assert_eq!(plane["face"]["q"], "f-box1/top", "{plane}");
    assert_level_placement(&plane["fallback"], [3.0, 4.0, 10.0], "fallback");
    assert_eq!(plane["fallback_frame"], "body", "{plane}");
}

#[test]
fn engine_t135_a_turned_over_face_passes_through_with_turn_the_drawing_back() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, TURNED_OVER_TOP_TEXT);

    let message = feature_message(&opened, "f-sk2", "sketch_turned_over");
    assert_eq!(
        message["message"],
        "The face this sketch is on, the top of 'Extrude 1', now faces the other way, so the drawing is mirrored on it."
    );
    assert_eq!(
        labels(&message),
        ["Turn the drawing back", "Pick a face again"]
    );
    assert_eq!(message["actions"][0]["run"]["kind"], "apply", "{message}");
}

/// The world -Z direction.
const MINUS_Z_AXIS: [f64; 3] = [0.0, 0.0, -1.0];

#[test]
fn engine_t135_apply_fix_of_turn_the_drawing_back_passes_through_with_the_mirrored_drawing() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, TURNED_OVER_TOP_TEXT);
    let message = feature_message(&opened, "f-sk2", "sketch_turned_over");

    let applied = sides.send(2, |side| {
        apply_action(side, &message, "Turn the drawing back")
    });

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sk2");
    assert_ok_without_messages(&applied, "f-hole1");
    let sketch = record(&mut sides, 3, "f-sk2");
    assert_eq!(sketch["entities"][0]["id"], "e-p1", "{sketch}");
    assert_vector(
        &sketch["entities"][0]["position"],
        [20.0, -10.0],
        "e-p1 position",
    );
    let plane = &sketch["plane"];
    assert_placement(
        &plane["fallback"],
        [0.0, 0.0, -30.0],
        X_AXIS,
        MINUS_Z_AXIS,
        "fallback",
    );
    assert_eq!(plane["fallback_frame"], "body", "{plane}");
}

#[test]
fn engine_t135_undo_of_turn_the_drawing_back_passes_through_restoring_the_text() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, TURNED_OVER_TOP_TEXT);
    let before = saved_text(&mut sides, 2);
    let message = feature_message(&opened, "f-sk2", "sketch_turned_over");
    sides.send(3, |side| {
        apply_action(side, &message, "Turn the drawing back")
    });

    let undone = sides.send(4, undo);

    assert_eq!(ok_result(&undone, "undo")["label"], "Turn the drawing back");
    assert_eq!(saved_text(&mut sides, 5), before);
}

#[test]
fn file_format_t057_a_version_1_face_sketch_after_a_move_passes_through_in_world_coordinates() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &at_version_1(&moved_box_with_a_face_sketch(None)));

    assert_eq!(ok_result(&opened, "open_document")["migrated_from"], 1);
    assert_level_placement(
        &sketch_solution(&opened, "f-sk2")["placement"],
        [0.0, 0.0, 10.0],
        "placement",
    );
    assert_eq!(feature_status(&opened, "f-hole1")["status"], "warning");
    feature_message(&opened, "f-hole1", "hole_misses_body");
}

#[test]
fn file_format_t057_a_version_1_face_sketch_after_a_move_passes_through_saved_at_version_2() {
    let version_2 = moved_box_with_a_face_sketch(None);
    let mut sides = SideBySide::new();
    sides.open(1, &at_version_1(&version_2));

    let saved = sides.send(2, save_document);

    assert_eq!(
        ok_result(&saved, "save_document")["text"],
        canonical(&version_2)
    );
}

#[test]
fn file_format_t057_a_body_fallback_after_a_move_passes_through_with_the_hole_on_the_moved_face() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &moved_box_with_a_face_sketch(Some("body")));

    assert_level_placement(
        &sketch_solution(&opened, "f-sk2")["placement"],
        [100.0, 0.0, 10.0],
        "placement",
    );
    assert_ok_without_messages(&opened, "f-hole1");
}

// Newcomer errors and document units (the reference stability plan, section 10.5). On the Emscripten target, the
// floating join and the fillet too large run the kernel's solid counts and widths across edges in WebAssembly
// (kernel.md K1 and K2).

/// The label of NC01's fix, which joins the open ends 0.3 mm apart.
const CONNECT_THE_ENDS: &str = "Connect the ends (0.3 mm apart)";

/// The label of NC04's first fix, which moves the floating boss into a body of its own.
const PUT_IT_IN_A_NEW_BODY: &str = "Put it in a new body";

/// The label of NC04's second fix, which stands the boss on the block.
const ATTACH_TO_THE_TOP: &str = "Attach 'Sketch 2' to the top of 'Extrude 1'";

/// The label of the units change of NC09.
const CHANGE_UNITS_TO_IN: &str = "Change units to in";

#[test]
fn engine_t126_nc01_a_profile_finding_passes_through_with_connect_the_ends() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &open_outline());

    let solution = sketch_solution(&opened, "f-sk1");
    assert_eq!(solution["regions"], json!([]), "{solution}");
    let finding = only_solution_message(&opened, "f-sk1");
    assert_message(
        &finding,
        "open_endpoint",
        "The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.",
        &[CONNECT_THE_ENDS],
    );
    assert_eq!(finding["field"], "/profile_findings/0");
    assert_eq!(
        finding["actions"][0]["run"],
        json!({"kind": "apply", "fix": {
            "feature": "f-sk1", "code": "open_endpoint", "field": "/profile_findings/0",
            "cause": false, "index": 0, "revision": 0
        }})
    );
}

#[test]
fn engine_t126_nc01_sketch_solutions_keep_messages_after_profile_findings_through_ps_call() {
    // Protocol.md test 80: a solution writes `messages` after `profile_findings`.
    let mut sides = SideBySide::new();
    sides.open(1, &open_outline());

    let solved = sides.send_raw(2, |side| solve_sketch(side, "f-sk1"));

    let keys = keys_of(&wire_response(&solved)["result"]["solution"]);
    assert_eq!(
        in_wire_order(&keys, &["profile_findings", "messages"]),
        ["profile_findings", "messages"],
        "{keys:?}"
    );
}

#[test]
fn engine_t126_nc01_a_drag_frame_passes_through_without_finding_actions() {
    let mut sides = SideBySide::new();
    sides.open(1, &open_outline());

    let dragged = sides.send(2, |side| {
        json!({"type": "drag_sketch", "doc": side.doc, "feature": "f-sk1", "point": "e-2.end", "to": [80, 50]})
    });

    let finding = only_message_of(&ok_result(&dragged, "drag_sketch")["solution"]);
    assert_eq!(finding["code"], "open_endpoint", "{finding}");
    assert!(
        finding.get("actions").is_none(),
        "a drag frame offers no fix: {finding}"
    );
}

#[test]
fn engine_t131_nc01_apply_fix_of_connect_the_ends_passes_through_with_one_region() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &open_outline());
    let finding = only_solution_message(&opened, "f-sk1");

    let applied = sides.send_generating(
        2,
        |side| apply_action(side, &finding, CONNECT_THE_ENDS),
        constraints_of_sketch_1,
    );

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    let constraints = &changed_record(&applied, "f-sk1")["constraints"];
    assert_eq!(
        constraints.as_array().map(Vec::len),
        Some(1),
        "{constraints}"
    );
    assert_eq!(constraints[0]["type"], "coincident", "{constraints}");
    assert_eq!(
        constraints[0]["points"],
        json!(["e-4.end", "e-1.start"]),
        "{constraints}"
    );
    let solution = sketch_solution(&applied, "f-sk1");
    assert_eq!(
        solution["regions"].as_array().map(Vec::len),
        Some(1),
        "{solution}"
    );
    assert_eq!(solution["messages"], json!([]), "{solution}");
}

#[test]
fn engine_t131_nc01_undo_of_connect_the_ends_passes_through_restoring_the_text() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &open_outline());
    let before = saved_text(&mut sides, 2);
    let finding = only_solution_message(&opened, "f-sk1");
    sides.send_generating(
        3,
        |side| apply_action(side, &finding, CONNECT_THE_ENDS),
        constraints_of_sketch_1,
    );

    let undone = sides.send(4, undo);

    assert_eq!(ok_result(&undone, "undo")["label"], CONNECT_THE_ENDS);
    assert_eq!(saved_text(&mut sides, 5), before);
}

#[test]
fn engine_t126_nc03_a_cut_that_points_away_passes_through_with_turn_on_reverse() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &circle_above_the_block("cut"));

    assert_message(
        &only_message(&opened, "f-ext2"),
        "no_effect",
        "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.",
        &["Turn on Reverse"],
    );
}

#[test]
fn engine_t126_nc04_a_floating_join_passes_through_with_a_new_body_and_attach() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &circle_above_the_block("join"));

    assert_eq!(feature_status(&opened, "f-ext2")["status"], "warning");
    assert_message(
        &only_message(&opened, "f-ext2"),
        "separate_solid",
        "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.",
        &[PUT_IT_IN_A_NEW_BODY, ATTACH_TO_THE_TOP],
    );
}

/// The bodies of the frame's recipe besides `b-1`: the one `Put it in a new body` adds to a document of `b-1` alone.
fn bodies_besides_body_1(frame: &ResponseFrame) -> Vec<String> {
    ids_of(&changed_recipe(frame)["bodies"])
        .into_iter()
        .filter(|body| body != "b-1")
        .collect()
}

#[test]
fn engine_t127_nc04_apply_fix_of_put_it_in_a_new_body_passes_through_with_body_2() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &circle_above_the_block("join"));
    let warning = only_message(&opened, "f-ext2");

    let applied = sides.send_generating(
        2,
        |side| apply_action(side, &warning, PUT_IT_IN_A_NEW_BODY),
        bodies_besides_body_1,
    );

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    let bodies = &changed_recipe(&applied)["bodies"];
    assert_eq!(bodies.as_array().map(Vec::len), Some(2), "{bodies}");
    assert_eq!(bodies[1]["name"], "Body 2", "{bodies}");
    let boss = changed_record(&applied, "f-ext2");
    assert_eq!(boss["body"], bodies[1]["id"], "{boss}");
    assert_eq!(boss["op"], "new", "{boss}");
    assert_ok_without_messages(&applied, "f-ext1");
    assert_ok_without_messages(&applied, "f-ext2");
}

#[test]
fn engine_t127_nc04_apply_fix_of_attach_passes_through_with_the_sketch_on_the_top() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &circle_above_the_block("join"));
    let warning = only_message(&opened, "f-ext2");

    let applied = sides.send(2, |side| apply_action(side, &warning, ATTACH_TO_THE_TOP));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sk2");
    assert_ok_without_messages(&applied, "f-ext2");
    let plane = record(&mut sides, 3, "f-sk2")["plane"].clone();
    assert_eq!(plane["kind"], "face", "{plane}");
    assert_eq!(plane["body"], "b-1", "{plane}");
    assert_eq!(plane["face"]["q"], "f-ext1/end-cap", "{plane}");
    assert_level_placement(&plane["fallback"], [0.0, 0.0, 30.0], "fallback");
    assert_eq!(plane["fallback_frame"], "body", "{plane}");
}

#[test]
fn engine_t150_nc05_a_fillet_too_large_passes_through_naming_the_wall_that_limits_it() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &fillet_on_a_low_step());

    assert_eq!(feature_status(&opened, "f-fil1")["status"], "error");
    assert_message(
        &only_message(&opened, "f-fil1"),
        "fillet_too_large",
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.",
        &["Use 2.3 mm"],
    );
}

#[test]
fn engine_t127_nc05_apply_fix_of_use_2_3_mm_passes_through_with_the_fillet_built() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &fillet_on_a_low_step());
    let error = only_message(&opened, "f-fil1");

    let applied = sides.send(2, |side| apply_action(side, &error, "Use 2.3 mm"));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-fil1");
    assert_eq!(record(&mut sides, 3, "f-fil1")["radius"], "2.3 mm");
}

#[test]
fn engine_t126_nc07_a_constraint_conflict_passes_through_with_delete_and_make_a_reference() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &conflicting_dimensions());

    assert_eq!(feature_status(&opened, "f-sk1")["status"], "error");
    assert_message(
        &only_message(&opened, "f-sk1"),
        "sketch_conflict",
        "In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.",
        &[
            "Delete Distance 30 mm",
            "Make Distance 30 mm a reference",
            "Delete Distance 40 mm",
            "Make Distance 40 mm a reference",
        ],
    );
}

#[test]
fn engine_t150_nc07_the_extrude_of_a_conflicting_sketch_passes_through_naming_its_root() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &conflicting_dimensions());

    let upstream = only_message(&opened, "f-ext1");
    assert_message(
        &upstream,
        "upstream_failed",
        "'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first.",
        &["Show 'Sketch 1'"],
    );
    assert_eq!(upstream["root"], "f-sk1", "{upstream}");
}

#[test]
fn engine_t127_nc07_apply_fix_of_delete_passes_through_without_the_dimension() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &conflicting_dimensions());
    let conflict = only_message(&opened, "f-sk1");

    let applied = sides.send(2, |side| {
        apply_action(side, &conflict, "Delete Distance 30 mm")
    });

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sk1");
    assert_ok_without_messages(&applied, "f-ext1");
    assert_eq!(
        ids_of(&record(&mut sides, 3, "f-sk1")["constraints"]),
        ["c-1"]
    );
}

#[test]
fn engine_t127_nc07_apply_fix_of_make_a_reference_passes_through_with_the_dimension_not_driving() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &conflicting_dimensions());
    let conflict = only_message(&opened, "f-sk1");

    let applied = sides.send(2, |side| {
        apply_action(side, &conflict, "Make Distance 30 mm a reference")
    });

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_eq!(feature_status(&applied, "f-sk1")["status"], "ok");
    assert_eq!(feature_status(&applied, "f-ext1")["status"], "ok");
    let constraints = record(&mut sides, 3, "f-sk1")["constraints"].clone();
    assert_eq!(ids_of(&constraints), ["c-1", "c-2"]);
    assert_eq!(constraints[1]["driving"], false, "{constraints}");
}

#[test]
fn engine_t126_nc10_a_misspelled_parameter_passes_through_with_its_near_match() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &misspelled_parameter());

    assert_eq!(feature_status(&opened, "f-ext1")["status"], "error");
    assert_message(
        &only_message(&opened, "f-ext1"),
        "expression",
        "Distance: 'hieght' is not a parameter. Did you mean 'height'?",
        &["Replace hieght with height"],
    );
}

#[test]
fn engine_t127_nc10_apply_fix_of_replace_passes_through_with_the_parameter() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &misspelled_parameter());
    let error = only_message(&opened, "f-ext1");

    let applied = sides.send(2, |side| {
        apply_action(side, &error, "Replace hieght with height")
    });

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-ext1");
    assert_eq!(record(&mut sides, 3, "f-ext1")["extent"]["expr"], "height");
}

#[test]
fn engine_t131_an_unknown_setting_passes_through_with_rename_key() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &misspelled_shell_setting());

    let warning = only_message(&opened, "f-sh1");
    assert_eq!(warning["severity"], "warning", "{warning}");
    assert_message(
        &warning,
        "unknown_params",
        "'remove_face' is not a Shell setting, so it is ignored. Did you mean Faces to remove?",
        &["Rename remove_face to remove_faces"],
    );
}

#[test]
fn engine_t131_apply_fix_of_rename_key_passes_through_with_the_key_renamed() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, &misspelled_shell_setting());
    let warning = only_message(&opened, "f-sh1");

    let applied = sides.send(2, |side| {
        apply_action(side, &warning, "Rename remove_face to remove_faces")
    });

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sh1");
    let shell = record(&mut sides, 3, "f-sh1");
    assert!(shell.get("remove_face").is_none(), "{shell}");
    assert_eq!(
        shell["remove_faces"].as_array().map(Vec::len),
        Some(1),
        "{shell}"
    );
}

#[test]
fn engine_t145_document_changed_passes_through_with_bare_numbers() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &numbers_without_units());

    let changed = document_changed(&opened);
    assert_eq!(changed["bare_numbers"], 4, "{changed}");
}

#[test]
fn engine_t145_document_changed_writes_bare_numbers_last_through_ps_call() {
    // Protocol.md test 80: `bare_numbers` is the last key of `document_changed`.
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());

    let switched = sides.send_raw(2, |side| set_document_units(side, "in", true));

    let keys = keys_of(&wire_event(&switched, "document_changed"));
    assert_eq!(
        keys.last().map(String::as_str),
        Some("bare_numbers"),
        "{keys:?}"
    );
}

#[test]
fn engine_t145_set_document_units_with_keep_sizes_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());

    let switched = sides.send(2, |side| set_document_units(side, "in", true));

    assert_eq!(
        ok_result(&switched, "set_document_units"),
        json!({"kind": "ok", "type": "set_document_units", "revision": 1, "kept": 4})
    );
    let changed = document_changed(&switched);
    assert_eq!(changed["document"]["units"], "in", "{changed}");
    assert_eq!(changed["bare_numbers"], 0, "{changed}");
    assert_eq!(changed["undo"], CHANGE_UNITS_TO_IN, "{changed}");
    assert_eq!(
        changed_record(&switched, "f-box1")["size"],
        json!(["40 mm", "20 mm", "10 mm"])
    );
    assert_eq!(changed_record(&switched, "f-fil1")["radius"], "2 mm");
    assert_ok_without_messages(&switched, "f-fil1");
}

#[test]
fn engine_t145_set_document_units_without_keep_sizes_passes_through_with_the_numbers_in_inches() {
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());

    let switched = sides.send(2, |side| set_document_units(side, "in", false));

    assert_eq!(
        ok_result(&switched, "set_document_units"),
        json!({"kind": "ok", "type": "set_document_units", "revision": 1, "kept": 0})
    );
    assert_eq!(document_changed(&switched)["bare_numbers"], 4);
    assert_eq!(
        changed_record(&switched, "f-box1")["size"],
        json!(["40", "20", "10"])
    );
    assert_ok_without_messages(&switched, "f-fil1");
}

#[test]
fn engine_t145_a_request_without_keep_sizes_passes_through_keeping_no_sizes() {
    // Protocol.md test 76: `keep_sizes` is false when absent.
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());

    let switched = sides.send(
        2,
        |side| json!({"type": "set_document_units", "doc": side.doc, "units": "in"}),
    );

    assert_eq!(ok_result(&switched, "set_document_units")["kept"], 0);
    assert_eq!(document_changed(&switched)["bare_numbers"], 4);
}

#[test]
fn engine_t145_set_document_units_to_the_current_units_passes_through_without_events() {
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());

    let same = sides.send(2, |side| set_document_units(side, "mm", true));

    assert_eq!(
        ok_result(&same, "set_document_units"),
        json!({"kind": "ok", "type": "set_document_units", "revision": 0, "kept": 0})
    );
    assert!(same.events.is_empty(), "nothing changes: {:?}", same.events);
}

#[test]
fn engine_t145_undo_of_set_document_units_passes_through_restoring_the_text() {
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());
    let before = saved_text(&mut sides, 2);
    sides.send(3, |side| set_document_units(side, "in", true));

    let undone = sides.send(4, undo);

    assert_eq!(ok_result(&undone, "undo")["label"], CHANGE_UNITS_TO_IN);
    assert_eq!(document_changed(&undone)["bare_numbers"], 4);
    assert_eq!(saved_text(&mut sides, 5), before);
}

#[test]
fn engine_t145_set_document_units_in_a_transaction_passes_through_joining_it() {
    let mut sides = SideBySide::new();
    sides.open(1, &numbers_without_units());
    sides.send(2, |side| begin_transaction(side, "Units"));

    let switched = sides.send(3, |side| set_document_units(side, "in", true));

    assert_eq!(ok_result(&switched, "set_document_units")["kept"], 4);
    let changed = document_changed(&switched);
    assert!(
        changed.get("undo").is_none(),
        "no undo entry of its own: {changed}"
    );
    assert_eq!(changed["transaction"], "Units", "{changed}");
}

#[test]
fn engine_t150_nc09_a_bare_radius_too_large_in_inches_passes_through_with_use_2_mm() {
    let mut sides = SideBySide::new();
    sides.open(1, &cube_with_a_bare_radius());

    let switched = sides.send(2, |side| set_document_units(side, "in", false));

    let error = only_message(&switched, "f-fil1");
    assert_eq!(error["code"], "fillet_too_large", "{error}");
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
    assert_eq!(labels(&error).first(), Some(&"Use 2 mm"), "{error}");
}

#[test]
fn engine_t150_nc09_apply_fix_of_use_2_mm_passes_through_with_the_fillet_built() {
    let mut sides = SideBySide::new();
    sides.open(1, &cube_with_a_bare_radius());
    let switched = sides.send(2, |side| set_document_units(side, "in", false));
    let error = only_message(&switched, "f-fil1");

    let applied = sides.send(3, |side| apply_action(side, &error, "Use 2 mm"));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 2);
    assert_ok_without_messages(&applied, "f-fil1");
    assert_eq!(record(&mut sides, 4, "f-fil1")["radius"], "2 mm");
}

// Causes, ghosts and uses (the reference stability plan, section 10.6). Each corpus setup is opened as written, without
// picks, and its edit is applied as one `apply_edits` labeled `Edit`, so the actions it offers carry revision 1 (R26's
// delete, a second `apply_edits`, makes revision 2). On the Emscripten target, ghost capture runs the kernel's display
// geometry of single elements in WebAssembly (kernel.md K3).

/// The label of the cause action of corpus R26 that stores the placement the sketch was built with.
const KEEP_THE_SKETCH_WHERE_IT_WAS: &str = "Keep the sketch where it was";

/// The cause line of every message newcomer error NC11 gives.
const NC11_CAUSE: &str =
    "This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.";

#[test]
fn engine_t136_r12_a_cause_passes_through_with_set_distance_back_and_undo() {
    let mut sides = SideBySide::new();

    let edited = after_edit(&mut sides, &r12());

    let message = first_message(&edited, "f-fil1");
    let cause = assert_cause(
        &message,
        "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
        Some("f-ext2"),
        &[
            ("Set Distance of 'Extrude 2' back to 5 mm", "apply"),
            ("Undo", "undo"),
        ],
    );
    assert_eq!(
        actions(&cause)[0]["run"],
        json!({"kind": "apply", "fix": {
            "feature": "f-fil1", "code": message["code"], "field": message["field"],
            "cause": true, "index": 0, "revision": 1
        }})
    );
    assert!(
        cause.get("ghost").is_none(),
        "a field change has no ghost: {cause}"
    );
}

#[test]
fn engine_t136_r12_a_message_writes_its_cause_right_after_its_actions_through_ps_call() {
    // Protocol.md test 72: a diagnostic writes `cause` after `actions`, and a cause `text`, `feature`, then `actions`.
    let mut sides = SideBySide::new();
    let scenario = r12();
    sides.open(1, &scenario.setup);

    let edited = sides.send_raw(2, |side| apply_edits(side, EDIT_LABEL, &scenario.edits));

    let status = wire_status(&edited, "f-fil1");
    let message = &status["messages"][0];
    let keys = keys_of(message);
    let actions_at = keys.iter().position(|key| key == "actions");
    let cause_at = keys.iter().position(|key| key == "cause");
    assert!(
        actions_at.is_some() && cause_at == actions_at.map(|position| position + 1),
        "{keys:?}"
    );
    assert_eq!(keys_of(&message["cause"]), ["text", "feature", "actions"]);
}

#[test]
fn engine_t136_r12_apply_fix_of_set_distance_back_passes_through_giving_back_the_text() {
    let mut sides = SideBySide::new();
    let scenario = r12();
    sides.open(1, &scenario.setup);
    let before = saved_text(&mut sides, 2);
    let edited = sides.send(3, |side| apply_edits(side, EDIT_LABEL, &scenario.edits));
    let revert = actions(cause_of(&first_message(&edited, "f-fil1")))[0].clone();

    let applied = sides.send(4, |side| apply_fix(side, &revert["run"]["fix"]));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 2);
    assert_ok_without_messages(&applied, "f-fil1");
    assert_eq!(saved_text(&mut sides, 5), before);
}

#[test]
fn engine_t136_r16_a_cause_of_an_inserted_cut_passes_through_with_suppress_and_undo() {
    let mut sides = SideBySide::new();

    let edited = after_edit(&mut sides, &r16());

    // The added sketch reaches the fillet only through the added extrude, so it is not counted.
    assert_cause(
        &first_message(&edited, "f-fil1"),
        "This started after you added 'Extrude 1' before this fillet.",
        Some("f-ext2"),
        &[("Suppress 'Extrude 1'", "apply"), ("Undo", "undo")],
    );
}

/// The cause line of corpus R26 after its delete: the boss's extrude is 'Extrude 1' now.
const R26_CAUSE: &str = "This started after you deleted the former 'Extrude 1'.";

#[test]
fn engine_t136_r26_a_plane_fallback_passes_through_with_keep_the_sketch_where_it_was() {
    let mut sides = SideBySide::new();

    let deleted = r26_after_the_delete(&mut sides);

    let cause = assert_cause(
        &feature_message(&deleted, "f-sk2", "plane_fallback"),
        R26_CAUSE,
        Some("f-ext1"),
        &[(KEEP_THE_SKETCH_WHERE_IT_WAS, "apply"), ("Undo", "undo")],
    );
    assert_eq!(
        actions(&cause)[0]["run"],
        json!({"kind": "apply", "fix": {
            "feature": "f-sk2", "code": "plane_fallback", "field": "/plane/face",
            "cause": true, "index": 0, "revision": 2
        }})
    );
}

#[test]
fn engine_t136_r26_apply_fix_of_keep_the_sketch_where_it_was_passes_through_with_the_fallback_in_body_coordinates()
 {
    let mut sides = SideBySide::new();
    let deleted = r26_after_the_delete(&mut sides);
    let warning = feature_message(&deleted, "f-sk2", "plane_fallback");
    let keep = action_labeled(cause_of(&warning), KEEP_THE_SKETCH_WHERE_IT_WAS);

    let applied = sides.send(4, |side| apply_fix(side, &keep["run"]["fix"]));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 3);
    // The placement the sketch was built with before the delete, on the 80 tall block's top.
    let plane = record(&mut sides, 5, "f-sk2")["plane"].clone();
    assert_level_placement(&plane["fallback"], [0.0, 0.0, 80.0], "fallback");
    assert_eq!(plane["fallback_frame"], "body", "{plane}");
}

#[test]
fn engine_t141_r16_an_element_ghost_passes_through_with_the_lost_edge_in_its_buffer() {
    let mut sides = SideBySide::new();

    let edited = after_edit(&mut sides, &r16());

    let repair = feature_repair(&edited, "f-fil1", "/edges/0");
    let ghost = &repair["ghost"];
    assert_eq!(ghost["frame"], "world", "{repair}");
    assert_ghost_segments(
        &edited,
        ghost,
        &[([-10.0, -10.0, 20.0], [10.0, -10.0, 20.0])],
    );
}

#[test]
fn engine_t142_nc11_a_region_ghost_passes_through_with_the_outline_in_its_buffer() {
    let mut sides = SideBySide::new();

    let edited = after_edit(&mut sides, &nc11());

    let repair = feature_repair(&edited, "f-ext1", "/profile/regions/0");
    let ghost = &repair["ghost"];
    assert_eq!(ghost["frame"], "sketch", "{repair}");
    assert_ghost_segments(
        &edited,
        ghost,
        &[
            ([0.0, 0.0], [80.0, 0.0]),
            ([80.0, 0.0], [80.0, 50.0]),
            ([80.0, 50.0], [0.0, 50.0]),
            ([0.0, 50.0], [0.0, 0.0]),
        ],
    );
}

#[test]
fn engine_t142_nc11_a_cause_ghost_passes_through_with_the_deleted_line_in_its_buffer() {
    let mut sides = SideBySide::new();

    let edited = after_edit(&mut sides, &nc11());

    let cause = assert_cause(
        &feature_message(&edited, "f-ext1", "profile_not_found"),
        NC11_CAUSE,
        Some("f-sk1"),
        &[("Undo", "undo"), ("Show the line", "show")],
    );
    assert_eq!(
        actions(&cause)[1]["run"],
        json!({"kind": "show", "feature": "f-sk1", "entity": "e-3"})
    );
    let ghost = &cause["ghost"];
    assert_eq!(ghost["frame"], "sketch", "{cause}");
    assert_ghost_segments(&edited, ghost, &[([80.0, 50.0], [0.0, 50.0])]);
}

#[test]
fn engine_t142_nc11_an_empty_body_passes_through_with_its_root_and_the_same_cause() {
    let mut sides = SideBySide::new();

    let edited = after_edit(&mut sides, &nc11());

    let empty = feature_message(&edited, "f-fil1", "body_empty");
    assert_eq!(empty["root"], "f-ext1", "{empty}");
    assert_eq!(cause_of(&empty)["text"], NC11_CAUSE);
}

#[test]
fn engine_t142_nc11_ghosts_are_written_last_through_ps_call() {
    // Protocol.md tests 68 and 72: a repair and a cause each write `ghost` after their other keys.
    let mut sides = SideBySide::new();
    let scenario = nc11();
    sides.open(1, &scenario.setup);

    let edited = sides.send_raw(2, |side| apply_edits(side, EDIT_LABEL, &scenario.edits));

    let status = wire_status(&edited, "f-ext1");
    let repair = entry_of(&status, "repairs", "path", "/profile/regions/0");
    assert_eq!(
        keys_of(&repair).last().map(String::as_str),
        Some("ghost"),
        "{repair}"
    );
    let message = entry_of(&status, "messages", "code", "profile_not_found");
    assert_eq!(
        keys_of(&message["cause"]),
        ["text", "feature", "actions", "ghost"]
    );
}

/// The `list_uses` command for the side's document.
fn list_uses(side: &Side) -> Value {
    json!({"type": "list_uses", "doc": side.doc})
}

#[test]
fn engine_t143_list_uses_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    let right_side_plane = json!({
        "kind": "face", "body": "b-1",
        "face": {"q": "f-ext1/lateral/e-2", "fp": "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4"},
        "fallback": {"origin": [80, 0, 0], "x_axis": [0, 1, 0], "normal": [1, 0, 0]}
    });
    sides.open(
        1,
        &block_with_a_back_top_fillet(vec![sketch(
            "f-sk2",
            right_side_plane,
            vec![circle("e-5", [25.0, 15.0], 4.0)],
            Vec::new(),
        )]),
    );

    let listed = sides.send(2, list_uses);

    assert_eq!(
        ok_result(&listed, "list_uses")["uses"],
        json!([
            {"user": "f-ext1", "field": "/profile/sketch", "feature": "f-sk1"},
            {"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1"},
            {"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1", "entity": "e-3"},
            {"user": "f-sk2", "field": "/plane/face", "feature": "f-ext1", "entity": "e-2"}
        ])
    );
    assert!(listed.events.is_empty(), "list_uses emits nothing");
}
