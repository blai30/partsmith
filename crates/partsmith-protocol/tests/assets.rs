//! Acceptance tests 105 to 108 of `docs/spec/protocol.md` ("Assets", ADR 0064): command 38 `add_assets`, its
//! request and result types, the asset types in the schema and `missing_assets` in `document_changed` (sections 2.4,
//! 2.5 and 2.7). Test 109 goes through `Session::call` and lives in partsmith-engine (`tests/assets.rs`), and test 34's
//! final command list is in `json_shapes.rs`.

use crate::common;

use common::examples::{
    ADD_ASSETS_REQUEST, ADD_ASSETS_RESULT, ADD_PICKED_FONT_REQUEST, ADD_PICKED_FONT_RESULT,
    DOCUMENT_CHANGED_EVENT, MESSAGE_EXAMPLES,
};
use common::{
    compact, definition, id, message, request_frame, schema_json, string_set, string_values,
    variant, variant_tags,
};
use partsmith_protocol::{
    AddedAsset, AssetInput, AssetKind, AssetUse, BufferIndex, Command, CommandResult, DocHandle,
    ErrorCode, Event, FrameError, Message, decode_request_frame,
};
use serde_json::{Value, json};

/// The asset name `S` of `examples/import/step-block.ptsm`.
const S: &str = "b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step";

