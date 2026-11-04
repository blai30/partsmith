//! Acceptance test 88 of `docs/spec/protocol.md` section 6 ("Re-attaching by pick"): the command `attach_plane`
//! (section 2.5, command 35) and its result, which carries the closed file-format type `SketchPlane` (section 3.6),
//! plus small unit tests of the same shapes.

use crate::common;

use common::examples::{
    ATTACH_FACE_REQUEST, ATTACH_NO_PLANE_RESULT, ATTACH_PLANE_REQUEST, ATTACH_PLANE_RESULT,
};
use common::json_schema::validate;
use common::{
    compact, frame_with_messages, id, parse, parse_json, property, request_frame, schema_json,
    variant, with_member,
};
use partsmith_protocol::{
    Command, CommandResult, DocHandle, ErrorCode, FrameError, Outcome, decode_request_frame,
    decode_response_frame,
};
use partsmith_sketch::{OriginPlane, SketchPlane};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// The origin plane XY as a typed sketch plane.
fn xy_plane() -> SketchPlane {
    SketchPlane::Origin {
        plane: OriginPlane::Xy,
    }
}

/// The value `text` holds, after checking that it writes back as `text` in compact form.
fn round_trips<T: DeserializeOwned + Serialize>(text: &str) -> T {
    let value: T = parse(text);
    let written = serde_json::to_string(&value).expect("a protocol value serializes");
    assert_eq!(written, compact(text), "{text} writes back unchanged");
    value
}

/// Checks that a request frame whose one request carries `command`, a JSON text, fails as `InvalidMessage`, which
/// hosts answer with `malformed_request`.
fn assert_malformed_request(command: &str) {
    let error = decode_request_frame(&request_frame(command)).expect_err(command);

    assert!(
        matches!(error, FrameError::InvalidMessage { .. }),
        "{command}: got {error:?}"
    );
    assert_eq!(error.code(), ErrorCode::MalformedRequest, "{command}");
}

/// A response frame whose one message is a successful response (id 1) with `result`, a result as JSON.
fn result_frame(result: &Value) -> Vec<u8> {
    let outcome = with_member(result, "kind", json!("ok"));
    frame_with_messages(&json!([{"kind": "response", "id": 1, "result": outcome}]).to_string())
}

/// True when the schema of a property names `$defs/{name}` itself, or in one of its `anyOf`, `oneOf` or `allOf`
/// members (an optional field may allow `null` next to it).
fn refers_to(property_schema: &Value, name: &str) -> bool {
    let target = format!("#/$defs/{name}");
    if property_schema.get("$ref").and_then(Value::as_str) == Some(target.as_str()) {
        return true;
    }
    ["anyOf", "oneOf", "allOf"].iter().any(|keyword| {
        property_schema
            .get(keyword)
            .and_then(Value::as_array)
            .is_some_and(|members| members.iter().any(|member| refers_to(member, name)))
    })
}

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

#[test]
fn t088_an_attach_plane_request_names_its_document_sketch_and_plane() {
    let command: Command = parse(
        r#"{"type":"attach_plane","doc":1,"feature":"f-sk1","plane":{"kind":"origin","plane":"XY"}}"#,
    );

    assert_eq!(
        command,
        Command::AttachPlane {
            doc: DocHandle(1),
            feature: id("f-sk1"),
            plane: xy_plane(),
        }
    );
    assert_eq!(command.type_name(), "attach_plane");
}

#[test]
fn t088_an_attach_plane_request_with_an_unknown_member_does_not_deserialize() {
    let text = r#"{"type":"attach_plane","doc":1,"feature":"f-sk1","plane":{"kind":"origin","plane":"XY"},"extra":1}"#;

    let parsed: Result<Command, _> = serde_json::from_str(text);

    assert!(parsed.is_err(), "{text} deserialized as {parsed:?}");
    assert_malformed_request(text);
}

#[test]
fn t088_the_requests_of_command_35_round_trip() {
    let origin: Command = round_trips(ATTACH_PLANE_REQUEST);
    assert_eq!(
        origin,
        Command::AttachPlane {
            doc: DocHandle(1),
            feature: id("f-sk1"),
            plane: xy_plane(),
        }
    );

    let face: Command = round_trips(ATTACH_FACE_REQUEST);
    let written = serde_json::to_value(&face).expect("a command serializes");
    assert_eq!(written["plane"]["fallback_frame"], "body", "{written}");
}

