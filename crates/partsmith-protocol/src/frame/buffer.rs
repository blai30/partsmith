//! Raw buffers and their entries in the frame header (protocol.md sections 2.1 and 2.2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Location of one buffer inside the frame's buffer section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BufferInfo {
    /// Byte offset from the start of the buffer section; a multiple of 8.
    pub offset: u64,
    /// Length in bytes; a multiple of the element size.
    pub length: u64,
    /// Element type of the buffer.
    pub element_type: ElementType,
}

/// Element type of a raw buffer. All multi-byte values are little-endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    /// Unsigned bytes (file contents).
    U8,
    /// Unsigned 32-bit integers (indices, element ids).
    U32,
    /// IEEE 754 binary32 floats (positions, normals, analysis values).
    F32,
}

impl ElementType {
    /// Size of one element in bytes.
    pub(crate) fn size(self) -> usize {
        match self {
            ElementType::U8 => 1,
            ElementType::U32 | ElementType::F32 => 4,
        }
    }

    /// The type name as it appears on the wire.
    pub(crate) fn name(self) -> &'static str {
        match self {
            ElementType::U8 => "u8",
            ElementType::U32 => "u32",
            ElementType::F32 => "f32",
        }
    }
}

/// An owned raw buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum Buffer {
    /// Bytes.
    U8(Vec<u8>),
    /// 32-bit unsigned integers.
    U32(Vec<u32>),
    /// 32-bit floats.
    F32(Vec<f32>),
}

impl Buffer {
    /// The buffer's element type.
    pub fn element_type(&self) -> ElementType {
        match self {
            Buffer::U8(_) => ElementType::U8,
            Buffer::U32(_) => ElementType::U32,
            Buffer::F32(_) => ElementType::F32,
        }
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        match self {
            Buffer::U8(values) => values.len(),
            Buffer::U32(values) => values.len(),
            Buffer::F32(values) => values.len(),
        }
    }

    /// Whether the buffer has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Length in bytes on the wire.
    pub fn byte_length(&self) -> usize {
        self.len() * self.element_type().size()
    }

    /// The contents if this is a `U8` buffer.
    pub fn as_u8(&self) -> Option<&[u8]> {
        match self {
            Buffer::U8(values) => Some(values),
            _ => None,
        }
    }

    /// The contents if this is a `U32` buffer.
    pub fn as_u32(&self) -> Option<&[u32]> {
        match self {
            Buffer::U32(values) => Some(values),
            _ => None,
        }
    }

    /// The contents if this is an `F32` buffer.
    pub fn as_f32(&self) -> Option<&[f32]> {
        match self {
            Buffer::F32(values) => Some(values),
            _ => None,
        }
    }

    /// Writes the elements little-endian into `target`, which is exactly `byte_length` bytes long.
    pub(crate) fn write_to(&self, target: &mut [u8]) {
        match self {
            Buffer::U8(values) => target.copy_from_slice(values),
            Buffer::U32(values) => write_little_endian(target, values, u32::to_le_bytes),
            Buffer::F32(values) => write_little_endian(target, values, f32::to_le_bytes),
        }
    }

    /// Reads little-endian elements of `element_type` from `bytes`, whose length is a multiple of the element size.
    pub(crate) fn read(element_type: ElementType, bytes: &[u8]) -> Buffer {
        match element_type {
            ElementType::U8 => Buffer::U8(bytes.to_vec()),
            ElementType::U32 => Buffer::U32(read_little_endian(bytes, u32::from_le_bytes)),
            ElementType::F32 => Buffer::F32(read_little_endian(bytes, f32::from_le_bytes)),
        }
    }
}

/// Writes 4-byte elements into consecutive 4-byte chunks of `target`, each turned into its
/// little-endian bytes by `to_bytes`.
fn write_little_endian<Element: Copy>(
    target: &mut [u8],
    values: &[Element],
    to_bytes: impl Fn(Element) -> [u8; 4],
) {
    for (chunk, &value) in target.as_chunks_mut::<4>().0.iter_mut().zip(values) {
        *chunk = to_bytes(value);
    }
}

