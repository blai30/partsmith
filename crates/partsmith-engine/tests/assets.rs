//! Acceptance tests 211 to 224 and 233 of `docs/spec/engine.md` section 6, "Assets through the protocol" (section
//! 3.14, ADR 0064): the missing assets of a document, `add_assets` of named and picked files, its events, its
//! recompute, its stop, the words of `asset_mismatch` and a result built without an asset that is not reused.
//!
//! Protocol test 109 goes through `Session::call` and is here too, named `protocol_t109`, beside the document it
//! opens: partsmith-protocol cannot reach a session.
//!
//! `T` is the text of `examples/import/step-block.ptsm`, `S` its asset name and `B` the bytes of
//! `examples/import/assets/S`. Sessions are headless unless a test says otherwise. "Opened bare" means
//! `open_document(T, vec![])`.

use crate::common;

use std::path::{Path, PathBuf};

use common::{
    Sent, apply, assert_near, assert_relative, begin, cancel, commit, doc_handle, headless_session,
    import_step, measure_body, new_doc, number, open, open_text, point3, request_frame, revision,
    saved_text, send, send_with_buffers, step_bytes, typed_handle,
};
use partsmith_engine::{
    AddedAsset, Asset, AssetKind, AssetUse, CancelFlag, NewAsset, Session, SessionOptions,
    ValidateOptions,
};
use partsmith_model::Id;
use partsmith_protocol::{
    Buffer, ErrorCode, FeatureState, FeatureStatus, Severity, decode_response_frame,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `S`: the asset name of `examples/import/step-block.ptsm`.
const S: &str = "b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step";

/// The words of `asset_mismatch` for `S`, which 'Import 1' reads (section 4).
const STEP_MISMATCH: &str = "The STEP file of 'Import 1' in the assets folder is not the file it imported. Copy the assets folder that came with the document next to it.";

/// The volume of the imported block, 10 by 20 by 30 mm.
const BLOCK_VOLUME: f64 = 6000.0;

/// The path of a file of `examples/import/`.
fn import_example(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/import")
        .join(relative)
}

/// `T`: the text of `examples/import/step-block.ptsm`.
fn step_block() -> String {
    let path = import_example("step-block.ptsm");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// `B`: the bytes of `examples/import/assets/S`.
fn block_bytes() -> Vec<u8> {
    let path = import_example(&format!("assets/{S}"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// `T` as JSON, edited by `edit`, as text.
fn edited_step_block(edit: impl FnOnce(&mut Value)) -> String {
    let mut document: Value = serde_json::from_str(&step_block()).expect("T is JSON");
    edit(&mut document);
    serde_json::to_string_pretty(&document).expect("a JSON value serializes")
}

/// The asset name of `bytes` with `extension`: 64 lowercase hex digits of their BLAKE3 hash.
fn hashed_name(bytes: &[u8], extension: &str) -> String {
    format!("{}.{extension}", blake3::hash(bytes).to_hex())
}

/// An id.
fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|_| panic!("{text:?} is a valid id"))
}

/// The missing asset `S`, read by `features`.
fn missing(name: &str, features: &[&str]) -> AssetUse {
    AssetUse {
        name: name.to_owned(),
        features: features.iter().map(|feature| id(feature)).collect(),
    }
}

/// What `add_assets` reports for one file.
fn added(name: &str, stored: bool) -> AddedAsset {
    AddedAsset {
        name: name.to_owned(),
        stored,
    }
}

/// A `NewAsset` of kind `step` read under `name`.
fn named_step(bytes: Vec<u8>, name: &str) -> NewAsset {
    NewAsset {
        kind: AssetKind::Step,
        bytes,
        name: Some(name.to_owned()),
    }
}

/// Opens `text` with no assets through `execute` and returns its handle and reply.
fn opened_bare(session: &mut Session, text: &str) -> (u64, Sent) {
    let sent = open_text(session, text);
    (doc_handle(&sent), sent)
}

/// Opens `text` with `S` holding `bytes`, as request buffer 0, through `execute`.
fn open_with_s(session: &mut Session, text: &str, bytes: &[u8]) -> Sent {
    send_with_buffers(
        session,
        json!({"type": "open_document", "text": text, "assets": [{"name": S, "data": 0}]}),
        vec![Buffer::U8(bytes.to_vec())],
    )
}

/// One entry of an `add_assets` request: its kind, its bytes and the name it was read under, if any.
struct Entry<'a> {
    kind: &'a str,
    bytes: Vec<u8>,
    name: Option<&'a str>,
}

/// An entry of kind `step` read under `name`.
fn step_entry(bytes: Vec<u8>, name: &str) -> Entry<'_> {
    Entry {
        kind: "step",
        bytes,
        name: Some(name),
    }
}

/// The `add_assets` command of `entries`, with each entry's bytes in the request buffer of its position.
fn add_assets_command(doc: u64, entries: &[Entry<'_>]) -> (Value, Vec<Buffer>) {
    let assets: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(position, entry)| {
            let mut asset = json!({"kind": entry.kind, "data": position});
            if let Some(name) = entry.name {
                asset["name"] = json!(name);
            }
            asset
        })
        .collect();
    let buffers = entries
        .iter()
        .map(|entry| Buffer::U8(entry.bytes.clone()))
        .collect();
    (
        json!({"type": "add_assets", "doc": doc, "assets": assets}),
        buffers,
    )
}

/// Sends `add_assets` of `entries` through `execute`.
fn add_assets(session: &mut Session, doc: u64, entries: &[Entry<'_>]) -> Sent {
    let (command, buffers) = add_assets_command(doc, entries);
    send_with_buffers(session, command, buffers)
}

/// The `assets` of a successful `add_assets` reply.
fn added_assets(sent: &Sent) -> Value {
    let result = sent.ok();
    assert_eq!(result["type"], "add_assets", "{result}");
    result["assets"].clone()
}

/// The status of `feature` from the typed `feature_statuses`.
fn status_of(session: &Session, doc: u64, feature: &str) -> FeatureStatus {
    session
        .feature_statuses(typed_handle(doc))
        .expect("the document is open")
        .into_iter()
        .find(|status| status.id.as_str() == feature)
        .unwrap_or_else(|| panic!("the document has a status for {feature}"))
}

/// Asserts that `feature` failed with `asset_missing`.
fn assert_asset_missing(session: &Session, doc: u64, feature: &str) {
    let status = status_of(session, doc, feature);
    assert_eq!(status.status, FeatureState::Error, "{status:?}");
    assert_eq!(status.messages[0].code, "asset_missing", "{status:?}");
}

/// Asserts that `feature` built without a message of severity error.
fn assert_ok(session: &Session, doc: u64, feature: &str) {
    let status = status_of(session, doc, feature);
    assert_eq!(status.status, FeatureState::Ok, "{status:?}");
}

/// The typed `missing_assets` of an open document.
fn missing_assets(session: &Session, doc: u64) -> Vec<AssetUse> {
    session
        .missing_assets(typed_handle(doc))
        .expect("the document is open")
}

/// The JSON form of `missing_assets` as `document_changed` carries it.
fn missing_json(uses: &[AssetUse]) -> Value {
    serde_json::to_value(uses).expect("asset uses serialize")
}

/// Asserts that `b-1` holds the imported block, moved by `offset` along X.
fn assert_block_at(session: &mut Session, doc: u64, offset: f64) {
    let measured = measure_body(session, doc, "b-1");
    assert_relative(
        number(&measured["volume"]),
        BLOCK_VOLUME,
        1e-6,
        "volume of b-1",
    );
    let corners = [
        ("min", [offset, 0.0, 0.0]),
        ("max", [offset + 10.0, 20.0, 30.0]),
    ];
    for (corner, expected) in corners {
        let actual = point3(&measured["bounding_box"][corner]);
        for axis in 0..3 {
            assert_near(
                actual[axis],
                expected[axis],
                1e-6,
                &format!("bounding box {corner} component {axis} (got {actual:?})"),
            );
        }
    }
}

#[test]
fn t211_an_open_bare_reports_the_missing_asset() {
    let mut session = headless_session();

    let (doc, opened) = opened_bare(&mut session, &step_block());

    let expected = vec![missing(S, &["f-imp1"])];
    assert_eq!(missing_assets(&session, doc), expected);
    assert_eq!(
        opened.event("document_changed")["missing_assets"],
        missing_json(&expected)
    );
}

#[test]
fn t211_an_open_with_the_asset_misses_nothing() {
    let mut session = headless_session();

    let with_asset = open_with_s(&mut session, &step_block(), &block_bytes());
    let box_document = open(&mut session, "box.ptsm");

    for opened in [with_asset, box_document] {
        let doc = doc_handle(&opened);
        assert_eq!(missing_assets(&session, doc), Vec::new());
        assert_eq!(
            opened.event("document_changed")["missing_assets"],
            json!([])
        );
    }
}

#[test]
fn t212_a_named_file_fills_a_missing_asset() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());

    let result = session
        .add_assets(typed_handle(doc), vec![named_step(block_bytes(), S)])
        .expect("add_assets succeeds");

    assert_eq!(result, vec![added(S, true)]);
    assert_ok(&session, doc, "f-imp1");
    assert_block_at(&mut session, doc, 0.0);
    assert_eq!(missing_assets(&session, doc), Vec::new());
    assert_eq!(revision(&mut session, doc), 0);
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));
    assert_eq!(undone.error_code(), "nothing_to_undo");
    // A transaction's start reports the dirty state.
    let begun = begin(&mut session, doc, "Check");
    assert_eq!(begun.event("document_changed")["dirty"], false);
}

