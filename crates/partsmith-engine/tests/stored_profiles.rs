//! Acceptance tests 172 to 174, 176 to 191, 193 and 194 of `docs/spec/engine.md` section 6, "Format version 3 and
//! stored printer profiles" (ADR 0061, ADR 0062): the stored profile on open, its resolution before session and
//! built-in profiles, the completion of `set_printer`, `put_printer_profile_text`, `remove_printer_profile`, the reason
//! for `printer.nozzle` on resin and the `.ptsm` schema. Test 171 (a version 2 text) is in `errors_and_versions.rs`,
//! test 175 (newcomer error corpus NC13) in `newcomer_errors.rs` and test 192 (print analysis) in
//! `measure_and_print.rs`.
//!
//! The tests of protocol.md section 6 that go through `Session::call` (89 to 91, 94 and 95) are here too, named
//! `protocol_tNNN`, beside the fixture they open: partsmith-protocol cannot reach a session.

use crate::common;

use common::{
    GEOMETRY_TOLERANCE, apply, assert_relative, doc_handle, fixture, get_document, measure_body,
    number, open, open_text, replace_once, request_frame, saved_text, send, session, typed_handle,
};
use partsmith_engine::{EngineError, ValidateOptions};
use partsmith_protocol::{TessellationQuality, decode_response_frame};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

/// `E11P`: the `printer_profile` object of file-format.md's E11, members in the written form's order.
pub fn e11p() -> Value {
    json!({
        "id": "my-mk4",
        "name": "My MK4",
        "technology": "fdm",
        "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
        "nozzle_diameter": 0.4,
        "layer_height": 0.15,
        "max_overhang_angle": 0.7853981633974483,
        "max_bridge_length": 10,
        "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3}
    })
}

/// `MK4`: the input form whose written form is `E11P`.
pub fn mk4() -> Value {
    json!({
        "id": "my-mk4",
        "name": "My MK4",
        "technology": "fdm",
        "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
        "layer_height": 0.15,
        "fit": {"slip": 0.15, "loose": 0.3}
    })
}

/// The new words of the `unknown_printer_profile` warning (engine.md section 3.4).
pub fn unknown_printer_message(id: &str) -> String {
    format!(
        "Printer profile \"{id}\" is not among your printers. Printer values and checks use the default FDM profile until you add that printer or pick another one."
    )
}

/// The message of a layer height of 0.5 mm on a 0.4 mm nozzle (print.md test 13).
const TALL_LAYER_MESSAGE: &str =
    "Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.";

/// The text of the stored profile of `custom-printer.ptsm`, as the fixture writes it.
const STORED_PROFILE_LINES: &str = r#"  "printer_profile": {
    "id": "my-mk4",
    "name": "My MK4",
    "technology": "fdm",
    "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
    "nozzle_diameter": 0.4,
    "layer_height": 0.15,
    "max_overhang_angle": 0.7853981633974483,
    "max_bridge_length": 10,
    "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3}
  },
"#;

/// The same profile with its members in reversed order.
const REVERSED_PROFILE_LINES: &str = r#"  "printer_profile": {
    "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3},
    "max_bridge_length": 10,
    "max_overhang_angle": 0.7853981633974483,
    "layer_height": 0.15,
    "nozzle_diameter": 0.4,
    "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
    "technology": "fdm",
    "name": "My MK4",
    "id": "my-mk4"
  },
"#;

/// `custom-printer.ptsm` (engine.md section 3.20).
pub fn custom_printer() -> String {
    fixture("custom-printer.ptsm")
}

/// `custom-printer.ptsm` with the members of its stored profile in reversed order.
fn custom_printer_reversed() -> String {
    replace_once(
        &custom_printer(),
        STORED_PROFILE_LINES,
        REVERSED_PROFILE_LINES,
    )
}

/// `custom-printer.ptsm` with `from` replaced by `to` once.
fn custom_printer_with(from: &str, to: &str) -> String {
    replace_once(&custom_printer(), from, to)
}

