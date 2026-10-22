//! Acceptance tests for frame encoding and round trips (protocol.md section 6, "Frame encoding and decoding", tests 1 to 8).

use crate::common;

use common::examples::{
    ERROR_OUTCOMES, FEATURE_STATUS_R03, MESSAGE_EXAMPLES, R16_DIAGNOSTIC, REGION_REPAIR,
    all_commands, all_results,
};
use common::{
    EXAMPLE_B_BUFFER, buffer_start, example_a, example_b, header_json, message, parse_json,
};
use partsmith_protocol::{
    Buffer, BufferIndex, Command, CommandResult, DocHandle, Frame, MAX_SAFE_INTEGER, Message,
    Outcome, Request, Response, decode_frame, decode_request_frame, encode_frame,
    encode_request_frame,
};
use proptest::prelude::*;

/// A frame with no messages and the given buffers.
fn frame_of_buffers(buffers: Vec<Buffer>) -> Frame {
    Frame {
        messages: Vec::new(),
        buffers,
    }
}

/// One message of every kind the spec shows: a request per command example, an `ok` response per result example, the error responses, every event example, and the diagnostic events.
fn message_pool() -> Vec<Message> {
    let requests = all_commands().into_iter().map(|text| {
        let command: Command = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("the command deserializes ({error}): {text}"));
        Message::Request(Request { id: 1, command })
    });
    let ok_responses = all_results().into_iter().map(|text| {
        let result: CommandResult = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("the result deserializes ({error}): {text}"));
        Message::Response(Response {
            id: 1,
            result: Outcome::Ok(result),
        })
    });
    let error_responses = ERROR_OUTCOMES.into_iter().map(|text| {
        let result: Outcome = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("the outcome deserializes ({error}): {text}"));
        Message::Response(Response { id: 1, result })
    });
    let examples = MESSAGE_EXAMPLES.into_iter().map(message);
    requests
        .chain(ok_responses)
        .chain(error_responses)
        .chain(examples)
        .chain(diagnostic_events())
        .collect()
}

/// `recomputed` events with the diagnostics and repairs of ADR 0024: the R03 fillet with its cause and element repair beside an extrude with the region repair of test 68, and the R16 fillet with its actions, cause and reason.
fn diagnostic_events() -> [Message; 2] {
    [
        message(&format!(
            r#"{{"kind": "event", "type": "recomputed", "doc": 1, "revision": 2, "features": [{FEATURE_STATUS_R03}, {{"id": "f-ext1", "status": "warning", "messages": [], "repairs": [{REGION_REPAIR}]}}], "parameters": [], "diagnostics": []}}"#
        )),
        message(&format!(
            r#"{{"kind": "event", "type": "recomputed", "doc": 1, "revision": 7, "features": [{{"id": "f-fil1", "status": "error", "messages": [{R16_DIAGNOSTIC}], "repairs": []}}], "parameters": [], "diagnostics": []}}"#
        )),
    ]
}

/// Any message of the pool, with a random request or response id.
fn any_message() -> impl Strategy<Value = Message> {
    (prop::sample::select(message_pool()), 0..=MAX_SAFE_INTEGER).prop_map(|(mut message, id)| {
        match &mut message {
            Message::Request(request) => request.id = id,
            Message::Response(response) => response.id = id,
            Message::Event(_) => {}
        }
        message
    })
}

/// Finite `f32` values of both signs: normal, subnormal and zero (section 2.2: `f32` values are finite).
fn finite_f32() -> impl Strategy<Value = f32> {
    prop::num::f32::POSITIVE
        | prop::num::f32::NEGATIVE
        | prop::num::f32::NORMAL
        | prop::num::f32::SUBNORMAL
        | prop::num::f32::ZERO
}

/// A buffer of a random element type with 0 to 100 elements.
fn any_buffer() -> impl Strategy<Value = Buffer> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..=100).prop_map(Buffer::U8),
        prop::collection::vec(any::<u32>(), 0..=100).prop_map(Buffer::U32),
        prop::collection::vec(finite_f32(), 0..=100).prop_map(Buffer::F32),
    ]
}

/// A frame with up to 8 messages and up to 6 buffers.
fn any_frame() -> impl Strategy<Value = Frame> {
    (
        prop::collection::vec(any_message(), 0..=8),
        prop::collection::vec(any_buffer(), 0..=6),
    )
        .prop_map(|(messages, buffers)| Frame { messages, buffers })
}

