//! The protocol passes through `ps_call` unchanged (protocol.md section 2.10; the reference stability plan, sections 10.3 item 9 and 10.4). The bridge answers the diagnostics, fixes and errors of ADR 0024, the documents of format version 2 (ADR 0025) and the frames on the model of ADR 0023 (body-coordinate fallbacks and axis directions in descriptors, drag commits, `Attach` and `Turn the drawing back`) with the response frame that `Session::call` gives a session of the module's options (display on, random ids): nothing is added, dropped or reordered on the way through.
//!
//! Each test sends its requests through `ps_call` (the `Block` helper) and to a reference `Session`, and compares the decoded frames. Document handles and mesh ids count per session, and on the Emscripten target every test shares the module's one session, so the comparison leaves those two numbers out, and a command that names a document or a mesh, such as a pick, names each side's own; everything else, buffers included, must be equal. Each test also checks the new shape it is about in the bridge's frame, so an empty answer on both sides cannot pass.
//!
//! A test named `engine_tNNN` sends the documents and requests of engine.md section 6 test NNN, and one named `file_format_tNNN` those of file-format.md section 6 test NNN; the bridge's own tests are protocol.md's (`tNNN` in the other test files).

mod common;

use std::collections::BTreeMap;

use common::block::Block;
use common::{error_info, ok_result, outcome_json, request_frame, response_frame};
use partsmith_engine::{Session, SessionOptions};
use partsmith_model::{FORMAT_VERSION, load, save};
use partsmith_protocol::{ErrorCode, ResponseFrame};
use serde_json::{Value, json};

/// File-format.md example E10: every field format version 2 adds (`nb`, `merged`, `outline`, `fallback_frame` and `direction`), exactly as `save` writes it.
const E10_TEXT: &str = include_str!("../../partsmith-model/tests/fixtures/e10_version_2.ptsm");

/// The line of canonical text at format version 2, E10 among them, that holds its format version.
const VERSION_2_LINE: &str = "  \"format_version\": 2,";

/// CUBE10 with a linear pattern of 2.5 copies along X (features/linear_pattern.md test 6): `f-lp1` fails with `not_whole_number` and offers `Use 2` and `Use 3` (engine.md test 126).
const FRACTIONAL_PATTERN_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
    {
      "id": "f-lp1",
      "type": "linear_pattern",
      "body": "b-1",
      "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "2.5", "spacing": "20"}
    }
  ]
}
"#;

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

/// Corpus R16 after its edit (engine.md test 115): CUBE20 with a 5 x 5 rabbet ('Sketch 1' and 'Extrude 1') inserted before the fillet of its front top edge, so `f-fil1` fails with `references_not_found` and an element repair whose candidates are the rabbet's edges.
const CUT_AWAY_EDGE_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"},
    {
      "id": "f-sk2",
      "type": "sketch",
      "plane": {"kind": "origin", "plane": "YZ"},
      "entities": [
        {"id": "e-1", "type": "line", "start": [-11, 15], "end": [-5, 15]},
        {"id": "e-2", "type": "line", "start": [-5, 15], "end": [-5, 21]},
        {"id": "e-3", "type": "line", "start": [-5, 21], "end": [-11, 21]},
        {"id": "e-4", "type": "line", "start": [-11, 21], "end": [-11, 15]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext2",
      "type": "extrude",
      "body": "b-1",
      "profile": {"sketch": "f-sk2", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=-8,18|a=36"}]},
      "extent": {"kind": "symmetric", "expr": "30"},
      "op": "cut"
    },
    {
      "id": "f-fil1",
      "type": "fillet",
      "body": "b-1",
      "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"}],
      "radius": "2"
    }
  ]
}
"#;

/// The edge of E10's fillet: the front edge of the right part of the block's top, which the slot splits (engine.md test 122).
const SPLIT_TOP_FRONT_EDGE: &str = "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)";

/// The keys whose values count per session, so a fresh session and the module's shared one give them different values.
const SESSION_NUMBERS: [&str; 2] = ["doc", "mesh_id"];

/// The document one side opened last, with what that side's frames said about it.
struct Side {
    /// The document's handle in that side's session.
    doc: Value,
    /// The mesh id of each body's model layer in the latest `body_mesh` event of that body.
    model_meshes: BTreeMap<String, Value>,
}

