//! Edits: the changes of a corpus edit made into model edits against the current records ([`edits`]), applied as one
//! `apply_edits` ([`after_edit`]) or inside a transaction, and the documents they give.

use partsmith_engine::Session;
use serde_json::{Value, json};

use super::scenarios::{Change, Scenario};
use crate::common::{
    self, Sent, apply, begin, commit, doc_handle, get_document, headless_session, open_text,
    saved_text,
};

/// The label of the `apply_edits` that makes a corpus edit.
pub const EDIT_LABEL: &str = "Edit";

/// The feature records of a document value.
pub fn features(document: &Value) -> Vec<Value> {
    document["features"]
        .as_array()
        .expect("documents list their features")
        .clone()
}

/// The current record of `feature`.
pub fn record(session: &mut Session, doc: u64, feature: &str) -> Value {
    features(&get_document(session, doc)["document"])
        .into_iter()
        .find(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has no feature {feature}"))
}

/// The model edits that make `changes` against the document's current records: one `update_feature` per changed
/// record (each with all its changes, in the order first changed), then the insertions, removals, moves and units in
/// order.
pub fn edits(session: &mut Session, doc: u64, changes: &[Change]) -> Vec<Value> {
    let timeline = features(&get_document(session, doc)["document"]);
    let position = |feature: &str| {
        timeline
            .iter()
            .position(|record| record["id"] == feature)
            .unwrap_or_else(|| panic!("the document has no feature {feature}"))
    };
    let mut updated: Vec<Value> = Vec::new();
    let mut structural = Vec::new();
    for change in changes {
        match change {
            Change::Entities(feature, entities) => {
                set_in(
                    &mut updated,
                    &timeline,
                    feature,
                    "/entities",
                    json!(entities),
                );
            }
            Change::Set(feature, pointer, value) => {
                set_in(&mut updated, &timeline, feature, pointer, value.clone());
            }
            Change::InsertBefore(feature, records) => {
                push_insertions(&mut structural, position(feature), records);
            }
            Change::InsertAt(index, records) => push_insertions(&mut structural, *index, records),
            Change::Remove(removed) => {
                structural.extend(
                    removed
                        .iter()
                        .map(|feature| json!({"type": "remove_feature", "id": feature})),
                );
            }
            Change::Move(feature, index) => {
                structural.push(json!({"type": "move_feature", "id": feature, "index": index}));
            }
            Change::Units(units) => structural.push(json!({"type": "set_units", "units": units})),
        }
    }
    updated
        .into_iter()
        .map(|record| json!({"type": "update_feature", "feature": record}))
        .chain(structural)
        .collect()
}

/// Adds one `add_feature` edit per record of `records`, in order, from timeline position `index` on.
fn push_insertions(structural: &mut Vec<Value>, index: usize, records: &[Value]) {
    for (offset, inserted) in records.iter().enumerate() {
        structural
            .push(json!({"type": "add_feature", "index": index + offset, "feature": inserted}));
    }
}

/// Sets `pointer` of `feature`'s record among the `updated` records, taking the record from `timeline` the first time.
fn set_in(
    updated: &mut Vec<Value>,
    timeline: &[Value],
    feature: &str,
    pointer: &str,
    value: Value,
) {
    let position = match updated.iter().position(|record| record["id"] == feature) {
        Some(position) => position,
        None => {
            let record = timeline
                .iter()
                .find(|record| record["id"] == feature)
                .unwrap_or_else(|| panic!("the document has no feature {feature}"))
                .clone();
            updated.push(record);
            updated.len() - 1
        }
    };
    set_pointer(&mut updated[position], pointer, value);
}

/// Sets the value at a JSON Pointer, adding the last key to its object when it is not there, or appending to its array
/// when the position is the array's length.
fn set_pointer(target: &mut Value, pointer: &str, value: Value) {
    let (parent_pointer, key) = pointer
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("{pointer} is a JSON Pointer"));
    let parent = target
        .pointer_mut(parent_pointer)
        .unwrap_or_else(|| panic!("the record has {parent_pointer}"));
    match parent {
        Value::Object(object) => {
            object.insert(key.to_owned(), value);
        }
        Value::Array(items) => {
            let index: usize = key.parse().expect("array positions are numbers");
            if index == items.len() {
                items.push(value);
            } else {
                items[index] = value;
            }
        }
        other => panic!("{parent_pointer} of the record is {other}, not an object or array"),
    }
}

/// Opens a scenario text and returns the handle, the reply and the saved text before any fix.
pub fn opened(session: &mut Session, text: &str) -> (u64, Sent, String) {
    let sent = open_text(session, text);
    let doc = doc_handle(&sent);
    let before = saved_text(session, doc);
    (doc, sent, before)
}

/// Applies the model edits that make `changes` ([`edits`]) as one `apply_edits`, labeled `label` when one is given,
/// and checks that it succeeds; returns its reply.
pub fn apply_changes(
    session: &mut Session,
    doc: u64,
    label: Option<&str>,
    changes: &[Change],
) -> Sent {
    let made = edits(session, doc, changes);
    let applied = apply(session, doc, label, Value::Array(made));
    applied.ok();
    applied
}

/// Opens the scenario's setup in `session` (revision 0) and applies its edit as one `apply_edits` labeled
/// [`EDIT_LABEL`] (revision 1); returns the handle and the edit's reply.
pub fn after_edit(session: &mut Session, scenario: &Scenario) -> (u64, Sent) {
    let doc = doc_handle(&open_text(session, &scenario.setup));
    let edited = apply_changes(session, doc, Some(EDIT_LABEL), &scenario.changes);
    (doc, edited)
}

/// Begins a transaction labeled [`EDIT_LABEL`] on `doc`, focused on `focus` when one is given.
pub fn begin_edit(session: &mut Session, doc: u64, focus: Option<&str>) {
    begin(session, doc, EDIT_LABEL).ok();
    if let Some(feature) = focus {
        common::focus(session, doc, feature).ok();
    }
}

/// Opens `scenario`'s setup and applies its edit the way the inspector edits a feature: inside a transaction focused on
/// `focus`, which rolls back every later feature until the commit; the commit's reply.
pub fn edited_in_a_transaction_focused_on(
    session: &mut Session,
    scenario: &Scenario,
    focus: &str,
) -> Sent {
    let (doc, _, _) = opened(session, &scenario.setup);
    begin_edit(session, doc, Some(focus));
    apply_changes(session, doc, None, &scenario.changes);
    let committed = commit(session, doc);
    committed.ok();
    committed
}

/// The scenario's text after its edit, saved from a headless session.
pub fn edited_text(scenario: &Scenario) -> String {
    let mut session = headless_session();
    let (doc, _) = after_edit(&mut session, scenario);
    saved_text(&mut session, doc)
}
