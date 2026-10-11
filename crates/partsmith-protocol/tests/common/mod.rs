//! Fixtures and helpers shared by the acceptance tests of `docs/spec/protocol.md`.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

pub mod examples;
pub mod json_schema;

use std::collections::BTreeSet;

use partsmith_model::Id;
use partsmith_protocol::{Message, protocol_schema};
use serde_json::Value;

/// Example A of section 2.2: request id 1, `engine_info`, no buffers. 120 bytes, copied from the spec's hex dump.
pub const EXAMPLE_A_HEX: &str = "
    50 53 4d 31 6b 00 00 00 7b 22 70 72 6f 74 6f 63
    6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
    65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
    22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
    3a 31 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
    79 70 65 22 3a 22 65 6e 67 69 6e 65 5f 69 6e 66
    6f 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73 22 3a
    5b 5d 7d 00 00 00 00 00
";

/// Example B of section 2.2: `import_step` with one 14-byte `u8` buffer. 214 bytes, copied from the spec's hex dump.
pub const EXAMPLE_B_HEX: &str = "
    50 53 4d 31 b9 00 00 00 7b 22 70 72 6f 74 6f 63
    6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
    65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
    22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
    3a 32 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
    79 70 65 22 3a 22 69 6d 70 6f 72 74 5f 73 74 65
    70 22 2c 22 64 6f 63 22 3a 31 2c 22 64 61 74 61
    22 3a 30 2c 22 6e 61 6d 65 22 3a 22 42 72 61 63
    6b 65 74 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73
    22 3a 5b 7b 22 6f 66 66 73 65 74 22 3a 30 2c 22
    6c 65 6e 67 74 68 22 3a 31 34 2c 22 65 6c 65 6d
    65 6e 74 5f 74 79 70 65 22 3a 22 75 38 22 7d 5d
    7d 00 00 00 00 00 00 00 49 53 4f 2d 31 30 33 30
    33 2d 32 31 3b 0a
";

/// The JSON header of example A (107 bytes).
pub const EXAMPLE_A_HEADER: &str = r#"{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[]}"#;

/// The JSON header of example B (185 bytes).
pub const EXAMPLE_B_HEADER: &str = r#"{"protocol_version":1,"messages":[{"kind":"request","id":2,"command":{"type":"import_step","doc":1,"data":0,"name":"Bracket"}}],"buffers":[{"offset":0,"length":14,"element_type":"u8"}]}"#;

/// The contents of example B's only buffer.
pub const EXAMPLE_B_BUFFER: &[u8] = b"ISO-10303-21;\n";

/// The bytes of example A.
pub fn example_a() -> Vec<u8> {
    hex_bytes(EXAMPLE_A_HEX)
}

/// The bytes of example B.
pub fn example_b() -> Vec<u8> {
    hex_bytes(EXAMPLE_B_HEX)
}

/// Parses whitespace-separated pairs of hex digits.
pub fn hex_bytes(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| {
            u8::from_str_radix(pair, 16).unwrap_or_else(|_| panic!("{pair:?} is not a hex byte"))
        })
        .collect()
}

/// Builds frame bytes by hand, without the encoder: magic, header length, header, zero padding to 8, then `buffer_section` as given.
pub fn frame_bytes(header: &[u8], buffer_section: &[u8]) -> Vec<u8> {
    let header_length =
        u32::try_from(header.len()).expect("test headers are far smaller than 4 GiB");
    let padding = (8 - header.len() % 8) % 8;
    let mut bytes = Vec::with_capacity(8 + header.len() + padding + buffer_section.len());
    bytes.extend_from_slice(b"PSM1");
    bytes.extend_from_slice(&header_length.to_le_bytes());
    bytes.extend_from_slice(header);
    bytes.resize(bytes.len() + padding, 0);
    bytes.extend_from_slice(buffer_section);
    bytes
}

/// Builds frame bytes by hand from a header text.
pub fn frame_from_header(header: &str, buffer_section: &[u8]) -> Vec<u8> {
    frame_bytes(header.as_bytes(), buffer_section)
}

/// Builds a frame without buffers whose header holds `messages`, a JSON array text.
pub fn frame_with_messages(messages: &str) -> Vec<u8> {
    frame_from_header(
        &format!(r#"{{"protocol_version":1,"messages":{messages},"buffers":[]}}"#),
        &[],
    )
}

/// The value of a frame's little-endian header length field.
pub fn header_length(frame: &[u8]) -> usize {
    let field: [u8; 4] = frame[4..8]
        .try_into()
        .expect("a four-byte slice converts to a four-byte array");
    usize::try_from(u32::from_le_bytes(field)).expect("a u32 fits in usize")
}

/// The header bytes of a frame, located through its length field.
pub fn header_bytes(frame: &[u8]) -> &[u8] {
    &frame[8..8 + header_length(frame)]
}

/// The header of a frame parsed as JSON.
pub fn header_json(frame: &[u8]) -> Value {
    serde_json::from_slice(header_bytes(frame)).expect("the frame header is JSON")
}

/// Absolute offset of a frame's buffer section: after the header and its zero padding.
pub fn buffer_start(frame: &[u8]) -> usize {
    let length = header_length(frame);
    8 + length + (8 - length % 8) % 8
}

/// The compact `serde_json` form of a JSON text, for comparing output with spec examples written with spaces.
pub fn compact(text: &str) -> String {
    let value: Value = serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("the example is JSON ({error}): {text}"));
    serde_json::to_string(&value).expect("a JSON value always serializes")
}