#[test]
fn t213_the_events_of_an_add() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());

    let sent = add_assets(&mut session, doc, &[step_entry(block_bytes(), S)]);

    assert_eq!(sent.event_types(), ["document_changed", "recomputed"]);
    let changed = sent.event("document_changed");
    assert_eq!(changed["revision"], 0);
    assert_eq!(changed["missing_assets"], json!([]));
    assert_eq!(sent.feature_status("f-imp1")["status"], "ok");
    assert_eq!(added_assets(&sent), json!([{"name": S, "stored": true}]));
}

#[test]
fn t214_a_wrong_named_file_is_refused_alone() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());
    let mut wrong = block_bytes();
    let last = wrong.last_mut().expect("B is not empty");
    *last = last.wrapping_add(1);

    let refused = add_assets(&mut session, doc, &[step_entry(wrong, S)]);

    assert_eq!(
        added_assets(&refused),
        json!([{"name": S, "stored": false}])
    );
    assert!(refused.events.is_empty(), "{:?}", refused.event_types());
    assert_asset_missing(&session, doc, "f-imp1");
    assert_eq!(missing_assets(&session, doc), vec![missing(S, &["f-imp1"])]);

    let stored = add_assets(&mut session, doc, &[step_entry(block_bytes(), S)]);

    assert_eq!(added_assets(&stored), json!([{"name": S, "stored": true}]));
}

