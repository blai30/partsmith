//! Acceptance tests 14 to 25, 106 and 107 of `docs/spec/engine.md` section 6: edit batches, undo and redo, saving, parameter rename and suggested fixes.

use crate::common;

use common::{
    GEOMETRY_TOLERANCE, apply, assert_relative, display_options, doc_handle, fixture, get_document,
    number, open_doc, open_text, replace_once, revision, send, session, set_height,
};
use partsmith_engine::{Session, SessionOptions};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The value of `p-h` in a reply's `recomputed` event.
fn height_value(sent: &common::Sent) -> f64 {
    number(&sent.parameter_value("p-h")["value"])
}

#[test]
fn t014_edits_outside_a_transaction_need_a_label() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = apply(&mut session, doc, None, json!([set_height("50 mm")]));

    assert_eq!(sent.error_code(), "label_required");
    assert!(
        sent.events.is_empty(),
        "no events: {:?}",
        sent.event_types()
    );
    assert_eq!(revision(&mut session, doc), 0);
}

#[test]
fn t015_labeled_edit_bumps_the_revision_and_updates_the_mesh() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("50 mm")]),
    );

    assert_eq!(sent.ok()["revision"], 1);
    let changed = sent.event("document_changed");
    assert_eq!(changed["revision"], 1);
    assert_eq!(changed["dirty"], true);
    assert_eq!(changed["undo"], "Change height");
    assert_relative(height_value(&sent), 50.0, GEOMETRY_TOLERANCE, "height");
    let mesh = sent.body_mesh("b-1", "model");
    let (_, max) = common::bounding_box(&mesh["mesh"]["bounding_box"]);
    assert_relative(max[2], 50.0, GEOMETRY_TOLERANCE, "bounding box max z");
}

#[test]
fn t016_a_rejected_edit_rolls_back_the_whole_batch() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let original = get_document(&mut session, doc)["document"].clone();

    let sent = apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("50 mm"), {"type": "remove_feature", "id": "f-nope"}]),
    );

    assert_eq!(sent.error_code(), "edit_rejected");
    assert_eq!(sent.error()["edit_index"], 1);
    let after = get_document(&mut session, doc);
    assert_eq!(after["revision"], 0);
    assert_eq!(after["document"], original);
}

#[test]
fn t017_invalid_params_name_the_field_and_keep_the_feature_type_in_the_detail() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = apply(
        &mut session,
        doc,
        Some("Add box"),
        json!([
            set_height("45 mm"),
            {"type": "add_feature", "index": 1, "feature": {"id": "f-box2", "type": "box", "size": ["1 mm", "2 mm", "3 mm"], "op": "new"}}
        ]),
    );

    assert_eq!(sent.error_code(), "invalid_params");
    assert_eq!(sent.error()["edit_index"], 1);
    assert_eq!(
        sent.error_message(),
        "Body of 'Box 2' cannot be read, so this change was not applied. Set Body again."
    );
    let detail = sent.error()["detail"]
        .as_str()
        .expect("the error has a detail");
    assert!(
        detail.starts_with("box f-box2"),
        "the detail starts with the type and id: {detail}"
    );
}

#[test]
fn t018_added_params_are_saved_in_canonical_order() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let record = json!({
        "id": "f-box2",
        "type": "box",
        "op": "join",
        "position": ["0 mm", "0 mm", "40 mm"],
        "size": ["10 mm", "10 mm", "10 mm"],
        "body": "b-1"
    });
    apply(
        &mut session,
        doc,
        Some("Add box"),
        json!([{"type": "add_feature", "index": 1, "feature": record}]),
    )
    .ok();

    let text = common::saved_text(&mut session, doc);

    let saved: Value = serde_json::from_str(&text).expect("saved text is JSON");
    let keys: Vec<&str> = saved["features"][1]
        .as_object()
        .expect("the record is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["id", "type", "body", "size", "position", "op"]);
}

#[test]
fn t019_undo_and_redo_step_the_revision_and_the_values() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("50 mm")]),
    )
    .ok();

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(undone.ok()["revision"], 2);
    assert_eq!(undone.ok()["label"], "Change height");
    assert_relative(
        height_value(&undone),
        40.0,
        GEOMETRY_TOLERANCE,
        "height after undo",
    );

    let redone = send(&mut session, json!({"type": "redo", "doc": doc}));

    assert_eq!(redone.ok()["revision"], 3);
    assert_relative(
        height_value(&redone),
        50.0,
        GEOMETRY_TOLERANCE,
        "height after redo",
    );
}

#[test]
fn t020_empty_stacks_have_nothing_to_undo_or_redo() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let undo = send(&mut session, json!({"type": "undo", "doc": doc}));
    let redo = send(&mut session, json!({"type": "redo", "doc": doc}));

    assert_eq!(undo.error_code(), "nothing_to_undo");
    assert_eq!(redo.error_code(), "nothing_to_redo");
}

