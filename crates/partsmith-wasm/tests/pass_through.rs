//! The protocol passes through `ps_call` unchanged (protocol.md section 2.10; the reference stability plan, sections 10.3 item 9, 10.4 and 10.5). The bridge answers the diagnostics, fixes and errors of ADR 0024, the documents of format version 2 (ADR 0025), the frames on the model of ADR 0023 (body-coordinate fallbacks and axis directions in descriptors, drag commits, `Attach` and `Turn the drawing back`) and the newcomer errors of phase 3 (sketch finding messages with `Connect the ends`, the conflict's Delete and Make a reference, `Put it in a new body`, renamed settings and misspelled parameter names, the limits that the kernel's solid counts and widths across edges find, `set_document_units` and `bare_numbers`) with the response frame that `Session::call` gives a session of the module's options (display on, random ids): nothing is added, dropped or reordered on the way through.
//!
//! Each test sends its requests through `ps_call` (the `Block` helper) and to a reference `Session`, and compares the decoded frames. Document handles and mesh ids count per session, and on the Emscripten target every test shares the module's one session, so the comparison leaves those two numbers out, and a command that names a document or a mesh, such as a pick, names each side's own. A fix whose edits take a new id from the session's id generator (`Put it in a new body`, `Connect the ends`) gets a random id on each side, so the comparison of that frame reads each side's new ids as the same placeholders. Everything else, buffers included, must be equal. Each test also checks the new shape it is about in the bridge's frame, so an empty answer on both sides cannot pass.
//!
//! A test named `engine_tNNN` sends the documents and requests of engine.md section 6 test NNN, one named `engine_tNNN_ncMM` those of newcomer error corpus scenario NCMM in that test, and one named `file_format_tNNN` those of file-format.md section 6 test NNN; the bridge's own tests are protocol.md's (`tNNN` in the other test files).

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

        let (_, bridge, reference) = self.exchange(&request, &request, &no_generated_ids);

        if outcome_json(&bridge)["kind"] == "ok" {
            self.documents = Some((Side::opened(&bridge), Side::opened(&reference)));
        }
        bridge
    }

    /// Sends request `id` with the command `command` builds for each side's document, and returns the bridge's frame.
    fn send(&mut self, id: u64, command: impl Fn(&Side) -> Value) -> ResponseFrame {
        self.send_with_bytes(id, command, &no_generated_ids).1
    }

    /// Sends request `id` like [`SideBySide::send`], and returns the bridge's frame as `ps_call` returned it.
    fn send_raw(&mut self, id: u64, command: impl Fn(&Side) -> Value) -> Vec<u8> {
        self.send_with_bytes(id, command, &no_generated_ids).0
    }

    /// Sends request `id` like [`SideBySide::send`] for a command whose edits take ids from the session's id generator, and returns the bridge's frame. Each side generates its own random ids, so the comparison reads the ids that `generated` finds in each side's frame, which must be some, as the same placeholders, in the order it lists them.
    fn send_generating(
        &mut self,
        id: u64,
        command: impl Fn(&Side) -> Value,
        generated: impl Fn(&ResponseFrame) -> Vec<String>,
    ) -> ResponseFrame {
        let found = |frame: &ResponseFrame| {
            let ids = generated(frame);
            assert!(!ids.is_empty(), "the command generated no id");
            ids
        };
        self.send_with_bytes(id, command, &found).1
    }

    /// Sends request `id` with the command `command` builds for each side's document, compares the answers with the ids `generated` finds read as placeholders, keeps each side's model meshes, and returns the bridge's frame as `ps_call` returned it and decoded.
    fn send_with_bytes(
        &mut self,
        id: u64,
        command: impl Fn(&Side) -> Value,
        generated: &dyn Fn(&ResponseFrame) -> Vec<String>,
    ) -> (Vec<u8>, ResponseFrame) {
        let (bridge_side, reference_side) = self
            .documents
            .as_ref()
            .expect("a document is open on both sides");
        let bridge_request = request_frame(id, &command(bridge_side).to_string());
        let reference_request = request_frame(id, &command(reference_side).to_string());

        let (bytes, bridge, reference) =
            self.exchange(&bridge_request, &reference_request, generated);

        if let Some((bridge_side, reference_side)) = &mut self.documents {
            bridge_side.record_meshes(&bridge);
            reference_side.record_meshes(&reference);
        }
        (bytes, bridge)
    }

    /// Sends `bridge_request` through `ps_call` and `reference_request` to the reference session, asserts that the answers are equal but for the session numbers and the ids `generated` finds in each, and returns the bridge's frame as `ps_call` returned it, then both decoded frames.
    fn exchange(
        &mut self,
        bridge_request: &[u8],
        reference_request: &[u8],
        generated: &dyn Fn(&ResponseFrame) -> Vec<String>,
    ) -> (Vec<u8>, ResponseFrame, ResponseFrame) {
        let bridge_bytes = through_ps_call(bridge_request);
        let bridge = response_frame(&bridge_bytes);
        let reference = response_frame(&self.reference.call(reference_request));

        let bridge_ids = generated(&bridge);
        let reference_ids = generated(&reference);
        assert_eq!(
            bridge_ids.len(),
            reference_ids.len(),
            "ps_call generated {bridge_ids:?} where Session::call generated {reference_ids:?}"
        );
        assert_eq!(
            comparable_messages(&bridge, &bridge_ids),
            comparable_messages(&reference, &reference_ids),
            "ps_call answered differently from Session::call"
        );
        assert!(
            bridge.buffers == reference.buffers,
            "ps_call answered with other buffers than Session::call ({} and {} buffers)",
            bridge.buffers.len(),
            reference.buffers.len()
        );
        (bridge_bytes, bridge, reference)
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

/// What a command that takes no id from the session's id generator generated: nothing.
fn no_generated_ids(_frame: &ResponseFrame) -> Vec<String> {
    Vec::new()
}

/// The events and the response of a frame as JSON, with every value of a [`SESSION_NUMBERS`] key set to null and each id of `generated` read as its placeholder.
fn comparable_messages(frame: &ResponseFrame, generated: &[String]) -> Value {
    let mut messages = json!({
        "events": serde_json::to_value(&frame.events).expect("events always serialize"),
        "response": serde_json::to_value(&frame.response).expect("responses always serialize"),
    });
    forget_session_numbers(&mut messages);
    replace_generated_ids(&mut messages, generated);
    messages
}

/// Replaces each id of `generated` in every string inside `value` with the placeholder of its position in `generated`.
fn replace_generated_ids(value: &mut Value, generated: &[String]) {
    match value {
        Value::String(text) => {
            for (position, id) in generated.iter().enumerate() {
                if text.contains(id.as_str()) {
                    *text = text.replace(id.as_str(), &format!("<generated id {position}>"));
                }
            }
        }
        Value::Object(members) => members
            .values_mut()
            .for_each(|member| replace_generated_ids(member, generated)),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| replace_generated_ids(item, generated)),
        _ => {}
    }
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

/// The only item of `items`, where `what` names one item for the failure text.
#[track_caller]
fn only(items: Vec<Value>, what: &str) -> Value {
    match <[Value; 1]>::try_from(items) {
        Ok([item]) => item,
        Err(items) => panic!("one {what}, not {}: {items:?}", items.len()),
    }
}

/// The only message of `holder`, a feature status or a sketch solution.
fn only_message_of(holder: &Value) -> Value {
    let messages = holder["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("{holder} lists its messages"));
    only(messages.clone(), "message")
}

/// The only message of `feature` in the frame's `recomputed` event.
fn only_message(frame: &ResponseFrame, feature: &str) -> Value {
    only_message_of(&feature_status(frame, feature))
}

/// The frame's only `document_changed` event.
fn document_changed(frame: &ResponseFrame) -> Value {
    only(
        events_of(frame, "document_changed"),
        "document_changed event",
    )
}

/// The recipe in the frame's only `document_changed` event.
fn changed_recipe(frame: &ResponseFrame) -> Value {
    document_changed(frame)["document"].clone()
}

/// The record of `feature` in the recipe of the frame's `document_changed` event.
fn changed_record(frame: &ResponseFrame, feature: &str) -> Value {
    feature_record(&changed_recipe(frame), feature)
}

/// The `id` of each object of the JSON array `items`, in order.
fn ids_of(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap_or_else(|| panic!("{items} is a list"))
        .iter()
        .map(|item| item["id"].as_str().expect("ids are text").to_owned())
        .collect()
}

/// The actions of a message, in order.
fn actions(message: &Value) -> &[Value] {
    message["actions"]
        .as_array()
        .unwrap_or_else(|| panic!("the message has actions: {message}"))
}

/// The labels of a message's actions, in order.
fn labels(message: &Value) -> Vec<&str> {
    actions(message)
        .iter()
        .map(|action| action["label"].as_str().expect("actions have labels"))
        .collect()
}

/// The action labeled `label` among a message's actions.
fn action_labeled(message: &Value, label: &str) -> Value {
    actions(message)
        .iter()
        .find(|action| action["label"] == label)
        .unwrap_or_else(|| panic!("the message offers {label:?}: {message}"))
        .clone()
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

/// The `apply_fix` command for the `apply` action labeled `label` among `message`'s actions, for the side's document.
fn apply_action(side: &Side, message: &Value, label: &str) -> Value {
    apply_fix(side, &action_labeled(message, label)["run"]["fix"])
}

/// The `save_document` command for the side's document.
fn save_document(side: &Side) -> Value {
    json!({"type": "save_document", "doc": side.doc})
}

/// The text `save_document` gives for the bridge's document, read as request `id`.
fn saved_text(sides: &mut SideBySide, id: u64) -> Value {
    ok_result(&sides.send(id, save_document), "save_document")["text"].clone()
}

/// The `get_document` command for the side's document.
fn get_document(side: &Side) -> Value {
    json!({"type": "get_document", "doc": side.doc})
}

/// The `begin_transaction` command labeled `label` for the side's document.
fn begin_transaction(side: &Side, label: &str) -> Value {
    json!({"type": "begin_transaction", "doc": side.doc, "label": label})
}

/// The `set_document_units` command for the side's document.
fn set_document_units(side: &Side, units: &str, keep_sizes: bool) -> Value {
    json!({"type": "set_document_units", "doc": side.doc, "units": units, "keep_sizes": keep_sizes})
}

/// The `undo` command for the side's document.
fn undo(side: &Side) -> Value {
    json!({"type": "undo", "doc": side.doc})
}

/// The `solve_sketch` command for the sketch `feature` of the side's document.
fn solve_sketch(side: &Side, feature: &str) -> Value {
    json!({"type": "solve_sketch", "doc": side.doc, "feature": feature})
}

/// The `list_elements` command for the faces of `b-1` of the side's document.
fn list_faces(side: &Side) -> Value {
    json!({"type": "list_elements", "doc": side.doc, "body": "b-1", "kind": "face"})
}

/// The `list_elements` command for the edges of `b-1` of the side's document.
fn list_edges(side: &Side) -> Value {
    json!({"type": "list_elements", "doc": side.doc, "body": "b-1", "kind": "edge"})
}

/// The `pick_reference` command for the element of `kind` at `index` in the model mesh of `b-1` the side displays now.
fn pick(side: &Side, kind: &str, index: usize) -> Value {
    json!({
        "type": "pick_reference",
        "doc": side.doc,
        "pick": {"mesh_id": side.model_mesh("b-1"), "kind": kind, "index": index}
    })
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

/// The record of `feature` in the recipe `document`.
fn feature_record(document: &Value, feature: &str) -> Value {
    document["features"]
        .as_array()
        .expect("documents list their features")
        .iter()
        .find(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has {feature}"))
        .clone()
}

/// The record of `feature` that the bridge's document holds, read with `get_document` as request `id`.
fn record(sides: &mut SideBySide, id: u64, feature: &str) -> Value {
    let got = sides.send(id, get_document);
    feature_record(&ok_result(&got, "get_document")["document"], feature)
}

/// The solution of `feature` in the frame's only `sketch_solved` event for it.
fn sketch_solution(frame: &ResponseFrame, feature: &str) -> Value {
    let solved = events_of(frame, "sketch_solved")
        .into_iter()
        .filter(|event| event["solution"]["feature"] == feature)
        .collect();
    only(solved, &format!("sketch_solved event of {feature}"))["solution"].clone()
}

/// The only message of `feature`'s solution in the frame's `sketch_solved` event.
fn only_solution_message(frame: &ResponseFrame, feature: &str) -> Value {
    only_message_of(&sketch_solution(frame, feature))
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

/// The canonical text `save` writes for the document `text`.
fn canonical(text: &str) -> String {
    let loaded = load(text).unwrap_or_else(|error| panic!("the document loads: {error}"));
    save(&loaded.document).unwrap_or_else(|error| panic!("the document saves: {error}"))
}

/// The canonical text of the document `text` as a file written at format version 1 holds it: only its `format_version` line differs.
fn at_version_1(text: &str) -> String {
    at_version(&canonical(text), 1)
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

/// A millimeter `.ptsm` text at format version 2 of the one body `b-1`, 'Body 1', with `parameters` and `features`.
fn body_1_text(parameters: Vec<Value>, features: Vec<Value>) -> String {
    let document = json!({
        "format": "partsmith",
        "format_version": 2,
        "units": "mm",
        "parameters": parameters,
        "bodies": [{"id": "b-1", "name": "Body 1"}],
        "features": features
    });
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// The box `feature` of `b-1` of `size`, at `position` when there is one, with Operation `op`.
fn box_feature(feature: &str, size: [&str; 3], position: Option<[&str; 3]>, op: &str) -> Value {
    let mut record = json!({"id": feature, "type": "box", "body": "b-1", "size": size});
    if let Some(position) = position {
        record["position"] = json!(position);
    }
    record["op"] = json!(op);
    record
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
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["40", "40", "10"], None, "new"),
            moved_by_100(),
            json!({
                "id": "f-sk2", "type": "sketch", "plane": plane,
                "entities": [{"id": "e-p1", "type": "point", "position": [15, 15]}],
                "constraints": []
            }),
            json!({
                "id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2",
                "points": ["e-p1"], "diameter": "4", "extent": {"kind": "through_all"}
            }),
        ],
    )
}

/// Asserts that `feature` has status ok and no message in the frame's `recomputed` event.
#[track_caller]
fn assert_ok_without_messages(frame: &ResponseFrame, feature: &str) {
    let status = feature_status(frame, feature);
    assert_eq!(status["status"], "ok", "{status}");
    assert_eq!(status["messages"], json!([]), "{status}");
}

/// The `N` components of a JSON vector.
fn vector<const N: usize>(value: &Value) -> [f64; N] {
    let components: Vec<f64> = value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is a vector"))
        .iter()
        .map(|component| component.as_f64().expect("vectors hold numbers"))
        .collect();
    components
        .try_into()
        .unwrap_or_else(|components: Vec<f64>| panic!("{components:?} has {N} components"))
}

/// Asserts that each component of the JSON vector `actual` lies within [`GEOMETRY_TOLERANCE`] of `expected`.
#[track_caller]
fn assert_vector<const N: usize>(actual: &Value, expected: [f64; N], what: &str) {
    let components: [f64; N] = vector(actual);
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

/// Asserts that the JSON placement `placement` lies level at `origin`: x axis along world X, normal along world Z.
#[track_caller]
fn assert_level_placement(placement: &Value, origin: [f64; 3], what: &str) {
    assert_placement(placement, origin, X_AXIS, Z_AXIS, what);
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
    let response = wire_response(frame);
    let elements = response["result"]["elements"]
        .as_array()
        .expect("list_elements lists elements");
    keys_of(named(elements, name))
}

/// The messages of a frame as its bytes hold them, in order (see [`wire_header`]).
fn wire_messages(frame: &[u8]) -> Vec<Value> {
    match wire_header(frame)["messages"].take() {
        Value::Array(messages) => messages,
        other => panic!("frames list their messages: {other}"),
    }
}

/// The response of a frame as its bytes hold it (see [`wire_header`]).
fn wire_response(frame: &[u8]) -> Value {
    wire_messages(frame)
        .pop()
        .expect("a response frame ends with its response")
}

/// The only event of type `event_type` of a frame as its bytes hold it (see [`wire_header`]).
fn wire_event(frame: &[u8], event_type: &str) -> Value {
    let events = wire_messages(frame)
        .into_iter()
        .filter(|message| message["kind"] == "event" && message["type"] == event_type)
        .collect();
    only(events, &format!("{event_type} event"))
}

/// The keys of the JSON object `object`, in the order it holds them.
fn keys_of(object: &Value) -> Vec<String> {
    object
        .as_object()
        .unwrap_or_else(|| panic!("{object} is an object"))
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

/// A line entity from `start` to `end`.
fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// The lines `e-1` to `e-4` of the rectangle from the origin to (`width`, `depth`), counterclockwise from the origin, as features.md section 6.1 `RECT` draws them.
fn rectangle(width: f64, depth: f64) -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [width, 0.0]),
        line("e-2", [width, 0.0], [width, depth]),
        line("e-3", [width, depth], [0.0, depth]),
        line("e-4", [0.0, depth], [0.0, 0.0]),
    ]
}

/// The origin plane XY, as a sketch or a datum plane stores it.
fn xy_plane() -> Value {
    json!({"kind": "origin", "plane": "XY"})
}

/// The sketch `feature` on `plane` with `entities` and `constraints`.
fn sketch(feature: &str, plane: Value, entities: Vec<Value>, constraints: Vec<Value>) -> Value {
    json!({
        "id": feature, "type": "sketch", "plane": plane,
        "entities": entities, "constraints": constraints
    })
}

/// The extrude `feature` into `b-1` of the profile regions `regions` of `sketch`, by `distance`, with Operation `op`.
fn extrude(feature: &str, sketch: &str, regions: Value, distance: &str, op: &str) -> Value {
    json!({
        "id": feature, "type": "extrude", "body": "b-1",
        "profile": {"sketch": sketch, "regions": regions},
        "extent": {"kind": "distance", "expr": distance}, "op": op
    })
}

/// The fillet `f-fil1` of `b-1` of radius `radius` on the one edge `edge`.
fn fillet(edge: Value, radius: &str) -> Value {
    json!({"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [edge], "radius": radius})
}

/// BLOCK of the newcomer error corpus with 'Extrude 1' extruded `distance`: the 80 x 50 rectangle `e-1` to `e-4` of `f-sk1` on XY, extruded by `f-ext1` into `b-1` (x 0..80, y 0..50, z 0..30 at distance 30).
fn block(distance: &str) -> Vec<Value> {
    vec![
        sketch("f-sk1", xy_plane(), rectangle(80.0, 50.0), Vec::new()),
        extrude(
            "f-ext1",
            "f-sk1",
            json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]),
            distance,
            "new",
        ),
    ]
}

/// Newcomer error NC01: BLOCK's outline in `f-sk1` with a 0.3 mm gap at the origin (`e-4` ends at (0, 0.3)), which no feature uses yet.
fn open_outline() -> String {
    let mut outline = rectangle(80.0, 50.0);
    outline[3] = line("e-4", [0.0, 50.0], [0.0, 0.3]);
    body_1_text(
        Vec::new(),
        vec![sketch("f-sk1", xy_plane(), outline, Vec::new())],
    )
}

/// NC03 (`op` cut) and NC04 (`op` join): BLOCK, the datum plane `f-dp1` 40 above XY, and the circle `e-5` of `f-sk2` on it at (40, 25) with radius 5, extruded 15 by `f-ext2` with Operation `op`: it starts 10 above the block and points away from it.
fn circle_above_the_block(op: &str) -> String {
    let mut features = block("30");
    features.extend([
        json!({"id": "f-dp1", "type": "datum_plane", "base": xy_plane(), "offset": "40"}),
        sketch(
            "f-sk2",
            json!({"kind": "datum", "feature": "f-dp1"}),
            vec![json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5})],
            Vec::new(),
        ),
        extrude(
            "f-ext2",
            "f-sk2",
            json!([{"key": "e-5", "fp": "c=40,25|a=78.539816"}]),
            "15",
            op,
        ),
    ]);
    body_1_text(Vec::new(), features)
}

/// Newcomer error NC05: 'Box 1' 40 x 20 x 10 (x -20..20), 'Box 2' 20 x 30 x 5 at (10, 0, 7.6) cutting a step 2.4 mm deep whose riser is `f-box2/left` at x = 0, and a fillet of radius 3 on the edge between the top of 'Box 1' and the riser.
fn fillet_on_a_low_step() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["40", "20", "10"], None, "new"),
            box_feature("f-box2", ["20", "30", "5"], Some(["10", "0", "7.6"]), "cut"),
            fillet(
                json!({"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}),
                "3",
            ),
        ],
    )
}

