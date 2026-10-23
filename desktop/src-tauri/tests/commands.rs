//! Acceptance tests for the command wiring and the capability (desktop.md section 6, tests 8 to 13 and 63).
//!
//! The app is `configure(tauri::test::mock_builder())` with `Arc<Files>` managed and the real configuration and capability compiled in by `tauri::generate_context!(test = true)`, so Tauri's mock runtime enforces the ACL as the app does (2.7). No window system is needed.

use crate::common;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use common::{error_info, example_a, grant_by_open, new_document_frame, ok_result, response_frame};
use partsmith_desktop::{COMMAND_NAMES, Files, Host, Unsaved, configure};
use partsmith_protocol::{CommandResult, DocHandle, ErrorCode};
use serde_json::{Value, json};
use tauri::http::{HeaderMap, HeaderName, HeaderValue};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

/// The header that carries the grant of `write_file` and `write_asset` (2.6); header names are case-insensitive.
const GRANT_HEADER: &str = "partsmith-grant";

/// The header that carries the asset name of `write_asset` (2.6).
const ASSET_HEADER: &str = "partsmith-asset";

/// The message of a grant that is not in the list, or a missing grant header (section 4).
const UNKNOWN_GRANT_MESSAGE: &str =
    "Partsmith has no access to this file. Choose it again in a file dialog.";

/// The desktop app on the mock runtime, managing `files`.
fn app_with(files: Arc<Files>) -> App<MockRuntime> {
    configure(mock_builder())
        .manage(files)
        .build(tauri::generate_context!(test = true))
        .expect("the desktop app builds on the mock runtime")
}

/// The desktop app on the mock runtime with in-memory file grants.
fn app() -> App<MockRuntime> {
    app_with(Arc::new(Files::in_memory()))
}

/// Opens a webview window with `label` on the mock runtime.
fn window(app: &App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::default())
        .build()
        .expect("the mock runtime opens a window")
}

/// Sends one invoke from `window` at the app's own origin and returns what the command answered.
fn invoke(
    window: &WebviewWindow<MockRuntime>,
    command: &str,
    body: InvokeBody,
    headers: HeaderMap,
) -> Result<InvokeResponseBody, Value> {
    // The app's own origin in the mock runtime: Tauri serves it as http://tauri.localhost on Windows.
    let origin = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.to_owned(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().expect("the app origin is a URL"),
            body,
            headers,
            invoke_key: INVOKE_KEY.to_owned(),
        },
    )
}

/// A short description of an invoke answer for failure messages.
fn describe(body: &InvokeResponseBody) -> String {
    match body {
        InvokeResponseBody::Json(text) => format!("the JSON {text}"),
        InvokeResponseBody::Raw(bytes) => format!("{} raw bytes", bytes.len()),
    }
}

/// The bytes of an answer that must be a raw response.
fn raw_answer(answer: Result<InvokeResponseBody, Value>) -> Vec<u8> {
    match answer {
        Ok(InvokeResponseBody::Raw(bytes)) => bytes,
        Ok(body) => panic!("expected a raw response, got {}", describe(&body)),
        Err(rejection) => panic!("expected a raw response, got the rejection {rejection}"),
    }
}

/// The JSON value of an answer that must be a JSON response.
fn json_answer(answer: Result<InvokeResponseBody, Value>) -> Value {
    match answer {
        Ok(InvokeResponseBody::Json(text)) => {
            serde_json::from_str(&text).expect("a JSON response holds JSON")
        }
        Ok(body) => panic!("expected a JSON response, got {}", describe(&body)),
        Err(rejection) => panic!("expected a JSON response, got the rejection {rejection}"),
    }
}

/// The value of an answer that must be a rejection.
fn rejection(answer: Result<InvokeResponseBody, Value>) -> Value {
    match answer {
        Err(rejection) => rejection,
        Ok(body) => panic!("expected a rejection, got {}", describe(&body)),
    }
}

/// The text of a rejection that must be a string, as Tauri's own rejections are.
fn rejection_text(answer: Result<InvokeResponseBody, Value>) -> String {
    match rejection(answer) {
        Value::String(text) => text,
        other => panic!("expected a string rejection, got {other}"),
    }
}

/// Headers carrying `grant` in `Partsmith-Grant`.
fn grant_headers(grant: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static(GRANT_HEADER),
        HeaderValue::from_str(grant).expect("grants are visible ASCII"),
    );
    headers
}

/// A temporary folder holding `part.ptsm` with `hello`, and in-memory grants that include it.
fn granted_part() -> (TempDir, PathBuf, Arc<Files>, String) {
    let folder = tempfile::tempdir().expect("a temporary folder can be created");
    let path = folder.path().join("part.ptsm");
    fs::write(&path, b"hello").expect("the test file can be written");
    let files = Arc::new(Files::in_memory());
    let grant = grant_by_open(&files, &path).grant;
    (folder, path, files, grant)
}