#[test]
fn t215_every_name_is_checked() {
    let mut session = headless_session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let as_ttf = S.replace(".step", ".ttf");

    let sent = add_assets(
        &mut session,
        doc,
        &[
            step_entry(block_bytes(), &as_ttf),
            step_entry(block_bytes(), "bracket.step"),
            Entry {
                kind: "ttf",
                bytes: block_bytes(),
                name: Some(S),
            },
        ],
    );

    assert_eq!(
        added_assets(&sent),
        json!([
            {"name": as_ttf, "stored": false},
            {"name": "bracket.step", "stored": false},
            {"name": S, "stored": false}
        ])
    );
    let saved = session
        .save_document(typed_handle(doc))
        .expect("the document saves");
    assert_eq!(saved.assets, Vec::new());
}

#[test]
fn t216_a_picked_file_is_named_by_its_kind() {
    let mut session = headless_session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));

    let sent = add_assets(
        &mut session,
        doc,
        &[Entry {
            kind: "step",
            bytes: block_bytes(),
            name: None,
        }],
    );

    assert_eq!(added_assets(&sent), json!([{"name": S, "stored": true}]));
    assert!(
        sent.events.is_empty(),
        "no record reads S: {:?}",
        sent.event_types()
    );
}

#[test]
fn t216_a_picked_font_is_named_by_its_kind() {
    let mut session = headless_session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let digits = blake3::hash(b"font").to_hex().to_string();

    for (kind, extension) in [(AssetKind::Ttf, "ttf"), (AssetKind::Otf, "otf")] {
        let result = session
            .add_assets(
                typed_handle(doc),
                vec![NewAsset {
                    kind,
                    bytes: b"font".to_vec(),
                    name: None,
                }],
            )
            .expect("add_assets succeeds");

        assert_eq!(result, vec![added(&format!("{digits}.{extension}"), true)]);
    }
}