/// A JSON text parsed into a value.
pub fn parse_json(text: &str) -> Value {
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("the example is JSON ({error}): {text}"))
}

/// A valid id.
pub fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|_| panic!("{text:?} is a valid id"))
}

/// A message parsed from JSON with the protocol's own types.
pub fn message(text: &str) -> Message {
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("the message deserializes ({error}): {text}"))
}

/// The `type` string of a JSON record.
pub fn type_of(text: &str) -> String {
    parse_json(text)["type"]
        .as_str()
        .unwrap_or_else(|| panic!("the record has a string type: {text}"))
        .to_string()
}

/// A list of strings as a set.
pub fn string_set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// The protocol JSON Schema parsed as JSON.
pub fn schema_json() -> Value {
    serde_json::from_str(&protocol_schema()).expect("the protocol schema is JSON")
}

/// The `$defs` entry `name` of the protocol schema.
pub fn definition<'a>(schema: &'a Value, name: &str) -> &'a Value {
    schema
        .pointer(&format!("/$defs/{name}"))
        .unwrap_or_else(|| panic!("the schema has no $defs entry {name}"))
}

/// The variant schemas of an enum definition: its `oneOf` entries, or its `anyOf` entries.
fn variant_entries<'a>(definition: &'a Value, name: &str) -> &'a Vec<Value> {
    definition
        .get("oneOf")
        .or_else(|| definition.get("anyOf"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("the $defs entry {name} lists no variants in oneOf or anyOf"))
}

/// The tag value a variant schema declares for `tag`: a `const`, or an `enum` with one value, in its `properties` or in an `allOf` member.
fn variant_tag(entry: &Value, tag: &str) -> Option<String> {
    let declared = entry
        .get("properties")
        .and_then(|properties| properties.get(tag));
    if let Some(declared) = declared {
        if let Some(constant) = declared.get("const").and_then(Value::as_str) {
            return Some(constant.to_string());
        }
        if let Some([single]) = declared
            .get("enum")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
        {
            return single.as_str().map(str::to_string);
        }
    }
    entry
        .get("allOf")
        .and_then(Value::as_array)
        .and_then(|members| members.iter().find_map(|member| variant_tag(member, tag)))
}

/// The tag values of every variant of the tagged enum definition `name`.
pub fn variant_tags(schema: &Value, name: &str, tag: &str) -> BTreeSet<String> {
    let entries = variant_entries(definition(schema, name), name);
    let tags: BTreeSet<String> = entries
        .iter()
        .map(|entry| {
            variant_tag(entry, tag)
                .unwrap_or_else(|| panic!("a variant of {name} declares no {tag}: {entry}"))
        })
        .collect();
    assert_eq!(
        tags.len(),
        entries.len(),
        "the variants of {name} have distinct {tag} values"
    );
    tags
}

/// The variant schema of the tagged enum definition `name` whose `tag` is `value`.
pub fn variant<'a>(schema: &'a Value, name: &str, tag: &str, value: &str) -> &'a Value {
    variant_entries(definition(schema, name), name)
        .iter()
        .find(|entry| variant_tag(entry, tag).as_deref() == Some(value))
        .unwrap_or_else(|| panic!("{name} has no variant with {tag} {value}"))
}

/// The strings a unit-only enum definition allows: its `enum` values, or the `const` or `enum` values of its `oneOf` or `anyOf` entries.
pub fn string_values(schema: &Value, name: &str) -> BTreeSet<String> {
    let definition = definition(schema, name);
    let mut values = BTreeSet::new();
    let mut collect = |subschema: &Value| {
        if let Some(constant) = subschema.get("const").and_then(Value::as_str) {
            values.insert(constant.to_string());
        }
        for item in subschema
            .get("enum")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = item
                .as_str()
                .unwrap_or_else(|| panic!("{name} allows a value that is not a string: {item}"));
            values.insert(text.to_string());
        }
    };
    collect(definition);
    for keyword in ["oneOf", "anyOf"] {
        for entry in definition
            .get(keyword)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect(entry);
        }
    }
    values
}

/// The schema of property `name` in `subschema`, looking through `$ref` and `allOf`.
pub fn property<'a>(schema: &'a Value, subschema: &'a Value, name: &str) -> Option<&'a Value> {
    if let Some(found) = subschema
        .get("properties")
        .and_then(|properties| properties.get(name))
    {
        return Some(found);
    }
    if let Some(reference) = subschema.get("$ref").and_then(Value::as_str) {
        let target = reference
            .strip_prefix('#')
            .and_then(|pointer| schema.pointer(pointer))
            .unwrap_or_else(|| panic!("the schema reference {reference} does not resolve"));
        if let Some(found) = property(schema, target, name) {
            return Some(found);
        }
    }
    subschema
        .get("allOf")
        .and_then(Value::as_array)
        .and_then(|members| {
            members
                .iter()
                .find_map(|member| property(schema, member, name))
        })
}