#[test]
fn t008_a_raw_call_answers_with_the_raw_response_frame_of_the_host() {
    let app = app();
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "call",
        InvokeBody::Raw(example_a()),
        HeaderMap::new(),
    );

    assert_eq!(raw_answer(answer), Host::default().call(&example_a()));
}

#[test]
fn t009_a_call_with_a_json_body_answers_with_a_raw_malformed_frame_error() {
    let app = app();
    let main = window(&app, "main");

    let answer = invoke(&main, "call", InvokeBody::Json(json!({})), HeaderMap::new());

    let frame = response_frame(&raw_answer(answer));
    assert_eq!(frame.response.id, 0);
    assert_eq!(error_info(&frame).code, ErrorCode::MalformedFrame);
}

#[test]
fn t010_write_file_with_a_json_body_rejects_with_not_bytes() {
    let (_folder, path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_file",
        InvokeBody::Json(json!({ "bytes": [1, 2, 3] })),
        grant_headers(&grant),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "io", "message": "The file contents must be sent as bytes." })
    );
    assert_eq!(fs::read(&path).expect("the file exists"), b"hello");
}

#[test]
fn t010_write_file_without_a_grant_header_rejects_with_permission() {
    let (_folder, path, files, _grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_file",
        InvokeBody::Raw(b"new".to_vec()),
        HeaderMap::new(),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "permission", "message": UNKNOWN_GRANT_MESSAGE })
    );
    assert_eq!(fs::read(&path).expect("the file exists"), b"hello");
}

#[test]
fn t011_command_names_are_the_eight_commands_in_registration_order() {
    assert_eq!(
        COMMAND_NAMES,
        [
            "call",
            "pick_open_file",
            "pick_save_file",
            "read_file",
            "write_file",
            "read_asset",
            "write_asset",
            "set_unsaved"
        ]
    );
}

#[test]
fn t012_a_call_from_another_window_is_rejected_by_the_acl() {
    let app = app();
    let _main = window(&app, "main");
    let other = window(&app, "other");

    let answer = invoke(
        &other,
        "call",
        InvokeBody::Raw(example_a()),
        HeaderMap::new(),
    );

    let message = rejection_text(answer);
    assert!(
        message.contains("not allowed"),
        "the ACL rejects the call: {message}"
    );
}

#[test]
fn t012_a_call_the_acl_rejects_never_reaches_the_host() {
    let app = app();
    let main = window(&app, "main");
    let other = window(&app, "other");

    rejection(invoke(
        &other,
        "call",
        InvokeBody::Raw(new_document_frame(1)),
        HeaderMap::new(),
    ));
    let answer = invoke(
        &main,
        "call",
        InvokeBody::Raw(new_document_frame(2)),
        HeaderMap::new(),
    );

    let frame = response_frame(&raw_answer(answer));
    assert_eq!(
        ok_result(&frame),
        &CommandResult::NewDocument { doc: DocHandle(1) },
        "the rejected new_document did not open a document"
    );
}

#[test]
fn t013_the_dialog_plugin_commands_are_not_allowed_from_the_main_window() {
    let app = app();
    let main = window(&app, "main");

    for command in ["plugin:dialog|open", "plugin:dialog|save"] {
        let answer = invoke(
            &main,
            command,
            InvokeBody::Json(json!({ "options": {} })),
            HeaderMap::new(),
        );

        let message = rejection_text(answer);
        assert!(
            message.contains("not allowed"),
            "the ACL rejects {command}: {message}"
        );
    }
}

#[test]
fn t013_the_fs_plugin_is_not_registered() {
    let app = app();
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "plugin:fs|read_file",
        InvokeBody::Json(json!({ "path": "/etc/hostname" })),
        HeaderMap::new(),
    );

    rejection(answer);
}

#[test]
fn set_unsaved_keeps_the_value_the_ui_reports() {
    let app = app();
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "set_unsaved",
        InvokeBody::Json(json!({ "unsaved": true })),
        HeaderMap::new(),
    );

    assert_eq!(json_answer(answer), Value::Null);
    assert!(app.state::<Unsaved>().get());
}

#[test]
fn read_file_answers_with_the_raw_bytes_of_a_granted_file() {
    let (_folder, _path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "read_file",
        InvokeBody::Json(json!({ "grant": grant })),
        HeaderMap::new(),
    );

    assert_eq!(raw_answer(answer), b"hello");
}