/// Sends `put_printer_profile` with `profile` in the input form.
pub fn put_profile(session: &mut partsmith_engine::Session, profile: Value) -> common::Sent {
    send(
        session,
        json!({"type": "put_printer_profile", "profile": profile}),
    )
}

/// Sends `remove_printer_profile` of `id`.
fn remove_profile(session: &mut partsmith_engine::Session, id: &str) -> common::Sent {
    send(
        session,
        json!({"type": "remove_printer_profile", "id": id}),
    )
}

/// The `set_printer` edit of `printer`, with `profile` when given.
fn set_printer(printer: &str, profile: Option<Value>) -> Value {
    let mut edit = json!({"type": "set_printer", "printer": printer});
    if let Some(profile) = profile {
        edit["profile"] = profile;
    }
    edit
}

/// The document of the `get_document` result.
fn document(session: &mut partsmith_engine::Session, doc: u64) -> Value {
    get_document(session, doc)["document"].clone()
}

/// The `value` of parameter `id` from `parameter_values`, in internal units.
fn parameter(session: &partsmith_engine::Session, doc: u64, id: &str) -> f64 {
    let values = session
        .parameter_values(typed_handle(doc))
        .expect("the document is open");
    values
        .iter()
        .find(|value| value.id.as_str() == id)
        .and_then(|value| value.value)
        .unwrap_or_else(|| panic!("{id} has a value: {values:?}"))
}

/// The `clearance` parameter (`p-clear`) of `custom-printer.ptsm`.
fn clearance(session: &partsmith_engine::Session, doc: u64) -> f64 {
    parameter(session, doc, "p-clear")
}

/// The document diagnostics of the only `recomputed` event of `sent`.
fn diagnostics(sent: &common::Sent) -> Vec<Value> {
    sent.event("recomputed")["diagnostics"]
        .as_array()
        .expect("recomputed lists diagnostics")
        .clone()
}

/// The `recomputed` events of `sent` for document `doc`.
fn recomputed_for(sent: &common::Sent, doc: u64) -> Vec<&Value> {
    sent.events_of("recomputed")
        .into_iter()
        .filter(|event| event["doc"] == doc)
        .collect()
}

/// Asserts that `value` and `expected` are equal, member for member and in order.
#[track_caller]
fn assert_same_members_in_order(value: &Value, expected: &Value) {
    assert_eq!(value, expected);
    assert_eq!(
        serde_json::to_string(value).expect("JSON serializes"),
        serde_json::to_string(expected).expect("JSON serializes")
    );
}

/// The `profile` of a written form, as a map, for typed calls.
fn as_map(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("{other} is not an object"),
    }
}

// Test 172: a stored profile is used.

