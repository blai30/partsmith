//! The protocol passes through `ps_call` unchanged (protocol.md section 2.10; the reference stability plan, section 10.3 item 9). The bridge answers the diagnostics, fixes and errors of ADR 0024 and the documents of format version 2 (ADR 0025) with the response frame that `Session::call` gives a session of the module's options (display on, random ids): nothing is added, dropped or reordered on the way through.
//!
//! Each test sends its requests through `ps_call` (the `Block` helper) and to a reference `Session`, and compares the decoded frames. Document handles and mesh ids count per session, and on the Emscripten target every test shares the module's one session, so the comparison leaves those two numbers out, and a command that names a document or a mesh, such as a pick, names each side's own; everything else, buffers included, must be equal. Each test also checks the new shape it is about in the bridge's frame, so an empty answer on both sides cannot pass.

mod common;

use std::collections::BTreeMap;

use common::block::Block;
use common::{error_info, ok_result, outcome_json, request_frame, response_frame};
use partsmith_engine::{Session, SessionOptions};
use partsmith_model::FORMAT_VERSION;
use partsmith_protocol::{ErrorCode, ResponseFrame};
use serde_json::{Value, json};

/// File-format.md example E10: every field format version 2 adds (`nb`, `merged`, `outline`, `fallback_frame` and `direction`), exactly as `save` writes it.
const E10_TEXT: &str = include_str!("../../partsmith-model/tests/fixtures/e10_version_2.ptsm");

/// The line of E10 that holds its format version.
const E10_VERSION_LINE: &str = "  \"format_version\": 2,";

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
        let bridge = response_frame(&through_ps_call(bridge_request));
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

/// E10 with its format version line set to `version`.
fn e10_at_version(version: u32) -> String {
    assert_eq!(
        E10_TEXT.matches(E10_VERSION_LINE).count(),
        1,
        "E10 states its format version once"
    );
    E10_TEXT.replace(
        E10_VERSION_LINE,
        &format!("  \"format_version\": {version},"),
    )
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
