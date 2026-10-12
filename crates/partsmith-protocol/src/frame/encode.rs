//! Frame encoding (protocol.md section 3.1).

use serde::Serialize;

use super::{ALIGNMENT, Buffer, BufferInfo, Frame, FrameError, PREFIX_LENGTH, header_padding};
use crate::{
    ErrorCode, ErrorInfo, Event, MAGIC, MAX_HEADER_LENGTH, Message, Outcome, PROTOCOL_VERSION,
    Request, Response,
};

/// Longest message `error_response_frame` sends, in bytes. JSON escaping at most sextuples a string, so a header holding such a message stays below `MAX_HEADER_LENGTH`.
const MAX_ERROR_MESSAGE_LENGTH: usize = MAX_HEADER_LENGTH / 8;

/// A message to serialize, borrowed so encoding never copies requests, responses or events.
/// It serializes exactly like the `Message` with the same content.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MessageRef<'a> {
    /// A request.
    Request(&'a Request),
    /// A response.
    Response(&'a Response),
    /// An event.
    Event(&'a Event),
}

impl<'a> From<&'a Message> for MessageRef<'a> {
    fn from(message: &'a Message) -> MessageRef<'a> {
        match message {
            Message::Request(request) => MessageRef::Request(request),
            Message::Response(response) => MessageRef::Response(response),
            Message::Event(event) => MessageRef::Event(event),
        }
    }
}

/// The frame header to serialize; the same fields, in the same order, as `FrameHeader`.
#[derive(Serialize)]
struct HeaderRef<'a> {
    /// Always `PROTOCOL_VERSION`.
    protocol_version: u32,
    /// Messages in wire order.
    messages: &'a [MessageRef<'a>],
    /// Location and type of each buffer.
    buffers: &'a [BufferInfo],
}

/// Encodes a frame. Fails only when the header exceeds `MAX_HEADER_LENGTH`.
pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, FrameError> {
    let messages: Vec<MessageRef<'_>> = frame.messages.iter().map(MessageRef::from).collect();
    encode(&messages, &frame.buffers)
}

/// Encodes a request frame.
pub fn encode_request_frame(request: &Request, buffers: &[Buffer]) -> Result<Vec<u8>, FrameError> {
    encode(&[MessageRef::Request(request)], buffers)
}

/// Encodes a response frame: the events in order, then the response.
pub fn encode_response_frame(
    events: &[Event],
    response: &Response,
    buffers: &[Buffer],
) -> Result<Vec<u8>, FrameError> {
    let messages: Vec<MessageRef<'_>> = events
        .iter()
        .map(MessageRef::Event)
        .chain([MessageRef::Response(response)])
        .collect();
    encode(&messages, buffers)
}

/// Encodes a frame holding a single error response and no buffers; never fails.
/// A message longer than 8 MiB is cut at a character boundary so the header stays within `MAX_HEADER_LENGTH`.
pub fn error_response_frame(id: u64, code: ErrorCode, message: &str) -> Vec<u8> {
    let message = &message[..message.floor_char_boundary(MAX_ERROR_MESSAGE_LENGTH)];
    let response = Response {
        id,
        result: Outcome::Error(ErrorInfo::new(code, message)),
    };
    encode_response_frame(&[], &response, &[])
        .expect("an error response with a message of at most 8 MiB has a header below the limit")
}