#[test]
fn t217_a_picked_file_serves_a_later_edit() {
    let mut session = headless_session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    add_assets(
        &mut session,
        doc,
        &[Entry {
            kind: "step",
            bytes: block_bytes(),
            name: None,
        }],
    )
    .ok();

    let imported = apply(
        &mut session,
        doc,
        Some("Import"),
        json!([
            {"type": "add_body", "index": 1, "body": {"id": "b-2", "name": "Part"}},
            {"type": "add_feature", "index": 1, "feature": {"id": "f-imp1", "type": "import", "body": "b-2", "asset": S, "op": "new"}}
        ]),
    );

    assert_eq!(imported.feature_status("f-imp1")["status"], "ok");
    let saved = session
        .save_document(typed_handle(doc))
        .expect("the document saves");
    assert_eq!(
        saved.assets,
        vec![Asset {
            name: S.to_owned(),
            bytes: block_bytes(),
        }]
    );

    session.undo(typed_handle(doc)).expect("undo succeeds");
    let after_undo = session
        .save_document(typed_handle(doc))
        .expect("the document saves");
    assert_eq!(after_undo.assets, Vec::new());

    session.redo(typed_handle(doc)).expect("redo succeeds");
    assert_ok(&session, doc, "f-imp1");
}

/// `U` of test 218: the text `save_document` returns after `import_step` of `B` and then `import_step` of the STEP
/// export of `circle.ptsm` in a new document, with the ids of the two import features and `S2`, the second import's
/// asset name.
struct TwoImports {
    text: String,
    first: String,
    second: String,
    second_asset: String,
}

/// Builds `U` in a session of its own.
fn two_imports() -> TwoImports {
    let mut session = headless_session();
    let circle = step_bytes(&mut session, "circle.ptsm");
    let doc = new_doc(&mut session);
    let feature = |sent: Sent| {
        sent.ok()["feature"]
            .as_str()
            .expect("the import names its feature")
            .to_owned()
    };
    let first = feature(import_step(&mut session, doc, &block_bytes(), None));
    let second = feature(import_step(&mut session, doc, &circle, None));
    TwoImports {
        text: saved_text(&mut session, doc),
        first,
        second,
        second_asset: hashed_name(&circle, "step"),
    }
}

