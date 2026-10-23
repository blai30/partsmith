//! Acceptance tests 98 to 102 and 151 of `docs/spec/engine.md` section 6: byte-level calls, determinism, warm
//! reopening, headless sessions and buffer checks, and determinism with causes and ghosts (sections 3.18, 3.21 and
//! 3.23).

use crate::common;
use crate::corpus;

use common::{
    Sent, display_options, doc_handle, fixture, headless_session, open, open_text, request_frame,
    send, session, set_height,
};
use corpus::{EDIT_LABEL, actions, cause, edits, message, r16, repair_ghost};
use partsmith_engine::{Session, SessionOptions};
use partsmith_protocol::{Buffer, decode_response_frame};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The 13 commands of test 98, in order.
fn script() -> Vec<Value> {
    vec![
        json!({"type": "open_document", "text": fixture("fillet.ptsm"), "assets": []}),
        json!({"type": "apply_edits", "doc": 1, "label": "Change height", "edits": [set_height("45 mm")]}),
        json!({"type": "begin_transaction", "doc": 1, "label": "Edit fillet"}),
        json!({"type": "set_focus", "doc": 1, "feature": "f-fil1"}),
        json!({"type": "commit_transaction", "doc": 1}),
        json!({"type": "open_document", "text": fixture("drag.ptsm"), "assets": []}),
        json!({"type": "drag_sketch", "doc": 2, "feature": "f-sk1", "point": "e-1.end", "to": [20.0, 5.0]}),
        json!({"type": "end_sketch_drag", "doc": 2, "feature": "f-sk1", "commit": true}),
        json!({"type": "measure", "doc": 1, "targets": [{"kind": "body", "body": "b-1"}]}),
        json!({"type": "analyze_print", "doc": 1, "orientation": {"kind": "as_modeled"}}),
        json!({"type": "export", "doc": 1, "format": {"kind": "stl", "binary": true, "per_body": false}}),
        json!({"type": "export", "doc": 1, "format": {"kind": "step"}}),
        json!({"type": "export_python", "doc": 1}),
    ]
}

/// Runs the script of test 98 through `Session::call` with request ids 1 to 13 and returns the response frames.
fn run_script(options: SessionOptions) -> Vec<Vec<u8>> {
    let mut session = Session::new(options);
    (1u64..)
        .zip(script())
        .map(|(id, command)| session.call(&request_frame(id, command, &[])))
        .collect()
}

/// Asserts that every frame decodes to a successful response with the expected id.
fn assert_all_succeed(frames: &[Vec<u8>]) {
    for (id, frame) in (1u64..).zip(frames) {
        let decoded = decode_response_frame(frame).expect("response frames decode");
        let response = serde_json::to_value(&decoded.response).expect("responses serialize");
        assert_eq!(response["id"], id);
        assert_eq!(
            response["result"]["kind"], "ok",
            "request {id} succeeds: {}",
            response["result"]
        );
    }
}

/// An event with its `doc` and `mesh_id` values replaced by 0.
fn without_handles(event: &Value) -> Value {
    let mut event = event.clone();
    if let Some(fields) = event.as_object_mut() {
        for key in ["doc", "mesh_id"] {
            if fields.contains_key(key) {
                fields.insert(key.to_owned(), json!(0));
            }
        }
    }
    event
}

#[test]
fn t098_the_same_frames_give_byte_identical_responses() {
    let first = run_script(display_options());
    let second = run_script(display_options());

    assert_all_succeed(&first);
    assert_eq!(first.len(), 13);
    for (index, (one, two)) in first.iter().zip(&second).enumerate() {
        assert!(one == two, "response frame {} is byte-identical", index + 1);
    }
}

#[test]
fn t099_the_cache_budget_never_changes_output() {
    let with_default_budget = run_script(display_options());
    let without_cache = run_script(SessionOptions {
        cache_budget: 0,
        ..display_options()
    });

    assert_all_succeed(&without_cache);
    for (index, (one, two)) in with_default_budget.iter().zip(&without_cache).enumerate() {
        assert!(one == two, "response frame {} is byte-identical", index + 1);
    }
}

