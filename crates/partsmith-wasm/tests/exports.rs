//! Acceptance tests for the C ABI exports `ps_alloc`, `ps_free` and `ps_call`, called natively (protocol.md section 2.10 and section 6, "WebAssembly bridge", tests 47 and 49).
//!
//! The exports trade raw pointers, so the tests go through `common::block::Block`, which holds the only `unsafe` code of the tests: each block it touches comes from `ps_alloc` or `ps_call` with its exact length, which is the caller contract of section 2.10.

use crate::common;

use common::block::Block;
use common::{error_of, example_a, response_frame};
use partsmith_protocol::ErrorCode;
use partsmith_wasm::call;

#[test]
fn t047_alloc_returns_8_aligned_blocks_and_null_for_zero_and_free_accepts_both() {
    let block = Block::allocate(16);
    assert!(!block.is_null(), "ps_alloc(16) returned null");
    assert_eq!(block.address() % 8, 0, "ps_alloc(16) is not 8-aligned");
    // Frees the 16-byte block.
    drop(block);

    let empty = Block::allocate(0);
    assert!(empty.is_null(), "ps_alloc(0) did not return null");
    // `ps_free(null, 0)` does nothing.
    drop(empty);
}

#[test]
fn t049_ps_call_writes_and_returns_exactly_what_call_returns() {
    let request = Block::holding(&example_a());
    let expected = call(&example_a());

    let response = request.call();

    assert_eq!(response.length(), expected.len());
    assert_eq!(response.bytes(), expected);
}

#[test]
fn ps_call_leaves_the_request_block_unchanged_and_allocated() {
    let request = Block::holding(&example_a());

    let _response = request.call();

    // Reading the request and freeing it on drop would fail if `ps_call` had freed it.
    assert_eq!(request.bytes(), example_a());
}

#[test]
fn ps_call_with_an_empty_request_answers_malformed_frame_with_id_0() {
    let request = Block::allocate(0);

    let response = request.call();

    assert_eq!(
        error_of(&response_frame(&response.bytes())),
        (0, &ErrorCode::MalformedFrame)
    );
}

#[test]
fn alloc_blocks_of_many_sizes_are_8_aligned_and_writable() {
    for length in [1, 3, 8, 13, 64, 1000, 65_536] {
        let block = Block::holding(&vec![0xa5; length]);

        assert_eq!(
            block.address() % 8,
            0,
            "ps_alloc({length}) is not 8-aligned"
        );
        assert_eq!(block.bytes(), vec![0xa5; length]);
    }
}