#[test]
fn t218_several_files_in_one_command_and_one_recompute() {
    let imports = two_imports();
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &imports.text);

    let sent = add_assets(
        &mut session,
        doc,
        &[
            step_entry(block_bytes(), S),
            step_entry(b"hello".to_vec(), &imports.second_asset),
        ],
    );

    assert_eq!(
        added_assets(&sent),
        json!([{"name": S, "stored": true}, {"name": imports.second_asset, "stored": false}])
    );
    assert_eq!(sent.events_of("recomputed").len(), 1);
    assert_ok(&session, doc, &imports.first);
    assert_asset_missing(&session, doc, &imports.second);
    assert_eq!(
        missing_assets(&session, doc),
        vec![missing(&imports.second_asset, &[&imports.second])]
    );
}

#[test]
fn t219_nothing_new_nothing_done() {
    let mut session = headless_session();
    let doc = doc_handle(&open_with_s(&mut session, &step_block(), &block_bytes()));

    let again = add_assets(&mut session, doc, &[step_entry(block_bytes(), S)]);
    let empty = add_assets(&mut session, doc, &[]);

    assert_eq!(added_assets(&again), json!([{"name": S, "stored": true}]));
    assert!(again.events.is_empty(), "{:?}", again.event_types());
    assert_eq!(added_assets(&empty), json!([]));
    assert!(empty.events.is_empty(), "{:?}", empty.event_types());
}

#[test]
fn t220_inside_a_transaction() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());
    begin(&mut session, doc, "Edit").ok();

    let sent = add_assets(&mut session, doc, &[step_entry(block_bytes(), S)]);

    assert_eq!(added_assets(&sent), json!([{"name": S, "stored": true}]));
    assert_eq!(sent.event("document_changed")["transaction_open"], true);
    assert_ok(&session, doc, "f-imp1");
    let committed = commit(&mut session, doc);
    assert_eq!(committed.ok()["recorded"], false);
}

#[test]
fn t220_a_canceled_transaction_keeps_the_stored_file() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());
    begin(&mut session, doc, "Edit").ok();
    add_assets(&mut session, doc, &[step_entry(block_bytes(), S)]).ok();

    cancel(&mut session, doc).ok();

    assert_eq!(missing_assets(&session, doc), Vec::new());
    assert_ok(&session, doc, "f-imp1");
}

#[test]
fn t221_a_suppressed_record_still_reads_its_asset() {
    let text = edited_step_block(|document| {
        document["features"][0]["suppressed"] = json!(true);
    });
    let mut session = headless_session();

    let (doc, _) = opened_bare(&mut session, &text);

    assert_eq!(missing_assets(&session, doc), vec![missing(S, &["f-imp1"])]);
}

#[test]
fn t221_every_record_that_reads_an_asset_is_listed_in_timeline_order() {
    let text = edited_step_block(|document| {
        document["bodies"]
            .as_array_mut()
            .expect("T has bodies")
            .push(json!({"id": "b-2", "name": "Body 2"}));
        document["features"]
            .as_array_mut()
            .expect("T has features")
            .push(
                json!({"id": "f-imp2", "type": "import", "body": "b-2", "asset": S, "op": "new"}),
            );
    });
    let mut session = headless_session();

    let (doc, _) = opened_bare(&mut session, &text);

    assert_eq!(
        missing_assets(&session, doc),
        vec![missing(S, &["f-imp1", "f-imp2"])]
    );
}

#[test]
fn t221_a_record_whose_params_fail_reads_no_asset() {
    let text = edited_step_block(|document| {
        document["features"][0]["asset"] = json!("box.step");
    });
    let mut session = headless_session();

    let (doc, _) = opened_bare(&mut session, &text);

    assert_eq!(missing_assets(&session, doc), Vec::new());
}

#[test]
fn t221_a_rolled_back_record_still_reads_its_asset() {
    let text = edited_step_block(|document| {
        document["rollback"] = json!("f-imp1");
    });
    let mut session = headless_session();

    let (doc, _) = opened_bare(&mut session, &text);

    assert_eq!(missing_assets(&session, doc), vec![missing(S, &["f-imp1"])]);
}

