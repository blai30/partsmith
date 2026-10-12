//! Implementation tests for the blocks `ps_call` returns and the request pointers it accepts (protocol.md section 2.10), beyond the acceptance tests in `exports.rs`.
//!
//! The exports trade raw pointers, so each test holds `unsafe` code; every block it touches comes from `ps_alloc` or `ps_call` with its exact length.

mod common;

use common::{error_of, example_a, response_frame};
use partsmith_protocol::ErrorCode;
use partsmith_wasm::{call, ps_alloc, ps_call, ps_free};

/// Runs `ps_call` on `request` and `request_length`, returns a copy of the response and the response block's address, and frees the response.
#[allow(unsafe_code)]
fn call_through_the_export(request: *const u8, request_length: usize) -> (Vec<u8>, usize) {
    let mut response_length = usize::MAX;
    // SAFETY: the request is null or a live block of `request_length` bytes, and the length slot is a live local.
    let pointer = unsafe { ps_call(request, request_length, &mut response_length) };
    assert!(!pointer.is_null(), "ps_call returned null");
    // SAFETY: a non-null response holds `response_length` initialized bytes until it is freed below.
    let response = unsafe { std::slice::from_raw_parts(pointer, response_length) }.to_vec();
    // SAFETY: the response came from `ps_call` with this length and is freed once.
    unsafe { ps_free(pointer, response_length) };
    (response, pointer as usize)
}

#[test]
fn a_null_request_with_a_nonzero_length_reads_as_empty() {
    let (response, _address) = call_through_the_export(std::ptr::null(), 120);

    assert_eq!(
        error_of(&response_frame(&response)),
        (0, &ErrorCode::MalformedFrame)
    );
}

#[test]
#[allow(unsafe_code)]
fn response_blocks_are_8_aligned_like_ps_alloc_blocks() {
    let request = example_a();
    let block = ps_alloc(request.len());
    assert!(!block.is_null(), "ps_alloc returned null");
    // SAFETY: `block` is a fresh allocation of `request.len()` bytes.
    unsafe { std::ptr::copy_nonoverlapping(request.as_ptr(), block, request.len()) };

    let (response, address) = call_through_the_export(block, request.len());
    // SAFETY: `block` came from `ps_alloc` with this length and is freed once.
    unsafe { ps_free(block, request.len()) };

    assert_eq!(address % 8, 0, "the response block is not 8-aligned");
    assert_eq!(response, call(&example_a()));
}

#[test]
#[allow(unsafe_code)]
fn free_of_null_with_any_length_does_nothing() {
    // SAFETY: a null pointer is never dereferenced, whatever the length.
    unsafe { ps_free(std::ptr::null_mut(), 64) };
}