/// Newcomer error NC07 after its edit: the 40 x 20 rectangle of `f-sk1` with Distance `c-1` 40 mm and the added Distance `c-2` 30 mm on `e-1`, extruded 10 by `f-ext1`. Its messages and actions are those of the setup with the edit applied (the newcomer error corpus, rule 5).
fn conflicting_dimensions() -> String {
    let distance = |constraint: &str, expr: &str| json!({"id": constraint, "type": "distance", "line": "e-1", "expr": expr});
    body_1_text(
        Vec::new(),
        vec![
            sketch(
                "f-sk1",
                xy_plane(),
                rectangle(40.0, 20.0),
                vec![distance("c-1", "40 mm"), distance("c-2", "30 mm")],
            ),
            extrude(
                "f-ext1",
                "f-sk1",
                json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"}]),
                "10",
                "new",
            ),
        ],
    )
}

/// Newcomer error NC09: 'Box 1' of size `40`, `20` and `10` and a fillet of radius `2` on its front top edge, four numbers without a unit in a millimeter document.
fn numbers_without_units() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["40", "20", "10"], None, "new"),
            fillet(
                json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}),
                "2",
            ),
        ],
    )
}

/// CUBE20 with its sizes in millimeters and a fillet of radius `2`, without a unit, on its front top edge: in inches the radius is 50.8 mm, too large (NC09, with the sentence of corpus R48).
fn cube_with_a_bare_radius() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["20 mm", "20 mm", "20 mm"], None, "new"),
            fillet(
                json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"}),
                "2",
            ),
        ],
    )
}

