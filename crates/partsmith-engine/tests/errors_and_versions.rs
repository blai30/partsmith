//! Acceptance tests 146 to 149 of `docs/spec/engine.md` section 6: error texts with their details and actions, the
//! actions of `transaction_open`, format version 1 texts, and invalid params in edits (section 4).
//!
//! Test 150 (the newcomer error corpus) comes with phase 3 of the reference stability plan and test 151 (determinism
//! with causes and ghosts) with phase 4. Test 149's invalid candidate sketch waits for phase 3 too: its message is the
//! sketch crate's rendering of sketch.md section 4.1 (`SketchError::render`, which names entities by their words), so
//! it stays ignored until phase 3 step 3 lands.

mod common;

use common::{
    apply, code_text, fixture, get_document, new_doc, open_doc, open_text, replace_once,
    saved_text, send, session, set_height,
};
use partsmith_engine::EngineError;
use partsmith_model::FORMAT_VERSION;
use partsmith_protocol::ErrorCode;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `box.ptsm` with its `format_version` set to `version`.
fn box_at_version(version: u32) -> String {
    replace_once(
        &fixture("box.ptsm"),
        &format!(r#""format_version": {FORMAT_VERSION},"#),
        &format!(r#""format_version": {version},"#),
    )
}

/// The `finish_transaction` action of a `transaction_open` error.
fn finish(label: &str) -> Value {
    json!({"label": label, "run": {"kind": "finish_transaction"}})
}

// Test 146: error texts and details.

#[test]
fn t146_internal_errors_name_the_command_and_keep_the_panic_text_aside() {
    let error = EngineError::Internal {
        command: "apply edits".to_owned(),
        detail: "boom".to_owned(),
    };

    let info = error.to_info();

    assert_eq!(info.code, ErrorCode::InternalError);
    assert_eq!(
        info.message,
        "Partsmith hit a bug while running apply edits. Your document is unchanged. Save a copy, and report this with the details."
    );
    assert_eq!(info.detail.as_deref(), Some("boom"));
    assert!(info.actions.is_empty(), "{:?}", info.actions);
}

#[test]
fn t146_invalid_json_says_where_and_keeps_the_parser_text_in_the_detail() {
    let mut session = session();

    let sent = open_text(&mut session, r#"{"format": "partsmith","#);

    assert_eq!(sent.error_code(), "parse_error");
    assert_eq!(
        sent.error_message(),
        "The file is not valid JSON at line 1, column 23."
    );
    let detail = sent.error()["detail"]
        .as_str()
        .expect("the error has a detail");
    assert!(detail.contains("EOF while parsing"), "{detail}");
    let typed = session
        .open_document(r#"{"format": "partsmith","#, Vec::new())
        .expect_err("the text is not JSON");
    let EngineError::Parse { line, .. } = typed else {
        panic!("a parse error, got {typed:?}");
    };
    assert_eq!(line, Some(1));
}

#[test]
fn t146_a_newer_format_version_names_both_versions() {
    let mut session = session();

    let sent = open_text(&mut session, &box_at_version(3));

    assert_eq!(sent.error_code(), "unsupported_format_version");
    assert_eq!(
        sent.error_message(),
        "This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it."
    );
}

#[test]
fn t146_a_value_of_the_wrong_form_hides_the_deserializer_text() {
    let mut session = session();
    let text = replace_once(&fixture("box.ptsm"), r#""name": "Block""#, r#""name": 5"#);

    let sent = open_text(&mut session, &text);

    assert_eq!(sent.error_code(), "parse_error");
    assert_eq!(
        sent.error_message(),
        "A value in this file has the wrong form, so it cannot be opened."
    );
    let detail = sent.error()["detail"]
        .as_str()
        .expect("the error has a detail");
    assert!(detail.contains("invalid type"), "{detail}");
}

#[test]
fn t146_an_id_problem_reads_its_own_message() {
    let mut session = session();
    let text = replace_once(&fixture("box.ptsm"), r#""id": "b-1""#, r#""id": "b 1""#);

    let sent = open_text(&mut session, &text);

    assert_eq!(sent.error_code(), "parse_error");
    assert!(
        sent.error_message().contains("The id 'b 1' contains ' '."),
        "{}",
        sent.error_message()
    );
}

#[test]
fn t146_errors_without_technical_text_have_no_detail_and_no_actions() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(sent.error_code(), "nothing_to_undo");
    assert!(sent.error().get("detail").is_none(), "{}", sent.error());
    assert!(sent.error().get("actions").is_none(), "{}", sent.error());
}

// Test 147: `transaction_open` actions.

#[test]
fn t147_save_during_a_transaction_offers_to_finish_and_save() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Resize"}),
    )
    .ok();

    let saved = send(&mut session, json!({"type": "save_document", "doc": doc}));

    assert_eq!(saved.error_code(), "transaction_open");
    assert_eq!(
        saved.error()["actions"],
        json!([finish("Finish \"Resize\" and save")])
    );
}

#[test]
fn t147_undo_redo_and_begin_during_a_transaction_say_what_they_do_after_finishing() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("45 mm")]),
    )
    .ok();
    send(&mut session, json!({"type": "undo", "doc": doc})).ok();
    send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Resize"}),
    )
    .ok();

    let undo = send(&mut session, json!({"type": "undo", "doc": doc}));
    let redo = send(&mut session, json!({"type": "redo", "doc": doc}));
    let begin = send(
        &mut session,
        json!({"type": "begin_transaction", "doc": doc, "label": "Fillet"}),
    );

    assert_eq!(
        undo.error()["actions"],
        json!([finish("Finish \"Resize\" and undo")])
    );
    assert_eq!(
        redo.error()["actions"],
        json!([finish("Finish \"Resize\" and redo")])
    );
    assert_eq!(
        begin.error()["actions"],
        json!([finish("Finish \"Resize\" and start \"Fillet\"")])
    );
}