#[test]
fn read_file_with_an_unknown_grant_rejects_with_permission() {
    let app = app();
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "read_file",
        InvokeBody::Json(json!({ "grant": "0123456789abcdef0123456789abcdef" })),
        HeaderMap::new(),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "permission", "message": UNKNOWN_GRANT_MESSAGE })
    );
}

#[test]
fn write_file_writes_a_raw_body_to_the_file_its_header_grants() {
    let (_folder, path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_file",
        InvokeBody::Raw(b"new".to_vec()),
        grant_headers(&grant),
    );

    assert_eq!(json_answer(answer), Value::Null);
    assert_eq!(fs::read(&path).expect("the file exists"), b"new");
}

#[test]
fn write_file_with_a_grant_header_that_is_not_visible_ascii_rejects_with_permission() {
    let (_folder, path, files, _grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static(GRANT_HEADER),
        HeaderValue::from_bytes(&[0xe9]).expect("HTTP allows opaque header bytes"),
    );

    let answer = invoke(
        &main,
        "write_file",
        InvokeBody::Raw(b"new".to_vec()),
        headers,
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "permission", "message": UNKNOWN_GRANT_MESSAGE })
    );
    assert_eq!(fs::read(&path).expect("the file exists"), b"hello");
}

/// `S` of desktop.md section 6: the BLAKE3 hash of `solid` followed by `.step`.
fn solid_name() -> String {
    format!("{}.step", blake3::hash(b"solid").to_hex())
}

/// Headers carrying `grant` in `Partsmith-Grant` and `name` in `Partsmith-Asset`.
fn asset_headers(grant: &str, name: &str) -> HeaderMap {
    let mut headers = grant_headers(grant);
    headers.insert(
        HeaderName::from_static(ASSET_HEADER),
        HeaderValue::from_str(name).expect("asset names are visible ASCII"),
    );
    headers
}

/// Headers carrying only `name` in `Partsmith-Asset`.
fn asset_header_only(name: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static(ASSET_HEADER),
        HeaderValue::from_str(name).expect("asset names are visible ASCII"),
    );
    headers
}

#[test]
fn t063_read_asset_answers_with_the_raw_bytes_of_the_asset() {
    let (folder, _path, files, grant) = granted_part();
    let assets = folder.path().join("assets");
    fs::create_dir(&assets).expect("the test can create a folder");
    fs::write(assets.join(solid_name()), b"solid").expect("the test asset can be written");
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "read_asset",
        InvokeBody::Json(json!({ "grant": grant, "name": solid_name() })),
        HeaderMap::new(),
    );

    assert_eq!(raw_answer(answer), b"solid");
}

#[test]
fn t063_write_asset_writes_a_raw_body_under_the_asset_name_its_headers_give() {
    let (folder, _path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_asset",
        InvokeBody::Raw(b"solid".to_vec()),
        asset_headers(&grant, &solid_name()),
    );

    assert_eq!(json_answer(answer), Value::Null);
    assert_eq!(
        fs::read(folder.path().join("assets").join(solid_name())).expect("the asset exists"),
        b"solid"
    );
}

#[test]
fn t063_write_asset_with_a_json_body_rejects_with_not_bytes() {
    let (folder, _path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_asset",
        InvokeBody::Json(json!({ "bytes": [115, 111, 108, 105, 100] })),
        asset_headers(&grant, &solid_name()),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "io", "message": "The file contents must be sent as bytes." })
    );
    assert!(!folder.path().join("assets").exists());
}

#[test]
fn t063_write_asset_without_a_grant_header_rejects_with_permission() {
    let (folder, _path, files, _grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_asset",
        InvokeBody::Raw(b"solid".to_vec()),
        asset_header_only(&solid_name()),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "permission", "message": UNKNOWN_GRANT_MESSAGE })
    );
    assert!(!folder.path().join("assets").exists());
}

#[test]
fn t063_write_asset_without_an_asset_header_rejects_with_an_empty_name() {
    let (folder, _path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "write_asset",
        InvokeBody::Raw(b"solid".to_vec()),
        grant_headers(&grant),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "io", "message": "\"\" is not the name of an asset." })
    );
    assert!(!folder.path().join("assets").exists());
}

#[test]
fn t063_read_asset_of_a_missing_asset_rejects_with_not_found() {
    let (_folder, _path, files, grant) = granted_part();
    let app = app_with(files);
    let main = window(&app, "main");

    let answer = invoke(
        &main,
        "read_asset",
        InvokeBody::Json(json!({ "grant": grant, "name": solid_name() })),
        HeaderMap::new(),
    );

    assert_eq!(
        rejection(answer),
        json!({ "reason": "not_found", "message": "assets was not found." })
    );
}