/// Reads one 4-byte element from each whole 4-byte chunk of `bytes`, whose little-endian bytes
/// `from_bytes` turns into the element.
fn read_little_endian<Element>(
    bytes: &[u8],
    from_bytes: impl Fn([u8; 4]) -> Element,
) -> Vec<Element> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&chunk| from_bytes(chunk))
        .collect()
}

/// Index of a buffer in the enclosing frame; serialized as a bare integer.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct BufferIndex(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_sizes_are_one_and_four_bytes() {
        assert_eq!(ElementType::U8.size(), 1);
        assert_eq!(ElementType::U32.size(), 4);
        assert_eq!(ElementType::F32.size(), 4);
    }

    #[test]
    fn element_type_names_match_their_json() {
        for element_type in [ElementType::U8, ElementType::U32, ElementType::F32] {
            let json = serde_json::to_value(element_type).expect("an element type serializes");

            assert_eq!(json, element_type.name());
        }
    }

    #[test]
    fn buffer_reports_its_element_type() {
        assert_eq!(Buffer::U8(Vec::new()).element_type(), ElementType::U8);
        assert_eq!(Buffer::U32(Vec::new()).element_type(), ElementType::U32);
        assert_eq!(Buffer::F32(Vec::new()).element_type(), ElementType::F32);
    }

    #[test]
    fn length_counts_elements_and_byte_length_counts_bytes() {
        let buffer = Buffer::U32(vec![1, 2, 3]);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.byte_length(), 12);
    }

    #[test]
    fn empty_buffer_is_empty() {
        assert!(Buffer::F32(Vec::new()).is_empty());
        assert!(!Buffer::U8(vec![0]).is_empty());
    }

    #[test]
    fn typed_accessors_return_only_their_own_type() {
        let bytes = Buffer::U8(vec![7]);

        assert_eq!(bytes.as_u8(), Some(&[7][..]));
        assert_eq!(bytes.as_u32(), None);
        assert_eq!(bytes.as_f32(), None);
        assert_eq!(Buffer::U32(vec![7]).as_u32(), Some(&[7][..]));
        assert_eq!(Buffer::F32(vec![7.0]).as_f32(), Some(&[7.0][..]));
    }

    #[test]
    fn u32_elements_are_written_little_endian() {
        let mut target = [0; 8];

        Buffer::U32(vec![1, 0x0403_0201]).write_to(&mut target);

        assert_eq!(target, [1, 0, 0, 0, 1, 2, 3, 4]);
    }

    #[test]
    fn f32_elements_are_written_little_endian() {
        let mut target = [0; 4];

        Buffer::F32(vec![-2.0]).write_to(&mut target);

        assert_eq!(target, [0x00, 0x00, 0x00, 0xc0]);
    }

    #[test]
    fn reading_written_elements_returns_the_buffer() {
        let buffers = [
            Buffer::U8(vec![1, 2, 3]),
            Buffer::U32(vec![u32::MAX, 0, 42]),
            Buffer::F32(vec![f32::MIN_POSITIVE, -0.5, 1e30]),
        ];

        for buffer in buffers {
            let mut bytes = vec![0; buffer.byte_length()];
            buffer.write_to(&mut bytes);

            assert_eq!(Buffer::read(buffer.element_type(), &bytes), buffer);
        }
    }

    #[test]
    fn f32_reading_keeps_the_exact_bits() {
        let bits = 0x7fc0_0001_u32;

        let buffer = Buffer::read(ElementType::F32, &bits.to_le_bytes());

        let values = buffer.as_f32().expect("an f32 buffer");
        assert_eq!(values[0].to_bits(), bits);
    }

    #[test]
    fn buffer_index_serializes_as_a_bare_integer() {
        let text = serde_json::to_string(&BufferIndex(3)).expect("an index serializes");

        assert_eq!(text, "3");
    }

    #[test]
    fn buffer_info_rejects_unknown_fields() {
        let result = serde_json::from_str::<BufferInfo>(
            r#"{"offset":0,"length":4,"element_type":"u8","stride":4}"#,
        );

        assert!(result.is_err(), "got {result:?}");
    }
}