/// The names a schema object lists as `required`.
fn required_names(subschema: &Value) -> Vec<&str> {
    subschema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// Asserts that `command`, a JSON text, sent in a request frame, is refused as a malformed request.
fn assert_malformed(command: &str) {
    let error =
        decode_request_frame(&request_frame(command)).expect_err("the request does not decode");

    assert!(
        matches!(error, FrameError::InvalidMessage { .. }),
        "{command}: got {error:?}"
    );
    assert_eq!(error.code(), ErrorCode::MalformedRequest, "{command}");
}

#[test]
fn t105_the_request_deserializes_with_a_named_and_a_picked_entry() {
    let text = format!(
        r#"{{"type":"add_assets","doc":1,"assets":[{{"kind":"step","data":0,"name":"{S}"}},{{"kind":"ttf","data":1}}]}}"#
    );

    let command: Command = serde_json::from_str(&text).expect("the request deserializes");

    assert_eq!(
        command,
        Command::AddAssets {
            doc: DocHandle(1),
            assets: vec![
                AssetInput {
                    kind: AssetKind::Step,
                    data: BufferIndex(0),
                    name: Some(S.to_owned()),
                },
                AssetInput {
                    kind: AssetKind::Ttf,
                    data: BufferIndex(1),
                    name: None,
                },
            ],
        }
    );
    assert_eq!(
        serde_json::to_string(&command).expect("the request serializes"),
        text
    );
    assert_eq!(command.type_name(), "add_assets");
}

#[test]
fn t105_missing_assets_deserialize_as_empty() {
    let command: Command =
        serde_json::from_str(r#"{"type":"add_assets","doc":1}"#).expect("the request deserializes");

    assert_eq!(
        command,
        Command::AddAssets {
            doc: DocHandle(1),
            assets: Vec::new(),
        }
    );
}

#[test]
fn t105_an_unknown_kind_is_a_malformed_request() {
    assert_malformed(r#"{"type":"add_assets","doc":1,"assets":[{"kind":"woff","data":0}]}"#);
}

#[test]
fn t105_an_extra_member_of_an_entry_is_a_malformed_request() {
    assert_malformed(
        r#"{"type":"add_assets","doc":1,"assets":[{"kind":"step","data":0,"extra":1}]}"#,
    );
}

#[test]
fn an_extra_member_of_the_command_is_a_malformed_request() {
    assert_malformed(r#"{"type":"add_assets","doc":1,"assets":[],"extra":1}"#);
}

#[test]
fn an_entry_without_data_is_a_malformed_request() {
    assert_malformed(r#"{"type":"add_assets","doc":1,"assets":[{"kind":"step"}]}"#);
}

#[test]
fn the_spec_examples_of_command_38_round_trip() {
    for text in [ADD_ASSETS_REQUEST, ADD_PICKED_FONT_REQUEST] {
        let command: Command = serde_json::from_str(text).expect("the request deserializes");
        assert_eq!(
            serde_json::to_string(&command).expect("the request serializes"),
            compact(text)
        );
    }
    for text in [ADD_ASSETS_RESULT, ADD_PICKED_FONT_RESULT] {
        let result: CommandResult = serde_json::from_str(text).expect("the result deserializes");
        assert_eq!(
            serde_json::to_string(&result).expect("the result serializes"),
            compact(text)
        );
    }
}

#[test]
fn t106_the_result_round_trips() {
    let text = r#"{"type":"add_assets","assets":[{"name":"S","stored":true},{"name":"S","stored":false}]}"#;

    let result: CommandResult = serde_json::from_str(text).expect("the result deserializes");

    assert_eq!(
        result,
        CommandResult::AddAssets {
            assets: vec![
                AddedAsset {
                    name: "S".to_owned(),
                    stored: true,
                },
                AddedAsset {
                    name: "S".to_owned(),
                    stored: false,
                },
            ],
        }
    );
    assert_eq!(
        serde_json::to_string(&result).expect("the result serializes"),
        text
    );
}

#[test]
fn t106_an_empty_result_round_trips() {
    let text = r#"{"type":"add_assets","assets":[]}"#;

    let result: CommandResult = serde_json::from_str(text).expect("the result deserializes");

    assert_eq!(
        serde_json::to_string(&result).expect("the result serializes"),
        text
    );
}

#[test]
fn t107_asset_kinds_serialize_as_their_extensions() {
    for (kind, text) in [
        (AssetKind::Step, "step"),
        (AssetKind::Ttf, "ttf"),
        (AssetKind::Otf, "otf"),
    ] {
        assert_eq!(
            serde_json::to_value(kind).expect("a kind serializes"),
            json!(text)
        );
    }
}

#[test]
fn t107_the_schema_holds_the_asset_types() {
    let schema = schema_json();

    assert_eq!(
        string_values(&schema, "AssetKind"),
        string_set(&["step", "ttf", "otf"])
    );
    assert_eq!(
        required_names(definition(&schema, "AssetInput")),
        ["kind", "data"]
    );
    assert_eq!(
        required_names(definition(&schema, "AddedAsset")),
        ["name", "stored"]
    );
    assert_eq!(
        required_names(definition(&schema, "AssetUse")),
        ["name", "features"]
    );
    assert!(
        variant_tags(&schema, "Command", "type").contains("add_assets"),
        "Command lists add_assets"
    );
}

#[test]
fn t108_the_document_changed_examples_round_trip() {
    let examples: Vec<&str> = MESSAGE_EXAMPLES
        .into_iter()
        .filter(|text| text.contains(r#""type": "document_changed""#))
        .collect();
    assert_eq!(examples.len(), 2, "sections 2.3 and 2.7 each have one");

    for text in examples {
        let event = message(text);
        let Message::Event(Event::DocumentChanged { missing_assets, .. }) = &event else {
            panic!("the example is a document_changed event, got {event:?}");
        };
        assert!(missing_assets.is_empty(), "{text}");
        assert_eq!(
            serde_json::to_string(&event).expect("an event serializes"),
            compact(text)
        );
    }
}

#[test]
fn t108_missing_assets_are_written_before_bare_numbers() {
    let Message::Event(mut event) = message(DOCUMENT_CHANGED_EVENT) else {
        panic!("the example is an event");
    };
    let Event::DocumentChanged { missing_assets, .. } = &mut event else {
        panic!("the example is a document_changed event");
    };
    *missing_assets = vec![AssetUse {
        name: S.to_owned(),
        features: vec![id("f-imp1")],
    }];

    let text = serde_json::to_string(&event).expect("an event serializes");

    let end =
        format!(r#","missing_assets":[{{"name":"{S}","features":["f-imp1"]}}],"bare_numbers":0}}"#);
    assert!(text.ends_with(&end), "{text}");
}

#[test]
fn t108_the_schema_requires_missing_assets() {
    let schema = schema_json();

    let event = variant(&schema, "Event", "type", "document_changed");

    assert!(
        required_names(event).contains(&"missing_assets"),
        "{:?}",
        required_names(event)
    );
}