/// Newcomer error NC10: BLOCK with the parameter `height` 30 mm and the distance of 'Extrude 1' misspelled `hieght`.
fn misspelled_parameter() -> String {
    body_1_text(
        vec![json!({"id": "p-height", "name": "height", "expr": "30 mm"})],
        block("hieght"),
    )
}

/// CUBE20 with a shell `f-sh1` 2 thick whose params store the top under the misspelled key `remove_face` instead of `remove_faces` (engine.md test 131).
fn misspelled_shell_setting() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["20", "20", "20"], None, "new"),
            json!({
                "id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2",
                "remove_face": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4"}]
            }),
        ],
    )
}

/// Asserts a message's code, text and action labels, in order.
#[track_caller]
fn assert_message(message: &Value, code: &str, text: &str, offered: &[&str]) {
    assert_eq!(message["code"], code, "{message}");
    assert_eq!(message["message"], text, "{message}");
    assert_eq!(labels(message), offered, "{message}");
}

/// The bodies of the frame's recipe besides `b-1`: the one `Put it in a new body` adds to a document of `b-1` alone.
fn bodies_besides_body_1(frame: &ResponseFrame) -> Vec<String> {
    ids_of(&changed_recipe(frame)["bodies"])
        .into_iter()
        .filter(|body| body != "b-1")
        .collect()
}

/// The constraints of `f-sk1` in the frame's recipe: the one `Connect the ends` adds to NC01's sketch, which has none.
fn constraints_of_sketch_1(frame: &ResponseFrame) -> Vec<String> {
    ids_of(&changed_record(frame, "f-sk1")["constraints"])
}

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
