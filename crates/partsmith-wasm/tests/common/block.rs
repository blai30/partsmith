//! `Block`, the one place where the tests and the benchmark of this crate touch raw pointers (ADR 0010).
//!
//! Every block it touches comes from `ps_alloc` or `ps_call` with its exact length, or is null, which is the caller contract of protocol.md section 2.10.

// The test modules and the bench each use only some of these methods.
#![allow(dead_code)]

use std::ptr;

use partsmith_wasm::{ps_alloc, ps_call, ps_free};

/// A block of bridge memory, allocated by `ps_alloc` or returned by `ps_call`, and freed with `ps_free` and its length when dropped.
pub struct Block {
    pointer: *mut u8,
    length: usize,
}

impl Block {
    /// Allocates `length` bytes with `ps_alloc`.
    pub fn allocate(length: usize) -> Block {
        Block {
            pointer: ps_alloc(length),
            length,
        }
    }

    /// Allocates a block holding a copy of `bytes`.
    #[allow(unsafe_code)]
    pub fn holding(bytes: &[u8]) -> Block {
        let block = Block::allocate(bytes.len());
        assert!(
            !block.is_null(),
            "ps_alloc returned null for {} bytes",
            bytes.len()
        );
        // SAFETY: the block is a fresh, non-null allocation of exactly `bytes.len()` bytes that nothing else references.
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), block.pointer, bytes.len()) };
        block
    }

    /// A null pointer that claims `length` bytes, as a careless caller might pass it.
    pub fn null(length: usize) -> Block {
        Block {
            pointer: ptr::null_mut(),
            length,
        }
    }

    /// Runs `ps_call` with this block as the request; returns the response block, whose length is the one `ps_call` wrote to its length slot.
    #[allow(unsafe_code)]
    pub fn call(&self) -> Block {
        let mut response_length = usize::MAX;
        // SAFETY: the request is null or a live block of `self.length` bytes, and the length slot is a live local.
        let pointer = unsafe { ps_call(self.pointer, self.length, &mut response_length) };
        Block {
            pointer,
            length: response_length,
        }
    }

    /// Runs `ps_call` with this block as the request and a null length slot; returns the pointer `ps_call` returned.
    #[allow(unsafe_code)]
    pub fn call_without_length_slot(&self) -> *mut u8 {
        // SAFETY: the request is null or a live block of `self.length` bytes, and `ps_call` accepts a null length slot.
        unsafe { ps_call(self.pointer, self.length, ptr::null_mut()) }
    }

    /// A copy of the block's bytes.
    #[allow(unsafe_code)]
    pub fn bytes(&self) -> Vec<u8> {
        assert!(!self.is_null(), "the block is null");
        // SAFETY: a non-null block from `ps_alloc` or `ps_call` holds `length` initialized bytes until it is freed in `drop`.
        unsafe { std::slice::from_raw_parts(self.pointer, self.length) }.to_vec()
    }

    /// Whether the block is a null pointer.
    pub fn is_null(&self) -> bool {
        self.pointer.is_null()
    }

    /// The block's length in bytes.
    pub fn length(&self) -> usize {
        self.length
    }

    /// The block's address.
    pub fn address(&self) -> usize {
        self.pointer as usize
    }
}

impl Drop for Block {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: the pointer is null, or came from `ps_alloc` or `ps_call` with this length and is freed only here.
        unsafe { ps_free(self.pointer, self.length) };
    }
}