#[test]
fn t222_the_words_of_a_wrong_file_at_open() {
    let mut session = headless_session();

    let error = session
        .open_document(
            &step_block(),
            vec![Asset {
                name: S.to_owned(),
                bytes: b"hello".to_vec(),
            }],
        )
        .expect_err("a wrong file refuses the document");

    assert_eq!(error.code(), ErrorCode::AssetMismatch);
    let message = error.to_string();
    assert_eq!(message, STEP_MISMATCH);
    assert_eq!(
        error.detail(),
        Some(format!(
            "assets/{S}: the BLAKE3 hash of its bytes is {}.",
            blake3::hash(b"hello").to_hex()
        ))
    );
    let hex_runs = message
        .split(|character: char| !character.is_ascii_hexdigit())
        .filter(|run| run.len() >= 64)
        .count();
    assert_eq!(hex_runs, 0, "the message holds no hash: {message}");
    partsmith_message_lint::assert_passes_lint(&message);
}

#[test]
fn t222_the_wire_error_carries_the_words_and_the_detail() {
    let mut session = headless_session();

    let sent = open_with_s(&mut session, &step_block(), b"hello");

    assert_eq!(sent.error_code(), "asset_mismatch");
    assert_eq!(sent.error_message(), STEP_MISMATCH);
    assert_eq!(
        sent.error()["detail"],
        format!(
            "assets/{S}: the BLAKE3 hash of its bytes is {}.",
            blake3::hash(b"hello").to_hex()
        )
    );
}

#[test]
fn t223_the_words_in_validate() {
    let mut session = headless_session();

    let report = session.validate(
        &step_block(),
        &[Asset {
            name: S.to_owned(),
            bytes: b"hello".to_vec(),
        }],
        ValidateOptions::default(),
    );

    let mismatches: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "asset_mismatch")
        .collect();
    assert_eq!(mismatches.len(), 1, "{:?}", report.diagnostics);
    let diagnostic = mismatches[0];
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.path.as_deref(), Some("/features/0/asset"));
    assert_eq!(diagnostic.message, STEP_MISMATCH);
}

#[test]
fn t224_a_stop_during_an_add() {
    let flag = CancelFlag::word();
    let mut session = Session::new(SessionOptions {
        cancel: flag.clone(),
        ..SessionOptions::headless()
    });
    let opened = Sent::from_frame(&session.call(&request_frame(
        1,
        json!({"type": "open_document", "text": step_block(), "assets": []}),
        &[],
    )));
    let doc = doc_handle(&opened);
    flag.cancel(2);

    let (command, buffers) = add_assets_command(doc, &[step_entry(block_bytes(), S)]);
    let stopped = Sent::from_frame(&session.call(&request_frame(2, command, &buffers)));

    assert_eq!(added_assets(&stopped), json!([{"name": S, "stored": true}]));
    assert_eq!(stopped.feature_status("f-imp1")["status"], "not_run");
    assert_eq!(
        stopped.event("document_changed")["missing_assets"],
        json!([])
    );

    let rebuilt = Sent::from_frame(&session.call(&request_frame(
        3,
        json!({"type": "rebuild", "doc": doc}),
        &[],
    )));

    assert_eq!(rebuilt.feature_status("f-imp1")["status"], "ok");
}

/// `T2`: `T` with a move of `b-1` by 100 mm along X after `f-imp1`.
fn moved_step_block() -> String {
    edited_step_block(|document| {
        document["features"]
            .as_array_mut()
            .expect("T has features")
            .push(json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100 mm", "0 mm", "0 mm"]}));
    })
}

#[test]
fn t233_a_result_built_without_the_asset_is_not_reused() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &moved_step_block());

    add_assets(&mut session, doc, &[step_entry(block_bytes(), S)]).ok();

    assert_ok(&session, doc, "f-imp1");
    assert_ok(&session, doc, "f-mv1");
    assert_block_at(&mut session, doc, 100.0);
}

