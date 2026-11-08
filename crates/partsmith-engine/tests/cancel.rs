//! Acceptance tests 195 to 209 and 232 of `docs/spec/engine.md` section 6, "Cancel" (section 3.26, ADR 0063): a
//! host's `CancelFlag` stops the request whose id it holds, a stopped recompute keeps the request's edits and reports
//! `not_run`, `export`, `import_step` and `analyze_print` fail with `canceled`, and `rebuild` finishes an incomplete
//! document. Test 210 is a unit test in `src/session.rs` and test 231 one in `src/exchange.rs`.
//!
//! "The cancel thread" sleeps the time a test names, from just before the request starts, then calls `cancel` of the
//! session's flag with the request's id. The tests wait on the wall clock, as recompute.md open issue 3 notes: the
//! reference part takes far longer than 30 or 50 ms to build in any build profile.

use crate::common;

use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::{
    Sent, assert_bounding_box, display_options, doc_handle, fixture, get_document, request_frame,
    saved_text, set_height, typed_handle,
};
use partsmith_engine::{CancelFlag, Session, SessionOptions};
use partsmith_model::Edit;
use partsmith_protocol::{Buffer, FeatureState};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The message of a stopped `export` (section 3.26).
const EXPORT_STOPPED: &str = "The export was stopped before it finished. No file was made.";

/// The message of a stopped `import_step` (section 3.26).
const IMPORT_STOPPED: &str =
    "The STEP import was stopped before it finished. Nothing was added to the document.";

/// The message of a stopped `analyze_print` (section 3.26).
const PRINT_STOPPED: &str =
    "The print check was stopped before it finished. Run it again to see its findings.";

/// The text of the reference part, `examples/bench/fifty-features/model.ptsm`.
fn reference_part() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/bench/fifty-features/model.ptsm");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// A display session whose `cancel` is a clone of `flag` (the session of section 6's header).
fn display_session_with(flag: &CancelFlag) -> Session {
    Session::new(SessionOptions {
        cancel: flag.clone(),
        ..display_options()
    })
}

/// A headless session whose `cancel` is a clone of `flag`.
fn headless_session_with(flag: &CancelFlag) -> Session {
    Session::new(SessionOptions {
        cancel: flag.clone(),
        ..SessionOptions::headless()
    })
}

/// A twin: a headless session with `CancelFlag::never()`.
fn headless_twin() -> Session {
    Session::new(SessionOptions::headless())
}

/// Sends `command` as request `id` through `Session::call`, which goes through `execute`.
fn send_as(session: &mut Session, id: u64, command: Value) -> Sent {
    send_with_buffers_as(session, id, command, &[])
}

/// Sends `command` with request buffers as request `id` through `Session::call`.
fn send_with_buffers_as(
    session: &mut Session,
    id: u64,
    command: Value,
    buffers: &[Buffer],
) -> Sent {
    Sent::from_frame(&session.call(&request_frame(id, command, buffers)))
}

/// Calls `flag.cancel(request)` from another thread `delay` after now.
fn cancel_after(flag: &CancelFlag, request: u64, delay: Duration) -> JoinHandle<()> {
    let remote = flag.clone();
    thread::spawn(move || {
        thread::sleep(delay);
        remote.cancel(request);
    })
}

/// Sends `command` as request `id` with the cancel thread at `delay`.
fn send_canceled_after(
    session: &mut Session,
    flag: &CancelFlag,
    id: u64,
    command: Value,
    delay: Duration,
) -> Sent {
    let canceler = cancel_after(flag, id, delay);
    let sent = send_as(session, id, command);
    canceler.join().expect("the cancel thread finishes");
    sent
}

/// `open_document` of a text with no assets.
fn open_command(text: &str) -> Value {
    json!({"type": "open_document", "text": text, "assets": []})
}

/// "The base change": `apply_edits` of document 1, labeled `Change base` unless `label` is `None`, with one
/// `update_parameter` of `p-base` to `250 mm`.
fn base_change(label: Option<&str>) -> Value {
    let mut command = json!({
        "type": "apply_edits",
        "doc": 1,
        "edits": [{"type": "update_parameter", "parameter": {"id": "p-base", "name": "base", "expr": "250 mm"}}]
    });
    if let Some(label) = label {
        command["label"] = json!(label);
    }
    command
}

/// `apply_edits` of document 1 that sets the box fixture's height to `height`, with `label` when given.
fn box_height(height: &str, label: Option<&str>) -> Value {
    let mut command = json!({"type": "apply_edits", "doc": 1, "edits": [set_height(height)]});
    if let Some(label) = label {
        command["label"] = json!(label);
    }
    command
}

