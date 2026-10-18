//! Acceptance test 145 of `docs/spec/engine.md` section 6, document units with Keep sizes (section 3.25), on the
//! document of newcomer error corpus NC09, plus small tests of the same section: the label of each unit, the typed
//! method, the request without `keep_sizes`, `bare_numbers` after edits, parameters used as lengths, and the undo,
//! redo, cancel and dirty state of a units change. A test named `tNNN` is section 6 test NNN.
//!
//! Tests 141 and 142 (ghosts, section 3.23) and 143 and 144 (`list_uses`, section 3.24) come with phase 4 of the
//! reference stability plan.

mod common;
mod corpus;

use common::{
    GEOMETRY_TOLERANCE, Sent, apply, assert_bounding_box, assert_relative, doc_handle, fixture,
    get_document, measure_body, new_doc, open, open_text, revision, saved_text, send, session,
    set_document_units,
};
use corpus::{box_record, document_with, nc09, opened, parameter, record, volume};
use partsmith_model::Units;
use partsmith_protocol::DocHandle;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The document units in `document_changed` of `sent`.
fn units_of(sent: &Sent) -> Value {
    sent.event("document_changed")["document"]["units"].clone()
}

/// A millimeter document with the parameter `w` = `30`, used as the X size of a box sized `w`, `20 mm` and `10 mm`.
fn box_sized_by_a_bare_parameter() -> String {
    document_with(
        "mm",
        &[parameter("p-w", "w", "30")],
        &["b-1"],
        &[box_record(
            "f-box1",
            "b-1",
            ["w", "20 mm", "10 mm"],
            None,
            "new",
        )],
    )
}

// Test 145: document units (NC09).

#[test]
fn t145_opening_counts_the_four_numbers_without_a_unit() {
    let mut session = session();

    let (_, sent, _) = opened(&mut session, &nc09());

    assert_eq!(sent.event("document_changed")["bare_numbers"], 4);
}

#[test]
fn t145_keep_sizes_is_one_change_that_keeps_four_numbers() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(
        switched.ok(),
        &json!({"kind": "ok", "type": "set_document_units", "revision": 1, "kept": 4})
    );
    assert_eq!(
        switched.event_types()[..2],
        ["document_changed", "recomputed"],
        "{:?}",
        switched.event_types()
    );
    let changed = switched.event("document_changed");
    assert_eq!(changed["revision"], 1);
    assert_eq!(changed["document"]["units"], "in");
    assert_eq!(changed["bare_numbers"], 0);
    assert_eq!(changed["undo"], "Change units to in");
}

#[test]
fn t145_keep_sizes_writes_the_old_unit_after_each_number() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    set_document_units(&mut session, doc, "in", true).ok();

    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["40 mm", "20 mm", "10 mm"])
    );
    assert_eq!(record(&mut session, doc, "f-fil1")["radius"], "2 mm");
}

#[test]
fn t145_keep_sizes_keeps_the_volume() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());
    let before = volume(&mut session, doc, "b-1");

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.feature_status("f-fil1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        before,
        GEOMETRY_TOLERANCE,
        "volume",
    );
}

#[test]
fn t145_undo_restores_the_text_exactly() {
    let mut session = session();
    let (doc, _, before) = opened(&mut session, &nc09());
    set_document_units(&mut session, doc, "in", true).ok();

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(undone.ok()["label"], "Change units to in");
    assert_eq!(undone.event("document_changed")["bare_numbers"], 4);
    assert_eq!(saved_text(&mut session, doc), before);
}

#[test]
fn t145_without_keep_sizes_nothing_is_kept_and_the_numbers_mean_inches() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", false);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 0);
    let changed = switched.event("document_changed");
    assert_eq!(changed["document"]["units"], "in");
    assert_eq!(changed["bare_numbers"], 4);
    assert_eq!(changed["undo"], "Change units to in");
    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["40", "20", "10"])
    );
    assert_bounding_box(
        &measure_body(&mut session, doc, "b-1")["bounding_box"],
        [-508.0, -254.0, 0.0],
        [508.0, 254.0, 254.0],
    );
}

