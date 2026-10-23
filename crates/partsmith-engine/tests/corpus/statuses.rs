//! Readers of what the tests check: statuses, messages and actions, causes and ghosts, and results.

use partsmith_engine::Session;
use serde_json::{Map, Value, json};

use crate::common::{
    BUFFER_TOLERANCE, Sent, elements, measure_body, number, revision, saved_text, send,
};

// Statuses, messages and actions.

/// The statuses of the only `recomputed` event of `sent`, in timeline order.
pub fn statuses(sent: &Sent) -> Vec<Value> {
    sent.event("recomputed")["features"]
        .as_array()
        .expect("recomputed lists features")
        .clone()
}

/// The messages of `feature` in the only `recomputed` event of `sent`.
pub fn messages(sent: &Sent, feature: &str) -> Vec<Value> {
    sent.feature_status(feature)["messages"]
        .as_array()
        .expect("a status lists its messages")
        .clone()
}

/// The only message of `feature` with `code` in the only `recomputed` event of `sent`.
pub fn message(sent: &Sent, feature: &str, code: &str) -> Value {
    let found: Vec<Value> = messages(sent, feature)
        .into_iter()
        .filter(|message| message["code"] == code)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{feature} has exactly one {code} message: {}",
        sent.feature_status(feature)
    );
    found[0].clone()
}

/// Asserts that every feature named is ok with no message.
pub fn assert_ok_without_messages(sent: &Sent, features: &[&str]) {
    for feature in features {
        assert_eq!(sent.feature_status(feature)["status"], "ok", "{feature}");
        assert_eq!(messages(sent, feature), Vec::<Value>::new(), "{feature}");
    }
}

/// The messages of a solution (a `SketchSolution` JSON value), in order.
pub fn solution_messages(solution: &Value) -> Vec<Value> {
    solution["messages"]
        .as_array()
        .expect("a solution lists its messages")
        .clone()
}

/// The repairs of `feature` in the only `recomputed` event of `sent`.
pub fn repairs(sent: &Sent, feature: &str) -> Vec<Value> {
    sent.feature_status(feature)["repairs"]
        .as_array()
        .expect("a status lists its repairs")
        .clone()
}

/// The actions of a message (or of a cause).
pub fn actions(message: &Value) -> Vec<Value> {
    message["actions"].as_array().cloned().unwrap_or_default()
}

/// The labels of the actions of a message, in order.
pub fn labels(message: &Value) -> Vec<String> {
    actions(message)
        .iter()
        .map(|action| {
            action["label"]
                .as_str()
                .expect("actions have labels")
                .to_owned()
        })
        .collect()
}

/// The action of a message labeled `label`.
pub fn action(message: &Value, label: &str) -> Value {
    actions(message)
        .into_iter()
        .find(|action| action["label"] == label)
        .unwrap_or_else(|| panic!("no action labeled {label:?} in {message}"))
}

/// Sends `apply_fix` with the `FixRef` of an `apply` action.
pub fn apply_fix(session: &mut Session, doc: u64, action: &Value) -> Sent {
    assert_eq!(
        action["run"]["kind"], "apply",
        "{action} is an apply action"
    );
    send_fix(session, doc, action["run"]["fix"].clone())
}

/// Sends `apply_fix` with this `FixRef` JSON.
pub fn send_fix(session: &mut Session, doc: u64, fix: Value) -> Sent {
    send(
        session,
        json!({"type": "apply_fix", "doc": doc, "fix": fix}),
    )
}

/// The `FixRef` JSON of an `apply` action of `feature`'s message with `code` and `field`.
pub fn fix_ref(feature: &str, code: &str, field: &str, index: u32, revision: u64) -> Value {
    json!({
        "feature": feature, "code": code, "field": field,
        "cause": false, "index": index, "revision": revision
    })
}

/// Applies an action with `apply_fix` outside a transaction and checks that it is one recipe change recorded as one
/// undo entry labeled with the action's label, emitted like `apply_edits`.
pub fn apply_as_one_entry(session: &mut Session, doc: u64, action: &Value) -> Sent {
    let before = revision(session, doc);

    let applied = apply_fix(session, doc, action);

    assert_eq!(applied.ok()["revision"], before + 1, "{action}");
    assert_eq!(
        applied.event_types()[..2],
        ["document_changed", "recomputed"],
        "{:?}",
        applied.event_types()
    );
    let changed = applied.event("document_changed");
    assert_eq!(changed["revision"], before + 1);
    assert_eq!(changed["undo"], action["label"]);
    applied
}

/// Undoes the newest entry, checks its label, and checks that the document's text is `before` again.
pub fn assert_undo_restores(session: &mut Session, doc: u64, label: &Value, before: &str) {
    let undone = send(session, json!({"type": "undo", "doc": doc}));
    assert_eq!(&undone.ok()["label"], label);
    assert_eq!(saved_text(session, doc), before);
}

