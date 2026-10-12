//! The generated protocol JSON Schema (protocol.md section 2.9).

use schemars::generate::SchemaSettings;
use serde_json::Value;

use crate::{FrameHeader, PROTOCOL_VERSION};

/// Title of the protocol schema.
const TITLE: &str = "Partsmith protocol";

/// The protocol JSON Schema (draft 2020-12) as pretty-printed text with a trailing newline.
///
/// The root is the schema of `FrameHeader` with every reachable type in `$defs`, titled `Partsmith protocol` and marked with `x-protocol-version`.
pub fn protocol_schema() -> String {
    let mut schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<FrameHeader>();
    schema.insert("title".to_string(), Value::from(TITLE));
    schema.insert(
        "x-protocol-version".to_string(),
        Value::from(PROTOCOL_VERSION),
    );
    let mut text = serde_json::to_string_pretty(&schema).expect("a JSON Schema always serializes");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        serde_json::from_str(&protocol_schema()).expect("the schema is JSON")
    }

    #[test]
    fn root_describes_the_frame_header() {
        let required = &schema()["required"];

        assert_eq!(
            *required,
            Value::from(vec!["protocol_version", "messages", "buffers"])
        );
    }

    #[test]
    fn schema_uses_two_space_indentation() {
        assert!(protocol_schema().starts_with("{\n  \"$schema\""));
    }

    #[test]
    fn definitions_come_last() {
        let schema = schema();
        let keys: Vec<&String> = schema
            .as_object()
            .expect("the schema is an object")
            .keys()
            .collect();

        assert_eq!(keys.last().map(|key| key.as_str()), Some("$defs"));
    }

    #[test]
    fn request_is_a_named_definition() {
        let schema = schema();

        assert!(
            schema
                .pointer("/$defs/Request/properties/command")
                .is_some()
        );
    }

    /// The schema of the `Command` variant whose `type` is `name`.
    fn command_variant(schema: &Value, name: &str) -> Value {
        schema
            .pointer("/$defs/Command/oneOf")
            .and_then(Value::as_array)
            .expect("Command lists its variants")
            .iter()
            .find(|variant| variant.pointer("/properties/type/const") == Some(&Value::from(name)))
            .cloned()
            .unwrap_or_else(|| panic!("Command has a {name} variant"))
    }

    /// The `allOf` reference of a schema that `strict::closed` built, if it is one.
    fn closed_reference(schema: &Value) -> Option<&str> {
        if schema.get("unevaluatedProperties") != Some(&Value::Bool(false)) {
            return None;
        }
        schema.pointer("/allOf/0/$ref").and_then(Value::as_str)
    }

    #[test]
    fn orientation_and_assets_are_open_in_results_and_events() {
        let schema = schema();
        let orientation_variants = schema
            .pointer("/$defs/Orientation/oneOf")
            .and_then(Value::as_array)
            .expect("Orientation lists its variants");

        assert!(
            orientation_variants
                .iter()
                .all(|variant| variant.get("additionalProperties").is_none())
        );
        assert!(
            schema
                .pointer("/$defs/AssetData/additionalProperties")
                .is_none()
        );
        assert_eq!(
            schema.pointer("/$defs/PrintReport/properties/orientation/$ref"),
            Some(&Value::from("#/$defs/Orientation"))
        );
    }

    #[test]
    fn request_fields_of_shared_types_are_closed() {
        let schema = schema();
        let analyze_print = command_variant(&schema, "analyze_print");
        let open_document = command_variant(&schema, "open_document");
        let export = command_variant(&schema, "export");

        assert_eq!(
            closed_reference(&analyze_print["properties"]["orientation"]),
            Some("#/$defs/Orientation")
        );
        assert_eq!(
            closed_reference(&open_document["properties"]["assets"]["items"]),
            Some("#/$defs/AssetData")
        );
        assert_eq!(
            closed_reference(&export["properties"]["orientation"]["anyOf"][0]),
            Some("#/$defs/Orientation")
        );
        assert_eq!(
            closed_reference(&export["properties"]["tolerance"]["anyOf"][0]),
            Some("#/$defs/TessellationTolerance")
        );
    }

    #[test]
    fn optional_request_fields_of_shared_types_stay_optional() {
        let schema = schema();
        let export = command_variant(&schema, "export");

        assert_eq!(
            export["required"],
            Value::from(vec!["type", "doc", "format"])
        );
        assert_eq!(
            export["properties"]["orientation"]["anyOf"][1],
            serde_json::json!({"type": "null"})
        );
    }

    #[test]
    fn request_variant_rejects_unknown_fields() {
        let schema = schema();
        let variants = schema
            .pointer("/$defs/Message/oneOf")
            .and_then(Value::as_array)
            .expect("Message lists its variants");

        let request = variants
            .iter()
            .find(|variant| {
                variant.pointer("/properties/kind/const") == Some(&Value::from("request"))
            })
            .expect("Message has a request variant");

        assert_eq!(request["$ref"], "#/$defs/Request");
        assert_eq!(request["unevaluatedProperties"], false);
    }
}