#[test]
fn t145_the_current_units_change_nothing() {
    for keep_sizes in [true, false] {
        let mut session = session();
        let (doc, _, before) = opened(&mut session, &nc09());

        let same = set_document_units(&mut session, doc, "mm", keep_sizes);

        assert_eq!(
            same.ok(),
            &json!({"kind": "ok", "type": "set_document_units", "revision": 0, "kept": 0}),
            "keep_sizes {keep_sizes}"
        );
        assert!(same.events.is_empty(), "no event: {:?}", same.event_types());
        assert_eq!(revision(&mut session, doc), 0);
        assert_eq!(saved_text(&mut session, doc), before);
        let nothing = send(&mut session, json!({"type": "undo", "doc": doc}));
        assert_eq!(nothing.error_code(), "nothing_to_undo", "no undo entry");
    }
}

#[test]
fn t145_inside_a_transaction_the_edits_join_it() {
    let mut session = session();
    let (doc, _, before) = opened(&mut session, &nc09());
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Units"}),
    )
    .ok();

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 4);
    let changed = switched.event("document_changed");
    assert!(
        changed.get("undo").is_none(),
        "no undo entry of its own: {changed}"
    );
    assert_eq!(changed["transaction"], "Units");
    let committed = send(
        &mut session,
        json!({"type": "commit_transaction", "doc": doc}),
    );
    assert_eq!(committed.ok()["recorded"], true);
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undone.ok()["label"], "Units");
    assert_eq!(saved_text(&mut session, doc), before);
}

// Section 3.25: the change of units as a command.

#[test]
fn a_request_without_keep_sizes_does_not_keep_sizes() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = send(
        &mut session,
        json!({"type": "set_document_units", "doc": doc, "units": "in"}),
    );

    assert_eq!(switched.ok()["kept"], 0);
    assert_eq!(switched.event("document_changed")["bare_numbers"], 4);
}

#[test]
fn the_label_names_the_new_units_by_their_symbol() {
    let mut session = session();
    let doc = new_doc(&mut session);

    for (units, label) in [
        ("cm", "Change units to cm"),
        ("m", "Change units to m"),
        ("in", "Change units to in"),
        ("mm", "Change units to mm"),
    ] {
        let switched = set_document_units(&mut session, doc, units, true);

        assert_eq!(switched.ok()["kept"], 0, "{units}");
        assert_eq!(units_of(&switched), units);
        assert_eq!(switched.event("document_changed")["undo"], label);
    }
    assert_eq!(revision(&mut session, doc), 4);
}

#[test]
fn keep_sizes_from_inches_writes_inches() {
    let text = document_with(
        "in",
        &[],
        &["b-1"],
        &[box_record("f-box1", "b-1", ["2", "1", "1"], None, "new")],
    );
    let mut session = session();
    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);
    assert_eq!(opened.event("document_changed")["bare_numbers"], 3);
    let before = volume(&mut session, doc, "b-1");

    let switched = set_document_units(&mut session, doc, "mm", true);

    assert_eq!(switched.ok()["kept"], 3);
    assert_eq!(
        switched.event("document_changed")["undo"],
        "Change units to mm"
    );
    assert_eq!(
        record(&mut session, doc, "f-box1")["size"],
        json!(["2 in", "1 in", "1 in"])
    );
    assert_relative(
        volume(&mut session, doc, "b-1"),
        before,
        GEOMETRY_TOLERANCE,
        "volume",
    );
}

#[test]
fn keep_sizes_without_bare_numbers_only_changes_the_units() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let features_before = get_document(&mut session, doc)["document"]["features"].clone();

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["revision"], 1);
    assert_eq!(switched.ok()["kept"], 0);
    assert_eq!(units_of(&switched), "in");
    assert_eq!(
        get_document(&mut session, doc)["document"]["features"],
        features_before
    );
}