/// `export` of every body of document 1 to binary STL at standard quality.
fn binary_stl_export() -> Value {
    json!({"type": "export", "doc": 1, "format": {"kind": "stl", "binary": true, "per_body": false}})
}

/// `analyze_print` of document 1 as modeled.
fn print_check() -> Value {
    json!({"type": "analyze_print", "doc": 1, "orientation": {"kind": "as_modeled"}})
}

/// `rebuild` of `doc`.
fn rebuild(doc: u64) -> Value {
    json!({"type": "rebuild", "doc": doc})
}

/// The feature statuses of the reply's `recomputed` event.
fn statuses(sent: &Sent) -> Vec<Value> {
    sent.event("recomputed")["features"]
        .as_array()
        .expect("recomputed lists its features")
        .clone()
}

/// The `status` of a feature status.
fn state(status: &Value) -> &str {
    status["status"]
        .as_str()
        .expect("a feature status has a status")
}

/// Whether any status of the reply's `recomputed` is `not_run`.
fn has_not_run(sent: &Sent) -> bool {
    statuses(sent)
        .iter()
        .any(|status| state(status) == "not_run")
}

/// The position of the first `not_run` status of the reply's `recomputed`; fails when there is none, and when a
/// later feature is not `not_run` too (recompute.md section 3.5, a canceled run).
#[track_caller]
fn first_not_run(sent: &Sent) -> usize {
    let statuses = statuses(sent);
    let first = statuses
        .iter()
        .position(|status| state(status) == "not_run")
        .expect("recomputed holds at least one not_run");
    for status in &statuses[first..] {
        assert_eq!(
            state(status),
            "not_run",
            "every feature after the first not_run one is not_run: {}",
            status["id"]
        );
    }
    first
}

/// The `recomputed` event of a reply without its `revision`.
fn recomputed_without_revision(sent: &Sent) -> Value {
    let mut event = sent.event("recomputed").clone();
    event
        .as_object_mut()
        .expect("an event is an object")
        .remove("revision");
    event
}

/// The bytes of the one file of an `export` reply.
fn exported_bytes(sent: &Sent) -> Vec<u8> {
    let files = sent.ok()["files"]
        .as_array()
        .expect("export returns files")
        .clone();
    assert_eq!(files.len(), 1, "a binary STL of every body is one file");
    sent.file_bytes(&files[0])
}

/// Asserts that a reply failed with `canceled`, `message`, no `detail` and no events.
#[track_caller]
fn assert_canceled(sent: &Sent, message: &str) {
    assert_eq!(sent.error_code(), "canceled");
    assert_eq!(sent.error_message(), message);
    assert!(
        sent.error().get("detail").is_none(),
        "no detail: {}",
        sent.error()
    );
    assert!(
        sent.events.is_empty(),
        "no events: {:?}",
        sent.event_types()
    );
}

/// The STEP bytes of test 89: `box.ptsm` exported as STEP by a session of its own.
fn box_step_bytes() -> Vec<u8> {
    common::box_step_bytes(&mut common::session())
}

/// A headless session with a flag and its twin after request 3, the open of the reference part with the cancel
/// thread at 50 ms in the flag's session (test 195).
fn stopped_open() -> (CancelFlag, Session, Sent, Session, Sent) {
    let flag = CancelFlag::word();
    let mut session = headless_session_with(&flag);
    let mut twin = headless_twin();
    let text = reference_part();

    let opened = send_canceled_after(
        &mut session,
        &flag,
        3,
        open_command(&text),
        Duration::from_millis(50),
    );
    let twin_opened = send_as(&mut twin, 3, open_command(&text));
    (flag, session, opened, twin, twin_opened)
}

/// A headless session with a flag and its twin that opened the reference part with no cancel, then request 4, the
/// base change, with the cancel thread at 30 ms in the flag's session (test 197).
fn stopped_base_change() -> (CancelFlag, Session, Sent, Session, Sent) {
    let flag = CancelFlag::word();
    let mut session = headless_session_with(&flag);
    let mut twin = headless_twin();
    let text = reference_part();
    send_as(&mut session, 3, open_command(&text)).ok();
    send_as(&mut twin, 3, open_command(&text)).ok();

    let changed = send_canceled_after(
        &mut session,
        &flag,
        4,
        base_change(Some("Change base")),
        Duration::from_millis(30),
    );
    let twin_changed = send_as(&mut twin, 4, base_change(Some("Change base")));
    (flag, session, changed, twin, twin_changed)
}

