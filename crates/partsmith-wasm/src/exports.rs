//! The C ABI of the web core module: `ps_alloc`, `ps_free` and `ps_call` (protocol.md section 2.10).
//!
//! These three functions are the only items in the crate allowed to use `unsafe` (ADR 0010). They convert between raw pointers and slices and leave the frames to [`call`](crate::call).

use std::alloc::{self, Layout};
use std::{ptr, slice};

/// Alignment of every block the bridge hands out. Frame buffers start at multiples of 8 from the frame start, so typed array views of a frame inside a block are aligned.
const BLOCK_ALIGNMENT: usize = 8;

/// The layout of a block of `length` bytes, or `None` when `length` is 0 or larger than any allocation can be.
fn block_layout(length: usize) -> Option<Layout> {
    if length == 0 {
        return None;
    }
    Layout::from_size_align(length, BLOCK_ALIGNMENT).ok()
}

/// Allocates `length` bytes aligned to 8 and returns the block's address.
///
/// Returns null when `length` is 0 or the allocation fails. The caller frees the block with [`ps_free`] and the same length.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn ps_alloc(length: usize) -> *mut u8 {
    match block_layout(length) {
        // SAFETY: `block_layout` never returns a zero-sized layout, which is the one requirement of `alloc`.
        Some(layout) => unsafe { alloc::alloc(layout) },
        None => ptr::null_mut(),
    }
}

/// Frees a block returned by [`ps_alloc`] or [`ps_call`]. `ps_free(null, 0)` does nothing.
///
/// # Safety
///
/// `pointer` is null, or a block from `ps_alloc` or `ps_call` that is not freed yet. `length` is the block's length: the one passed to `ps_alloc`, or the one `ps_call` wrote to its length slot.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_free(pointer: *mut u8, length: usize) {
    if !pointer.is_null()
        && let Some(layout) = block_layout(length)
    {
        // SAFETY: the caller passes a live block with its length, and `ps_alloc` (which `ps_call` also uses) allocated it with exactly this layout.
        unsafe { alloc::dealloc(pointer, layout) };
    }
}

/// Handles one request frame and returns a newly allocated response frame, writing the response's length to `*response_length`.
///
/// Never frees the request, and answers every request, including bytes that are not a frame, with exactly one response frame (see [`call`](crate::call)). A null `request` reads as an empty request. Returns null and writes 0 only when the response block cannot be allocated. The caller frees the response with [`ps_free`] and the written length.
///
/// # Safety
///
/// `request` is null, or points to `request_length` initialized bytes that nothing writes during the call. `response_length` points to a writable `usize`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_call(
    request: *const u8,
    request_length: usize,
    response_length: *mut usize,
) -> *mut u8 {
    let request = if request.is_null() {
        &[]
    } else {
        // SAFETY: the caller passes `request_length` initialized bytes at `request` that stay unchanged until the call returns, and the slice does not outlive the call.
        unsafe { slice::from_raw_parts(request, request_length) }
    };
    let response = crate::call(request);
    let block = ps_alloc(response.len());
    let length = if block.is_null() {
        0
    } else {
        // SAFETY: `block` is a fresh allocation of `response.len()` bytes, so it is valid for the copy and cannot overlap the response.
        unsafe { ptr::copy_nonoverlapping(response.as_ptr(), block, response.len()) };
        response.len()
    };
    // SAFETY: the caller passes a writable length slot.
    unsafe { response_length.write(length) };
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_block_has_no_layout() {
        assert_eq!(block_layout(0), None);
    }

    #[test]
    fn a_one_byte_block_is_aligned_to_8() {
        let layout = block_layout(1).expect("one byte can be allocated");

        assert_eq!((layout.size(), layout.align()), (1, 8));
    }

    #[test]
    fn a_block_keeps_its_exact_length() {
        let layout = block_layout(13).expect("13 bytes can be allocated");

        assert_eq!(layout.size(), 13);
    }

    #[test]
    fn a_length_beyond_the_address_space_has_no_layout() {
        assert_eq!(block_layout(usize::MAX), None);
    }

    #[test]
    fn the_largest_alignable_length_has_a_layout() {
        let largest = isize::MAX as usize - (BLOCK_ALIGNMENT - 1);

        assert!(block_layout(largest).is_some());
    }

    #[test]
    fn alloc_of_zero_bytes_returns_null() {
        assert!(ps_alloc(0).is_null());
    }

    #[test]
    fn alloc_of_a_length_beyond_the_address_space_returns_null() {
        assert!(ps_alloc(usize::MAX).is_null());
    }
}