#[test]
fn t147_the_typed_error_offers_the_same_action() {
    let error = EngineError::TransactionOpen {
        label: "Resize".to_owned(),
        then: "save".to_owned(),
    };

    let info = error.to_info();

    assert_eq!(info.message, "Finish or cancel \"Resize\" first.");
    assert_eq!(
        serde_json::to_value(&info.actions).expect("actions serialize"),
        json!([finish("Finish \"Resize\" and save")])
    );
    assert_eq!(error.actions(), info.actions);
}

// Test 148: format version 1 texts.

#[test]
fn t148_a_version_1_text_opens_migrated_and_saves_at_version_2() {
    let mut session = session();
    let old = box_at_version(1);

    let opened = session
        .open_document(&old, Vec::new())
        .expect("a version 1 text opens");

    assert_eq!(opened.migrated_from, Some(1));
    let doc = u64::from(opened.doc.0);
    let saved = saved_text(&mut session, doc);
    assert_eq!(saved, fixture("box.ptsm"));
    let differing: Vec<(&str, &str)> = old
        .lines()
        .zip(saved.lines())
        .filter(|(before, after)| before != after)
        .collect();
    assert_eq!(
        differing,
        [(r#"  "format_version": 1,"#, r#"  "format_version": 2,"#)]
    );
}

#[test]
fn t148_new_documents_and_engine_info_are_at_version_2() {
    let mut session = session();

    let doc = new_doc(&mut session);

    assert_eq!(
        get_document(&mut session, doc)["document"]["format_version"],
        2
    );
    let info = send(&mut session, json!({"type": "engine_info"}));
    assert_eq!(info.ok()["format_version"], 2);
}

// Test 149: invalid params in edits.

#[test]
fn t149_an_unreadable_field_is_named_with_its_label_and_the_parser_text_is_in_the_detail() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");
    let mut fillet = get_document(&mut session, doc)["document"]["features"][1].clone();
    fillet["radius"] = json!({"x": 1});

    let sent = apply(
        &mut session,
        doc,
        Some("Change radius"),
        json!([{"type": "update_feature", "feature": fillet}]),
    );

    assert_eq!(sent.error_code(), "invalid_params");
    assert_eq!(sent.error()["edit_index"], 0);
    assert_eq!(
        sent.error_message(),
        "Radius of 'Fillet 1' cannot be read, so this change was not applied. Set Radius again."
    );
    let detail = sent.error()["detail"]
        .as_str()
        .expect("the error has a detail");
    assert!(detail.starts_with("fillet f-fil1: "), "{detail}");
}

#[test]
#[ignore = "phase 3 step 3: the sketch crate renders SketchError without ids (sketch.md section 4.1)"]
fn t149_an_invalid_candidate_sketch_names_no_id_but_its_detail_does() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    let candidate = json!({
        "plane": {"kind": "origin", "plane": "XY"},
        "entities": [
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]},
            {"id": "e-2", "type": "circle", "center": [0, 0], "radius": -1}
        ],
        "constraints": [
            {"id": "c-1", "type": "fix", "target": "e-1.start"},
            {"id": "c-2", "type": "horizontal", "line": "e-1"}
        ]
    });

    let sent = send(
        &mut session,
        json!({"type": "solve_sketch", "doc": doc, "feature": "f-sk1", "candidate": candidate}),
    );

    assert_eq!(sent.error_code(), "invalid_params");
    assert!(
        !sent.error_message().contains("e-2"),
        "{}",
        sent.error_message()
    );
    let detail = sent.error()["detail"]
        .as_str()
        .expect("the error has a detail");
    assert!(detail.contains("e-2"), "{detail}");
}

#[test]
fn t149_the_typed_errors_carry_their_detail() {
    let invalid_params = EngineError::InvalidParams {
        index: 1,
        feature: partsmith_model::Id::new("f-box2").expect("a valid id"),
        feature_type: "box".to_owned(),
        message: "Body of 'Box 2' cannot be read, so this change was not applied. Set Body again."
            .to_owned(),
        detail: "box f-box2: missing field `body`".to_owned(),
    };
    let invalid_sketch = EngineError::InvalidSketch {
        message: "A circle has a radius of -1 mm. Use a radius above 0 mm.".to_owned(),
        detail: "circle e-2 has radius -1".to_owned(),
    };

    assert_eq!(
        invalid_params.to_string(),
        "Body of 'Box 2' cannot be read, so this change was not applied. Set Body again."
    );
    assert_eq!(
        invalid_params.detail().as_deref(),
        Some("box f-box2: missing field `body`")
    );
    assert_eq!(code_text(invalid_sketch.code()), "invalid_params");
    assert_eq!(invalid_sketch.edit_index(), None);
    assert_eq!(
        invalid_sketch.to_info().detail.as_deref(),
        Some("circle e-2 has radius -1")
    );
}