#[test]
fn t001_engine_info_request_encodes_to_example_a() {
    let request = Request {
        id: 1,
        command: Command::EngineInfo,
    };

    let bytes = encode_request_frame(&request, &[]).expect("a small request encodes");

    assert_eq!(bytes.len(), 120);
    assert_eq!(bytes, example_a());
}

#[test]
fn t002_example_a_decodes_to_engine_info_request() {
    let decoded = decode_request_frame(&example_a());

    let expected = Request {
        id: 1,
        command: Command::EngineInfo,
    };
    assert_eq!(decoded, Ok((expected, Vec::new())));
}

#[test]
fn t003_import_step_request_with_buffer_encodes_to_example_b() {
    let request = Request {
        id: 2,
        command: Command::ImportStep {
            doc: DocHandle(1),
            data: BufferIndex(0),
            name: Some("Bracket".to_string()),
        },
    };

    let bytes = encode_request_frame(&request, &[Buffer::U8(EXAMPLE_B_BUFFER.to_vec())])
        .expect("a small request encodes");

    assert_eq!(bytes.len(), 214);
    assert_eq!(bytes, example_b());
}

#[test]
fn t004_example_b_buffer_is_read_from_absolute_offset_200() {
    let mut bytes = example_b();
    assert_eq!(&bytes[200..], EXAMPLE_B_BUFFER);

    let frame = decode_frame(&bytes).expect("example B is a valid frame");
    assert_eq!(frame.buffers, vec![Buffer::U8(EXAMPLE_B_BUFFER.to_vec())]);

    // Changing the byte at offset 200 changes the first byte of the decoded buffer, so the decoder read the buffer there.
    bytes[200] = b'X';
    let changed = decode_frame(&bytes).expect("the changed frame is still valid");
    assert_eq!(
        changed.buffers,
        vec![Buffer::U8(b"XSO-10303-21;\n".to_vec())]
    );
}

#[test]
fn t005_second_buffer_starts_at_the_next_multiple_of_8_after_a_zero_gap() {
    let frame = frame_of_buffers(vec![
        Buffer::U8(EXAMPLE_B_BUFFER.to_vec()),
        Buffer::U32(vec![1, 2, 3]),
    ]);

    let bytes = encode_frame(&frame).expect("a small frame encodes");

    let header = header_json(&bytes);
    assert_eq!(
        header["buffers"],
        parse_json(
            r#"[{"offset":0,"length":14,"element_type":"u8"},{"offset":16,"length":12,"element_type":"u32"}]"#
        )
    );
    let start = buffer_start(&bytes);
    assert_eq!(&bytes[start..start + 14], EXAMPLE_B_BUFFER);
    assert_eq!(&bytes[start + 14..start + 16], &[0, 0]);
    assert_eq!(&bytes[start + 16..], &[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(bytes.len(), start + 28);
}

#[test]
fn t006_buffer_elements_are_written_little_endian() {
    let float_bytes =
        encode_frame(&frame_of_buffers(vec![Buffer::F32(vec![1.0])])).expect("a frame encodes");
    assert_eq!(
        &float_bytes[buffer_start(&float_bytes)..],
        &[0x00, 0x00, 0x80, 0x3f]
    );

    let integer_bytes =
        encode_frame(&frame_of_buffers(vec![Buffer::U32(vec![1])])).expect("a frame encodes");
    assert_eq!(
        &integer_bytes[buffer_start(&integer_bytes)..],
        &[0x01, 0x00, 0x00, 0x00]
    );
}

proptest! {
    #[test]
    fn t007_decoding_an_encoded_frame_returns_the_frame(frame in any_frame()) {
        let bytes = encode_frame(&frame).expect("a frame with a small header encodes");

        let decoded = decode_frame(&bytes);

        prop_assert_eq!(decoded, Ok(frame));
    }

    #[test]
    fn t008_reencoding_a_decoded_frame_returns_the_same_bytes(frame in any_frame()) {
        let bytes = encode_frame(&frame).expect("a frame with a small header encodes");

        let decoded = decode_frame(&bytes).expect("an encoded frame decodes");
        let reencoded = encode_frame(&decoded).expect("a decoded frame encodes");

        prop_assert_eq!(reencoded, bytes);
    }
}
