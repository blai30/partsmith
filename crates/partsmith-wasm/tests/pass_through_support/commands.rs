//! The commands the pass-through tests send, built for each side's document, and the request sequences whose answers they read.

use partsmith_protocol::ResponseFrame;
use serde_json::{Value, json};

use crate::common::ok_result;

use super::documents::{Scenario, feature_record, r26, remove_feature, sketch_on_moved_top};
use super::frames::action_labeled;
use super::side_by_side::{Side, SideBySide};

/// The `apply_fix` command for the `fix` reference of an `apply` action, for the side's document.
pub fn apply_fix(side: &Side, fix: &Value) -> Value {
    json!({"type": "apply_fix", "doc": side.doc, "fix": fix})
}

/// The `apply_fix` command for the `apply` action labeled `label` among `message`'s actions, for the side's document.
pub fn apply_action(side: &Side, message: &Value, label: &str) -> Value {
    apply_fix(side, &action_labeled(message, label)["run"]["fix"])
}

/// The `save_document` command for the side's document.
pub fn save_document(side: &Side) -> Value {
    json!({"type": "save_document", "doc": side.doc})
}

/// The `get_document` command for the side's document.
fn get_document(side: &Side) -> Value {
    json!({"type": "get_document", "doc": side.doc})
}

/// The `begin_transaction` command labeled `label` for the side's document.
pub fn begin_transaction(side: &Side, label: &str) -> Value {
    json!({"type": "begin_transaction", "doc": side.doc, "label": label})
}

/// The `set_document_units` command for the side's document.
pub fn set_document_units(side: &Side, units: &str, keep_sizes: bool) -> Value {
    json!({"type": "set_document_units", "doc": side.doc, "units": units, "keep_sizes": keep_sizes})
}

/// The `undo` command for the side's document.
pub fn undo(side: &Side) -> Value {
    json!({"type": "undo", "doc": side.doc})
}

/// The `solve_sketch` command for the sketch `feature` of the side's document.
pub fn solve_sketch(side: &Side, feature: &str) -> Value {
    json!({"type": "solve_sketch", "doc": side.doc, "feature": feature})
}

/// The `list_elements` command for the faces of `b-1` of the side's document.
pub fn list_faces(side: &Side) -> Value {
    json!({"type": "list_elements", "doc": side.doc, "body": "b-1", "kind": "face"})
}

/// The `list_elements` command for the edges of `b-1` of the side's document.
pub fn list_edges(side: &Side) -> Value {
    json!({"type": "list_elements", "doc": side.doc, "body": "b-1", "kind": "edge"})
}

/// The `pick_reference` command for the element of `kind` at `index` in the model mesh of `b-1` the side displays now.
pub fn pick(side: &Side, kind: &str, index: usize) -> Value {
    json!({
        "type": "pick_reference",
        "doc": side.doc,
        "pick": {"mesh_id": side.model_mesh("b-1"), "kind": kind, "index": index}
    })
}

/// The label of the `apply_edits` that makes a corpus edit, as engine.md test 136 applies it.
pub const EDIT_LABEL: &str = "Edit";

/// The `apply_edits` command labeled `label` with the model edits `edits` for the side's document.
pub fn apply_edits(side: &Side, label: &str, edits: &Value) -> Value {
    json!({"type": "apply_edits", "doc": side.doc, "label": label, "edits": edits})
}

// Requests sent side by side whose answers the tests read.

/// The text `save_document` gives for the bridge's document, read as request `id`.
pub fn saved_text(sides: &mut SideBySide, id: u64) -> Value {
    ok_result(&sides.send(id, save_document), "save_document")["text"].clone()
}

/// The record of `feature` that the bridge's document holds, read with `get_document` as request `id`.
pub fn record(sides: &mut SideBySide, id: u64, feature: &str) -> Value {
    let got = sides.send(id, get_document);
    feature_record(&ok_result(&got, "get_document")["document"], feature)
}

/// The plane that `f-sk1` of [`sketch_on_moved_top`] in `frame` stores after a drag of `e-1.end` to (20, 5) and its commit, sent side by side, once the commit made revision 1.
pub fn plane_after_a_drag_commit(frame: Option<&str>) -> Value {
    let mut sides = SideBySide::new();
    sides.open(1, &sketch_on_moved_top(frame));
    sides.send(2, |side| {
        json!({"type": "drag_sketch", "doc": side.doc, "feature": "f-sk1", "point": "e-1.end", "to": [20, 5]})
    });
    let committed = sides.send(3, |side| {
        json!({"type": "end_sketch_drag", "doc": side.doc, "feature": "f-sk1", "commit": true})
    });
    assert_eq!(ok_result(&committed, "end_sketch_drag")["revision"], 1);
    record(&mut sides, 4, "f-sk1")["plane"].clone()
}

/// Opens the setup of `scenario` on both sides as request 1 and applies its edits as request 2; returns the bridge's frame of the edit.
pub fn after_edit(sides: &mut SideBySide, scenario: &Scenario) -> ResponseFrame {
    sides.open(1, &scenario.setup);
    let edited = sides.send(2, |side| apply_edits(side, EDIT_LABEL, &scenario.edits));
    assert_eq!(ok_result(&edited, "apply_edits")["revision"], 1);
    edited
}

/// Corpus R26 after edit (1) (request 2, labeled `Edit`) and the delete of 'Extrude 1' (request 3, labeled `Delete`), the first two of the three `apply_edits` engine.md test 136 applies; returns the bridge's frame of the delete, which makes the boss's extrude 'Extrude 1'.
pub fn r26_after_the_delete(sides: &mut SideBySide) -> ResponseFrame {
    after_edit(sides, &r26());
    let deleted = sides.send(3, |side| {
        apply_edits(side, "Delete", &json!([remove_feature("f-ext1")]))
    });
    assert_eq!(ok_result(&deleted, "apply_edits")["revision"], 2);
    deleted
}