#[test]
fn t021_a_new_edit_clears_the_redo_stack() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    apply(
        &mut session,
        doc,
        Some("First"),
        json!([set_height("50 mm")]),
    )
    .ok();
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();
    apply(
        &mut session,
        doc,
        Some("Second"),
        json!([set_height("60 mm")]),
    )
    .ok();

    let redo = send(&mut session, json!({"type": "redo", "doc": doc}));

    assert_eq!(redo.error_code(), "nothing_to_redo");
}

#[test]
fn t022_undoing_back_to_the_opened_recipe_is_clean() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("50 mm")]),
    )
    .ok();

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(undone.event("document_changed")["dirty"], false);
}

#[test]
fn t023_save_does_not_clean_and_mark_saved_moves_the_baseline() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("50 mm")]),
    )
    .ok();

    let saved = send(&mut session, json!({"type": "save_document", "doc": doc}));
    assert_eq!(saved.ok()["revision"], 1);
    assert!(
        saved.events.is_empty(),
        "save emits nothing: {:?}",
        saved.event_types()
    );

    let marked = send(
        &mut session,
        json!({"type": "mark_saved", "doc": doc, "revision": 1}),
    );
    marked.ok();
    // The baseline moved from dirty to clean, so exactly one document_changed reports it.
    assert_eq!(marked.event_types(), ["document_changed"]);
    assert_eq!(marked.events[0]["dirty"], false);

    let again = send(
        &mut session,
        json!({"type": "mark_saved", "doc": doc, "revision": 1}),
    );
    again.ok();
    assert!(
        again.events.is_empty(),
        "nothing changed: {:?}",
        again.event_types()
    );

    apply(
        &mut session,
        doc,
        Some("Change height again"),
        json!([set_height("60 mm")]),
    )
    .ok();
    let stale = send(
        &mut session,
        json!({"type": "mark_saved", "doc": doc, "revision": 0}),
    );
    stale.ok();
    assert!(
        stale.events.is_empty(),
        "nothing changed: {:?}",
        stale.event_types()
    );
    let opened = send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Probe"}),
    );
    assert_eq!(opened.event("document_changed")["dirty"], true);
}

#[test]
fn t024_the_undo_limit_drops_the_oldest_entries() {
    let mut session = Session::new(SessionOptions {
        undo_limit: 3,
        ..display_options()
    });
    let doc = open_doc(&mut session, "box.ptsm");
    for height in 41..=45 {
        apply(
            &mut session,
            doc,
            Some(&format!("Height {height}")),
            json!([set_height(&format!("{height} mm"))]),
        )
        .ok();
    }

    for _ in 0..3 {
        send(&mut session, json!({"type": "undo", "doc": doc})).ok();
    }
    let fourth = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(fourth.error_code(), "nothing_to_undo");
}

#[test]
fn t025_an_empty_labeled_batch_changes_nothing() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = apply(&mut session, doc, Some("Nothing"), json!([]));

    assert_eq!(sent.ok()["revision"], 0);
    assert_eq!(revision(&mut session, doc), 0);
    let undo = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undo.error_code(), "nothing_to_undo");
}

#[test]
fn t106_rename_parameter_rewrites_expressions_and_undoes_as_one_entry() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let original = get_document(&mut session, doc)["document"].clone();

    let renamed = send(
        &mut session,
        json!({"type": "rename_parameter", "doc": doc, "parameter": "p-h", "name": "h"}),
    );

    assert_eq!(renamed.ok()["revision"], 1);
    assert_eq!(renamed.feature_status("f-box1")["status"], "ok");
    let document = get_document(&mut session, doc)["document"].clone();
    assert_eq!(document["parameters"][0]["name"], "h");
    assert_eq!(
        document["features"][0]["size"],
        json!(["20 mm", "30 mm", "h"])
    );

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undone.ok()["label"], "Rename parameter height to h");
    assert_eq!(get_document(&mut session, doc)["document"], original);

    let reserved = send(
        &mut session,
        json!({"type": "rename_parameter", "doc": doc, "parameter": "p-h", "name": "pi"}),
    );
    assert_eq!(reserved.error_code(), "edit_rejected");
}

#[test]
fn t107_a_too_large_fillet_suggests_a_radius_that_works() {
    let mut session = session();
    let text = replace_once(
        &fixture("fillet.ptsm"),
        r#""radius": "2 mm""#,
        r#""radius": "30 mm""#,
    );
    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);

    let status = opened.feature_status("f-fil1");
    assert_eq!(status["status"], "error");
    let message = &status["messages"][0];
    assert_eq!(message["code"], "fillet_too_large");
    assert_eq!(message["field"], "/radius");
    let suggestion = message["suggestion"]
        .as_str()
        .unwrap_or_else(|| panic!("the message suggests a value: {message}"))
        .to_owned();

    let mut record = get_document(&mut session, doc)["document"]["features"][1].clone();
    record["radius"] = json!(suggestion);
    let fixed = apply(
        &mut session,
        doc,
        Some("Fix radius"),
        json!([{"type": "update_feature", "feature": record}]),
    );

    assert_eq!(fixed.feature_status("f-fil1")["status"], "ok");
}
