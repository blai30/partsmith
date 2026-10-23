//! The eight host commands the webview may invoke (desktop.md 2.6, 3.1 and 3.4).

use std::sync::Arc;

use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Runtime, State, WebviewWindow};

use crate::close_guard::Unsaved;
use crate::dialogs::TauriDialogs;
use crate::files::{FileError, FileKind, Files, Granted};
use crate::host::Host;

/// The header that carries the grant of `write_file` and `write_asset`; header names are case-insensitive, and the UI sends `Partsmith-Grant`.
const GRANT_HEADER: &str = "partsmith-grant";

/// The header that carries the asset name of `write_asset`; the UI sends `Partsmith-Asset`.
const ASSET_HEADER: &str = "partsmith-asset";

/// Forwards one request frame to the engine and answers with its response frame as raw bytes.
#[tauri::command]
pub(crate) async fn call(
    request: Request<'_>,
    host: State<'_, Arc<Host>>,
) -> Result<Response, String> {
    // A JSON body is passed on as its text, which the engine answers with a malformed_frame error.
    let frame = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(value) => value.to_string().into_bytes(),
    };
    let host = Arc::clone(&host);
    tauri::async_runtime::spawn_blocking(move || host.call(&frame))
        .await
        .map(Response::new)
        .map_err(|error| format!("The geometry core failed: {error}"))
}

/// Asks the user for a Partsmith model to open and grants it.
#[tauri::command]
pub(crate) async fn pick_open_file<R: Runtime>(
    window: WebviewWindow<R>,
    files: State<'_, Arc<Files>>,
) -> Result<Option<Granted>, FileError> {
    let files = Arc::clone(&files);
    on_blocking_pool(move || files.pick_open(&TauriDialogs::new(window))).await
}

/// Asks the user where to save a file of `kind` and grants it.
#[tauri::command]
pub(crate) async fn pick_save_file<R: Runtime>(
    window: WebviewWindow<R>,
    files: State<'_, Arc<Files>>,
    suggested_name: String,
    kind: FileKind,
) -> Result<Option<Granted>, FileError> {
    let files = Arc::clone(&files);
    on_blocking_pool(move || files.pick_save(&TauriDialogs::new(window), &suggested_name, kind))
        .await
}

/// Answers with the bytes of a granted file.
#[tauri::command]
pub(crate) async fn read_file(
    files: State<'_, Arc<Files>>,
    grant: String,
) -> Result<Response, FileError> {
    let files = Arc::clone(&files);
    on_blocking_pool(move || files.read(&grant))
        .await
        .map(Response::new)
}

/// Replaces the contents of the file the `Partsmith-Grant` header grants with the raw body.
#[tauri::command]
pub(crate) async fn write_file(
    request: Request<'_>,
    files: State<'_, Arc<Files>>,
) -> Result<(), FileError> {
    let grant = header(&request, GRANT_HEADER).ok_or(FileError::UnknownGrant)?;
    let bytes = raw_body(&request)?;
    let files = Arc::clone(&files);
    on_blocking_pool(move || files.write(&grant, &bytes)).await
}

/// Answers with the bytes of an asset next to a granted document.
#[tauri::command]
pub(crate) async fn read_asset(
    files: State<'_, Arc<Files>>,
    grant: String,
    name: String,
) -> Result<Response, FileError> {
    let files = Arc::clone(&files);
    on_blocking_pool(move || files.read_asset(&grant, &name))
        .await
        .map(Response::new)
}

/// Writes the raw body as the asset the `Partsmith-Asset` header names, next to the document the `Partsmith-Grant` header grants.
#[tauri::command]
pub(crate) async fn write_asset(
    request: Request<'_>,
    files: State<'_, Arc<Files>>,
) -> Result<(), FileError> {
    let grant = header(&request, GRANT_HEADER).ok_or(FileError::UnknownGrant)?;
    let name = header(&request, ASSET_HEADER).ok_or_else(|| FileError::NotAnAssetName {
        name: String::new(),
    })?;
    let bytes = raw_body(&request)?;
    let files = Arc::clone(&files);
    on_blocking_pool(move || files.write_asset(&grant, &name, &bytes)).await
}

/// Keeps whether the UI has unsaved changes, for the close guard.
#[tauri::command]
pub(crate) fn set_unsaved(unsaved: bool, state: State<'_, Unsaved>) {
    state.set(unsaved);
}

/// A request header's value; None when it is missing or not visible ASCII.
fn header(request: &Request<'_>, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// The bytes of a raw request body; a JSON body gives `NotBytes`.
fn raw_body(request: &Request<'_>) -> Result<Vec<u8>, FileError> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(_) => Err(FileError::NotBytes),
    }
}

/// Runs file work on Tauri's blocking pool, so a dialog or a slow disk never blocks the event loop.
async fn on_blocking_pool<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, FileError> + Send + 'static,
) -> Result<T, FileError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| FileError::Io {
            detail: error.to_string(),
        })?
}
