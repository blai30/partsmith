//! Acceptance tests 26 to 40 of `docs/spec/engine.md` section 6: transactions, coalescing, focus with live preview, and rollback.

mod common;

use common::{
    GEOMETRY_TOLERANCE, Sent, apply, assert_bounding_box, assert_relative, elements, get_document,
    measure_body, number, open_doc, revision, saved_text, send, session, set_height, set_rollback,
};
use partsmith_engine::Session;
use pretty_assertions::assert_eq;
use serde_json::json;

/// Opens a transaction with this label.
fn begin(session: &mut Session, doc: u64, label: &str) -> Sent {
    send(
        session,
        json!({"type": "begin_transaction", "doc": doc, "label": label}),
    )
}

/// Sets the focus feature of the open transaction.
fn focus(session: &mut Session, doc: u64, feature: &str) -> Sent {
    send(
        session,
        json!({"type": "set_focus", "doc": doc, "feature": feature}),
    )
}

/// Opens `fillet.ptsm`, begins `Edit fillet` and focuses `f-fil1`; returns the handle and the `set_focus` reply.
fn focused_fillet(session: &mut Session) -> (u64, Sent) {
    let doc = open_doc(session, "fillet.ptsm");
    begin(session, doc, "Edit fillet").ok();
    let focused = focus(session, doc, "f-fil1");
    focused.ok();
    (doc, focused)
}

/// The `face_count` of a `body_mesh` event's mesh.
fn face_count(event: &serde_json::Value) -> u64 {
    event["mesh"]["face_count"]
        .as_u64()
        .unwrap_or_else(|| panic!("the body_mesh event has a mesh: {event}"))
}

#[test]
fn t026_a_transaction_undoes_as_one_entry_with_its_label() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    begin(&mut session, doc, "Resize").ok();
    for height in ["41 mm", "42 mm", "43 mm"] {
        apply(&mut session, doc, None, json!([set_height(height)])).ok();
    }

    let committed = send(&mut session, json!({"type": "commit_transaction", "doc": doc}));
    assert_eq!(committed.ok()["recorded"], true);

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undone.ok()["label"], "Resize");
    assert_eq!(
        get_document(&mut session, doc)["document"]["parameters"][0]["expr"],
        "40 mm"
    );
}

#[test]
fn t027_edits_inside_a_transaction_take_no_label() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    begin(&mut session, doc, "Resize").ok();

    let sent = apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("41 mm")]),
    );

    assert_eq!(sent.error_code(), "label_not_allowed");
}

#[test]
fn t028_only_one_transaction_can_be_open() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    begin(&mut session, doc, "Resize").ok();

    let second = begin(&mut session, doc, "Move");

    assert_eq!(second.error_code(), "transaction_open");
    assert!(
        second.error_message().contains("Resize"),
        "the message names the open transaction: {}",
        second.error_message()
    );
}

#[test]
fn t029_commit_cancel_and_focus_need_a_transaction() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let commit = send(&mut session, json!({"type": "commit_transaction", "doc": doc}));
    let cancel = send(&mut session, json!({"type": "cancel_transaction", "doc": doc}));
    let focused = focus(&mut session, doc, "f-box1");

    assert_eq!(commit.error_code(), "no_transaction");
    assert_eq!(cancel.error_code(), "no_transaction");
    assert_eq!(focused.error_code(), "no_transaction");
}

#[test]
fn t030_cancel_restores_the_recipe_and_counts_as_a_revision() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let before = saved_text(&mut session, doc);
    begin(&mut session, doc, "Resize").ok();
    apply(&mut session, doc, None, json!([set_height("41 mm")])).ok();
    apply(&mut session, doc, None, json!([set_height("42 mm")])).ok();

    let canceled = send(&mut session, json!({"type": "cancel_transaction", "doc": doc}));

    assert_eq!(canceled.ok()["revision"], 3);
    assert_bounding_box(
        &canceled.body_mesh("b-1", "model")["mesh"]["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 40.0],
    );
    assert_eq!(saved_text(&mut session, doc), before);
}