/// Asserts that a reply is the `stale_fix` error with its message and no events, and that the document is still at
/// `revision_before` with `text`.
pub fn assert_stale(
    session: &mut Session,
    doc: u64,
    sent: &Sent,
    revision_before: u64,
    text: &str,
) {
    assert_eq!(sent.error_code(), "stale_fix");
    assert_eq!(
        sent.error_message(),
        "This fix no longer applies because the document changed after it was offered. Look at the problem again."
    );
    assert!(sent.events.is_empty(), "{:?}", sent.event_types());
    assert_eq!(revision(session, doc), revision_before);
    assert_eq!(saved_text(session, doc), text);
}

// Causes and ghosts (engine.md sections 3.21 and 3.23).

/// The cause of a message; panics when it has none.
pub fn cause(message: &Value) -> Value {
    message
        .get("cause")
        .filter(|cause| cause.is_object())
        .cloned()
        .unwrap_or_else(|| panic!("the message has a cause: {message}"))
}

/// The label and run kind of each action of a message or a cause, in order.
pub fn labels_and_kinds(message: &Value) -> Vec<(String, String)> {
    actions(message)
        .iter()
        .map(|action| {
            (
                action["label"]
                    .as_str()
                    .expect("actions have labels")
                    .to_owned(),
                action["run"]["kind"]
                    .as_str()
                    .expect("actions say how they run")
                    .to_owned(),
            )
        })
        .collect()
}

/// Pairs of words as owned strings, such as the label and run kind of each action [`labels_and_kinds`] gives.
pub fn owned_pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(first, second)| ((*first).to_owned(), (*second).to_owned()))
        .collect()
}

/// The ghost of the only repair of `feature` at `path` in the only `recomputed` event of `sent`; panics when that
/// repair has none.
pub fn repair_ghost(sent: &Sent, feature: &str, path: &str) -> Value {
    let found: Vec<Value> = repairs(sent, feature)
        .into_iter()
        .filter(|repair| repair["path"] == path)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{feature} has one repair at {path}: {}",
        sent.feature_status(feature)
    );
    found[0]
        .get("ghost")
        .filter(|ghost| ghost.is_object())
        .cloned()
        .unwrap_or_else(|| panic!("the repair at {path} has a ghost: {}", found[0]))
}

/// The points of a ghost whose buffer travels in `sent`: 3 coordinates each in frame `world`, 2 in frame `sketch`.
pub fn ghost_points(sent: &Sent, ghost: &Value) -> Vec<Vec<f64>> {
    let dimension = match ghost["frame"].as_str() {
        Some("world") => 3,
        Some("sketch") => 2,
        other => panic!("a ghost's frame is world or sketch, not {other:?}"),
    };
    let values = sent.f32_buffer(&ghost["positions"]);
    assert!(
        values.len().is_multiple_of(dimension),
        "a {dimension}D ghost holds whole points: {values:?}"
    );
    values
        .chunks(dimension)
        .map(|point| point.iter().map(|value| f64::from(*value)).collect())
        .collect()
}

/// Whether two points are equal within the buffer tolerance.
fn same_point(actual: &[f64], expected: &[f64]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(found, wanted)| {
            (found - wanted).abs() <= BUFFER_TOLERANCE * wanted.abs().max(1.0)
        })
}

/// Asserts that a `lines` ghost whose buffer travels in `sent` holds exactly the segments `expected` (each a start and
/// an end point), in any order and each in either direction.
pub fn assert_ghost_segments(sent: &Sent, ghost: &Value, expected: &[(&[f64], &[f64])]) {
    assert_eq!(ghost["kind"], "lines", "{ghost}");
    let points = ghost_points(sent, ghost);
    assert_eq!(
        points.len(),
        2 * expected.len(),
        "the ghost holds {} segments: {points:?}",
        expected.len()
    );
    let mut unmatched: Vec<&[Vec<f64>]> = points.chunks(2).collect();
    for (start, end) in expected {
        let position = unmatched
            .iter()
            .position(|segment| {
                (same_point(&segment[0], start) && same_point(&segment[1], end))
                    || (same_point(&segment[0], end) && same_point(&segment[1], start))
            })
            .unwrap_or_else(|| {
                panic!("the ghost has the segment from {start:?} to {end:?}: {unmatched:?}")
            });
        unmatched.remove(position);
    }
}

// Results.

/// The volume of a body at the display state.
pub fn volume(session: &mut Session, doc: u64, body: &str) -> f64 {
    number(&measure_body(session, doc, body)["volume"])
}

/// The names of a body's faces at the display state.
pub fn face_names(session: &mut Session, doc: u64, body: &str) -> Vec<String> {
    elements(session, doc, body, Some("face"))
        .iter()
        .map(|face| face["name"].as_str().expect("faces have names").to_owned())
        .collect()
}

/// How many faces of a body a feature named first (their names start with `<feature>/`).
pub fn faces_of(session: &mut Session, doc: u64, body: &str, feature: &str) -> usize {
    let prefix = format!("{feature}/");
    face_names(session, doc, body)
        .iter()
        .filter(|name| name.starts_with(&prefix))
        .count()
}

/// The value with every key named in `keys` removed, at any depth.
pub fn without_keys(value: &Value, keys: &[&str]) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !keys.contains(&key.as_str()))
                .map(|(key, child)| (key.clone(), without_keys(child, keys)))
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| without_keys(item, keys)).collect())
        }
        other => other.clone(),
    }
}
