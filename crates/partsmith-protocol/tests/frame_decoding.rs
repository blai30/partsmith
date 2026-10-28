//! Acceptance tests for strict frame decoding and frame composition (protocol.md section 6, "Frame encoding and decoding", tests 9 to 27), for the edits that requests carry ("Edits in requests", test 85), and for the closed profiles in results ("Closed types in results and events", test 87 (b)).

use crate::common;

use common::examples::LIST_PRINTER_PROFILES_RESULT;
use common::{
    EXAMPLE_A_HEADER, EXAMPLE_B_HEADER, example_a, example_b, frame_bytes, frame_from_header,
    frame_with_messages, header_bytes, parse_json, with_member, without_member,
};
use partsmith_model::Edit;
use partsmith_protocol::{
    Command, ErrorCode, ErrorInfo, Event, FrameError, Outcome, Response, ResponseFrame,
    decode_frame, decode_request_frame, decode_response_frame, error_response_frame,
};
use serde_json::{Value, json};

/// A request message (id 1, `engine_info`).
const REQUEST: &str = r#"{"kind":"request","id":1,"command":{"type":"engine_info"}}"#;
/// A second request message (id 2, `undo`).
const SECOND_REQUEST: &str = r#"{"kind":"request","id":2,"command":{"type":"undo","doc":1}}"#;
/// An event message (`datum_plane` of a failed datum, so without placement).
const EVENT: &str =
    r#"{"kind":"event","type":"datum_plane","doc":1,"revision":2,"feature":"f-dp1"}"#;
/// A second event message (`body_mesh` of a preview without mesh).
const SECOND_EVENT: &str = r#"{"kind":"event","type":"body_mesh","doc":1,"revision":5,"body":"b-1","layer":"preview","mesh_id":9}"#;
/// A response message (id 5, `close_document` succeeded).
const RESPONSE: &str =
    r#"{"kind":"response","id":5,"result":{"kind":"ok","type":"close_document"}}"#;

/// The messages `items` as a JSON array text.
fn messages(items: &[&str]) -> String {
    format!("[{}]", items.join(","))
}

/// A message text without its envelope tag `kind`.
fn without_kind(message: &str) -> serde_json::Value {
    without_member(&parse_json(message), "kind")
}

/// The event inside an event message text.
fn event(message: &str) -> Event {
    serde_json::from_value(without_kind(message)).expect("the event deserializes")
}

/// A request frame whose one request (id 5) is an `apply_edits` labeled `label` with the one edit `edit`, a JSON text.
fn apply_edits_frame(label: &str, edit: &str) -> Vec<u8> {
    frame_with_messages(&format!(
        r#"[{{"kind":"request","id":5,"command":{{"type":"apply_edits","doc":1,"label":"{label}","edits":[{edit}]}}}}]"#
    ))
}

/// A response frame whose one message is a successful response (id 1) with `result`, a result as JSON.
fn result_frame(result: &Value) -> Vec<u8> {
    let outcome = with_member(result, "kind", json!("ok"));
    frame_with_messages(&json!([{"kind": "response", "id": 1, "result": outcome}]).to_string())
}

/// The response inside a response message text.
fn response(message: &str) -> Response {
    serde_json::from_value(without_kind(message)).expect("the response deserializes")
}

#[test]
fn t009_three_bytes_are_truncated() {
    assert_eq!(
        decode_frame(b"PSM"),
        Err(FrameError::Truncated {
            length: 3,
            expected: 8
        })
    );
}

#[test]
fn t010_wrong_magic_is_rejected() {
    let mut bytes = example_a();
    bytes[0] = 0x51;

    assert_eq!(decode_frame(&bytes), Err(FrameError::BadMagic));
}

#[test]
fn t011_header_length_above_the_limit_is_rejected() {
    let mut bytes = b"PSM1".to_vec();
    bytes.extend_from_slice(&67_108_865_u32.to_le_bytes());

    assert_eq!(
        decode_frame(&bytes),
        Err(FrameError::HeaderTooLarge { length: 67_108_865 })
    );
}

#[test]
fn t012_example_a_without_its_last_two_bytes_is_truncated() {
    let bytes = example_a();
    let shortened = &bytes[..bytes.len() - 2];

    // The layout needs 8 + 107 header bytes + 5 padding bytes = 120 bytes.
    assert_eq!(
        decode_frame(shortened),
        Err(FrameError::Truncated {
            length: 118,
            expected: 120
        })
    );
}

#[test]
fn t013_header_that_is_not_utf8_is_invalid() {
    let bytes = frame_bytes(&[0xff, 0xfe], &[]);

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::InvalidHeader { .. })),
        "got {result:?}"
    );
}

