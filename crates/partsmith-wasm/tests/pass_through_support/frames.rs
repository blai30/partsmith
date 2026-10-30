//! Readers of the response frames the bridge returns, decoded or as their bytes hold them.

use partsmith_protocol::ResponseFrame;
use serde_json::Value;

use crate::common::{header_json, ok_result};

use super::documents::feature_record;

/// The frame's events of type `event_type`, as JSON.
pub fn events_of(frame: &ResponseFrame, event_type: &str) -> Vec<Value> {
    frame
        .events
        .iter()
        .map(|event| serde_json::to_value(event).expect("events always serialize"))
        .filter(|event| event["type"] == event_type)
        .collect()
}

/// The status of `feature` among the statuses of `recomputed`, a `recomputed` event as JSON.
fn status_in<'a>(recomputed: &'a Value, feature: &str) -> Option<&'a Value> {
    recomputed["features"]
        .as_array()
        .expect("recomputed lists its features")
        .iter()
        .find(|status| status["id"] == feature)
}

/// The status of `feature` in the frame's `recomputed` event.
pub fn feature_status(frame: &ResponseFrame, feature: &str) -> Value {
    events_of(frame, "recomputed")
        .iter()
        .find_map(|event| status_in(event, feature))
        .unwrap_or_else(|| panic!("the frame has no recomputed status of {feature}"))
        .clone()
}

/// The first entry of the list `list` of `status`, a feature status as JSON, whose `key` is `value`.
pub fn entry_of(status: &Value, list: &str, key: &str, value: &str) -> Value {
    status[list]
        .as_array()
        .unwrap_or_else(|| panic!("statuses list their {list}: {status}"))
        .iter()
        .find(|entry| entry[key] == value)
        .unwrap_or_else(|| panic!("the status has none of its {list} with {key} {value}: {status}"))
        .clone()
}

/// The first entry of the list `list` of `feature`'s status in the frame's `recomputed` event whose `key` is `value`.
fn status_entry(frame: &ResponseFrame, feature: &str, list: &str, key: &str, value: &str) -> Value {
    entry_of(&feature_status(frame, feature), list, key, value)
}

/// The message with `code` among the messages of `feature` in the frame's `recomputed` event.
pub fn feature_message(frame: &ResponseFrame, feature: &str, code: &str) -> Value {
    status_entry(frame, feature, "messages", "code", code)
}

/// The repair at `path` among the repairs of `feature` in the frame's `recomputed` event.
pub fn feature_repair(frame: &ResponseFrame, feature: &str, path: &str) -> Value {
    status_entry(frame, feature, "repairs", "path", path)
}

/// The only item of `items`, where `what` names one item for the failure text.
#[track_caller]
pub fn only(items: Vec<Value>, what: &str) -> Value {
    match <[Value; 1]>::try_from(items) {
        Ok([item]) => item,
        Err(items) => panic!("one {what}, not {}: {items:?}", items.len()),
    }
}

