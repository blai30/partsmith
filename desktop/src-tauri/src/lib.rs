//! Tauri 2 desktop host for Partsmith: one engine session behind the `call` command, native file dialogs, granted file access and the close guard (`docs/spec/desktop.md`).

mod close_guard;
mod command_names;
mod commands;
mod dialogs;
mod files;
mod host;

use std::sync::Arc;

use tauri::{App, Manager, Runtime};

pub use close_guard::Unsaved;
pub use command_names::COMMAND_NAMES;
pub use dialogs::TauriDialogs;
pub use files::{Dialogs, FileError, FileKind, Files, Granted, MAX_GRANTS, grant_of};
pub use host::Host;

/// The file in the app data directory that keeps the file grants across restarts.
const GRANT_STORE: &str = "file-grants.json";

/// Adds the dialog plugin, `Arc<Host>` (`Host::default()`), `Unsaved` and the six commands to a builder.
/// `run` and the command tests share it; the caller manages `Arc<Files>`.
pub fn configure<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(Host::default()))
        .manage(Unsaved::default())
        .invoke_handler(tauri::generate_handler![
            commands::call,
            commands::pick_open_file,
            commands::pick_save_file,
            commands::read_file,
            commands::write_file,
            commands::set_unsaved,
        ])
}

/// Builds and runs the desktop app; returns when the app exits.
pub fn run() -> Result<(), tauri::Error> {
    configure(tauri::Builder::default())
        .setup(|app| {
            app.manage(Arc::new(files_of(app)));
            Ok(())
        })
        .on_window_event(close_guard::on_window_event)
        .run(tauri::generate_context!())
}

/// The file grants kept in the app data directory, or in memory when that directory cannot be resolved.
fn files_of<R: Runtime>(app: &App<R>) -> Files {
    match app.path().app_data_dir() {
        Ok(folder) => Files::load(folder.join(GRANT_STORE)),
        Err(error) => {
            eprintln!("partsmith-desktop: file grants will not be kept after this session: {error}");
            Files::in_memory()
        }
    }
}
