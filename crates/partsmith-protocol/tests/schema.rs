//! Acceptance tests for the generated protocol JSON Schema (protocol.md section 6, test 41 of "JSON shapes" and "Schema", tests 43 to 46).

mod common;

use std::path::Path;

use common::examples::{COMMAND_EXAMPLES, MESSAGE_EXAMPLES, RESULT_EXAMPLES, TYPE_EXAMPLES};
use common::json_schema::validate;
use common::{definition, parse_json, property, schema_json, variant};
use partsmith_protocol::{MAX_SAFE_INTEGER, protocol_schema};
use serde_json::{Value, json};

/// The schema's `maximum` for property `name` of `subschema`.
fn maximum_of(schema: &Value, subschema: &Value, name: &str) -> Option<u64> {
    property(schema, subschema, name)
        .unwrap_or_else(|| panic!("the schema declares no property {name}"))
        .get("maximum")
        .and_then(Value::as_u64)
}

/// The `oneOf` entry of `Message` whose `kind` is `kind`.
fn message_variant<'a>(schema: &'a Value, kind: &str) -> &'a Value {
    variant(schema, "Message", "kind", kind)
}

#[test]
fn t041_spec_examples_validate_against_the_schema() {
    let schema = schema_json();
    let mut failures = Vec::new();

    // A message example validates as the only message of a frame header, against the schema root.
    for text in MESSAGE_EXAMPLES {
        let header = json!({"protocol_version": 1, "messages": [parse_json(text)], "buffers": []});
        if let Err(reason) = validate(&schema, "", &header) {
            failures.push(format!("{text}\n  {reason}"));
        }
    }
    // An example of a single type validates against its $defs entry.
    let single_types = COMMAND_EXAMPLES
        .into_iter()
        .map(|text| ("Command", text))
        .chain(
            RESULT_EXAMPLES
                .into_iter()
                .map(|text| ("CommandResult", text)),
        )
        .chain(TYPE_EXAMPLES);
    for (name, text) in single_types {
        if let Err(reason) = validate(&schema, &format!("/$defs/{name}"), &parse_json(text)) {
            failures.push(format!("{name}: {text}\n  {reason}"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} examples do not validate:\n{}",
        failures.len(),
        failures.join("\n")
    );

    // The validation is not vacuous: an unknown command does not validate.
    let unknown = json!({
        "protocol_version": 1,
        "messages": [{"kind": "request", "id": 1, "command": {"type": "no_such_command"}}],
        "buffers": []
    });
    assert!(validate(&schema, "", &unknown).is_err());
}

#[test]
fn t043_schema_has_the_draft_title_version_and_definitions() {
    let schema = schema_json();

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], "Partsmith protocol");
    assert_eq!(schema["x-protocol-version"], json!(1));
    for name in [
        "Command",
        "CommandResult",
        "Event",
        "Message",
        "ErrorCode",
        "BufferInfo",
        "SketchSolution",
        "MeshBuffers",
        "PrintReport",
        "FeatureManifest",
        "ElementRef",
        "Repair",
    ] {
        definition(&schema, name);
    }
}

#[test]
fn t044_schema_text_is_stable_and_ends_with_one_newline() {
    let first = protocol_schema();
    let second = protocol_schema();

    assert_eq!(first, second);
    assert!(first.ends_with('\n'), "the schema ends with a newline");
    assert!(
        !first.ends_with("\n\n"),
        "the schema ends with exactly one newline"
    );
}

#[test]
fn t045_schema_equals_the_checked_in_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/format/protocol.schema.json");
    let checked_in = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} cannot be read ({error}). Generate it with `cargo run -p partsmith-cli -- schema --write`.",
            path.display()
        )
    });

    assert!(
        protocol_schema() == checked_in,
        "protocol_schema() differs from {}. Regenerate it with `cargo run -p partsmith-cli -- schema --write`.",
        path.display()
    );
}

#[test]
fn t046_ids_and_mesh_ids_declare_the_javascript_safe_maximum() {
    let schema = schema_json();
    let request = schema
        .pointer("/$defs/Request")
        .unwrap_or_else(|| message_variant(&schema, "request"));
    let response = schema
        .pointer("/$defs/Response")
        .unwrap_or_else(|| message_variant(&schema, "response"));
    let body_mesh = variant(&schema, "Event", "type", "body_mesh");

    assert_eq!(maximum_of(&schema, request, "id"), Some(MAX_SAFE_INTEGER));
    assert_eq!(maximum_of(&schema, response, "id"), Some(MAX_SAFE_INTEGER));
    assert_eq!(
        maximum_of(&schema, body_mesh, "mesh_id"),
        Some(MAX_SAFE_INTEGER)
    );
}