#[test]
fn t172_a_stored_profile_is_used() {
    let mut session = session();

    let opened = open(&mut session, "custom-printer.ptsm");

    let doc = doc_handle(&opened);
    assert_eq!(diagnostics(&opened), Vec::<Value>::new());
    assert_relative(clearance(&session, doc), 0.15, 1e-12, "clearance");
    assert_relative(
        number(&measure_body(&mut session, doc, "b-1")["volume"]),
        24360.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_eq!(
        session
            .printer_profile(typed_handle(doc))
            .expect("the document is open"),
        "my-mk4"
    );
    let tolerance = session
        .tolerance(typed_handle(doc), TessellationQuality::Standard)
        .expect("the document is open");
    assert_relative(tolerance.chord_deviation, 0.0375, 1e-12, "chord");
}

// Test 173: a stored profile wins over a session profile with its id.

#[test]
fn t173_a_stored_profile_wins_over_a_session_profile_with_its_id() {
    let mut session = session();
    let other = json!({"id": "my-mk4", "name": "Other", "technology": "fdm", "fit": {"slip": 0.3, "loose": 0.4}});
    put_profile(&mut session, other.clone()).ok();

    let opened = open(&mut session, "custom-printer.ptsm");
    let doc = doc_handle(&opened);
    assert_relative(clearance(&session, doc), 0.15, 1e-12, "clearance");
    let revision = get_document(&mut session, doc)["revision"].clone();

    let again = put_profile(&mut session, other);

    again.ok();
    assert_eq!(recomputed_for(&again, doc), Vec::<&Value>::new());
    assert!(
        again.events.iter().all(|event| event["doc"] != doc),
        "{:?}",
        again.event_types()
    );
    assert_eq!(get_document(&mut session, doc)["revision"], revision);
    assert_relative(clearance(&session, doc), 0.15, 1e-12, "clearance");
    assert_same_members_in_order(&document(&mut session, doc)["printer_profile"], &e11p());
}

// Test 174: the members of a stored profile are ordered on open.

#[test]
fn t174_the_members_of_a_stored_profile_are_ordered_on_open() {
    let mut session = session();

    let opened = open_text(&mut session, &custom_printer_reversed());

    let doc = doc_handle(&opened);
    assert_eq!(opened.event("document_changed")["dirty"], false);
    assert_eq!(saved_text(&mut session, doc), custom_printer());
}

// Test 176: adding the printer clears the warning and stores nothing (after test 175's open).

/// `box.ptsm` at format version 2 with `"printer": "my-printer"` (test 175, newcomer error corpus NC13).
pub fn box_at_version_2_on_my_printer() -> String {
    replace_once(
        &replace_once(
            &fixture("box.ptsm"),
            r#"  "format_version": 3,"#,
            r#"  "format_version": 2,"#,
        ),
        "  \"units\": \"mm\",\n",
        "  \"units\": \"mm\",\n  \"printer\": \"my-printer\",\n",
    )
}

/// The profile `my-printer` of test 176.
fn my_printer() -> Value {
    json!({"id": "my-printer", "name": "Mine", "technology": "fdm", "layer_height": 0.1})
}

#[test]
fn t176_adding_that_printer_clears_the_warning_and_stores_nothing() {
    let mut session = session();
    let opened = open_text(&mut session, &box_at_version_2_on_my_printer());
    let doc = doc_handle(&opened);
    let before = get_document(&mut session, doc);
    let dirty = opened.event("document_changed")["dirty"].clone();

    let put = put_profile(&mut session, my_printer());

    put.ok();
    let recomputed = recomputed_for(&put, doc);
    assert_eq!(recomputed.len(), 1, "{:?}", put.event_types());
    assert_eq!(recomputed[0]["diagnostics"], json!([]));
    assert_eq!(
        session
            .printer_profile(typed_handle(doc))
            .expect("the document is open"),
        "my-printer"
    );
    let after = get_document(&mut session, doc);
    assert_eq!(after["revision"], before["revision"]);
    assert_eq!(after["document"].get("printer_profile"), None);
    assert!(
        put.events_of("document_changed")
            .iter()
            .all(|event| event["doc"] != doc || event["dirty"] == dirty),
        "the dirty flag is unchanged"
    );
}

// Tests 177 and 178: picking a session profile stores it, and redo restores the recorded profile.

#[test]
fn t177_picking_a_session_profile_stores_it_and_undo_removes_it() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    put_profile(&mut session, mk4()).ok();

    apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("my-mk4", None)]),
    )
    .ok();

    assert_same_members_in_order(&document(&mut session, doc)["printer_profile"], &e11p());
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();
    let undone = document(&mut session, doc);
    assert_eq!(undone.get("printer"), None);
    assert_eq!(undone.get("printer_profile"), None);
}

#[test]
fn t178_redo_restores_the_recorded_profile() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    put_profile(&mut session, mk4()).ok();
    apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("my-mk4", None)]),
    )
    .ok();
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();
    remove_profile(&mut session, "my-mk4").ok();

    let redone = send(&mut session, json!({"type": "redo", "doc": doc}));

    redone.ok();
    assert_same_members_in_order(&document(&mut session, doc)["printer_profile"], &e11p());
    assert_eq!(diagnostics(&redone), Vec::<Value>::new());
}

// Test 179: picking again replaces the stored profile.