/// A display session with a flag that opened the reference part, began the transaction `Edit`, focused `f-lp4` and
/// sent the base change without a label as request 5 with the cancel thread at 30 ms (test 200), and its opened text.
fn stopped_preview() -> (Session, Sent, String) {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&reference_part())).ok();
    let opened = saved_text(&mut session, 1);
    send_as(
        &mut session,
        2,
        json!({"type": "begin_transaction", "doc": 1, "label": "Edit"}),
    )
    .ok();
    send_as(
        &mut session,
        3,
        json!({"type": "set_focus", "doc": 1, "feature": "f-lp4"}),
    )
    .ok();

    let preview = send_canceled_after(
        &mut session,
        &flag,
        5,
        base_change(None),
        Duration::from_millis(30),
    );
    (session, preview, opened)
}

#[test]
fn t195_a_stopped_open_answers_with_the_features_that_finished() {
    let (_flag, _session, opened, _twin, twin_opened) = stopped_open();

    assert_eq!(opened.ok()["type"], "open_document");
    assert_eq!(doc_handle(&opened), 1);
    let changed = opened.event("document_changed");
    assert_eq!(changed["revision"], 0);
    assert_eq!(changed["dirty"], false);
    let first = first_not_run(&opened);
    assert_eq!(
        statuses(&opened)[..first],
        statuses(&twin_opened)[..first],
        "the features before the first not_run one have the twin's statuses, messages and repairs"
    );
}

#[test]
fn t196_a_not_run_status_holds_nothing() {
    let (_flag, _session, opened, _twin, _twin_opened) = stopped_open();

    let not_run: Vec<Value> = statuses(&opened)
        .into_iter()
        .filter(|status| state(status) == "not_run")
        .collect();
    assert!(!not_run.is_empty(), "the open stopped");
    for status in &not_run {
        assert_eq!(
            status["messages"],
            json!([]),
            "messages of {}",
            status["id"]
        );
        assert_eq!(status["repairs"], json!([]), "repairs of {}", status["id"]);
    }
    let not_run_ids: Vec<&Value> = not_run.iter().map(|status| &status["id"]).collect();
    for event in opened.events_of("sketch_solved") {
        assert!(
            !not_run_ids.contains(&&event["solution"]["feature"]),
            "sketch_solved names the not_run feature {}",
            event["solution"]["feature"]
        );
    }
    for event in opened.events_of("datum_plane") {
        assert!(
            !not_run_ids.contains(&&event["feature"]),
            "datum_plane names the not_run feature {}",
            event["feature"]
        );
    }
}

#[test]
fn t197_a_stop_keeps_the_edits() {
    let (_flag, mut session, changed, mut twin, twin_changed) = stopped_base_change();

    assert_eq!(changed.ok()["revision"], 1);
    assert_eq!(twin_changed.ok()["revision"], 1);
    assert_eq!(
        changed.event("document_changed"),
        twin_changed.event("document_changed")
    );
    let document_changed = changed.event("document_changed");
    assert_eq!(document_changed["dirty"], true);
    assert_eq!(document_changed["undo"], "Change base");
    assert!(document_changed.get("redo").is_none(), "no redo");
    assert_eq!(document_changed["transaction_open"], false);
    assert_eq!(
        get_document(&mut session, 1)["document"],
        get_document(&mut twin, 1)["document"]
    );
    assert!(has_not_run(&changed), "the base change stopped");
}

#[test]
fn t198_undo_after_a_stop_restores_the_opened_document() {
    let flag = CancelFlag::word();
    let mut session = headless_session_with(&flag);
    let mut twin = headless_twin();
    let text = reference_part();
    send_as(&mut session, 3, open_command(&text)).ok();
    let twin_opened = send_as(&mut twin, 3, open_command(&text));
    let opened_text = saved_text(&mut twin, 1);
    let changed = send_canceled_after(
        &mut session,
        &flag,
        4,
        base_change(Some("Change base")),
        Duration::from_millis(30),
    );
    assert!(has_not_run(&changed), "the base change stopped");

    let undone = send_as(&mut session, 5, json!({"type": "undo", "doc": 1}));

    assert_eq!(undone.ok()["label"], "Change base");
    assert!(!has_not_run(&undone), "the undo's recompute finished");
    assert_eq!(
        recomputed_without_revision(&undone),
        recomputed_without_revision(&twin_opened)
    );
    assert_eq!(saved_text(&mut session, 1), opened_text);
}

