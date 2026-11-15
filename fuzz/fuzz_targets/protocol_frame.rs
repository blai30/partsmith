//! Fuzzes the frame decoder, `partsmith_protocol::decode_frame` (delivery.md section 2.15). Each
//! input first goes through the engine's own call, `decode_request_frame`. A frame that decodes
//! and encodes must decode and encode again to the same bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The engine makes this call on every frame a host sends (partsmith-engine's dispatch.rs).
    // Every Ok and every Err passes.
    let _ = partsmith_protocol::decode_request_frame(data);
    let Ok(frame) = partsmith_protocol::decode_frame(data) else {
        return;
    };
    // A frame whose header is over MAX_HEADER_LENGTH does not encode. Nothing more is checked.
    let Ok(first) = partsmith_protocol::encode_frame(&frame) else {
        return;
    };
    // The bytes are compared, not the frames. An f32 buffer can hold a NaN, and a NaN equals
    // nothing.
    let again = partsmith_protocol::decode_frame(&first)
        .unwrap_or_else(|error| panic!("partsmith_protocol::decode_frame(&first) failed: {error}"));
    let second = partsmith_protocol::encode_frame(&again)
        .unwrap_or_else(|error| panic!("partsmith_protocol::encode_frame(&again) failed: {error}"));
    assert!(
        second == first,
        "partsmith_protocol::encode_frame(&again) wrote other bytes"
    );
});