#[test]
fn t088_the_results_of_command_35_round_trip() {
    let with_plane: CommandResult = round_trips(ATTACH_PLANE_RESULT);
    assert_eq!(
        with_plane,
        CommandResult::AttachPlane {
            plane: Some(xy_plane()),
        }
    );

    let without_plane: CommandResult = round_trips(ATTACH_NO_PLANE_RESULT);
    assert_eq!(without_plane, CommandResult::AttachPlane { plane: None });
    let written = serde_json::to_value(&without_plane).expect("a result serializes");
    assert!(
        written.get("plane").is_none(),
        "a result without a plane writes no plane key: {written}"
    );
}

#[test]
fn t088_the_command_and_its_result_carry_sketch_planes_and_the_result_does_not_require_one() {
    let schema = schema_json();

    let command = variant(&schema, "Command", "type", "attach_plane");
    let command_plane =
        property(&schema, command, "plane").expect("the attach_plane command declares plane");
    assert!(refers_to(command_plane, "SketchPlane"), "{command_plane}");

    let result = variant(&schema, "CommandResult", "type", "attach_plane");
    let result_plane =
        property(&schema, result, "plane").expect("the attach_plane result declares plane");
    assert!(refers_to(result_plane, "SketchPlane"), "{result_plane}");
    assert!(
        !required_names(result).contains(&"plane"),
        "{:?}",
        required_names(result)
    );
}

#[test]
fn t088_a_response_with_a_plane_decodes_and_an_unknown_member_in_the_plane_does_not() {
    let result = parse_json(ATTACH_PLANE_RESULT);

    let decoded =
        decode_response_frame(&result_frame(&result)).expect("the result example decodes");
    assert_eq!(
        decoded.response.result,
        Outcome::Ok(CommandResult::AttachPlane {
            plane: Some(xy_plane()),
        })
    );

    let mut extra = result.clone();
    extra["plane"]["extra"] = json!(1);
    let refused = decode_response_frame(&result_frame(&extra));
    assert!(
        matches!(refused, Err(FrameError::InvalidMessage { .. })),
        "got {refused:?}"
    );
}

#[test]
fn an_attach_plane_request_without_its_plane_is_malformed() {
    assert_malformed_request(r#"{"type":"attach_plane","doc":1,"feature":"f-sk1"}"#);
}

#[test]
fn an_attach_plane_request_with_an_unknown_plane_kind_is_malformed() {
    assert_malformed_request(
        r#"{"type":"attach_plane","doc":1,"feature":"f-sk1","plane":{"kind":"edge","plane":"XY"}}"#,
    );
}

#[test]
fn an_attach_plane_request_decodes_from_a_request_frame() {
    let (request, buffers) = decode_request_frame(&request_frame(ATTACH_FACE_REQUEST))
        .expect("the face plane request decodes");

    assert!(buffers.is_empty());
    let Command::AttachPlane { plane, .. } = request.command else {
        panic!("the request is attach_plane, got {:?}", request.command);
    };
    assert!(matches!(plane, SketchPlane::Face { .. }), "{plane:?}");
}

#[test]
fn the_attach_plane_examples_validate_against_the_schema() {
    let schema = schema_json();

    for text in [ATTACH_PLANE_REQUEST, ATTACH_FACE_REQUEST] {
        assert_eq!(
            validate(&schema, "/$defs/Command", &parse_json(text)),
            Ok(()),
            "{text}"
        );
    }
    for text in [ATTACH_PLANE_RESULT, ATTACH_NO_PLANE_RESULT] {
        assert_eq!(
            validate(&schema, "/$defs/CommandResult", &parse_json(text)),
            Ok(()),
            "{text}"
        );
    }
}

#[test]
fn a_result_whose_plane_has_an_unknown_member_fails_the_schema() {
    let schema = schema_json();
    let mut result = parse_json(ATTACH_PLANE_RESULT);
    result["plane"]["extra"] = json!(1);

    assert!(validate(&schema, "/$defs/CommandResult", &result).is_err());
}
