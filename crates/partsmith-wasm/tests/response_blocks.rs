//! Implementation tests for the blocks the exports return and the pointers and lengths they accept (protocol.md section 2.10), beyond the acceptance tests in `exports.rs`.
//!
//! The tests go through `common::block::Block`, which holds the only `unsafe` code of the tests.

use crate::common;

use common::block::Block;
use common::{
    document_command_frame, error_of, example_a, new_document, ok_result, response_frame,
    send_for_document,
};
use partsmith_protocol::ErrorCode;
use partsmith_wasm::call;

#[test]
fn alloc_of_a_length_beyond_the_address_space_returns_null() {
    let block = Block::allocate(usize::MAX);

    assert!(block.is_null());
}

#[test]
fn a_null_request_with_a_nonzero_length_reads_as_empty() {
    let request = Block::null(120);

    let response = request.call();

    assert_eq!(
        error_of(&response_frame(&response.bytes())),
        (0, &ErrorCode::MalformedFrame)
    );
}

#[test]
fn response_blocks_are_8_aligned_like_ps_alloc_blocks() {
    let request = Block::holding(&example_a());

    let response = request.call();

    assert_eq!(
        response.address() % 8,
        0,
        "the response block is not 8-aligned"
    );
    assert_eq!(response.bytes(), call(&example_a()));
}

#[test]
fn free_of_null_with_any_length_does_nothing() {
    let null = Block::null(64);

    // Dropping the block calls `ps_free(null, 64)`.
    drop(null);
}

#[test]
fn ps_call_without_a_length_slot_returns_null_and_handles_nothing() {
    let doc = new_document(1);
    let close = Block::holding(&document_command_frame(2, "close_document", &doc));

    let response = close.call_without_length_slot();

    assert!(
        response.is_null(),
        "ps_call returned a block it cannot size"
    );
    let saved = send_for_document(3, "save_document", &doc);
    ok_result(&saved, "save_document");
}
