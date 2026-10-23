//! The bridge and a reference session sent the same requests, and the comparison of their answers without the numbers that count per session.

use std::collections::BTreeMap;

use partsmith_engine::{Session, SessionOptions};
use partsmith_protocol::ResponseFrame;
use serde_json::{Value, json};

use crate::common::block::Block;
use crate::common::{ok_result, outcome_json, request_frame, response_frame};

use super::frames::events_of;

/// The keys whose values count per session, so a fresh session and the module's shared one give them different values.
const SESSION_NUMBERS: [&str; 2] = ["doc", "mesh_id"];

/// The document one side opened last, with what that side's frames said about it.
pub struct Side {
    /// The document's handle in that side's session.
    pub doc: Value,
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
    pub fn model_mesh(&self, body: &str) -> &Value {
        self.model_meshes
            .get(body)
            .unwrap_or_else(|| panic!("{body} has a model mesh"))
    }
}

/// The bridge and a reference session with the module's session options, sent the same requests.
pub struct SideBySide {
    /// The session the bridge's answers are compared with.
    reference: Session,
    /// The document each side opened last: the bridge's, then the reference session's.
    documents: Option<(Side, Side)>,
}

impl SideBySide {
    /// A fresh reference session with the options `main` gives the module's session (protocol.md section 2.10).
    pub fn new() -> SideBySide {
        SideBySide {
            reference: Session::new(SessionOptions {
                display: true,
                ..SessionOptions::default()
            }),
            documents: None,
        }
    }

    /// Opens `text` on both sides with request `id`, keeps both documents when it opens, and returns the bridge's frame.
    pub fn open(&mut self, id: u64, text: &str) -> ResponseFrame {
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
    pub fn send(&mut self, id: u64, command: impl Fn(&Side) -> Value) -> ResponseFrame {
        self.send_with_bytes(id, command, &no_generated_ids).1
    }

    /// Sends request `id` like [`SideBySide::send`], and returns the bridge's frame as `ps_call` returned it.
    pub fn send_raw(&mut self, id: u64, command: impl Fn(&Side) -> Value) -> Vec<u8> {
        self.send_with_bytes(id, command, &no_generated_ids).0
    }

    /// Sends request `id` like [`SideBySide::send`] for a command whose edits take ids from the session's id generator, and returns the bridge's frame. Each side generates its own random ids, so the comparison reads the ids that `generated` finds in each side's frame, which must be some, as the same placeholders, in the order it lists them.
    pub fn send_generating(
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
