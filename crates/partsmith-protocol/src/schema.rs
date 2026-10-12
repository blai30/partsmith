//! The generated protocol JSON Schema (protocol.md section 2.9).

use schemars::generate::SchemaSettings;
use serde_json::Value;

use crate::{FrameHeader, PROTOCOL_VERSION};

/// Title of the protocol schema.
const TITLE: &str = "Partsmith protocol";

/// The protocol JSON Schema (draft 2020-12) as pretty-printed text with a trailing newline.
///
/// The root is the schema of `FrameHeader`, with every reachable type in `$defs`, the title `Partsmith protocol` and `x-protocol-version`.
pub fn protocol_schema() -> String {
    let mut schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<FrameHeader>();
    let root = schema
        .as_object_mut()
        .expect("the schema of a struct is a JSON object");
    root.insert("title".to_string(), Value::from(TITLE));
    let after_title = root
        .keys()
        .position(|key| key == "title")
        .expect("the title was just inserted")
        + 1;
    root.shift_insert(
        after_title,
        "x-protocol-version".to_string(),
        Value::from(PROTOCOL_VERSION),
    );
    let mut text =
        serde_json::to_string_pretty(&schema).expect("a JSON Schema always serializes");
    text.push('\n');
    text
}