impl Side {
    /// The document `frame`, a successful `open_document` response, opened.
    fn opened(frame: &ResponseFrame) -> Side {
        let mut side = Side {
            doc: handle(frame),
            model_meshes: BTreeMap::new(),
        };
        side.record_meshes(frame);
        side
    }

    /// Keeps the model mesh ids of the `body_mesh` events in `frame`.
    fn record_meshes(&mut self, frame: &ResponseFrame) {
        for mesh in events_of(frame, "body_mesh") {
            if mesh["layer"] == "model" {
                let body = mesh["body"].as_str().expect("meshes name their body");
                self.model_meshes
                    .insert(body.to_owned(), mesh["mesh_id"].clone());
            }
        }
    }

    /// The mesh id of `body`'s model layer as this side displays it now.
    fn model_mesh(&self, body: &str) -> &Value {
        self.model_meshes
            .get(body)
            .unwrap_or_else(|| panic!("{body} has a model mesh"))
    }
}

/// The bridge and a reference session with the module's session options, sent the same requests.
struct SideBySide {
    /// The session the bridge's answers are compared with.
    reference: Session,
    /// The document each side opened last: the bridge's, then the reference session's.
    documents: Option<(Side, Side)>,
    /// The bridge's latest response frame, as `ps_call` returned it.
    bridge_bytes: Vec<u8>,
}

impl SideBySide {
    /// A fresh reference session with the options `main` gives the module's session (protocol.md section 2.10).
    fn new() -> SideBySide {
        SideBySide {
            reference: Session::new(SessionOptions {
                display: true,
                ..SessionOptions::default()
            }),
            documents: None,
            bridge_bytes: Vec::new(),
        }
    }

    /// Opens `text` on both sides with request `id`, keeps both documents when it opens, and returns the bridge's frame.
    fn open(&mut self, id: u64, text: &str) -> ResponseFrame {
        let request = request_frame(
            id,
            &json!({"type": "open_document", "text": text, "assets": []}).to_string(),
        );

        let (bridge, reference) = self.exchange(&request, &request);

        if outcome_json(&bridge)["kind"] == "ok" {
            self.documents = Some((Side::opened(&bridge), Side::opened(&reference)));
        }
        bridge
    }

    /// Sends request `id` with the command `command` builds for each side's document, and returns the bridge's frame.
    fn send(&mut self, id: u64, command: impl Fn(&Side) -> Value) -> ResponseFrame {
        let (bridge_side, reference_side) = self
            .documents
            .as_ref()
            .expect("a document is open on both sides");
        let bridge_request = request_frame(id, &command(bridge_side).to_string());
        let reference_request = request_frame(id, &command(reference_side).to_string());

        let (bridge, reference) = self.exchange(&bridge_request, &reference_request);

        if let Some((bridge_side, reference_side)) = &mut self.documents {
            bridge_side.record_meshes(&bridge);
            reference_side.record_meshes(&reference);
        }
        bridge
    }

    /// Sends `bridge_request` through `ps_call` and `reference_request` to the reference session, asserts that the answers are equal but for the session numbers, and returns both.
    fn exchange(
        &mut self,
        bridge_request: &[u8],
        reference_request: &[u8],
    ) -> (ResponseFrame, ResponseFrame) {
        self.bridge_bytes = through_ps_call(bridge_request);
        let bridge = response_frame(&self.bridge_bytes);
        let reference = response_frame(&self.reference.call(reference_request));

        assert_eq!(
            messages_without_session_numbers(&bridge),
            messages_without_session_numbers(&reference),
            "ps_call answered differently from Session::call"
        );
        assert!(
            bridge.buffers == reference.buffers,
            "ps_call answered with other buffers than Session::call ({} and {} buffers)",
            bridge.buffers.len(),
            reference.buffers.len()
        );
        (bridge, reference)
    }
}

/// Copies `request` into a block, calls `ps_call` on it, and returns a copy of the response block.
fn through_ps_call(request: &[u8]) -> Vec<u8> {
    Block::holding(request).call().bytes()
}

/// The handle an `open_document` response returned.
fn handle(frame: &ResponseFrame) -> Value {
    ok_result(frame, "open_document")["doc"].clone()
}

