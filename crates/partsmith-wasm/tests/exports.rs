//! Acceptance tests for the C ABI exports `ps_alloc`, `ps_free` and `ps_call`, called natively (protocol.md section 2.10 and section 6, "WebAssembly bridge", tests 47 and 49).
//!
//! The exports trade raw pointers, so the `Block` helper below holds the only `unsafe` code of these tests: each block it touches comes from `ps_alloc` or `ps_call` with its exact length, which is the caller contract of section 2.10.

mod common;

use common::{error_of, example_a, response_frame};
use partsmith_protocol::ErrorCode;
use partsmith_wasm::call;

/// A block of bridge memory, allocated by `ps_alloc` or returned by `ps_call`, and freed with `ps_free` and its length when dropped.
struct Block {
    pointer: *mut u8,
    length: usize,
}

impl Block {
    /// Allocates `length` bytes with `ps_alloc`.
    #[allow(unsafe_code)]
    fn allocate(length: usize) -> Block {
        // The export is called through an `unsafe` function pointer, so the test compiles whether the export is declared safe or `unsafe`.
        let ps_alloc: unsafe extern "C" fn(usize) -> *mut u8 = partsmith_wasm::ps_alloc;
        // SAFETY: `ps_alloc` accepts any length and returns null or a block of `length` bytes.
        let pointer = unsafe { ps_alloc(length) };
        Block { pointer, length }
    }

    /// Allocates a block holding a copy of `bytes`.
    #[allow(unsafe_code)]
    fn holding(bytes: &[u8]) -> Block {
        let block = Block::allocate(bytes.len());
        assert!(
            !block.pointer.is_null(),
            "ps_alloc returned null for {} bytes",
            bytes.len()
        );
        // SAFETY: the block is a fresh, non-null allocation of exactly `bytes.len()` bytes that nothing else references.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), block.pointer, bytes.len()) };
        block
    }

    /// Runs `ps_call` on this block as the request; returns the response block, whose length is the one `ps_call` wrote to `*response_length`.
    #[allow(unsafe_code)]
    fn call(&self) -> Block {
        let ps_call: unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8 =
            partsmith_wasm::ps_call;
        let mut response_length = usize::MAX;
        // SAFETY: the request is a live block of `self.length` bytes (or null with length 0), and the length slot is a live local.
        let pointer = unsafe { ps_call(self.pointer, self.length, &mut response_length) };
        Block {
            pointer,
            length: response_length,
        }
    }

    /// A copy of the block's bytes.
    #[allow(unsafe_code)]
    fn bytes(&self) -> Vec<u8> {
        assert!(!self.pointer.is_null(), "the block is null");
        // SAFETY: a non-null block from `ps_alloc` or `ps_call` holds `length` initialized bytes until it is freed in `drop`.
        unsafe { std::slice::from_raw_parts(self.pointer, self.length) }.to_vec()
    }

    /// The block's address.
    fn address(&self) -> usize {
        self.pointer as usize
    }
}

impl Drop for Block {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let ps_free: unsafe extern "C" fn(*mut u8, usize) = partsmith_wasm::ps_free;
        // SAFETY: the pointer came from `ps_alloc` or `ps_call` with this length, or is null with length 0, and is freed only here.
        unsafe { ps_free(self.pointer, self.length) };
    }
}

#[test]
fn t047_alloc_returns_8_aligned_blocks_and_null_for_zero_and_free_accepts_both() {
    let block = Block::allocate(16);
    assert!(!block.pointer.is_null(), "ps_alloc(16) returned null");
    assert_eq!(block.address() % 8, 0, "ps_alloc(16) is not 8-aligned");
    // Frees the 16-byte block.
    drop(block);

    let empty = Block::allocate(0);
    assert!(empty.pointer.is_null(), "ps_alloc(0) did not return null");
    // `ps_free(null, 0)` does nothing.
    drop(empty);
}

#[test]
fn t049_ps_call_writes_and_returns_exactly_what_call_returns() {
    let request = Block::holding(&example_a());
    let expected = call(&example_a());

    let response = request.call();

    assert_eq!(response.length, expected.len());
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