#[test]
fn t199_rebuild_finishes_the_build() {
    let (_flag, mut session, _changed, _twin, twin_changed) = stopped_base_change();

    let rebuilt = send_as(&mut session, 5, rebuild(1));

    assert_eq!(rebuilt.ok()["type"], "rebuild");
    assert!(
        rebuilt.events_of("document_changed").is_empty(),
        "rebuild emits no document_changed"
    );
    let recomputed = rebuilt.event("recomputed");
    assert_eq!(recomputed["revision"], 1);
    let expected = twin_changed.event("recomputed");
    assert_eq!(recomputed["features"], expected["features"]);
    assert_eq!(recomputed["parameters"], expected["parameters"]);
}

#[test]
fn t200_a_stopped_preview_and_its_canceled_transaction() {
    let (mut session, preview, opened) = stopped_preview();

    first_not_run(&preview);

    let canceled = send_as(
        &mut session,
        6,
        json!({"type": "cancel_transaction", "doc": 1}),
    );

    assert_eq!(canceled.ok()["revision"], 2);
    assert!(!has_not_run(&canceled), "the revert's recompute finished");
    assert!(
        canceled.event("document_changed").get("undo").is_none(),
        "no undo entry"
    );
    assert_eq!(saved_text(&mut session, 1), opened);
}

#[test]
fn t201_a_commit_after_a_stopped_preview() {
    let (mut session, preview, _opened) = stopped_preview();
    first_not_run(&preview);

    let committed = send_as(
        &mut session,
        6,
        json!({"type": "commit_transaction", "doc": 1}),
    );

    assert_eq!(committed.ok()["recorded"], true);
    assert_eq!(committed.event("document_changed")["undo"], "Edit");
    assert!(
        !has_not_run(&committed),
        "the commit's recompute, with no focus, finished"
    );
}

