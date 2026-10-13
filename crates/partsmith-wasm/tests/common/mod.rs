//! Fixtures and helpers shared by the bridge acceptance tests of `docs/spec/protocol.md` (section 2.10, tests 47 to 52).

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

pub mod block;

use partsmith_protocol::{ErrorCode, ErrorInfo, Outcome, ResponseFrame, decode_response_frame};
use partsmith_wasm::call;
use serde_json::Value;

/// Example A of protocol.md section 2.2: request id 1, `engine_info`, no buffers. 120 bytes, copied from the spec's hex dump.
pub const EXAMPLE_A_HEX: &str = "
    50 53 4d 31 6b 00 00 00 7b 22 70 72 6f 74 6f 63
    6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
    65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
    22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
    3a 31 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
    79 70 65 22 3a 22 65 6e 67 69 6e 65 5f 69 6e 66
    6f 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73 22 3a
    5b 5d 7d 00 00 00 00 00
";

/// The canonical text of an empty millimeter document (file-format.md example E1, 122 bytes).
pub const EMPTY_DOCUMENT_TEXT: &str = "{\n  \"format\": \"partsmith\",\n  \"format_version\": 1,\n  \"units\": \"mm\",\n  \"parameters\": [],\n  \"bodies\": [],\n  \"features\": []\n}\n";

/// The bytes of example A.
pub fn example_a() -> Vec<u8> {
    EXAMPLE_A_HEX
        .split_whitespace()
        .map(|pair| {
            u8::from_str_radix(pair, 16).unwrap_or_else(|_| panic!("{pair:?} is not a hex byte"))
        })
        .collect()
}

/// Builds frame bytes by hand, without the encoder: magic, little-endian header length, header, zero padding to 8. No buffers.
pub fn frame_from_header(header: &str) -> Vec<u8> {
    let header_length =
        u32::try_from(header.len()).expect("test headers are far smaller than 4 GiB");
    let padding = (8 - header.len() % 8) % 8;
    let mut bytes = Vec::with_capacity(8 + header.len() + padding);
    bytes.extend_from_slice(b"PSM1");
    bytes.extend_from_slice(&header_length.to_le_bytes());
    bytes.extend_from_slice(header.as_bytes());
    bytes.resize(bytes.len() + padding, 0);
    bytes
}

/// A protocol version 1 request frame without buffers holding request `id` with `command`, a JSON object text.
pub fn request_frame(id: u64, command: &str) -> Vec<u8> {
    frame_from_header(&format!(
        r#"{{"protocol_version":1,"messages":[{{"kind":"request","id":{id},"command":{command}}}],"buffers":[]}}"#
    ))
}

/// A request frame holding request `id` with a command of `command_type` whose one field is `doc`, a document handle as JSON.
pub fn document_command_frame(id: u64, command_type: &str, doc: &Value) -> Vec<u8> {
    request_frame(id, &format!(r#"{{"type":"{command_type}","doc":{doc}}}"#))
}

/// Creates an empty document with request `id` and returns its handle as JSON.
pub fn new_document(id: u64) -> Value {
    let created = response_frame(&call(&request_frame(id, r#"{"type":"new_document"}"#)));
    ok_result(&created, "new_document")["doc"].clone()
}

/// Sends request `id` with a command of `command_type` whose one field is `doc`, and decodes the response frame.
pub fn send_for_document(id: u64, command_type: &str, doc: &Value) -> ResponseFrame {
    response_frame(&call(&document_command_frame(id, command_type, doc)))
}

/// Decodes bytes the bridge returned as a response frame, failing the test with the decoder's message when they are not one.
pub fn response_frame(bytes: &[u8]) -> ResponseFrame {
    decode_response_frame(bytes)
        .unwrap_or_else(|error| panic!("the bridge returned a valid response frame: {error}"))
}

/// The JSON form of a response's outcome, for example `{"kind":"ok","type":"engine_info",...}`.
pub fn outcome_json(frame: &ResponseFrame) -> Value {
    serde_json::to_value(&frame.response.result).expect("an outcome always serializes")
}

/// The error of a response that must have failed.
pub fn error_info(frame: &ResponseFrame) -> &ErrorInfo {
    match &frame.response.result {
        Outcome::Error(info) => info,
        Outcome::Ok(_) => panic!("expected an error response, got {}", outcome_json(frame)),
    }
}

/// The id and error code of a response that must have failed.
pub fn error_of(frame: &ResponseFrame) -> (u64, &ErrorCode) {
    (frame.response.id, &error_info(frame).code)
}

/// The JSON form of a successful response's result, after checking that the request succeeded with a result of `type`.
pub fn ok_result(frame: &ResponseFrame, result_type: &str) -> Value {
    let outcome = outcome_json(frame);
    assert_eq!(
        outcome["kind"], "ok",
        "expected a successful {result_type} response, got {outcome}"
    );
    assert_eq!(
        outcome["type"], result_type,
        "expected a {result_type} result, got {outcome}"
    );
    outcome
}