#[test]
fn t014_header_that_is_not_json_is_invalid() {
    let bytes = frame_from_header(r#"{"protocol_version":1,"#, &[]);

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::InvalidHeader { .. })),
        "got {result:?}"
    );
}

#[test]
fn t015_other_protocol_version_is_a_version_mismatch_with_the_request_id() {
    let header = EXAMPLE_A_HEADER.replace(r#""protocol_version":1"#, r#""protocol_version":2"#);
    let bytes = frame_from_header(&header, &[]);

    let error = decode_frame(&bytes).expect_err("protocol version 2 is rejected");

    assert_eq!(
        error,
        FrameError::VersionMismatch {
            found: 2,
            request_id: Some(1)
        }
    );
    assert_eq!(error.code(), ErrorCode::ProtocolVersionMismatch);
}

#[test]
fn t016_protocol_version_as_a_string_is_an_invalid_message() {
    let header = EXAMPLE_A_HEADER.replace(r#""protocol_version":1"#, r#""protocol_version":"1""#);
    let bytes = frame_from_header(&header, &[]);

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::InvalidMessage { .. })),
        "got {result:?}"
    );
}

#[test]
fn t017_nonzero_padding_byte_is_rejected() {
    let mut bytes = example_a();
    // Example A's header ends at byte 115; bytes 115 to 119 are padding.
    bytes[117] = 0x01;

    assert_eq!(decode_frame(&bytes), Err(FrameError::NonZeroPadding));
}

#[test]
fn t018_buffer_offset_other_than_the_rule_is_a_bad_layout() {
    let header = EXAMPLE_B_HEADER.replace(r#""offset":0"#, r#""offset":8"#);
    let mut bytes = example_b();
    bytes[8..8 + header.len()].copy_from_slice(header.as_bytes());

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::BadBufferLayout { index: 0, .. })),
        "got {result:?}"
    );
}

#[test]
fn t019_f32_buffer_length_not_a_multiple_of_4_is_a_bad_layout() {
    let header = r#"{"protocol_version":1,"messages":[],"buffers":[{"offset":0,"length":6,"element_type":"f32"}]}"#;
    let bytes = frame_from_header(header, &[0; 6]);

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::BadBufferLayout { index: 0, .. })),
        "got {result:?}"
    );
}

#[test]
fn t020_second_buffer_overlapping_the_first_is_a_bad_layout() {
    let header = r#"{"protocol_version":1,"messages":[],"buffers":[{"offset":0,"length":16,"element_type":"u8"},{"offset":8,"length":4,"element_type":"u32"}]}"#;
    let bytes = frame_from_header(header, &[0; 20]);

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::BadBufferLayout { index: 1, .. })),
        "got {result:?}"
    );
}

#[test]
fn t021_example_b_without_its_last_byte_is_truncated() {
    let bytes = example_b();
    let shortened = &bytes[..bytes.len() - 1];

    // The buffer ends at 200 + 14 = 214.
    assert_eq!(
        decode_frame(shortened),
        Err(FrameError::Truncated {
            length: 213,
            expected: 214
        })
    );
}

#[test]
fn t022_bytes_after_the_last_buffer_or_the_padding_are_trailing() {
    let mut with_buffer = example_b();
    with_buffer.push(0);
    assert_eq!(
        decode_frame(&with_buffer),
        Err(FrameError::TrailingBytes { count: 1 })
    );

    let mut without_buffers = example_a();
    without_buffers.extend_from_slice(&[0; 8]);
    assert_eq!(
        decode_frame(&without_buffers),
        Err(FrameError::TrailingBytes { count: 8 })
    );
}

#[test]
fn t023_unknown_element_type_is_an_invalid_message() {
    let header = r#"{"protocol_version":1,"messages":[],"buffers":[{"offset":0,"length":8,"element_type":"f64"}]}"#;
    let bytes = frame_from_header(header, &[0; 8]);

    let result = decode_frame(&bytes);

    assert!(
        matches!(result, Err(FrameError::InvalidMessage { .. })),
        "got {result:?}"
    );
}

#[test]
fn t024_unknown_command_is_an_invalid_message_with_the_request_id() {
    let bytes =
        frame_with_messages(r#"[{"kind":"request","id":3,"command":{"type":"no_such_command"}}]"#);

    let error = decode_frame(&bytes).expect_err("an unknown command is rejected");

    assert!(
        matches!(
            error,
            FrameError::InvalidMessage {
                request_id: Some(3),
                ..
            }
        ),
        "got {error:?}"
    );
    assert_eq!(error.code(), ErrorCode::MalformedRequest);
}

#[test]
fn t025_request_frame_must_hold_exactly_one_request() {
    let frames = [
        ("zero messages", frame_with_messages("[]")),
        (
            "two requests",
            frame_with_messages(&messages(&[REQUEST, SECOND_REQUEST])),
        ),
        ("one event", frame_with_messages(&messages(&[EVENT]))),
    ];

    for (case, bytes) in frames {
        let result = decode_request_frame(&bytes);
        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "{case}: got {result:?}"
        );
    }
}