#[test]
fn t202_a_late_cancel_stops_nothing_later() {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    send_as(&mut session, 2, box_height("45 mm", None)).ok();
    flag.cancel(2);

    let changed = send_as(&mut session, 3, box_height("50 mm", None));

    assert!(!has_not_run(&changed), "request 3 is not request 2");
    let mesh = changed.body_mesh("b-1", "model");
    assert_bounding_box(
        &mesh["mesh"]["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 50.0],
    );
}

#[test]
fn t203_a_cancel_before_the_request_stops_it_at_its_first_check() {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    flag.cancel(3);

    let grown = send_as(&mut session, 3, box_height("50 mm", Some("Grow")));

    assert_eq!(grown.ok()["revision"], 1);
    assert_eq!(grown.event("document_changed")["undo"], "Grow");
    for status in statuses(&grown) {
        assert_eq!(state(&status), "not_run", "status of {}", status["id"]);
    }

    let rebuilt = send_as(&mut session, 4, rebuild(1));

    assert_eq!(rebuilt.feature_status("f-box1")["status"], "ok");
    assert_bounding_box(
        &rebuilt.body_mesh("b-1", "model")["mesh"]["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 50.0],
    );
}

#[test]
fn t204_typed_methods_and_request_0_never_stop() {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    let doc = typed_handle(1);
    flag.cancel(5);
    let edits: Vec<Edit> =
        serde_json::from_value(json!([set_height("45 mm")])).expect("the edit deserializes");

    session
        .apply_edits(doc, None, edits)
        .expect("the typed edit applies");

    let typed = session
        .feature_statuses(doc)
        .expect("the document has statuses");
    assert!(
        typed
            .iter()
            .all(|status| status.status != FeatureState::NotRun),
        "a typed method never stops: {typed:?}"
    );

    flag.cancel(0);
    let changed = send_as(&mut session, 0, box_height("50 mm", None));

    assert!(!has_not_run(&changed), "request 0 never stops");
}

#[test]
fn t205_a_stopped_export_makes_no_file() {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    flag.cancel(7);

    let exported = send_as(&mut session, 7, binary_stl_export());

    assert_canceled(&exported, EXPORT_STOPPED);
}

#[test]
fn t206_a_stopped_step_import_adds_nothing() {
    let bytes = box_step_bytes();
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    let opened = get_document(&mut session, 1);
    flag.cancel(7);

    let imported = send_with_buffers_as(
        &mut session,
        7,
        json!({"type": "import_step", "doc": 1, "data": 0}),
        &[Buffer::U8(bytes)],
    );

    assert_canceled(&imported, IMPORT_STOPPED);
    let after = get_document(&mut session, 1);
    assert_eq!(after["document"], opened["document"]);
    assert_eq!(after["revision"], 0);
}

#[test]
fn t207_a_stopped_print_check_reports_no_health() {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    flag.cancel(7);

    let analyzed = send_as(&mut session, 7, print_check());

    assert_canceled(&analyzed, PRINT_STOPPED);
    assert!(analyzed.events_of("print_health").is_empty());
}

#[test]
fn t208_an_export_of_an_incomplete_document_rebuilds_it_first() {
    let (_flag, mut session, _opened, mut twin, _twin_opened) = stopped_open();

    let exported = send_as(&mut session, 4, binary_stl_export());
    let twin_exported = send_as(&mut twin, 4, binary_stl_export());

    assert!(
        exported.events_of("recomputed").iter().all(|event| {
            event["features"]
                .as_array()
                .expect("recomputed lists its features")
                .iter()
                .all(|status| status["status"] != "not_run")
        }),
        "the export's rebuild finished"
    );
    assert!(
        !exported.events_of("recomputed").is_empty(),
        "the export rebuilt the incomplete document first"
    );
    assert!(
        exported_bytes(&exported) == exported_bytes(&twin_exported),
        "the export equals the twin's"
    );
}

#[test]
fn t209_rebuild_of_a_complete_document_changes_nothing() {
    let mut session = common::session();
    let opened = send_as(&mut session, 1, open_command(&fixture("box.ptsm")));

    let rebuilt = send_as(&mut session, 2, rebuild(1));

    assert_eq!(rebuilt.event_types(), ["recomputed"]);
    let recomputed = rebuilt.event("recomputed");
    assert_eq!(recomputed["revision"], 0);
    assert_eq!(
        recomputed["features"],
        opened.event("recomputed")["features"]
    );
    assert_eq!(
        send_as(&mut session, 3, rebuild(9)).error_code(),
        "unknown_document"
    );
}

#[test]
fn t232_a_stopped_rebuild_before_an_export() {
    let (flag, mut session, _opened, _twin, twin_opened) = stopped_open();
    flag.cancel(4);

    let exported = send_as(&mut session, 4, binary_stl_export());

    assert_canceled(&exported, EXPORT_STOPPED);

    let rebuilt = send_as(&mut session, 5, rebuild(1));

    assert!(!has_not_run(&rebuilt), "the rebuild finished");
    let states = |sent: &Sent| -> Vec<(Value, Value)> {
        statuses(sent)
            .into_iter()
            .map(|status| (status["id"].clone(), status["status"].clone()))
            .collect()
    };
    assert_eq!(states(&rebuilt), states(&twin_opened));
}

#[test]
fn a_word_flag_starts_at_0_and_holds_the_last_request_canceled() {
    let flag = CancelFlag::word();
    let clone = flag.clone();
    let mut session = headless_session_with(&clone);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();

    let first = send_as(&mut session, 1, box_height("45 mm", None));
    flag.cancel(2);
    let second = send_as(&mut session, 2, box_height("50 mm", None));

    assert!(!has_not_run(&first), "the word starts at 0");
    assert!(has_not_run(&second), "a clone shares the word");
}

#[test]
fn a_never_flag_stops_nothing() {
    let flag = CancelFlag::never();
    let mut session = headless_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    flag.cancel(2);

    let changed = send_as(&mut session, 2, box_height("50 mm", None));

    assert!(!has_not_run(&changed));
}

#[test]
fn the_default_options_never_stop() {
    let flag = CancelFlag::default();
    let mut session = Session::new(SessionOptions {
        cancel: flag.clone(),
        ..SessionOptions::headless()
    });
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    flag.cancel(2);

    let changed = send_as(&mut session, 2, box_height("50 mm", None));

    assert!(!has_not_run(&changed), "CancelFlag::default() is never()");
}

#[test]
fn a_stopped_export_leaves_the_tessellation_for_the_next_one() {
    let flag = CancelFlag::word();
    let mut session = display_session_with(&flag);
    send_as(&mut session, 1, open_command(&fixture("box.ptsm"))).ok();
    flag.cancel(7);
    assert_canceled(
        &send_as(&mut session, 7, binary_stl_export()),
        EXPORT_STOPPED,
    );

    let exported = send_as(&mut session, 8, binary_stl_export());
    let mut fresh = common::session();
    send_as(&mut fresh, 1, open_command(&fixture("box.ptsm"))).ok();
    let fresh_exported = send_as(&mut fresh, 2, binary_stl_export());

    assert!(
        exported_bytes(&exported) == exported_bytes(&fresh_exported),
        "a stopped tessellation cached no partial mesh"
    );
}