#[test]
fn t179_picking_again_replaces_the_stored_profile() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "custom-printer.ptsm"));
    put_profile(
        &mut session,
        json!({"id": "my-mk4", "name": "My MK4", "technology": "fdm", "fit": {"slip": 0.3, "loose": 0.4}}),
    )
    .ok();

    apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("my-mk4", None)]),
    )
    .ok();

    let stored = document(&mut session, doc)["printer_profile"].clone();
    assert_eq!(stored["fit"]["slip"], json!(0.3));
    assert_eq!(stored["bed"], json!({"kind": "rectangle", "width": 256, "depth": 256, "height": 256}));
    assert_relative(clearance(&session, doc), 0.3, 1e-12, "clearance");
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();
    assert_same_members_in_order(&document(&mut session, doc)["printer_profile"], &e11p());
    assert_relative(clearance(&session, doc), 0.15, 1e-12, "clearance");
}

// Test 180: a built-in printer stores no profile.

#[test]
fn t180_a_built_in_printer_stores_no_profile() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "custom-printer.ptsm"));

    apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("generic-fdm", None)]),
    )
    .ok();

    let changed = document(&mut session, doc);
    assert_eq!(changed["printer"], "generic-fdm");
    assert_eq!(changed.get("printer_profile"), None);
    assert_relative(clearance(&session, doc), 0.2, 1e-12, "clearance");
}

// Tests 181 to 184: the profile of a `set_printer` edit is checked.

/// `E11P` with `layer_height` 0.5.
fn e11p_with_tall_layers() -> Value {
    let mut profile = e11p();
    profile["layer_height"] = json!(0.5);
    profile
}

/// `apply_edits` on `box.ptsm` of `[set_units in, set_printer my-mk4 with profile]` (tests 181 and 182).
fn apply_units_then_profile(profile: Value) -> (common::Sent, String, String) {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let before = saved_text(&mut session, doc);
    let sent = apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([{"type": "set_units", "units": "in"}, set_printer("my-mk4", Some(profile))]),
    );
    let after = saved_text(&mut session, doc);
    (sent, before, after)
}

#[test]
fn t181_a_stored_profile_in_an_edit_is_checked() {
    let (sent, before, after) = apply_units_then_profile(e11p_with_tall_layers());

    assert_eq!(sent.error_code(), "invalid_profile");
    assert_eq!(sent.error()["edit_index"], 1);
    assert_eq!(sent.error_message(), TALL_LAYER_MESSAGE);
    assert_eq!(sent.events, Vec::<Value>::new());
    assert_eq!(after, before);
}

#[test]
fn t182_a_stored_profile_in_an_edit_lists_every_value() {
    let mut profile = e11p();
    profile
        .as_object_mut()
        .expect("E11P is an object")
        .shift_remove("bed");

    let (sent, before, after) = apply_units_then_profile(profile);

    assert_eq!(sent.error_code(), "invalid_profile");
    assert_eq!(sent.error()["edit_index"], 1);
    assert_eq!(
        sent.error_message(),
        "The printer profile has no bed. Add bed: a profile stored in a document lists every value."
    );
    assert_eq!(after, before);
}

#[test]
fn t183_a_built_in_profile_in_an_edit_is_refused() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let written: Value = serde_json::from_str(
        &partsmith_print::generic_fdm()
            .to_json()
            .expect("the built-in profile is valid"),
    )
    .expect("to_json writes JSON");

    let sent = apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("generic-fdm", Some(written))]),
    );

    assert_eq!(sent.error_code(), "profile_id_reserved");
    assert_eq!(sent.error()["edit_index"], 0);
}

#[test]
fn t184_a_profile_of_another_printer_in_an_edit_is_refused() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));

    let sent = apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("my-mk3", Some(e11p()))]),
    );

    assert_eq!(sent.error_code(), "edit_rejected");
    assert_eq!(sent.error()["edit_index"], 0);
    assert_eq!(
        sent.error_message(),
        "The printer profile has the id \"my-mk4\", but the printer is \"my-mk3\". Store the profile of the printer you name."
    );
}