/// The only message of `holder`, a feature status or a sketch solution.
pub fn only_message_of(holder: &Value) -> Value {
    let messages = holder["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("{holder} lists its messages"));
    only(messages.clone(), "message")
}

/// The only message of `feature` in the frame's `recomputed` event.
pub fn only_message(frame: &ResponseFrame, feature: &str) -> Value {
    only_message_of(&feature_status(frame, feature))
}

/// The first message of `feature` in the frame's `recomputed` event: its error, else its first warning or notice.
pub fn first_message(frame: &ResponseFrame, feature: &str) -> Value {
    let status = feature_status(frame, feature);
    status["messages"]
        .as_array()
        .and_then(|messages| messages.first())
        .cloned()
        .unwrap_or_else(|| panic!("{feature} has a message: {status}"))
}

/// The frame's only `document_changed` event.
pub fn document_changed(frame: &ResponseFrame) -> Value {
    only(
        events_of(frame, "document_changed"),
        "document_changed event",
    )
}

/// The recipe in the frame's only `document_changed` event.
pub fn changed_recipe(frame: &ResponseFrame) -> Value {
    document_changed(frame)["document"].clone()
}

/// The record of `feature` in the recipe of the frame's `document_changed` event.
pub fn changed_record(frame: &ResponseFrame, feature: &str) -> Value {
    feature_record(&changed_recipe(frame), feature)
}

/// The `id` of each object of the JSON array `items`, in order.
pub fn ids_of(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap_or_else(|| panic!("{items} is a list"))
        .iter()
        .map(|item| item["id"].as_str().expect("ids are text").to_owned())
        .collect()
}

/// The constraints of `f-sk1` in the frame's recipe: the one `Connect the ends` adds to NC01's sketch, which has none.
pub fn constraints_of_sketch_1(frame: &ResponseFrame) -> Vec<String> {
    ids_of(&changed_record(frame, "f-sk1")["constraints"])
}

/// The actions of a message, in order.
pub fn actions(message: &Value) -> &[Value] {
    message["actions"]
        .as_array()
        .unwrap_or_else(|| panic!("the message has actions: {message}"))
}

/// The labels of a message's actions, in order.
pub fn labels(message: &Value) -> Vec<&str> {
    actions(message)
        .iter()
        .map(|action| action["label"].as_str().expect("actions have labels"))
        .collect()
}

/// The action labeled `label` among a message's actions.
pub fn action_labeled(message: &Value, label: &str) -> Value {
    actions(message)
        .iter()
        .find(|action| action["label"] == label)
        .unwrap_or_else(|| panic!("the message offers {label:?}: {message}"))
        .clone()
}

/// The label and the run kind of each action of `holder`, a message or a cause, in order.
pub fn labels_and_kinds(holder: &Value) -> Vec<(&str, &str)> {
    actions(holder)
        .iter()
        .map(|action| {
            (
                action["label"].as_str().expect("actions have labels"),
                action["run"]["kind"]
                    .as_str()
                    .expect("actions say how they run"),
            )
        })
        .collect()
}

/// The cause of `message`.
pub fn cause_of(message: &Value) -> &Value {
    message
        .get("cause")
        .filter(|cause| cause.is_object())
        .unwrap_or_else(|| panic!("the message has a cause: {message}"))
}

/// The candidates of a repair.
pub fn candidates(repair: &Value) -> &[Value] {
    repair["candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("the repair lists its candidates: {repair}"))
}

/// The descriptors of a `list_elements` frame, in their order.
pub fn listed(frame: &ResponseFrame) -> Vec<Value> {
    ok_result(frame, "list_elements")["elements"]
        .as_array()
        .expect("list_elements lists elements")
        .clone()
}

/// The position of the descriptor named `name` in a `list_elements` frame of one kind, which a pick of that kind names as its `index`.
pub fn listed_index(frame: &ResponseFrame, name: &str) -> usize {
    listed(frame)
        .iter()
        .position(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("the list has {name}"))
}

/// The element named `name` among `elements`, the descriptors of a `list_elements` result.
pub fn named<'a>(elements: &'a [Value], name: &str) -> &'a Value {
    elements
        .iter()
        .find(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("the list has {name}"))
}

/// The descriptor named `name` in a `list_elements` frame.
pub fn listed_named(frame: &ResponseFrame, name: &str) -> Value {
    named(&listed(frame), name).clone()
}

/// The solution of `feature` in the frame's only `sketch_solved` event for it.
pub fn sketch_solution(frame: &ResponseFrame, feature: &str) -> Value {
    let solved = events_of(frame, "sketch_solved")
        .into_iter()
        .filter(|event| event["solution"]["feature"] == feature)
        .collect();
    only(solved, &format!("sketch_solved event of {feature}"))["solution"].clone()
}

/// The only message of `feature`'s solution in the frame's `sketch_solved` event.
pub fn only_solution_message(frame: &ResponseFrame, feature: &str) -> Value {
    only_message_of(&sketch_solution(frame, feature))
}

// Frames as their bytes hold them.

/// The header of a frame as its bytes hold it, each object's keys in the order the frame writes them (see [`header_json`]).
fn wire_header(frame: &[u8]) -> Value {
    header_json(frame).expect("frame headers are JSON")
}

/// The messages of a frame as its bytes hold them, in order (see [`wire_header`]).
fn wire_messages(frame: &[u8]) -> Vec<Value> {
    match wire_header(frame)["messages"].take() {
        Value::Array(messages) => messages,
        other => panic!("frames list their messages: {other}"),
    }
}

/// The response of a frame as its bytes hold it (see [`wire_header`]).
pub fn wire_response(frame: &[u8]) -> Value {
    wire_messages(frame)
        .pop()
        .expect("a response frame ends with its response")
}

/// The only event of type `event_type` of a frame as its bytes hold it (see [`wire_header`]).
pub fn wire_event(frame: &[u8], event_type: &str) -> Value {
    let events = wire_messages(frame)
        .into_iter()
        .filter(|message| message["kind"] == "event" && message["type"] == event_type)
        .collect();
    only(events, &format!("{event_type} event"))
}

/// The status of `feature` in the only `recomputed` event of a frame as its bytes hold it (see [`wire_header`]).
pub fn wire_status(frame: &[u8], feature: &str) -> Value {
    status_in(&wire_event(frame, "recomputed"), feature)
        .unwrap_or_else(|| panic!("the frame has no recomputed status of {feature}"))
        .clone()
}

/// The keys of the JSON object `object`, in the order it holds them.
pub fn keys_of(object: &Value) -> Vec<String> {
    object
        .as_object()
        .unwrap_or_else(|| panic!("{object} is an object"))
        .keys()
        .cloned()
        .collect()
}

/// The keys of `keys` that `all` holds, in the order `all` holds them.
pub fn in_wire_order<'a>(all: &'a [String], keys: &[&str]) -> Vec<&'a str> {
    all.iter()
        .map(String::as_str)
        .filter(|key| keys.contains(key))
        .collect()
}