/// The events and the response of a frame as JSON, with every value of a [`SESSION_NUMBERS`] key set to null.
fn messages_without_session_numbers(frame: &ResponseFrame) -> Value {
    let mut messages = json!({
        "events": serde_json::to_value(&frame.events).expect("events always serialize"),
        "response": serde_json::to_value(&frame.response).expect("responses always serialize"),
    });
    forget_session_numbers(&mut messages);
    messages
}

/// Sets every value of a [`SESSION_NUMBERS`] key inside `value` to null.
fn forget_session_numbers(value: &mut Value) {
    match value {
        Value::Object(members) => {
            for (key, member) in members.iter_mut() {
                if SESSION_NUMBERS.contains(&key.as_str()) {
                    *member = Value::Null;
                } else {
                    forget_session_numbers(member);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(forget_session_numbers),
        _ => {}
    }
}

/// The frame's events of type `event_type`, as JSON.
fn events_of(frame: &ResponseFrame, event_type: &str) -> Vec<Value> {
    frame
        .events
        .iter()
        .map(|event| serde_json::to_value(event).expect("events always serialize"))
        .filter(|event| event["type"] == event_type)
        .collect()
}

/// The status of `feature` in the frame's `recomputed` event.
fn feature_status(frame: &ResponseFrame, feature: &str) -> Value {
    events_of(frame, "recomputed")
        .iter()
        .flat_map(|event| {
            event["features"]
                .as_array()
                .expect("recomputed lists its features")
        })
        .find(|status| status["id"] == feature)
        .unwrap_or_else(|| panic!("the frame has no recomputed status of {feature}"))
        .clone()
}

/// The first entry of the list `list` of `feature`'s status in the frame's `recomputed` event whose `key` is `value`.
fn status_entry(frame: &ResponseFrame, feature: &str, list: &str, key: &str, value: &str) -> Value {
    let status = feature_status(frame, feature);
    status[list]
        .as_array()
        .unwrap_or_else(|| panic!("statuses list their {list}: {status}"))
        .iter()
        .find(|entry| entry[key] == value)
        .unwrap_or_else(|| panic!("{feature} has none of its {list} with {key} {value}: {status}"))
        .clone()
}

/// The message with `code` among the messages of `feature` in the frame's `recomputed` event.
fn feature_message(frame: &ResponseFrame, feature: &str, code: &str) -> Value {
    status_entry(frame, feature, "messages", "code", code)
}

/// The repair at `path` among the repairs of `feature` in the frame's `recomputed` event.
fn feature_repair(frame: &ResponseFrame, feature: &str, path: &str) -> Value {
    status_entry(frame, feature, "repairs", "path", path)
}

/// The labels of a message's actions, in order.
fn labels(message: &Value) -> Vec<String> {
    message["actions"]
        .as_array()
        .unwrap_or_else(|| panic!("the message has actions: {message}"))
        .iter()
        .map(|action| {
            action["label"]
                .as_str()
                .expect("actions have labels")
                .to_owned()
        })
        .collect()
}

/// The candidates of a repair.
fn candidates(repair: &Value) -> &[Value] {
    repair["candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("the repair lists its candidates: {repair}"))
}

/// The `apply_fix` command for the `fix` reference of an `apply` action, for the side's document.
fn apply_fix(side: &Side, fix: &Value) -> Value {
    json!({"type": "apply_fix", "doc": side.doc, "fix": fix})
}

/// The `save_document` command for the side's document.
fn save_document(side: &Side) -> Value {
    json!({"type": "save_document", "doc": side.doc})
}

/// The `begin_transaction` command labeled `label` for the side's document.
fn begin_transaction(side: &Side, label: &str) -> Value {
    json!({"type": "begin_transaction", "doc": side.doc, "label": label})
}

/// The `list_elements` command for the edges of `b-1` of the side's document.
fn list_edges(side: &Side) -> Value {
    json!({"type": "list_elements", "doc": side.doc, "body": "b-1", "kind": "edge"})
}

/// The `fix` reference of `Use 3`, the second action of the fractional pattern's message, offered at revision 0.
fn use_three() -> Value {
    json!({
        "feature": "f-lp1", "code": "not_whole_number", "field": "/direction/count",
        "cause": false, "index": 1, "revision": 0
    })
}

/// The canonical text `text`, at format version 2, with its format version line set to `version`.
fn at_version(text: &str, version: u32) -> String {
    assert_eq!(
        text.matches(VERSION_2_LINE).count(),
        1,
        "the text states its format version once"
    );
    text.replace(VERSION_2_LINE, &format!("  \"format_version\": {version},"))
}

/// E10 with its format version line set to `version`.
fn e10_at_version(version: u32) -> String {
    at_version(E10_TEXT, version)
}

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

#[test]
fn a_pick_with_its_full_reference_passes_through_unchanged() {
    let mut sides = SideBySide::new();
    sides.open(1, E10_TEXT);
    sides.send(2, |side| begin_transaction(side, "Edit Fillet 1"));
    sides.send(
        3,
        |side| json!({"type": "set_focus", "doc": side.doc, "feature": "f-fil1"}),
    );
    let listed = sides.send(4, list_edges);
    let index = ok_result(&listed, "list_elements")["elements"]
        .as_array()
        .expect("list_elements lists elements")
        .iter()
        .position(|edge| edge["name"] == SPLIT_TOP_FRONT_EDGE)
        .unwrap_or_else(|| panic!("the state before the fillet has {SPLIT_TOP_FRONT_EDGE}"));

    let picked = sides.send(5, |side| {
        json!({
            "type": "pick_reference",
            "doc": side.doc,
            "pick": {"mesh_id": side.model_mesh("b-1"), "kind": "edge", "index": index}
        })
    });

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

    let listed = sides.send(2, list_edges);

    let result = ok_result(&listed, "list_elements");
    let elements = result["elements"]
        .as_array()
        .expect("list_elements lists elements");
    assert!(!elements.is_empty(), "E10's block has edges");
    for element in elements {
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

/// `box.ptsm` of the engine tests: the 20 x 30 x 40 box `f-box1` of body `b-1`, centered on the Z axis from z = 0 to 40.
const BOX_TEXT: &str = include_str!("../../partsmith-engine/tests/fixtures/box.ptsm");

/// The top of the box of `box.ptsm`.
const BOX_TOP: &str = "f-box1/top";

/// The front top edge of the box of `box.ptsm`, which runs along X before anything moves the box.
const BOX_FRONT_TOP_EDGE: &str = "edge(f-box1/front|f-box1/top)";

/// How far a coordinate in a frame may lie from the value the spec gives it.
const GEOMETRY_TOLERANCE: f64 = 1e-6;

/// The world X direction.
const X_AXIS: [f64; 3] = [1.0, 0.0, 0.0];

/// The world Z direction.
const Z_AXIS: [f64; 3] = [0.0, 0.0, 1.0];

/// The world -Z direction.
const MINUS_Z_AXIS: [f64; 3] = [0.0, 0.0, -1.0];

/// features/sketch.md test 5 after a Move body (engine.md test 135): CUBE10 moved by (100, 0, 0), then a sketch `f-sk1` with the point `e-p1` on a face the cube does not have, its fallback at (3, 4, 0) in body coordinates, so it stands at its saved position, world (103, 4, 0), and offers to attach to the cube's faces.
const LOST_FACE_ON_A_MOVED_CUBE_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
    {"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]},
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {
        "kind": "face",
        "body": "b-1",
        "face": {"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"},
        "fallback": {"origin": [3, 4, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
        "fallback_frame": "body"
      },
      "entities": [
        {"id": "e-p1", "type": "point", "position": [0, 0]}
      ],
      "constraints": []
    }
  ]
}
"#;

/// Corpus R32 (b) after its edit (features/sketch.md test 24, engine.md tests 126 and 135): BLOCK with 'Extrude 1' reversed, so its end cap is the bottom at z = -30, facing -Z, and a face sketch `f-sk2` on that cap with its world fallback at (0, 0, 30) and the point `e-p1` at (20, 10), where the hole `f-hole1` of diameter 5 and depth 10 misses the body because the drawing lies mirrored on the turned-over face.
const TURNED_OVER_TOP_TEXT: &str = r#"{
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
        {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
        {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
        {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
        {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext1",
      "type": "extrude",
      "body": "b-1",
      "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
      "extent": {"kind": "distance", "expr": "30"},
      "reverse": true,
      "op": "new"
    },
    {
      "id": "f-sk2",
      "type": "sketch",
      "plane": {
        "kind": "face",
        "body": "b-1",
        "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},
        "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
      },
      "entities": [
        {"id": "e-p1", "type": "point", "position": [20, 10]}
      ],
      "constraints": []
    },
    {
      "id": "f-hole1",
      "type": "hole",
      "body": "b-1",
      "sketch": "f-sk2",
      "points": ["e-p1"],
      "diameter": "5",
      "extent": {"kind": "distance", "expr": "10"}
    }
  ]
}
"#;

/// `box.ptsm` with `more` features after `f-box1`.
fn box_then(more: Vec<Value>) -> String {
    let mut document: Value = serde_json::from_str(BOX_TEXT).expect("box.ptsm is JSON");
    document["features"]
        .as_array_mut()
        .expect("documents list their features")
        .extend(more);
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// The Move body `f-mv1` of `b-1` by (100, 0, 0), the move of engine.md tests 124, 125 and 135.
fn moved_by_100() -> Value {
    json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]})
}

/// `box.ptsm` moved by (100, 0, 0), the document of engine.md test 124.
fn moved_box() -> String {
    box_then(vec![moved_by_100()])
}

/// The Move body `f-mv1` of `b-1` that turns it 90 degrees about the origin Z axis (engine.md test 124).
fn quarter_turn() -> Value {
    json!({
        "id": "f-mv1", "type": "move_body", "body": "b-1",
        "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}
    })
}

/// The reference of the top of `box.ptsm` moved by (100, 0, 0), as a pick stores it (engine.md test 125).
fn moved_top() -> Value {
    json!({"q": BOX_TOP, "fp": "plane|n=0,0,1|c=100,0,40|a=600|adj=4,4"})
}

/// The plane of a sketch on the face `face` of `b-1` with its fallback at `origin`, x axis (1, 0, 0) and normal (0, 0, 1) in `frame` (`None` writes no `fallback_frame`: world coordinates).
fn face_plane(face: Value, origin: [f64; 3], frame: Option<&str>) -> Value {
    let mut plane = json!({
        "kind": "face", "body": "b-1", "face": face,
        "fallback": {"origin": origin, "x_axis": X_AXIS, "normal": Z_AXIS}
    });
    if let Some(frame) = frame {
        plane["fallback_frame"] = json!(frame);
    }
    plane
}

/// The document of engine.md test 125: `box.ptsm` moved by (100, 0, 0), then a face sketch `f-sk1` on its top with one line `e-1` from (0, 0) to (10, 0) and its fallback at (0, 0, 40) in `frame` (see [`face_plane`]).
fn sketch_on_moved_top(frame: Option<&str>) -> String {
    box_then(vec![
        moved_by_100(),
        json!({
            "id": "f-sk1", "type": "sketch",
            "plane": face_plane(moved_top(), [0.0, 0.0, 40.0], frame),
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
            "constraints": []
        }),
    ])
}

/// The plane that `f-sk1` of [`sketch_on_moved_top`] in `frame` stores after a drag of `e-1.end` to (20, 5) and its commit, sent side by side, once the commit made revision 1.
fn plane_after_a_drag_commit(frame: Option<&str>) -> Value {
    let mut sides = SideBySide::new();
    sides.open(1, &sketch_on_moved_top(frame));
    sides.send(2, |side| {
        json!({"type": "drag_sketch", "doc": side.doc, "feature": "f-sk1", "point": "e-1.end", "to": [20, 5]})
    });
    let committed = sides.send(3, |side| {
        json!({"type": "end_sketch_drag", "doc": side.doc, "feature": "f-sk1", "commit": true})
    });
    assert_eq!(ok_result(&committed, "end_sketch_drag")["revision"], 1);
    record(&mut sides, 4, "f-sk1")["plane"].clone()
}

/// The timeline of file-format.md test 57, corpus R32 (a) after its edit, at format version 2: a box 40 x 40 x 10, the Move body of (100, 0, 0), a face sketch `f-sk2` on the box's top with its fallback at (0, 0, 10) in `frame` (see [`face_plane`]) and its point `e-p1` at (15, 15), and a hole `f-hole1` of diameter 4 through all at the point.
fn moved_box_with_a_face_sketch(frame: Option<&str>) -> String {
    let top = json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"});
    let plane = face_plane(top, [0.0, 0.0, 10.0], frame);
    let document = json!({
        "format": "partsmith",
        "format_version": 2,
        "units": "mm",
        "parameters": [],
        "bodies": [{"id": "b-1", "name": "Body 1"}],
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"},
            moved_by_100(),
            {
                "id": "f-sk2", "type": "sketch", "plane": plane,
                "entities": [{"id": "e-p1", "type": "point", "position": [15, 15]}],
                "constraints": []
            },
            {
                "id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2",
                "points": ["e-p1"], "diameter": "4", "extent": {"kind": "through_all"}
            }
        ]
    });
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// The canonical text `save` writes for the document `text`.
fn canonical(text: &str) -> String {
    let loaded = load(text).unwrap_or_else(|error| panic!("the document loads: {error}"));
    save(&loaded.document).unwrap_or_else(|error| panic!("the document saves: {error}"))
}

/// The canonical text of the document `text` as a file written at format version 1 holds it: only its `format_version` line differs.
fn at_version_1(text: &str) -> String {
    at_version(&canonical(text), 1)
}

/// The `list_elements` command for the faces of `b-1` of the side's document.
fn list_faces(side: &Side) -> Value {
    json!({"type": "list_elements", "doc": side.doc, "body": "b-1", "kind": "face"})
}

/// The `get_document` command for the side's document.
fn get_document(side: &Side) -> Value {
    json!({"type": "get_document", "doc": side.doc})
}

/// The descriptors of a `list_elements` frame, in their order.
fn listed(frame: &ResponseFrame) -> Vec<Value> {
    ok_result(frame, "list_elements")["elements"]
        .as_array()
        .expect("list_elements lists elements")
        .clone()
}

/// The position of the descriptor named `name` in a `list_elements` frame of one kind, which a pick of that kind names as its `index`.
fn listed_index(frame: &ResponseFrame, name: &str) -> usize {
    listed(frame)
        .iter()
        .position(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("the list has {name}"))
}

/// The element named `name` among `elements`, the descriptors of a `list_elements` result.
fn named<'a>(elements: &'a [Value], name: &str) -> &'a Value {
    elements
        .iter()
        .find(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("the list has {name}"))
}

/// The descriptor named `name` in a `list_elements` frame.
fn listed_named(frame: &ResponseFrame, name: &str) -> Value {
    named(&listed(frame), name).clone()
}

/// The record of `feature` that the bridge's document holds, read with `get_document` as request `id`.
fn record(sides: &mut SideBySide, id: u64, feature: &str) -> Value {
    let got = sides.send(id, get_document);
    ok_result(&got, "get_document")["document"]["features"]
        .as_array()
        .expect("documents list their features")
        .iter()
        .find(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has {feature}"))
        .clone()
}

/// The solution of `feature` in the frame's only `sketch_solved` event for it.
fn sketch_solution(frame: &ResponseFrame, feature: &str) -> Value {
    let solved: Vec<Value> = events_of(frame, "sketch_solved")
        .into_iter()
        .filter(|event| event["solution"]["feature"] == feature)
        .collect();
    assert_eq!(
        solved.len(),
        1,
        "the frame solves {feature} once: {solved:?}"
    );
    solved[0]["solution"].clone()
}

/// The action labeled `label` among a message's actions.
fn action_labeled(message: &Value, label: &str) -> Value {
    message["actions"]
        .as_array()
        .unwrap_or_else(|| panic!("the message has actions: {message}"))
        .iter()
        .find(|action| action["label"] == label)
        .unwrap_or_else(|| panic!("the message offers {label:?}: {message}"))
        .clone()
}

/// Asserts that `feature` has status ok and no message in the frame's `recomputed` event.
#[track_caller]
fn assert_ok_without_messages(frame: &ResponseFrame, feature: &str) {
    let status = feature_status(frame, feature);
    assert_eq!(status["status"], "ok", "{status}");
    assert_eq!(status["messages"], json!([]), "{status}");
}

/// The three components of a JSON vector.
fn vector(value: &Value) -> [f64; 3] {
    let components: Vec<f64> = value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is a vector"))
        .iter()
        .map(|component| component.as_f64().expect("vectors hold numbers"))
        .collect();
    components
        .try_into()
        .unwrap_or_else(|components: Vec<f64>| panic!("{components:?} has three components"))
}

/// Asserts that each component of the JSON vector `actual` lies within [`GEOMETRY_TOLERANCE`] of `expected`.
#[track_caller]
fn assert_vector(actual: &Value, expected: [f64; 3], what: &str) {
    let components = vector(actual);
    assert!(
        components
            .iter()
            .zip(expected)
            .all(|(component, wanted)| (component - wanted).abs() <= GEOMETRY_TOLERANCE),
        "{what} is {components:?}, not {expected:?}"
    );
}

/// Asserts that the JSON placement `placement` has `origin`, `x_axis` and `normal`.
#[track_caller]
fn assert_placement(
    placement: &Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
    what: &str,
) {
    assert_vector(&placement["origin"], origin, &format!("{what} origin"));
    assert_vector(&placement["x_axis"], x_axis, &format!("{what} x axis"));
    assert_vector(&placement["normal"], normal, &format!("{what} normal"));
}

/// The header of a frame as its bytes hold it, each object's keys in the order the frame writes them (the workspace builds `serde_json` with `preserve_order`).
fn wire_header(frame: &[u8]) -> Value {
    let length_field: [u8; 4] = frame[4..8].try_into().expect("frames have a header length");
    let length =
        usize::try_from(u32::from_le_bytes(length_field)).expect("header lengths fit in usize");
    serde_json::from_slice(&frame[8..8 + length]).expect("frame headers are JSON")
}

/// The keys of the descriptor named `name` that a `list_elements` response frame writes, in wire order.
fn wire_keys(frame: &[u8], name: &str) -> Vec<String> {
    let header = wire_header(frame);
    let response = header["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .expect("a response frame ends with its response");
    let elements = response["result"]["elements"]
        .as_array()
        .expect("list_elements lists elements");
    named(elements, name)
        .as_object()
        .expect("descriptors are objects")
        .keys()
        .cloned()
        .collect()
}

/// The keys of `keys` that `all` holds, in the order `all` holds them.
fn in_wire_order<'a>(all: &'a [String], keys: &[&str]) -> Vec<&'a str> {
    all.iter()
        .map(String::as_str)
        .filter(|key| keys.contains(key))
        .collect()
}

#[test]
fn engine_t124_a_moved_face_passes_through_with_its_fallback_in_body_coordinates() {
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());

    let faces = sides.send(2, list_faces);

    let top = listed_named(&faces, BOX_TOP);
    assert_placement(
        &top["placement"],
        [0.0, 0.0, 40.0],
        X_AXIS,
        Z_AXIS,
        "placement",
    );
    assert_placement(
        &top["fallback"],
        [-100.0, 0.0, 40.0],
        X_AXIS,
        Z_AXIS,
        "fallback",
    );
}

#[test]
fn engine_t124_a_moved_line_edge_passes_through_with_its_axis_direction() {
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());

    let edges = sides.send(2, list_edges);

    let edge = listed_named(&edges, BOX_FRONT_TOP_EDGE);
    assert_vector(&edge["axis_direction"], X_AXIS, "axis direction");
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

#[test]
fn engine_t124_descriptors_keep_their_key_order_through_ps_call() {
    // Protocol.md test 79: `fallback` after `placement`, `axis_direction` after it, both before `adjacent`.
    let mut sides = SideBySide::new();
    sides.open(1, &moved_box());

    sides.send(2, list_faces);
    let face_keys = wire_keys(&sides.bridge_bytes, BOX_TOP);
    sides.send(3, list_edges);
    let edge_keys = wire_keys(&sides.bridge_bytes, BOX_FRONT_TOP_EDGE);

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

    let picked = sides.send(3, |side| {
        json!({
            "type": "pick_reference",
            "doc": side.doc,
            "pick": {"mesh_id": side.model_mesh("b-1"), "kind": "face", "index": index}
        })
    });

    let element = &ok_result(&picked, "pick_reference")["element"];
    assert_eq!(element["name"], BOX_TOP);
    assert_placement(
        &element["fallback"],
        [-100.0, 0.0, 40.0],
        X_AXIS,
        Z_AXIS,
        "fallback",
    );
}

#[test]
fn engine_t125_a_drag_commit_on_a_moved_face_passes_through_with_its_body_fallback() {
    let plane = plane_after_a_drag_commit(Some("body"));

    assert_placement(
        &plane["fallback"],
        [0.0, 0.0, 40.0],
        X_AXIS,
        Z_AXIS,
        "fallback",
    );
    assert_eq!(plane["fallback_frame"], "body", "{plane}");
}

#[test]
fn engine_t125_a_drag_commit_of_a_world_fallback_passes_through_in_world_coordinates() {
    let plane = plane_after_a_drag_commit(None);

    assert_placement(
        &plane["fallback"],
        [0.0, 0.0, 40.0],
        X_AXIS,
        Z_AXIS,
        "fallback",
    );
    assert!(plane.get("fallback_frame").is_none(), "{plane}");
}

#[test]
fn engine_t125_solve_sketch_of_a_body_fallback_passes_through_placed_on_the_moved_face() {
    let mut sides = SideBySide::new();
    sides.open(1, &sketch_on_moved_top(Some("body")));

    let solved = sides.send(
        2,
        |side| json!({"type": "solve_sketch", "doc": side.doc, "feature": "f-sk1"}),
    );

    assert_placement(
        &ok_result(&solved, "solve_sketch")["solution"]["placement"],
        [100.0, 0.0, 40.0],
        X_AXIS,
        Z_AXIS,
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
    let attach = action_labeled(&message, "Attach to the top of 'Box 1'");

    let applied = sides.send(2, |side| apply_fix(side, &attach["run"]["fix"]));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sk1");
    assert_placement(
        &sketch_solution(&applied, "f-sk1")["placement"],
        [103.0, 4.0, 10.0],
        X_AXIS,
        Z_AXIS,
        "placement",
    );
    let plane = record(&mut sides, 3, "f-sk1")["plane"].clone();
    assert_eq!(plane["face"]["q"], "f-box1/top", "{plane}");
    assert_placement(
        &plane["fallback"],
        [3.0, 4.0, 10.0],
        X_AXIS,
        Z_AXIS,
        "fallback",
    );
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

#[test]
fn engine_t135_apply_fix_of_turn_the_drawing_back_passes_through_with_the_mirrored_drawing() {
    let mut sides = SideBySide::new();
    let opened = sides.open(1, TURNED_OVER_TOP_TEXT);
    let message = feature_message(&opened, "f-sk2", "sketch_turned_over");
    let turn = action_labeled(&message, "Turn the drawing back");

    let applied = sides.send(2, |side| apply_fix(side, &turn["run"]["fix"]));

    assert_eq!(ok_result(&applied, "apply_fix")["revision"], 1);
    assert_ok_without_messages(&applied, "f-sk2");
    assert_ok_without_messages(&applied, "f-hole1");
    let sketch = record(&mut sides, 3, "f-sk2");
    assert_eq!(sketch["entities"][0]["id"], "e-p1", "{sketch}");
    let [x, y] = [0, 1].map(|axis| {
        sketch["entities"][0]["position"][axis]
            .as_f64()
            .expect("positions hold numbers")
    });
    assert!(
        (x - 20.0).abs() <= GEOMETRY_TOLERANCE && (y + 10.0).abs() <= GEOMETRY_TOLERANCE,
        "e-p1 lies at (20, -10) on the mirrored drawing: {sketch}"
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
    let before = ok_result(&sides.send(2, save_document), "save_document")["text"].clone();
    let message = feature_message(&opened, "f-sk2", "sketch_turned_over");
    let turn = action_labeled(&message, "Turn the drawing back");
    sides.send(3, |side| apply_fix(side, &turn["run"]["fix"]));

    let undone = sides.send(4, |side| json!({"type": "undo", "doc": side.doc}));

    assert_eq!(ok_result(&undone, "undo")["label"], "Turn the drawing back");
    let after = ok_result(&sides.send(5, save_document), "save_document")["text"].clone();
    assert_eq!(after, before);
}

#[test]
fn file_format_t057_a_version_1_face_sketch_after_a_move_passes_through_in_world_coordinates() {
    let mut sides = SideBySide::new();

    let opened = sides.open(1, &at_version_1(&moved_box_with_a_face_sketch(None)));

    assert_eq!(ok_result(&opened, "open_document")["migrated_from"], 1);
    assert_placement(
        &sketch_solution(&opened, "f-sk2")["placement"],
        [0.0, 0.0, 10.0],
        X_AXIS,
        Z_AXIS,
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

    assert_placement(
        &sketch_solution(&opened, "f-sk2")["placement"],
        [100.0, 0.0, 10.0],
        X_AXIS,
        Z_AXIS,
        "placement",
    );
    assert_ok_without_messages(&opened, "f-hole1");
}