/// Writes magic, header length, header, zero padding, then each buffer little-endian with zero gaps.
fn encode(messages: &[MessageRef<'_>], buffers: &[Buffer]) -> Result<Vec<u8>, FrameError> {
    let offsets = buffer_offsets(buffers);
    let table: Vec<BufferInfo> = offsets
        .iter()
        .zip(buffers)
        .map(|(&offset, buffer)| BufferInfo {
            offset: offset as u64,
            length: buffer.byte_length() as u64,
            element_type: buffer.element_type(),
        })
        .collect();
    let header = HeaderRef {
        protocol_version: PROTOCOL_VERSION,
        messages,
        buffers: &table,
    };
    let header = serde_json::to_vec(&header)
        .expect("protocol messages always serialize: every map in them has string keys");
    if header.len() > MAX_HEADER_LENGTH {
        return Err(FrameError::HeaderTooLarge {
            length: header.len(),
        });
    }
    let header_length =
        u32::try_from(header.len()).expect("a header within MAX_HEADER_LENGTH fits in a u32");

    let buffer_start = PREFIX_LENGTH + header.len() + header_padding(header.len());
    let section_length = offsets
        .last()
        .zip(buffers.last())
        .map_or(0, |(offset, buffer)| offset + buffer.byte_length());

    // Every byte not written below (padding and gaps) stays zero.
    let mut bytes = vec![0; buffer_start + section_length];
    bytes[..MAGIC.len()].copy_from_slice(&MAGIC);
    bytes[MAGIC.len()..PREFIX_LENGTH].copy_from_slice(&header_length.to_le_bytes());
    bytes[PREFIX_LENGTH..PREFIX_LENGTH + header.len()].copy_from_slice(&header);
    for (offset, buffer) in offsets.into_iter().zip(buffers) {
        let start = buffer_start + offset;
        buffer.write_to(&mut bytes[start..start + buffer.byte_length()]);
    }
    Ok(bytes)
}

/// Offset of each buffer from the start of the buffer section, by the layout rule of section 2.2: the first buffer is at 0, and each later one at the first multiple of 8 at or after the end of the one before.
fn buffer_offsets(buffers: &[Buffer]) -> Vec<usize> {
    buffers
        .iter()
        .scan(0, |next_offset: &mut usize, buffer| {
            let offset = *next_offset;
            *next_offset = (offset + buffer.byte_length()).next_multiple_of(ALIGNMENT);
            Some(offset)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, CommandResult, DocHandle, Layer};
    use partsmith_model::Id;

    fn request() -> Request {
        Request {
            id: 9,
            command: Command::Undo { doc: DocHandle(1) },
        }
    }

    fn response() -> Response {
        Response {
            id: 9,
            result: Outcome::Ok(CommandResult::CloseDocument),
        }
    }

    /// The header length field of encoded frame `bytes`.
    fn header_length_field(bytes: &[u8]) -> usize {
        let field = bytes[MAGIC.len()..PREFIX_LENGTH]
            .try_into()
            .expect("the length field is four bytes");
        u32::from_le_bytes(field) as usize
    }

    fn event() -> Event {
        Event::BodyMesh {
            doc: DocHandle(1),
            revision: 2,
            body: Id::new("b-1").expect("a valid id"),
            layer: Layer::Preview,
            mesh_id: 3,
            mesh: None,
        }
    }

    #[test]
    fn borrowed_messages_serialize_like_owned_messages() {
        let messages = [
            Message::Request(request()),
            Message::Response(response()),
            Message::Event(event()),
        ];

        for message in &messages {
            let borrowed = serde_json::to_string(&MessageRef::from(message))
                .expect("a borrowed message serializes");
            let owned = serde_json::to_string(message).expect("a message serializes");

            assert_eq!(borrowed, owned);
        }
    }

    #[test]
    fn buffer_offsets_align_each_buffer_to_eight() {
        let buffers = [
            Buffer::U8(vec![0; 3]),
            Buffer::F32(vec![0.0; 2]),
            Buffer::U32(Vec::new()),
            Buffer::U8(vec![0; 1]),
        ];

        assert_eq!(buffer_offsets(&buffers), [0, 8, 16, 16]);
    }

    #[test]
    fn buffer_table_lists_byte_lengths_and_types() {
        let frame = Frame {
            messages: Vec::new(),
            buffers: vec![Buffer::U32(vec![1, 2])],
        };

        let bytes = encode_frame(&frame).expect("a small frame encodes");

        let decoded = crate::decode_frame(&bytes).expect("an encoded frame decodes");
        assert_eq!(decoded.buffers, frame.buffers);
        let header_length = header_length_field(&bytes);
        let header = std::str::from_utf8(&bytes[PREFIX_LENGTH..PREFIX_LENGTH + header_length])
            .expect("the header is UTF-8");
        assert!(
            header.ends_with(r#""buffers":[{"offset":0,"length":8,"element_type":"u32"}]}"#),
            "{header}"
        );
    }

    #[test]
    fn frame_without_buffers_ends_at_the_buffer_section() {
        let bytes = encode_request_frame(&request(), &[]).expect("a small request encodes");

        let header_length = header_length_field(&bytes);
        assert_eq!(
            bytes.len(),
            PREFIX_LENGTH + header_length + header_padding(header_length)
        );
    }

    #[test]
    fn frame_ends_at_the_end_of_its_last_buffer() {
        let frame = Frame {
            messages: Vec::new(),
            buffers: vec![Buffer::U8(vec![1; 3]), Buffer::U8(vec![2; 5])],
        };

        let bytes = encode_frame(&frame).expect("a small frame encodes");

        assert_eq!(
            &bytes[bytes.len() - 13..],
            &[1, 1, 1, 0, 0, 0, 0, 0, 2, 2, 2, 2, 2]
        );
    }

    #[test]
    fn response_frame_lists_events_before_the_response() {
        let bytes = encode_response_frame(&[event(), event()], &response(), &[])
            .expect("a small response encodes");

        let decoded = crate::decode_frame(&bytes).expect("an encoded frame decodes");
        let ids: Vec<Option<u64>> = decoded.messages.iter().map(Message::id).collect();
        assert_eq!(ids, [None, None, Some(9)]);
    }

    #[test]
    fn error_response_frame_cuts_an_oversized_message_at_a_character_boundary() {
        // After the one-byte "a", every two-byte "é" ends at an odd offset, so the limit falls inside one.
        let message = format!("a{}", "é".repeat(MAX_ERROR_MESSAGE_LENGTH));

        let bytes = error_response_frame(1, ErrorCode::InternalError, &message);

        let frame = crate::decode_response_frame(&bytes).expect("the frame decodes");
        let Outcome::Error(info) = frame.response.result else {
            panic!("an error response");
        };
        assert_eq!(info.message.len(), MAX_ERROR_MESSAGE_LENGTH - 1);
        assert!(message.starts_with(&info.message));
    }

    #[test]
    fn error_response_frame_keeps_a_message_at_the_limit_whole() {
        let message = "a".repeat(MAX_ERROR_MESSAGE_LENGTH);

        let bytes = error_response_frame(1, ErrorCode::InternalError, &message);

        let frame = crate::decode_response_frame(&bytes).expect("the frame decodes");
        let Outcome::Error(info) = frame.response.result else {
            panic!("an error response");
        };
        assert_eq!(info.message, message);
    }

    #[test]
    fn header_over_the_limit_is_rejected() {
        let request = Request {
            id: 1,
            command: Command::OpenDocument {
                text: "x".repeat(MAX_HEADER_LENGTH),
                assets: Vec::new(),
            },
        };

        let result = encode_request_frame(&request, &[]);

        assert!(
            matches!(result, Err(FrameError::HeaderTooLarge { length }) if length > MAX_HEADER_LENGTH),
            "got {result:?}"
        );
    }
}