#[test]
fn t026_response_frame_is_events_then_one_response() {
    let accepted = decode_response_frame(&frame_with_messages(&messages(&[
        EVENT,
        SECOND_EVENT,
        RESPONSE,
    ])));
    let expected = ResponseFrame {
        events: vec![event(EVENT), event(SECOND_EVENT)],
        response: response(RESPONSE),
        buffers: Vec::new(),
    };
    assert_eq!(accepted, Ok(expected));

    let rejected = [
        ("response then event", messages(&[RESPONSE, EVENT])),
        ("event only", messages(&[EVENT])),
        ("two responses", messages(&[RESPONSE, RESPONSE])),
    ];
    for (case, items) in rejected {
        let result = decode_response_frame(&frame_with_messages(&items));
        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "{case}: got {result:?}"
        );
    }
}

#[test]
fn t027_error_response_frame_has_the_exact_header() {
    let message = "The frame does not start with PSM1.";

    let bytes = error_response_frame(0, ErrorCode::MalformedFrame, message);

    assert_eq!(
        std::str::from_utf8(header_bytes(&bytes)).expect("the header is UTF-8"),
        r#"{"protocol_version":1,"messages":[{"kind":"response","id":0,"result":{"kind":"error","code":"malformed_frame","message":"The frame does not start with PSM1."}}],"buffers":[]}"#
    );
    let expected = ResponseFrame {
        events: Vec::new(),
        response: Response {
            id: 0,
            result: Outcome::Error(ErrorInfo {
                code: ErrorCode::MalformedFrame,
                message: message.to_string(),
                edit_index: None,
                span: None,
                detail: None,
                actions: Vec::new(),
            }),
        },
        buffers: Vec::new(),
    };
    assert_eq!(decode_response_frame(&bytes), Ok(expected));
}

#[test]
fn t085_an_unknown_edit_field_is_an_invalid_message_with_the_request_id() {
    for edit in [
        r#"{"type":"update_parameter","parameter":{"id":"p-h","name":"height","expr":"50 mm"},"extra":1}"#,
        // Every field of set_rollback is optional, so the unknown field is all there is to reject.
        r#"{"type":"set_rollback","extra":1}"#,
    ] {
        let error =
            decode_request_frame(&apply_edits_frame("Change height", edit)).expect_err(edit);

        assert!(
            matches!(
                error,
                FrameError::InvalidMessage {
                    request_id: Some(5),
                    ..
                }
            ),
            "{edit}: got {error:?}"
        );
        assert_eq!(error.code(), ErrorCode::MalformedRequest, "{edit}");
    }
}

#[test]
fn t085_a_feature_record_in_an_edit_keeps_unknown_keys_as_params() {
    let edit = r#"{"type":"add_feature","index":0,"feature":{"id":"f-box1","type":"box","body":"b-1","extra":1}}"#;

    let (request, _) = decode_request_frame(&apply_edits_frame("Add box", edit))
        .expect("a feature record with an unknown key decodes");

    let Command::ApplyEdits { edits, .. } = request.command else {
        panic!("the request is apply_edits, got {:?}", request.command);
    };
    let [Edit::AddFeature { feature, .. }] = edits.as_slice() else {
        panic!("the one edit is add_feature, got {edits:?}");
    };
    assert_eq!(
        serde_json::to_string(&feature.params).expect("params serialize"),
        r#"{"body":"b-1","extra":1}"#
    );
}

#[test]
fn t087_a_list_result_with_an_unknown_member_decodes_as_without_it() {
    let result = parse_json(LIST_PRINTER_PROFILES_RESULT);
    let expected =
        decode_response_frame(&result_frame(&result)).expect("the result example decodes");

    let decoded = decode_response_frame(&result_frame(&with_member(&result, "extra", json!(1))));

    assert_eq!(decoded, Ok(expected));
}

#[test]
fn t087_a_listed_profile_with_an_unknown_member_fails_to_decode() {
    let mut result = parse_json(LIST_PRINTER_PROFILES_RESULT);
    result["profiles"][0]["extra"] = json!(1);

    let decoded = decode_response_frame(&result_frame(&result));

    assert!(
        matches!(decoded, Err(FrameError::InvalidMessage { .. })),
        "got {decoded:?}"
    );
}