#[test]
fn the_typed_errors_carry_the_edit_index() {
    let reserved = EngineError::ProfileIdReserved {
        id: "generic-fdm".to_owned(),
        index: Some(0),
    };
    let invalid = EngineError::InvalidProfile {
        message: TALL_LAYER_MESSAGE.to_owned(),
        index: Some(1),
    };
    let put = EngineError::ProfileIdReserved {
        id: "generic-fdm".to_owned(),
        index: None,
    };

    assert_eq!(reserved.edit_index(), Some(0));
    assert_eq!(invalid.edit_index(), Some(1));
    assert_eq!(put.edit_index(), None);
    assert_eq!(
        reserved.to_string(),
        "\"generic-fdm\" is a built-in profile. Save your changes under a new name."
    );
}

// Tests 185 to 188: the open check of a stored profile.

#[test]
fn t185_a_stored_profile_the_engine_rejects_stops_the_open() {
    let mut session = session();
    let text = custom_printer_with(r#"    "layer_height": 0.15,"#, r#"    "layer_height": 0.5,"#);

    let sent = open_text(&mut session, &text);
    let typed = session
        .open_document(&text, Vec::new())
        .expect_err("the stored profile is refused");
    let formatted = session.format(&text);
    let report = session.validate(
        &text,
        &[],
        ValidateOptions {
            rebuild: false,
            canonical: false,
        },
    );

    assert_eq!(sent.error_code(), "parse_error");
    assert_eq!(sent.error_message(), TALL_LAYER_MESSAGE);
    assert_eq!(
        sent.error()["detail"],
        format!("printer_profile: {TALL_LAYER_MESSAGE}")
    );
    let EngineError::Parse { path, line, .. } = &typed else {
        panic!("a parse error, got {typed:?}");
    };
    assert_eq!(path.as_deref(), Some("/printer_profile"));
    assert_eq!(*line, None);
    let new = session.new_document(partsmith_model::Units::Mm, None);
    assert_eq!(
        new.expect("a new document opens").0,
        1,
        "the failed opens took no handle"
    );
    assert_eq!(formatted, Err(typed.clone()));
    assert_eq!(report.diagnostics.len(), 1, "{:?}", report.diagnostics);
    let diagnostic = &report.diagnostics[0];
    assert_eq!(diagnostic.severity, partsmith_protocol::Severity::Error);
    assert_eq!(diagnostic.code, "parse_error");
    assert_eq!(diagnostic.message, TALL_LAYER_MESSAGE);
    assert_eq!(diagnostic.path.as_deref(), Some("/printer_profile"));
}

#[test]
fn t186_a_stored_profile_of_the_wrong_form_stops_the_open() {
    let mut session = session();
    let text = custom_printer_with(
        r#"    "name": "My MK4","#,
        "    \"name\": \"My MK4\",\n    \"speed\": 100,",
    );

    let sent = open_text(&mut session, &text);

    assert_eq!(sent.error_code(), "parse_error");
    assert_eq!(
        sent.error_message(),
        "The printer profile stored in this document has a value of the wrong form. Fix that value in the file to open it."
    );
    let detail = sent.error()["detail"]
        .as_str()
        .expect("the error has a detail");
    assert!(
        detail.starts_with("printer_profile: The printer profile could not be read: "),
        "{detail}"
    );
}

#[test]
fn t187_a_stored_copy_of_a_built_in_profile_stops_the_open() {
    let mut session = session();
    let text = replace_once(
        &custom_printer_with(r#"  "printer": "my-mk4","#, r#"  "printer": "generic-fdm","#),
        r#"    "id": "my-mk4","#,
        r#"    "id": "generic-fdm","#,
    );

    let sent = open_text(&mut session, &text);

    assert_eq!(sent.error_code(), "parse_error");
    assert_eq!(
        sent.error_message(),
        "This document stores a copy of the built-in printer \"generic-fdm\". Remove \"printer_profile\" from it: a built-in printer is named by its id alone."
    );
}

#[test]
fn t188_format_orders_a_stored_profile() {
    let session = session();

    let formatted = session.format(&custom_printer_reversed());

    assert_eq!(formatted, Ok(custom_printer()));
}

// Test 189: `put_printer_profile_text`.

#[test]
fn t189_put_printer_profile_text() {
    let mut session = session();
    let text = serde_json::to_string(&mk4()).expect("MK4 serializes");

    let stored = session
        .put_printer_profile_text(&text)
        .expect("MK4 is a valid profile");

    assert_same_members_in_order(
        &serde_json::to_value(&stored).expect("profiles serialize"),
        &e11p(),
    );
    let ids: Vec<String> = session
        .printer_profiles()
        .iter()
        .map(|profile| profile.id.as_str().to_owned())
        .collect();
    assert_eq!(ids, ["generic-fdm", "generic-resin", "my-mk4"]);
    let reserved = session
        .put_printer_profile_text(r#"{"id": "generic-fdm", "name": "X", "technology": "fdm"}"#)
        .expect_err("a built-in id is reserved");
    assert!(
        matches!(reserved, EngineError::ProfileIdReserved { index: None, .. }),
        "{reserved:?}"
    );
    for text in [r#"{"id": "x", "name": "X"}"#, "not json"] {
        let error = session
            .put_printer_profile_text(text)
            .expect_err("the text is not a profile");
        let EngineError::InvalidProfile { message, index } = &error else {
            panic!("an invalid profile for {text:?}, got {error:?}");
        };
        assert_eq!(*index, None);
        assert!(
            message.starts_with("The printer profile could not be read: "),
            "{message}"
        );
    }
}

// Test 190: `remove_printer_profile`.

#[test]
fn t190_remove_printer_profile() {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, &box_at_version_2_on_my_printer()));
    put_profile(&mut session, my_printer()).ok();

    let removed = remove_profile(&mut session, "my-printer");

    assert_eq!(removed.ok()["type"], "remove_printer_profile");
    let recomputed = recomputed_for(&removed, doc);
    assert_eq!(recomputed.len(), 1, "{:?}", removed.event_types());
    let warnings = recomputed[0]["diagnostics"]
        .as_array()
        .expect("recomputed lists diagnostics");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["code"], "unknown_printer_profile");
    assert_eq!(warnings[0]["message"], unknown_printer_message("my-printer"));
    let ids: Vec<String> = session
        .printer_profiles()
        .iter()
        .map(|profile| profile.id.as_str().to_owned())
        .collect();
    assert_eq!(ids, ["generic-fdm", "generic-resin"]);
    for id in ["generic-fdm", "nope"] {
        let revision = get_document(&mut session, doc)["revision"].clone();
        let sent = remove_profile(&mut session, id);
        sent.ok();
        assert_eq!(sent.events, Vec::<Value>::new(), "{id}");
        assert_eq!(get_document(&mut session, doc)["revision"], revision, "{id}");
        assert_eq!(session.printer_profiles().len(), 2, "{id}");
    }
}

