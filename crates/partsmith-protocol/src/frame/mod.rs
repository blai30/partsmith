//! The binary frame: magic, header length, JSON header, padding and raw buffers (protocol.md sections 2.1, 2.2, 3.1 and 3.2).

mod buffer;
mod decode;
mod encode;

pub use buffer::{Buffer, BufferIndex, BufferInfo, ElementType};
pub use decode::{decode_frame, decode_request_frame, decode_response_frame};
pub use encode::{encode_frame, encode_request_frame, encode_response_frame, error_response_frame};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, Event, MAX_HEADER_LENGTH, Message, PROTOCOL_VERSION, Response};

/// Bytes before the header: the magic and the header length field.
const PREFIX_LENGTH: usize = 8;

/// Alignment of the buffer section and of every buffer in it, in bytes.
const ALIGNMENT: usize = 8;

/// Zero bytes that follow a header of `header_length` bytes, so the buffer section starts at a multiple of 8.
fn header_padding(header_length: usize) -> usize {
    header_length.next_multiple_of(ALIGNMENT) - header_length
}

/// A decoded frame: its messages and the raw buffers they reference by index.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Messages in wire order.
    pub messages: Vec<Message>,
    /// Buffers in wire order; message fields of type `BufferIndex` index into this list.
    pub buffers: Vec<Buffer>,
}

/// The JSON header exactly as it appears on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FrameHeader {
    /// Always `PROTOCOL_VERSION`.
    pub protocol_version: u32,
    /// Messages in wire order.
    pub messages: Vec<Message>,
    /// Location and type of each raw buffer.
    pub buffers: Vec<BufferInfo>,
}

/// A decoded response frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseFrame {
    /// Events in wire order.
    pub events: Vec<Event>,
    /// The response.
    pub response: Response,
    /// Buffers referenced by the events and the response.
    pub buffers: Vec<Buffer>,
}

/// Why a frame could not be encoded or decoded.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FrameError {
    /// Fewer bytes than the layout requires.
    #[error("The frame is {length} bytes long, but its layout needs {expected} bytes.")]
    Truncated {
        /// Length of the frame.
        length: usize,
        /// Length the layout needs.
        expected: usize,
    },
    /// The first four bytes are not `PSM1`.
    #[error("The frame does not start with PSM1.")]
    BadMagic,
    /// The header length exceeds `MAX_HEADER_LENGTH`.
    #[error("The frame header is {length} bytes, more than the {MAX_HEADER_LENGTH}-byte limit.")]
    HeaderTooLarge {
        /// The header length.
        length: usize,
    },
    /// The header is not UTF-8 or not JSON.
    #[error("The frame header is not valid JSON: {message}")]
    InvalidHeader {
        /// What is wrong with the header text.
        message: String,
    },
    /// The header names another protocol version.
    #[error(
        "The frame uses protocol version {found}, but this engine speaks version {PROTOCOL_VERSION}. Reload the app."
    )]
    VersionMismatch {
        /// The version the header names.
        found: u32,
        /// The id of the header's first message, when it is an integer of at most `MAX_SAFE_INTEGER`.
        request_id: Option<u64>,
    },
    /// A padding byte is not zero.
    #[error("Frame padding must be zero bytes.")]
    NonZeroPadding,
    /// A buffer's offset, length or order breaks the layout rules.
    #[error("Buffer {index} is laid out incorrectly: {reason}")]
    BadBufferLayout {
        /// Position of the buffer in the header's buffer list.
        index: usize,
        /// Which rule the buffer breaks.
        reason: String,
    },
    /// Bytes follow the last buffer.
    #[error("The frame has {count} unexpected bytes after its last buffer.")]
    TrailingBytes {
        /// Number of extra bytes.
        count: usize,
    },
    /// The header is JSON but does not match the message schema or frame composition rules.
    #[error("The frame header is not a valid message: {message}")]
    InvalidMessage {
        /// What is wrong with the header.
        message: String,
        /// The id of the header's first message, when it is an integer of at most `MAX_SAFE_INTEGER`.
        request_id: Option<u64>,
    },
}

impl FrameError {
    /// The error code a host reports for this failure.
    pub fn code(&self) -> ErrorCode {
        match self {
            FrameError::VersionMismatch { .. } => ErrorCode::ProtocolVersionMismatch,
            FrameError::InvalidMessage { .. } => ErrorCode::MalformedRequest,
            _ => ErrorCode::MalformedFrame,
        }
    }

    /// The request id to echo, if one could be read from the header.
    pub fn request_id(&self) -> Option<u64> {
        match self {
            FrameError::VersionMismatch { request_id, .. }
            | FrameError::InvalidMessage { request_id, .. } => *request_id,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_padding_rounds_up_to_eight() {
        assert_eq!(header_padding(107), 5);
        assert_eq!(header_padding(185), 7);
        assert_eq!(header_padding(8), 0);
        assert_eq!(header_padding(0), 0);
    }

    #[test]
    fn version_mismatch_maps_to_its_own_code() {
        let error = FrameError::VersionMismatch {
            found: 2,
            request_id: None,
        };

        assert_eq!(error.code(), ErrorCode::ProtocolVersionMismatch);
    }

    #[test]
    fn invalid_message_maps_to_malformed_request() {
        let error = FrameError::InvalidMessage {
            message: "no".to_string(),
            request_id: None,
        };

        assert_eq!(error.code(), ErrorCode::MalformedRequest);
    }

    #[test]
    fn layout_errors_map_to_malformed_frame() {
        let errors = [
            FrameError::BadMagic,
            FrameError::NonZeroPadding,
            FrameError::TrailingBytes { count: 1 },
            FrameError::HeaderTooLarge { length: 1 },
        ];

        for error in errors {
            assert_eq!(error.code(), ErrorCode::MalformedFrame, "{error:?}");
        }
    }

    #[test]
    fn request_id_comes_from_version_and_message_errors_only() {
        let mismatch = FrameError::VersionMismatch {
            found: 2,
            request_id: Some(4),
        };
        let invalid = FrameError::InvalidMessage {
            message: "no".to_string(),
            request_id: Some(5),
        };

        assert_eq!(mismatch.request_id(), Some(4));
        assert_eq!(invalid.request_id(), Some(5));
        assert_eq!(FrameError::BadMagic.request_id(), None);
    }

    #[test]
    fn messages_are_the_spec_sentences() {
        assert_eq!(
            FrameError::HeaderTooLarge { length: 67_108_865 }.to_string(),
            "The frame header is 67108865 bytes, more than the 67108864-byte limit."
        );
        assert_eq!(
            FrameError::VersionMismatch {
                found: 2,
                request_id: Some(1)
            }
            .to_string(),
            "The frame uses protocol version 2, but this engine speaks version 1. Reload the app."
        );
        assert_eq!(
            FrameError::Truncated {
                length: 3,
                expected: 8
            }
            .to_string(),
            "The frame is 3 bytes long, but its layout needs 8 bytes."
        );
    }
}