#[test]
fn t031_undo_redo_and_save_wait_for_the_transaction() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    begin(&mut session, doc, "Resize").ok();

    let undo = send(&mut session, json!({"type": "undo", "doc": doc}));
    let redo = send(&mut session, json!({"type": "redo", "doc": doc}));
    let save = send(&mut session, json!({"type": "save_document", "doc": doc}));

    assert_eq!(undo.error_code(), "transaction_open");
    assert_eq!(redo.error_code(), "transaction_open");
    assert_eq!(save.error_code(), "transaction_open");
}

#[test]
fn t032_committing_an_empty_transaction_records_nothing() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    begin(&mut session, doc, "Nothing").ok();

    let committed = send(&mut session, json!({"type": "commit_transaction", "doc": doc}));

    assert_eq!(committed.ok()["recorded"], false);
    assert_eq!(committed.ok()["revision"], 0);
    assert_eq!(revision(&mut session, doc), 0);
}

#[test]
fn t033_coalesced_slider_edits_undo_to_the_original_text() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let before = saved_text(&mut session, doc);
    begin(&mut session, doc, "Slide height").ok();
    for height in 41..=140 {
        apply(
            &mut session,
            doc,
            None,
            json!([set_height(&format!("{height} mm"))]),
        )
        .ok();
    }
    send(&mut session, json!({"type": "commit_transaction", "doc": doc})).ok();

    send(&mut session, json!({"type": "undo", "doc": doc})).ok();

    assert_eq!(saved_text(&mut session, doc), before);
}

#[test]
fn t034_focus_shows_the_state_before_the_feature_and_previews_its_output() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");
    begin(&mut session, doc, "Edit fillet").ok();

    let focused = focus(&mut session, doc, "f-fil1");

    assert_eq!(
        focused.event_types(),
        ["document_changed", "recomputed", "body_mesh", "body_mesh"]
    );
    assert_eq!(focused.event("document_changed")["focus"], "f-fil1");
    assert_eq!(focused.feature_status("f-fil1")["status"], "ok");
    assert_eq!(focused.events[2]["layer"], "model");
    assert_eq!(focused.events[3]["layer"], "preview");
    assert_eq!(face_count(focused.body_mesh("b-1", "model")), 6);
    assert_eq!(face_count(focused.body_mesh("b-1", "preview")), 7);
}

#[test]
fn t035_queries_use_the_display_state_during_a_focus() {
    let mut session = session();
    let (doc, _) = focused_fillet(&mut session);

    let faces = elements(&mut session, doc, "b-1", Some("face"));
    let measured = measure_body(&mut session, doc, "b-1");

    assert_eq!(faces.len(), 6);
    assert_relative(
        number(&measured["volume"]),
        24000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
}

#[test]
fn t036_commit_after_a_focus_shows_the_result_and_removes_the_preview() {
    let mut session = session();
    let (doc, _) = focused_fillet(&mut session);

    let committed = send(&mut session, json!({"type": "commit_transaction", "doc": doc}));

    committed.ok();
    assert_eq!(face_count(committed.body_mesh("b-1", "model")), 7);
    let preview = committed.body_mesh("b-1", "preview");
    assert!(preview.get("mesh").is_none(), "the preview is removed: {preview}");
}

#[test]
fn t037_a_rolled_back_feature_cannot_be_focused() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");
    apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-fil1"))]),
    )
    .ok();
    begin(&mut session, doc, "Edit fillet").ok();

    let focused = focus(&mut session, doc, "f-fil1");

    assert_eq!(focused.error_code(), "focus_inactive");
}

#[test]
fn t038_preview_meshes_are_not_pickable() {
    let mut session = session();
    let (doc, focused) = focused_fillet(&mut session);
    let preview_id = focused.body_mesh("b-1", "preview")["mesh_id"].clone();

    let picked = send(
        &mut session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": preview_id, "kind": "face", "index": 0}}),
    );

    assert_eq!(picked.error_code(), "preview_not_pickable");
}

#[test]
fn t039_rollback_is_an_undoable_edit() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");

    let rolled = apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-fil1"))]),
    );

    assert_eq!(rolled.feature_status("f-fil1")["status"], "rolled_back");
    assert_eq!(face_count(rolled.body_mesh("b-1", "model")), 6);

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(face_count(undone.body_mesh("b-1", "model")), 7);
}

#[test]
fn t040_rollback_to_an_unknown_feature_is_rejected() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");

    let sent = apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-nope"))]),
    );

    assert_eq!(sent.error_code(), "edit_rejected");
}
