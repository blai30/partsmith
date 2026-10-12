//! Output buffers that grow with `try_reserve` (spec section 3.1.1), so a buffer that cannot grow
//! returns `OutputTooLarge` instead of panicking.

use std::fmt;

use crate::IoError;

/// The least a growing buffer reserves at once. `try_reserve` also at least doubles the capacity,
/// so the chunks keep pace with the buffer.
const CHUNK: usize = 64 * 1024;

/// An empty buffer with room for exactly `length` bytes, for an output whose size is known.
pub(crate) fn exact_buffer(length: u64) -> Result<Vec<u8>, IoError> {
    let length = usize::try_from(length).map_err(|_| IoError::OutputTooLarge)?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(length)
        .map_err(|_| IoError::OutputTooLarge)?;
    Ok(buffer)
}

/// Makes room for `additional` more bytes in `buffer`, reserving at least [`CHUNK`] bytes when its
/// spare capacity is too small.
pub(crate) fn reserve(buffer: &mut Vec<u8>, additional: usize) -> Result<(), IoError> {
    if buffer.capacity() - buffer.len() >= additional {
        return Ok(());
    }
    buffer
        .try_reserve(additional.max(CHUNK))
        .map_err(|_| IoError::OutputTooLarge)
}

/// The text of a text format, growing through [`reserve`]. Writing to it fails only when the
/// buffer cannot grow.
#[derive(Debug, Default)]
pub(crate) struct TextOutput {
    /// The UTF-8 text written so far.
    bytes: Vec<u8>,
}

impl TextOutput {
    /// The text written so far.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The text written, as the bytes of the file.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl fmt::Write for TextOutput {
    fn write_str(&mut self, piece: &str) -> fmt::Result {
        reserve(&mut self.bytes, piece.len()).map_err(|_| fmt::Error)?;
        self.bytes.extend_from_slice(piece.as_bytes());
        Ok(())
    }
}

/// The error of a failed write to a [`TextOutput`]: its buffer could not grow.
pub(crate) fn too_large(_: fmt::Error) -> IoError {
    IoError::OutputTooLarge
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    /// One byte more than any buffer can hold.
    const BEYOND_ANY_BUFFER: usize = isize::MAX as usize + 1;

    #[test]
    fn reserve_beyond_isize_max_is_output_too_large() {
        let mut buffer = Vec::new();
        assert_eq!(
            reserve(&mut buffer, BEYOND_ANY_BUFFER),
            Err(IoError::OutputTooLarge)
        );
    }

    #[test]
    fn reserve_past_usize_max_is_output_too_large() {
        let mut buffer = vec![0; 8];
        assert_eq!(
            reserve(&mut buffer, usize::MAX),
            Err(IoError::OutputTooLarge)
        );
    }

    #[test]
    fn reserve_grows_by_at_least_a_chunk() {
        let mut buffer = Vec::new();
        reserve(&mut buffer, 1).expect("one byte fits");
        assert!(buffer.capacity() >= CHUNK);
    }

    #[test]
    fn reserve_grows_by_a_request_larger_than_a_chunk() {
        let mut buffer = Vec::new();
        reserve(&mut buffer, CHUNK + 1).expect("the request fits");
        assert!(buffer.capacity() > CHUNK);
    }

    #[test]
    fn reserve_keeps_a_buffer_with_enough_room() {
        let mut buffer = Vec::with_capacity(10);
        buffer.push(1);
        reserve(&mut buffer, 9).expect("the room is there");
        assert_eq!(buffer.capacity(), 10);
    }

    #[test]
    fn exact_buffer_has_exactly_the_length() {
        let buffer = exact_buffer(134).expect("134 bytes fit");
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 134);
    }

    #[test]
    fn exact_buffer_beyond_isize_max_is_output_too_large() {
        assert_eq!(
            exact_buffer(BEYOND_ANY_BUFFER as u64),
            Err(IoError::OutputTooLarge)
        );
    }

    #[test]
    fn exact_buffer_of_u64_max_is_output_too_large() {
        assert_eq!(exact_buffer(u64::MAX), Err(IoError::OutputTooLarge));
    }

    #[test]
    fn text_output_holds_what_is_written() {
        let (index, coordinate) = (1, 2.5_f32);
        let mut text = TextOutput::default();
        write!(text, "v {index} {coordinate}").expect("the text grows");
        text.write_char('\n').expect("the text grows");
        assert_eq!(text.as_bytes(), b"v 1 2.5\n");
        assert_eq!(text.into_bytes(), b"v 1 2.5\n");
    }

    #[test]
    fn failed_text_write_is_output_too_large() {
        assert_eq!(too_large(fmt::Error), IoError::OutputTooLarge);
    }
}
