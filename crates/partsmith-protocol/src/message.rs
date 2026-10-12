//! The message envelope: requests, responses and events (protocol.md section 2.3).

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Command, CommandResult, ErrorInfo, Event, MAX_SAFE_INTEGER};

/// One message in a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(transform = close_request_variant)]
pub enum Message {
    /// A command from a client.
    Request(Request),
    /// The engine's answer to one request.
    Response(Response),
    /// A state change caused by the request answered in the same frame.
    Event(Event),
}

impl Message {
    /// The id of a request or response; events have none.
    pub fn id(&self) -> Option<u64> {
        match self {
            Message::Request(request) => Some(request.id),
            Message::Response(response) => Some(response.id),
            Message::Event(_) => None,
        }
    }
}

/// A command with a client-chosen id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = open_request)]
pub struct Request {
    /// Client-chosen id, echoed in the response; at most `MAX_SAFE_INTEGER` (0 is reserved for the worker's own calls).
    #[schemars(range(max = MAX_SAFE_INTEGER))]
    pub id: u64,
    /// The command.
    pub command: Command,
}

/// Drops `additionalProperties: false` from the `Request` schema so that `Message` can reference it next to its `kind` tag; `close_request_variant` restores the strictness there.
fn open_request(schema: &mut Schema) {
    schema.remove("additionalProperties");
}

/// Rejects unknown request fields in the `request` variant of the `Message` schema, which references `Request` and adds `kind`.
fn close_request_variant(schema: &mut Schema) {
    let variants = schema
        .get_mut("oneOf")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten();
    for variant in variants {
        if variant.pointer("/properties/kind/const") == Some(&Value::from("request"))
            && let Some(variant) = variant.as_object_mut()
        {
            variant.insert("unevaluatedProperties".to_string(), Value::Bool(false));
        }
    }
}

/// The engine's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Response {
    /// The request id, or 0 when the request id could not be read.
    #[schemars(range(max = MAX_SAFE_INTEGER))]
    pub id: u64,
    /// Success value or error.
    pub result: Outcome,
}

/// Success or failure of one request. A value enum inside `Response`, so it is tagged `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[expect(
    clippy::large_enum_variant,
    reason = "one outcome exists per response, so the space an error leaves unused costs nothing, and boxing would change the shape the spec gives"
)]
pub enum Outcome {
    /// The command succeeded; its `type` equals the command's `type`.
    Ok(CommandResult),
    /// The command failed and changed nothing.
    Error(ErrorInfo),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocHandle, ErrorCode};

    #[test]
    fn requests_and_responses_have_ids_and_events_do_not() {
        let request = Message::Request(Request {
            id: 3,
            command: Command::EngineInfo,
        });
        let response = Message::Response(Response {
            id: 4,
            result: Outcome::Error(ErrorInfo::new(ErrorCode::Superseded, "Replaced.")),
        });
        let event = Message::Event(Event::DatumPlane {
            doc: DocHandle(1),
            revision: 0,
            feature: partsmith_model::Id::new("f-dp1").expect("a valid id"),
            placement: None,
        });

        assert_eq!(request.id(), Some(3));
        assert_eq!(response.id(), Some(4));
        assert_eq!(event.id(), None);
    }

    #[test]
    fn request_rejects_unknown_fields() {
        let result = serde_json::from_str::<Message>(
            r#"{"kind":"request","id":1,"command":{"type":"engine_info"},"priority":2}"#,
        );

        assert!(result.is_err(), "got {result:?}");
    }

    #[test]
    fn response_ignores_unknown_fields() {
        let message: Message = serde_json::from_str(
            r#"{"kind":"response","id":1,"result":{"kind":"ok","type":"set_focus"},"elapsed":3}"#,
        )
        .expect("a response with an unknown field deserializes");

        assert_eq!(
            message,
            Message::Response(Response {
                id: 1,
                result: Outcome::Ok(CommandResult::SetFocus),
            })
        );
    }

    #[test]
    fn error_outcome_writes_kind_then_the_error_fields() {
        let outcome = Outcome::Error(ErrorInfo::new(
            ErrorCode::NothingToUndo,
            "There is nothing to undo.",
        ));

        let text = serde_json::to_string(&outcome).expect("serializes");

        assert_eq!(
            text,
            r#"{"kind":"error","code":"nothing_to_undo","message":"There is nothing to undo."}"#
        );
    }

    #[test]
    fn unknown_message_kind_is_rejected() {
        let result = serde_json::from_str::<Message>(r#"{"kind":"notification","id":1}"#);

        assert!(result.is_err(), "got {result:?}");
    }
}
