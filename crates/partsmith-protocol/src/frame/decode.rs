//! Strict frame decoding (protocol.md section 3.2). Checks run in the spec's order and the first failure is returned.

use std::ops::Range;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{
    ALIGNMENT, Buffer, BufferInfo, Frame, FrameError, PREFIX_LENGTH, ResponseFrame, header_padding,
};
use crate::{MAGIC, MAX_HEADER_LENGTH, Message, PROTOCOL_VERSION, Request};

/// Decodes and validates a frame (layout, version, message types).
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, FrameError> {
    // 1 and 2: the prefix holds the magic and the header length.
    let truncated = || FrameError::Truncated {
        length: bytes.len(),
        expected: PREFIX_LENGTH,
    };
    let (magic, rest) = bytes.split_first_chunk::<4>().ok_or_else(truncated)?;
    let (length_field, _) = rest.split_first_chunk::<4>().ok_or_else(truncated)?;
    if *magic != MAGIC {
        return Err(FrameError::BadMagic);
    }

    // 3 and 4: the header and its padding fit in the frame.
    let header_length = u32::from_le_bytes(*length_field) as usize;
    if header_length > MAX_HEADER_LENGTH {
        return Err(FrameError::HeaderTooLarge {
            length: header_length,
        });
    }
    let header_end = PREFIX_LENGTH + header_length;
    let buffer_start = header_end + header_padding(header_length);
    if buffer_start > bytes.len() {
        return Err(FrameError::Truncated {
            length: bytes.len(),
            expected: buffer_start,
        });
    }

    // 5 to 7: the header is JSON with protocol version 1 and a buffer table.
    let header = parse_header(&bytes[PREFIX_LENGTH..header_end])?;
    let request_id = header.pointer("/messages/0/id").and_then(Value::as_u64);
    let invalid = |message: String| FrameError::InvalidMessage {
        message,
        request_id,
    };
    let Value::Object(fields) = &header else {
        return Err(invalid("the header is not a JSON object.".to_string()));
    };
    let version = fields
        .get("protocol_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or_else(|| {
            invalid("protocol_version is missing or not a 32-bit unsigned integer.".to_string())
        })?;
    if version != PROTOCOL_VERSION {
        return Err(FrameError::VersionMismatch {
            found: version,
            request_id,
        });
    }
    let table = Vec::<BufferInfo>::deserialize(field(fields, "buffers").map_err(&invalid)?)
        .map_err(|error| invalid(format!("buffers: {error}.")))?;

    // 8 to 10: zero padding, buffers where the layout rule puts them, nothing after the last one.
    if !is_zero(&bytes[header_end..buffer_start]) {
        return Err(FrameError::NonZeroPadding);
    }
    let ranges = buffer_ranges(bytes, buffer_start, &table)?;
    let frame_end = ranges.last().map_or(buffer_start, |range| range.end);
    if bytes.len() > frame_end {
        return Err(FrameError::TrailingBytes {
            count: bytes.len() - frame_end,
        });
    }

    // 11: the messages match the message schema.
    let Value::Array(items) = field(fields, "messages").map_err(&invalid)? else {
        return Err(invalid("the messages field is not a list.".to_string()));
    };
    let messages = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            Message::deserialize(item)
                .map_err(|error| invalid(format!("message {index}: {error}.")))
        })
        .collect::<Result<Vec<Message>, FrameError>>()?;

    let buffers = table
        .iter()
        .zip(ranges)
        .map(|(info, range)| Buffer::read(info.element_type, &bytes[range]))
        .collect();
    Ok(Frame { messages, buffers })
}

/// Decodes a frame that must contain exactly one request message.
pub fn decode_request_frame(bytes: &[u8]) -> Result<(Request, Vec<Buffer>), FrameError> {
    let frame = decode_frame(bytes)?;
    let request_id = frame.messages.first().and_then(Message::id);
    let message = match <[Message; 1]>::try_from(frame.messages) {
        Ok([Message::Request(request)]) => return Ok((request, frame.buffers)),
        Ok([other]) => format!(
            "a request frame must hold a request, but this one holds {}.",
            kind_with_article(&other)
        ),
        Err(messages) => format!(
            "a request frame must hold exactly one message, but this one holds {}.",
            messages.len()
        ),
    };
    Err(FrameError::InvalidMessage {
        message,
        request_id,
    })
}

