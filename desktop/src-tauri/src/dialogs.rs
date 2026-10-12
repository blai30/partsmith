//! The native file dialogs over tauri-plugin-dialog (desktop.md 2.4 and 3.3).

use std::path::PathBuf;

use tauri::{Runtime, WebviewWindow, Wry};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

use crate::files::{Dialogs, FileKind};

/// Native dialogs from tauri-plugin-dialog, parented to one window.
///
/// Each pick blocks its thread until the user answers, so callers run it on the blocking pool, never on the main thread.
pub struct TauriDialogs<R: Runtime = Wry> {
    window: WebviewWindow<R>,
}

impl<R: Runtime> TauriDialogs<R> {
    /// Dialogs parented to `window`.
    pub fn new(window: WebviewWindow<R>) -> TauriDialogs<R> {
        TauriDialogs { window }
    }

    /// A file dialog parented to the window with one filter for `kind`.
    fn file_dialog(&self, kind: FileKind) -> FileDialogBuilder<R> {
        self.window
            .dialog()
            .file()
            .set_parent(&self.window)
            .add_filter(kind.description(), kind.extensions())
    }
}

impl<R: Runtime> Dialogs for TauriDialogs<R> {
    fn pick_open(&self) -> Option<PathBuf> {
        self.file_dialog(FileKind::Ptsm)
            .blocking_pick_file()
            .and_then(local_path)
    }

    fn pick_save(&self, suggested_name: &str, kind: FileKind) -> Option<PathBuf> {
        self.file_dialog(kind)
            .set_file_name(suggested_name)
            .blocking_save_file()
            .and_then(local_path)
    }
}

/// The local path of a dialog result; a result that is not one (possible only on mobile platforms) counts as a cancel.
fn local_path(chosen: FilePath) -> Option<PathBuf> {
    chosen.into_path().ok()
}