#[test]
fn the_typed_remove_of_an_unknown_id_succeeds() {
    let mut session = session();

    assert_eq!(session.remove_printer_profile("nope"), Ok(()));
    assert_eq!(session.remove_printer_profile("generic-resin"), Ok(()));
    assert_eq!(session.printer_profiles().len(), 2);
}

#[test]
fn removing_a_profile_leaves_a_document_that_stores_it_alone() {
    let mut session = session();
    put_profile(&mut session, mk4()).ok();
    let doc = doc_handle(&open(&mut session, "custom-printer.ptsm"));

    let removed = remove_profile(&mut session, "my-mk4");

    removed.ok();
    assert_eq!(recomputed_for(&removed, doc), Vec::<&Value>::new());
    assert_relative(clearance(&session, doc), 0.15, 1e-12, "clearance");
}

// Test 191: a new document stores a session profile.

#[test]
fn t191_a_new_document_stores_a_session_profile() {
    let mut session = session();
    put_profile(&mut session, mk4()).ok();

    let created = send(
        &mut session,
        json!({"type": "new_document", "printer": "my-mk4"}),
    );

    let doc = doc_handle(&created);
    assert_same_members_in_order(&document(&mut session, doc)["printer_profile"], &e11p());
    assert_eq!(created.event("document_changed")["dirty"], false);
    let undo = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undo.error_code(), "nothing_to_undo");
}