/// Decodes a frame that must contain zero or more events followed by exactly one response.
pub fn decode_response_frame(bytes: &[u8]) -> Result<ResponseFrame, FrameError> {
    let frame = decode_frame(bytes)?;
    let request_id = frame.messages.first().and_then(Message::id);
    let invalid = |message: String| FrameError::InvalidMessage {
        message,
        request_id,
    };
    let mut messages = frame.messages;
    let response = match messages.pop() {
        Some(Message::Response(response)) => response,
        Some(other) => {
            return Err(invalid(format!(
                "a response frame must end with a response, but this one ends with {}.",
                kind_with_article(&other)
            )));
        }
        None => {
            return Err(invalid(
                "a response frame must hold a response, but this one holds no messages."
                    .to_string(),
            ));
        }
    };
    let events = messages
        .into_iter()
        .enumerate()
        .map(|(index, message)| match message {
            Message::Event(event) => Ok(event),
            other => Err(invalid(format!(
                "message {index} of a response frame must be an event, but it is {}.",
                kind_with_article(&other)
            ))),
        })
        .collect::<Result<_, _>>()?;
    Ok(ResponseFrame {
        events,
        response,
        buffers: frame.buffers,
    })
}

/// Parses the header bytes as UTF-8 JSON (step 5).
fn parse_header(header: &[u8]) -> Result<Value, FrameError> {
    let text = std::str::from_utf8(header).map_err(|error| FrameError::InvalidHeader {
        message: format!("it is not UTF-8 text ({error})."),
    })?;
    serde_json::from_str(text).map_err(|error| FrameError::InvalidHeader {
        message: format!("{error}."),
    })
}

/// The header field `name`, or the reason it is missing.
fn field<'a>(fields: &'a Map<String, Value>, name: &str) -> Result<&'a Value, String> {
    fields
        .get(name)
        .ok_or_else(|| format!("the header has no {name} field."))
}

/// Checks each buffer against the layout rule (step 9) and returns its absolute byte range.
fn buffer_ranges(
    bytes: &[u8],
    buffer_start: usize,
    table: &[BufferInfo],
) -> Result<Vec<Range<usize>>, FrameError> {
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(table.len());
    for (index, info) in table.iter().enumerate() {
        let previous_end = ranges.last().map_or(buffer_start, |range| range.end);
        let start = previous_end.next_multiple_of(ALIGNMENT);
        let expected_offset = (start - buffer_start) as u64;
        if info.offset != expected_offset {
            return Err(FrameError::BadBufferLayout {
                index,
                reason: format!(
                    "its offset is {}, but the layout rule puts it at {expected_offset}.",
                    info.offset
                ),
            });
        }
        let element_size = info.element_type.size();
        if info.length % element_size as u64 != 0 {
            return Err(FrameError::BadBufferLayout {
                index,
                reason: format!(
                    "its length {} is not a multiple of the {element_size}-byte size of {}.",
                    info.length,
                    info.element_type.name()
                ),
            });
        }
        let end = usize::try_from(info.length)
            .ok()
            .and_then(|length| start.checked_add(length));
        let end = match end {
            Some(end) if end <= bytes.len() => end,
            _ => {
                return Err(FrameError::Truncated {
                    length: bytes.len(),
                    expected: end.unwrap_or(usize::MAX),
                });
            }
        };
        if !is_zero(&bytes[previous_end..start]) {
            return Err(FrameError::NonZeroPadding);
        }
        ranges.push(start..end);
    }
    Ok(ranges)
}

/// Whether every byte is zero.
fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&byte| byte == 0)
}

