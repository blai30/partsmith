//! Acceptance tests 96 to 98 of `docs/spec/protocol.md` ("Cancel", ADR 0063): the feature state `not_run`, command
//! 37 `rebuild` and the error code `canceled` (sections 2.5, 2.7 and 4). Tests 99 to 104 live in the UI, the web core
//! and its build script.

use crate::common;

use common::{id, request_frame, schema_json, string_values, variant_tags};
use partsmith_protocol::{
    Command, CommandResult, DocHandle, ErrorCode, FeatureState, FeatureStatus, FrameError,
    decode_request_frame,
};
use serde_json::json;

#[test]
fn t096_not_run_round_trips_as_a_feature_state() {
    let text = r#"{"id":"f-box1","status":"not_run","messages":[],"repairs":[]}"#;

    let status: FeatureStatus = serde_json::from_str(text).expect("the status deserializes");

    assert_eq!(
        serde_json::to_value(FeatureState::NotRun).expect("a state serializes"),
        json!("not_run")
    );
    assert_eq!(
        status,
        FeatureStatus {
            id: id("f-box1"),
            status: FeatureState::NotRun,
            messages: Vec::new(),
            repairs: Vec::new(),
        }
    );
    assert_eq!(
        serde_json::to_string(&status).expect("the status serializes"),
        text
    );
}

#[test]
fn t097_rebuild_round_trips_and_names_its_type() {
    let text = r#"{"type":"rebuild","doc":1}"#;

    let command: Command = serde_json::from_str(text).expect("the request deserializes");

    assert_eq!(command, Command::Rebuild { doc: DocHandle(1) });
    assert_eq!(command.type_name(), "rebuild");
    assert_eq!(
        serde_json::to_string(&command).expect("the request serializes"),
        text
    );
}

#[test]
fn t097_the_rebuild_result_round_trips() {
    let text = r#"{"type":"rebuild"}"#;

    let result: CommandResult = serde_json::from_str(text).expect("the result deserializes");

    assert_eq!(result, CommandResult::Rebuild);
    assert_eq!(
        serde_json::to_string(&result).expect("the result serializes"),
        text
    );
}

#[test]
fn t097_rebuild_without_doc_is_a_malformed_request() {
    let error = decode_request_frame(&request_frame(r#"{"type":"rebuild"}"#))
        .expect_err("a rebuild without doc does not decode");

    assert!(
        matches!(error, FrameError::InvalidMessage { .. }),
        "got {error:?}"
    );
    assert_eq!(error.code(), ErrorCode::MalformedRequest);
}

#[test]
fn t098_canceled_serializes_as_its_code() {
    assert_eq!(
        serde_json::to_value(ErrorCode::Canceled).expect("a code serializes"),
        json!("canceled")
    );
}

#[test]
fn t098_the_schema_lists_not_run_canceled_and_rebuild() {
    let schema = schema_json();

    assert!(
        string_values(&schema, "FeatureState").contains("not_run"),
        "FeatureState lists not_run"
    );
    assert!(
        string_values(&schema, "ErrorCode").contains("canceled"),
        "ErrorCode lists canceled"
    );
    assert!(
        variant_tags(&schema, "Command", "type").contains("rebuild"),
        "Command lists rebuild"
    );
}