// Test 193: the reason for `printer.nozzle` on resin.

/// The reason of `printer.nozzle` on `generic-resin` (print.md section 3.5).
const RESIN_NOZZLE_REASON: &str = "printer.nozzle has no value with 'Generic resin MSLA', a resin printer with no nozzle. Pick an FDM printer, or write a length instead.";

#[test]
fn t193_the_reason_for_printer_nozzle_on_resin() {
    let mut session = session();
    let text = replace_once(
        &replace_once(
            &fixture("box.ptsm"),
            "  \"units\": \"mm\",\n",
            "  \"units\": \"mm\",\n  \"printer\": \"generic-resin\",\n",
        ),
        r#"    {"id": "p-h", "name": "height", "expr": "40 mm"}"#,
        "    {\"id\": \"p-h\", \"name\": \"height\", \"expr\": \"40 mm\"},\n    {\"id\": \"p-w\", \"name\": \"wall\", \"expr\": \"2 * printer.nozzle\"}",
    );

    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);
    let values = session
        .parameter_values(typed_handle(doc))
        .expect("the document is open");
    let evaluated = send(
        &mut session,
        json!({"type": "evaluate", "doc": doc, "expr": "printer.nozzle"}),
    );
    let report = session.validate(
        &text,
        &[],
        ValidateOptions {
            rebuild: false,
            canonical: false,
        },
    );

    let wall = values
        .iter()
        .find(|value| value.id.as_str() == "p-w")
        .expect("wall has a value record");
    assert_eq!(wall.error.as_deref(), Some(RESIN_NOZZLE_REASON));
    assert_eq!(evaluated.error_code(), "expression_error");
    assert_eq!(evaluated.error_message(), RESIN_NOZZLE_REASON);
    let typed = session
        .evaluate(typed_handle(doc), &partsmith_model::Expr::new("printer.nozzle"))
        .expect_err("printer.nozzle has no value on resin");
    let EngineError::Expression { span, .. } = typed else {
        panic!("an expression error, got {typed:?}");
    };
    assert_eq!(
        span.map(|span| (span.start, span.end)),
        Some((0, 14))
    );
    let unavailable: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "unavailable_value")
        .collect();
    assert_eq!(unavailable.len(), 1, "{:?}", report.diagnostics);
    assert!(
        unavailable[0].message.contains(RESIN_NOZZLE_REASON),
        "{}",
        unavailable[0].message
    );
}

// Test 194: the `.ptsm` schema holds the profile.

#[test]
fn t194_the_ptsm_schema_holds_the_profile() {
    let schema: Value =
        serde_json::from_str(&partsmith_engine::ptsm_schema()).expect("the schema is JSON");
    // The protocol schema's `PrinterProfile` is the schema schemars generates for print.md's `PrinterProfile` as a
    // definition: the root schema without its `$schema`, `$defs` and `title` (protocol.md section 2.9). The engine
    // tests reach schemars only through it.
    let protocol: Value =
        serde_json::from_str(&partsmith_engine::protocol_schema()).expect("the schema is JSON");

    assert_eq!(
        schema.pointer("/properties/printer_profile/$ref"),
        Some(&json!("#/$defs/PrinterProfile"))
    );
    let profile = schema
        .pointer("/$defs/PrinterProfile")
        .expect("the schema defines PrinterProfile");
    assert_eq!(Some(profile), protocol.pointer("/$defs/PrinterProfile"));
    for key in ["$schema", "$defs", "title"] {
        assert_eq!(profile.get(key), None, "{key}");
    }
    for definition in ["Bed", "FitClasses", "Technology"] {
        assert_eq!(
            schema.pointer(&format!("/$defs/{definition}")),
            protocol.pointer(&format!("/$defs/{definition}")),
            "{definition}"
        );
    }
}