/// The message's kind with its article, for composition errors ("a request", "an event").
fn kind_with_article(message: &Message) -> &'static str {
    match message {
        Message::Request(_) => "a request",
        Message::Response(_) => "a response",
        Message::Event(_) => "an event",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementType;

    /// Frame bytes around `header` and `buffer_section`, built by hand.
    fn frame(header: &str, buffer_section: &[u8]) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
        bytes.extend_from_slice(header.as_bytes());
        bytes.resize(bytes.len() + header_padding(header.len()), 0);
        bytes.extend_from_slice(buffer_section);
        bytes
    }

    /// A header with no messages and the given buffer table.
    fn header_with_buffers(buffers: &str) -> String {
        format!(r#"{{"protocol_version":1,"messages":[],"buffers":{buffers}}}"#)
    }

    fn info(offset: u64, length: u64, element_type: ElementType) -> BufferInfo {
        BufferInfo {
            offset,
            length,
            element_type,
        }
    }

    #[test]
    fn empty_input_is_truncated_at_eight_bytes() {
        assert_eq!(
            decode_frame(&[]),
            Err(FrameError::Truncated {
                length: 0,
                expected: 8
            })
        );
    }

    #[test]
    fn short_input_is_truncated_even_with_a_wrong_magic() {
        assert_eq!(
            decode_frame(b"XXXXXX"),
            Err(FrameError::Truncated {
                length: 6,
                expected: 8
            })
        );
    }

    #[test]
    fn header_that_is_not_an_object_is_an_invalid_message() {
        let result = decode_frame(&frame("[1]", &[]));

        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn missing_protocol_version_is_an_invalid_message() {
        let result = decode_frame(&frame(r#"{"messages":[],"buffers":[]}"#, &[]));

        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn protocol_version_beyond_32_bits_is_an_invalid_message() {
        let result = decode_frame(&frame(
            r#"{"protocol_version":4294967297,"messages":[],"buffers":[]}"#,
            &[],
        ));

        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn version_mismatch_without_messages_has_no_request_id() {
        let result = decode_frame(&frame(
            r#"{"protocol_version":0,"messages":[],"buffers":[]}"#,
            &[],
        ));

        assert_eq!(
            result,
            Err(FrameError::VersionMismatch {
                found: 0,
                request_id: None
            })
        );
    }

    #[test]
    fn version_is_checked_before_the_buffer_table() {
        let result = decode_frame(&frame(r#"{"protocol_version":3}"#, &[]));

        assert!(
            matches!(result, Err(FrameError::VersionMismatch { found: 3, .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn missing_buffer_table_is_an_invalid_message() {
        let result = decode_frame(&frame(r#"{"protocol_version":1,"messages":[]}"#, &[]));

        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn missing_message_list_is_an_invalid_message() {
        let result = decode_frame(&frame(r#"{"protocol_version":1,"buffers":[]}"#, &[]));

        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn invalid_message_names_the_message_index() {
        let header =
            r#"{"protocol_version":1,"messages":[{"kind":"event","type":"nothing"}],"buffers":[]}"#;

        let error = decode_frame(&frame(header, &[])).expect_err("the event is unknown");

        assert!(error.to_string().contains("message 0"), "{error}");
    }

    #[test]
    fn invalid_message_carries_the_first_message_id() {
        let header = r#"{"protocol_version":1,"messages":[{"kind":"request","id":12,"command":{"type":"undo"}}],"buffers":[]}"#;

        let error = decode_frame(&frame(header, &[])).expect_err("undo needs a doc");

        assert_eq!(error.request_id(), Some(12));
    }

    #[test]
    fn layout_errors_are_found_before_message_errors() {
        let header = r#"{"protocol_version":1,"messages":[{"kind":"nothing"}],"buffers":[]}"#;
        let mut bytes = frame(header, &[]);
        bytes.push(0);

        assert_eq!(
            decode_frame(&bytes),
            Err(FrameError::TrailingBytes { count: 1 })
        );
    }

    #[test]
    fn nonzero_gap_between_buffers_is_rejected() {
        let header = header_with_buffers(
            r#"[{"offset":0,"length":1,"element_type":"u8"},{"offset":8,"length":1,"element_type":"u8"}]"#,
        );
        let section = [1, 0, 0, 9, 0, 0, 0, 0, 2];

        assert_eq!(
            decode_frame(&frame(&header, &section)),
            Err(FrameError::NonZeroPadding)
        );
    }

    #[test]
    fn buffer_length_beyond_the_address_space_is_truncated() {
        let header = header_with_buffers(
            r#"[{"offset":0,"length":18446744073709551615,"element_type":"u8"}]"#,
        );

        let result = decode_frame(&frame(&header, &[]));

        assert!(
            matches!(result, Err(FrameError::Truncated { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn buffer_ranges_follow_the_layout_rule() {
        let bytes = [0; 40];
        let table = [
            info(0, 3, ElementType::U8),
            info(8, 8, ElementType::F32),
            info(16, 0, ElementType::U32),
            info(16, 4, ElementType::U32),
        ];

        let ranges = buffer_ranges(&bytes, 16, &table).expect("the layout is valid");

        assert_eq!(ranges, [16..19, 24..32, 32..32, 32..36]);
    }

    #[test]
    fn decoded_buffers_have_their_element_types() {
        let header = header_with_buffers(
            r#"[{"offset":0,"length":4,"element_type":"u32"},{"offset":8,"length":4,"element_type":"f32"}]"#,
        );
        let section = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3f];

        let decoded = decode_frame(&frame(&header, &section)).expect("the frame is valid");

        assert_eq!(
            decoded.buffers,
            [Buffer::U32(vec![1]), Buffer::F32(vec![1.0])]
        );
    }

    #[test]
    fn request_frame_with_a_response_names_what_it_holds() {
        let header = r#"{"protocol_version":1,"messages":[{"kind":"response","id":5,"result":{"kind":"ok","type":"close_document"}}],"buffers":[]}"#;

        let error =
            decode_request_frame(&frame(header, &[])).expect_err("a response is not a request");

        assert!(error.to_string().contains("a response"), "{error}");
    }

    #[test]
    fn response_frame_with_a_request_before_the_response_is_rejected() {
        let header = r#"{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}},{"kind":"response","id":1,"result":{"kind":"ok","type":"close_document"}}],"buffers":[]}"#;

        let result = decode_response_frame(&frame(header, &[]));

        assert!(
            matches!(result, Err(FrameError::InvalidMessage { .. })),
            "got {result:?}"
        );
    }
}
