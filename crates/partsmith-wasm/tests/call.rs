//! Acceptance tests for the safe core of the bridge, `partsmith_wasm::call` (protocol.md section 2.10 and section 6, "WebAssembly bridge", tests 48 and 50 to 52), plus the frame handling rules of section 2.10: `call` never traps and answers every input with exactly one response frame.
//!
//! Tests that open documents use the handle the session returns, so they pass whether or not other tests ran on the same thread's session (natively each test has its own thread; on the Emscripten target all tests share one).

mod common;

use common::{
    EMPTY_DOCUMENT_TEXT, error_info, error_of, example_a, frame_from_header, new_document,
    ok_result, request_frame, response_frame, send_for_document,
};
use partsmith_model::FORMAT_VERSION;
use partsmith_protocol::{ErrorCode, Event, FrameError, MAX_SAFE_INTEGER};
use partsmith_wasm::{call, initialize_session};
use proptest::prelude::*;
use serde_json::Value;

/// A document with one body holding a 10 x 20 x 30 mm box (`examples/box/default.ptsm`).
const BOX_DOCUMENT: &str = r#"{
  "format": "partsmith",
  "format_version": 1,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}
  ]
}
"#;

/// The header of an `engine_info` request with id 1 in protocol version 2, which this engine does not speak.
const VERSION_2_HEADER: &str = r#"{"protocol_version":2,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[]}"#;

#[test]
fn t048_engine_info_through_call_reports_the_protocol_engine_and_format_versions() {
    let frame = response_frame(&call(&example_a()));

    assert!(frame.events.is_empty(), "engine_info emits no events");
    assert_eq!(frame.response.id, 1);
    let result = ok_result(&frame, "engine_info");
    assert_eq!(result["protocol_version"], 1);
    assert_eq!(result["engine_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["format_version"], FORMAT_VERSION);
}

#[test]
fn t050_bytes_that_are_not_a_frame_answer_malformed_frame_with_id_0() {
    let frame = response_frame(&call(b"hello"));

    assert_eq!(error_of(&frame), (0, &ErrorCode::MalformedFrame));
}

#[test]
fn t051_protocol_version_2_answers_version_mismatch_with_the_request_id() {
    let request = frame_from_header(VERSION_2_HEADER);

    let frame = response_frame(&call(&request));

    assert_eq!(error_of(&frame), (1, &ErrorCode::ProtocolVersionMismatch));
}

#[test]
fn t052_the_session_keeps_a_new_document_for_the_next_call() {
    let doc = new_document(1);

    let saved = send_for_document(2, "save_document", &doc);

    let result = ok_result(&saved, "save_document");
    assert_eq!(result["text"], EMPTY_DOCUMENT_TEXT);
    assert_eq!(result["revision"], 0);
}

#[test]
fn malformed_frame_response_carries_the_frame_error_message() {
    let frame = response_frame(&call(b"hello"));

    let expected = FrameError::Truncated {
        length: 5,
        expected: 8,
    }
    .to_string();
    assert_eq!(error_info(&frame).message, expected);
}

#[test]
fn empty_input_answers_malformed_frame_with_id_0() {
    let frame = response_frame(&call(&[]));

    assert_eq!(error_of(&frame), (0, &ErrorCode::MalformedFrame));
}

#[test]
fn bytes_after_the_frame_answer_malformed_frame_with_id_0() {
    let mut request = example_a();
    request.extend_from_slice(&[0; 8]);

    let frame = response_frame(&call(&request));

    assert_eq!(error_of(&frame), (0, &ErrorCode::MalformedFrame));
}

#[test]
fn version_mismatch_response_names_the_version_found() {
    let request = frame_from_header(VERSION_2_HEADER);

    let frame = response_frame(&call(&request));

    assert_eq!(
        error_info(&frame).message,
        "The frame uses protocol version 2, but this engine speaks version 1. Reload the app."
    );
}

#[test]
fn unknown_command_answers_malformed_request_with_the_request_id() {
    let request = request_frame(3, r#"{"type":"no_such_command"}"#);

    let frame = response_frame(&call(&request));

    assert_eq!(error_of(&frame), (3, &ErrorCode::MalformedRequest));
}

#[test]
fn frame_holding_a_response_instead_of_a_request_answers_malformed_request() {
    let request = frame_from_header(
        r#"{"protocol_version":1,"messages":[{"kind":"response","id":5,"result":{"kind":"ok","type":"close_document"}}],"buffers":[]}"#,
    );

    let frame = response_frame(&call(&request));

    assert_eq!(error_info(&frame).code, ErrorCode::MalformedRequest);
}

#[test]
fn a_malformed_frame_leaves_the_session_working() {
    let _rejected = call(b"hello");

    let frame = response_frame(&call(&example_a()));

    ok_result(&frame, "engine_info");
}

#[test]
fn engine_info_echoes_the_largest_safe_request_id() {
    let request = request_frame(MAX_SAFE_INTEGER, r#"{"type":"engine_info"}"#);

    let frame = response_frame(&call(&request));

    assert_eq!(frame.response.id, MAX_SAFE_INTEGER);
}

#[test]
fn a_closed_document_is_unknown_to_later_calls() {
    let doc = new_document(1);
    send_for_document(2, "close_document", &doc);

    let saved = send_for_document(3, "save_document", &doc);

    assert_eq!(error_info(&saved).code, ErrorCode::UnknownDocument);
}

#[test]
fn initializing_the_session_again_keeps_the_open_documents() {
    initialize_session();
    let doc = new_document(1);

    initialize_session();
    let saved = send_for_document(2, "save_document", &doc);

    ok_result(&saved, "save_document");
}

#[test]
fn opening_a_document_with_a_body_sends_its_display_mesh() {
    let request = request_frame(
        1,
        &format!(
            r#"{{"type":"open_document","text":{},"assets":[]}}"#,
            Value::from(BOX_DOCUMENT)
        ),
    );

    let opened = response_frame(&call(&request));

    ok_result(&opened, "open_document");
    assert!(
        opened
            .events
            .iter()
            .any(|event| matches!(event, Event::BodyMesh { mesh: Some(_), .. })),
        "a display session sends body meshes"
    );
    assert!(!opened.buffers.is_empty(), "meshes travel as raw buffers");
}

proptest! {
    #[test]
    fn any_bytes_get_one_response_frame_without_events_or_buffers(
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        let frame = response_frame(&call(&bytes));

        prop_assert!(frame.events.is_empty() && frame.buffers.is_empty());
    }

    #[test]
    fn example_a_with_any_byte_replaced_gets_one_response_frame_without_events(
        index in 0usize..120,
        value in any::<u8>(),
    ) {
        let mut request = example_a();
        request[index] = value;

        let frame = response_frame(&call(&request));

        prop_assert!(frame.events.is_empty());
    }

    #[test]
    fn engine_info_echoes_any_client_request_id(id in 1..=MAX_SAFE_INTEGER) {
        let frame = response_frame(&call(&request_frame(id, r#"{"type":"engine_info"}"#)));

        prop_assert_eq!(frame.response.id, id);
    }
}