#[test]
fn t233_a_reopen_with_the_asset_does_not_reuse_the_bare_open() {
    let mut session = headless_session();
    let (bare, _) = opened_bare(&mut session, &moved_step_block());
    send(&mut session, json!({"type": "close_document", "doc": bare})).ok();

    let doc = doc_handle(&open_with_s(
        &mut session,
        &moved_step_block(),
        &block_bytes(),
    ));

    assert_ok(&session, doc, "f-imp1");
    assert_ok(&session, doc, "f-mv1");
    assert_block_at(&mut session, doc, 100.0);
}

#[test]
fn an_unknown_document_fails_add_assets() {
    let mut session = headless_session();

    let sent = add_assets(&mut session, 9, &[step_entry(block_bytes(), S)]);

    assert_eq!(sent.error_code(), "unknown_document");
}

#[test]
fn an_entry_whose_data_is_not_a_u8_buffer_is_a_malformed_request() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());

    let sent = send_with_buffers(
        &mut session,
        json!({"type": "add_assets", "doc": doc, "assets": [{"kind": "step", "data": 0, "name": S}]}),
        vec![Buffer::F32(vec![1.0])],
    );

    assert_eq!(sent.error_code(), "malformed_request");
    assert_eq!(missing_assets(&session, doc), vec![missing(S, &["f-imp1"])]);
}

#[test]
fn an_undo_that_brings_an_import_back_reports_its_asset_again() {
    let mut session = headless_session();
    let (doc, _) = opened_bare(&mut session, &step_block());

    let removed = apply(
        &mut session,
        doc,
        Some("Delete"),
        json!([{"type": "remove_feature", "id": "f-imp1"}]),
    );
    assert_eq!(
        removed.event("document_changed")["missing_assets"],
        json!([])
    );
    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(
        undone.event("document_changed")["missing_assets"],
        missing_json(&[missing(S, &["f-imp1"])])
    );
}

#[test]
fn a_file_no_record_reads_stays_stored_until_an_edit_names_it() {
    let mut session = headless_session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    add_assets(
        &mut session,
        doc,
        &[Entry {
            kind: "step",
            bytes: block_bytes(),
            name: None,
        }],
    )
    .ok();
    let unread = session
        .save_document(typed_handle(doc))
        .expect("the document saves");
    assert_eq!(unread.assets, Vec::new());

    apply(
        &mut session,
        doc,
        Some("Import"),
        json!([
            {"type": "add_body", "index": 1, "body": {"id": "b-2", "name": "Part"}},
            {"type": "add_feature", "index": 1, "feature": {"id": "f-imp1", "type": "import", "body": "b-2", "asset": S, "op": "new"}}
        ]),
    );

    assert_eq!(missing_assets(&session, doc), Vec::new());
}

#[test]
fn protocol_t109_the_names_and_the_command_through_call() {
    let mut session = headless_session();
    let opened_frame = session.call(&request_frame(
        1,
        json!({"type": "open_document", "text": step_block(), "assets": []}),
        &[],
    ));
    decode_response_frame(&opened_frame).expect("the open's frame decodes");
    let opened = Sent::from_frame(&opened_frame);
    assert_eq!(
        opened.event("document_changed")["missing_assets"],
        json!([{"name": S, "features": ["f-imp1"]}])
    );
    let doc = doc_handle(&opened);

    let (command, buffers) = add_assets_command(doc, &[step_entry(block_bytes(), S)]);
    let added_frame = session.call(&request_frame(2, command, &buffers));

    decode_response_frame(&added_frame).expect("the add's frame decodes");
    let sent = Sent::from_frame(&added_frame);
    assert_eq!(sent.event_types(), ["document_changed", "recomputed"]);
    assert_eq!(sent.event("document_changed")["missing_assets"], json!([]));
    assert_eq!(
        sent.result(),
        &json!({"kind": "ok", "type": "add_assets", "assets": [{"name": S, "stored": true}]})
    );
}