#[test]
fn t100_reopening_the_same_text_gives_the_same_events() {
    let mut session = session();
    let first = open(&mut session, "plate.ptsm");
    let doc = common::doc_handle(&first);
    send(&mut session, json!({"type": "close_document", "doc": doc})).ok();

    let second = open(&mut session, "plate.ptsm");

    assert_ne!(common::doc_handle(&second), doc);
    let first_events: Vec<Value> = first.events.iter().map(without_handles).collect();
    let second_events: Vec<Value> = second.events.iter().map(without_handles).collect();
    assert_eq!(second_events, first_events);
}

#[test]
fn t101_headless_sessions_emit_no_meshes() {
    let mut session = headless_session();

    let opened = open(&mut session, "box.ptsm");

    assert_eq!(opened.event_types(), ["document_changed", "recomputed"]);
}

#[test]
fn t102_a_buffer_index_past_the_request_buffers_is_malformed() {
    let mut session = session();
    let frame = request_frame(
        1,
        json!({"type": "import_step", "doc": 1, "data": 3}),
        &[Buffer::U8(b"ISO-10303-21;\n".to_vec())],
    );

    let response = decode_response_frame(&session.call(&frame)).expect("response frames decode");

    assert!(response.events.is_empty(), "no events");
    let result =
        serde_json::to_value(&response.response).expect("responses serialize")["result"].clone();
    assert_eq!(result["kind"], "error");
    assert_eq!(result["code"], "malformed_request");
    assert_eq!(
        result["message"],
        "The request is not valid: Buffer 3 does not exist."
    );
}

// Test 151: determinism with causes and ghosts.

/// The model edits of corpus R16's edit, made against its setup.
fn r16_edits() -> Value {
    let scenario = r16();
    let mut helper = headless_session();
    let doc = doc_handle(&open_text(&mut helper, &scenario.setup));
    Value::Array(edits(&mut helper, doc, &scenario.changes))
}

/// The response frames of the script of test 98 through `Session::call`, followed by corpus R16's setup opened, its
/// edit applied as one `apply_edits` and `apply_fix` of the first action of the fillet's message, with request ids 1
/// to 16. Also asserts that the edit's reply carries the fillet's cause and its ghost, so the frames compared hold
/// both.
fn run_script_and_r16(options: SessionOptions) -> Vec<Vec<u8>> {
    let mut session = Session::new(options);
    let mut frames: Vec<Vec<u8>> = Vec::new();
    let mut call = |session: &mut Session, command: Value| {
        let id = u64::try_from(frames.len()).expect("few requests") + 1;
        let frame = session.call(&request_frame(id, command, &[]));
        frames.push(frame.clone());
        Sent::from_frame(&frame)
    };
    for command in script() {
        call(&mut session, command);
    }
    let opened = call(
        &mut session,
        json!({"type": "open_document", "text": r16().setup, "assets": []}),
    );
    let doc = doc_handle(&opened);
    let edited = call(
        &mut session,
        json!({"type": "apply_edits", "doc": doc, "label": EDIT_LABEL, "edits": r16_edits()}),
    );
    let lost = message(&edited, "f-fil1", "references_not_found");
    assert!(cause(&lost)["text"].is_string(), "{lost}");
    repair_ghost(&edited, "f-fil1", "/edges/0");
    let first = actions(&lost)[0].clone();
    let applied = call(
        &mut session,
        json!({"type": "apply_fix", "doc": doc, "fix": first["run"]["fix"]}),
    );
    applied.ok();
    frames
}

#[test]
fn t151_causes_and_ghosts_give_byte_identical_responses() {
    let first = run_script_and_r16(display_options());
    let second = run_script_and_r16(display_options());

    assert_all_succeed(&first);
    assert_eq!(first.len(), 16);
    for (index, (one, two)) in first.iter().zip(&second).enumerate() {
        assert!(one == two, "response frame {} is byte-identical", index + 1);
    }
}