// Protocol tests through `Session::call` (protocol.md section 6, tests 89 to 91, 94 and 95).

/// The only response of the frame `Session::call` answers `command` with, and its events, as JSON.
fn call(session: &mut partsmith_engine::Session, command: Value) -> common::Sent {
    common::Sent::from_frame(&session.call(&request_frame(1, command, &[])))
}

#[test]
fn protocol_t089_a_profile_as_text() {
    let mut session = session();
    let text = r#"{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}"#;

    let sent = call(
        &mut session,
        json!({"type": "put_printer_profile", "text": text}),
    );

    let result = sent.ok();
    assert_eq!(result["type"], "put_printer_profile");
    let profile = &result["profile"];
    assert_eq!(profile["nozzle_diameter"], json!(0.4));
    assert_eq!(profile["layer_height"], json!(0.15));
    assert_eq!(
        profile["bed"],
        json!({"kind": "rectangle", "width": 256.0, "depth": 256.0, "height": 256.0})
    );
    assert_eq!(
        profile["fit"],
        json!({"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4})
    );
}

#[test]
fn protocol_t090_exactly_one_input() {
    let mut session = session();
    let both = json!({"type": "put_printer_profile", "profile": mk4(), "text": "{}"});
    let neither = json!({"type": "put_printer_profile"});

    for command in [both, neither] {
        let frame = session.call(&request_frame(1, command.clone(), &[]));
        let sent = common::Sent::from_frame(&frame);

        assert_eq!(sent.error_code(), "malformed_request", "{command}");
        assert_eq!(
            sent.error_message(),
            "The request is not valid: put_printer_profile takes exactly one of profile and text.",
            "{command}"
        );
    }
}

#[test]
fn protocol_t091_text_the_print_crate_cannot_read_is_a_profile_error() {
    let mut session = session();

    let sent = call(
        &mut session,
        json!({"type": "put_printer_profile", "text": "{\"id\": \"x\"}"}),
    );

    assert_eq!(sent.error_code(), "invalid_profile");
    assert!(
        sent.error_message()
            .starts_with("The printer profile could not be read: "),
        "{}",
        sent.error_message()
    );
}

#[test]
fn protocol_t094_a_stored_profile_reaches_the_host() {
    let mut session = session();

    let frame = session.call(&request_frame(
        1,
        json!({"type": "open_document", "text": custom_printer(), "assets": []}),
        &[],
    ));

    decode_response_frame(&frame).expect("the response frame decodes");
    let sent = common::Sent::from_frame(&frame);
    assert_same_members_in_order(
        &sent.event("document_changed")["document"]["printer_profile"],
        &e11p(),
    );
}

#[test]
fn protocol_t095_an_edit_profile_the_engine_rejects_is_an_invalid_profile() {
    let mut session = session();
    let doc = doc_handle(&call(
        &mut session,
        json!({"type": "open_document", "text": fixture("box.ptsm"), "assets": []}),
    ));
    let mut profile = e11p();
    profile["speed"] = json!(1);

    let sent = call(
        &mut session,
        json!({"type": "apply_edits", "doc": doc, "label": "Change printer", "edits": [set_printer("my-mk4", Some(profile))]}),
    );

    assert_eq!(sent.error_code(), "invalid_profile");
    assert_eq!(sent.error()["edit_index"], 0);
}

#[test]
fn a_typed_set_printer_with_a_profile_is_completed_in_the_written_form() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let mut reversed: Map<String, Value> = as_map(e11p()).into_iter().rev().collect();
    reversed.insert("max_bridge_length".to_owned(), json!(10.0));

    apply(
        &mut session,
        doc,
        Some("Change printer"),
        json!([set_printer("my-mk4", Some(Value::Object(reversed)))]),
    )
    .ok();

    assert_same_members_in_order(&document(&mut session, doc)["printer_profile"], &e11p());
    assert_eq!(saved_text(&mut session, doc).matches("\"printer_profile\"").count(), 1);
}