#[test]
fn a_parameter_used_as_a_length_is_counted_and_kept() {
    let mut session = session();
    let opened = open_text(&mut session, &box_sized_by_a_bare_parameter());
    let doc = doc_handle(&opened);
    assert_eq!(opened.event("document_changed")["bare_numbers"], 1);

    let switched = set_document_units(&mut session, doc, "in", true);

    assert_eq!(switched.ok()["kept"], 1);
    assert_eq!(
        switched.event("document_changed")["document"]["parameters"],
        json!([{"id": "p-w", "name": "w", "expr": "30 mm"}])
    );
    assert_eq!(switched.parameter_value("p-w")["value"], 30.0);
}

#[test]
fn redo_changes_the_units_again() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());
    set_document_units(&mut session, doc, "in", true).ok();
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();

    let redone = send(&mut session, json!({"type": "redo", "doc": doc}));

    assert_eq!(redone.ok()["label"], "Change units to in");
    assert_eq!(units_of(&redone), "in");
    assert_eq!(redone.event("document_changed")["bare_numbers"], 0);
}

#[test]
fn a_units_change_makes_the_document_dirty_and_undo_makes_it_clean() {
    let mut session = session();
    let (doc, _, _) = opened(&mut session, &nc09());

    let switched = set_document_units(&mut session, doc, "in", true);
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(switched.event("document_changed")["dirty"], true);
    assert_eq!(undone.event("document_changed")["dirty"], false);
}

#[test]
fn cancelling_the_transaction_reverts_the_units() {
    let mut session = session();
    let (doc, _, before) = opened(&mut session, &nc09());
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Units"}),
    )
    .ok();
    set_document_units(&mut session, doc, "in", true).ok();

    let cancelled = send(
        &mut session,
        json!({"type": "cancel_transaction", "doc": doc}),
    );

    assert_eq!(units_of(&cancelled), "mm");
    assert_eq!(cancelled.event("document_changed")["bare_numbers"], 4);
    assert_eq!(saved_text(&mut session, doc), before);
}

#[test]
fn an_unknown_document_fails_before_anything_changes() {
    let mut session = session();

    let sent = set_document_units(&mut session, 9, "in", true);

    assert_eq!(sent.error_code(), "unknown_document");
    assert!(sent.events.is_empty(), "{:?}", sent.event_types());
}

#[test]
fn the_typed_method_returns_the_revision_and_the_kept_count() {
    let mut session = session();
    let opened = session
        .open_document(&nc09(), Vec::new())
        .expect("the NC09 document opens");

    let changed = session
        .set_document_units(opened.doc, Units::In, true)
        .expect("the units change");
    let unchanged = session
        .set_document_units(opened.doc, Units::In, true)
        .expect("the same units change nothing");

    assert_eq!((changed.revision, changed.kept), (1, 4));
    assert_eq!((unchanged.revision, unchanged.kept), (1, 0));
    let document = session.document(opened.doc).expect("the document is open");
    assert_eq!(document.units, Units::In);
    assert_eq!(document.bare_numbers(session.registry()), 0);
}

#[test]
fn the_typed_method_fails_for_a_document_that_is_not_open() {
    let mut session = session();

    let failed = session.set_document_units(DocHandle(3), Units::In, false);

    assert!(
        matches!(
            failed,
            Err(partsmith_engine::EngineError::UnknownDocument { .. })
        ),
        "{failed:?}"
    );
}

// Section 3.25: `bare_numbers` in `document_changed`.

#[test]
fn documents_without_bare_numbers_report_none() {
    let mut session = session();

    let created = send(&mut session, json!({"type": "new_document"}));
    let opened = open(&mut session, "box.ptsm");

    assert_eq!(created.event("document_changed")["bare_numbers"], 0);
    assert_eq!(opened.event("document_changed")["bare_numbers"], 0);
}

#[test]
fn bare_numbers_follow_each_edit() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let mut sized = get_document(&mut session, doc)["document"]["features"][0].clone();
    sized["size"] = json!(["20", "30 mm", "height"]);

    let edited = apply(
        &mut session,
        doc,
        Some("Size"),
        json!([{"type": "update_feature", "feature": sized}]),
    );
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(edited.event("document_changed")["bare_numbers"], 1);
    assert_eq!(undone.event("document_changed")["bare_numbers"], 0);
    assert_eq!(saved_text(&mut session, doc), fixture("box.ptsm"));
}
