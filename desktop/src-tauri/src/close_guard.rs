//! The close guard: asks before a window with unsaved changes closes (desktop.md 2.5 and 3.6).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Manager, Runtime, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

/// The label of the window the close guard protects (`tauri.conf.json`).
const MAIN_WINDOW: &str = "main";

/// The title of the confirmation shown when a window with unsaved changes is closed.
pub const CLOSE_TITLE: &str = "Unsaved changes";
/// The message of the confirmation.
pub const CLOSE_MESSAGE: &str =
    "Close Partsmith without saving? Changes since your last save may be lost.";
/// The button that closes the window anyway.
pub const CLOSE_CONFIRM: &str = "Close without saving";
/// The button that keeps the window open.
pub const CLOSE_CANCEL: &str = "Cancel";

/// Whether the UI reported unsaved changes (the last `set_unsaved` value; false at start).
#[derive(Debug, Default)]
pub struct Unsaved {
    unsaved: AtomicBool,
}

impl Unsaved {
    /// Keeps the value the UI reported.
    pub fn set(&self, unsaved: bool) {
        self.unsaved.store(unsaved, Ordering::Relaxed);
    }

    /// The last value the UI reported.
    pub fn get(&self) -> bool {
        self.unsaved.load(Ordering::Relaxed)
    }
}

/// Keeps the main window open while it has unsaved changes and asks the user whether to close it.
pub(crate) fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && window.label() == MAIN_WINDOW
        && window.state::<Unsaved>().get()
    {
        api.prevent_close();
        confirm_close(window.clone());
    }
}

/// Shows the confirmation without blocking: this runs on the main thread, where a blocking dialog would deadlock.
fn confirm_close<R: Runtime>(window: Window<R>) {
    let confirmation = window
        .dialog()
        .message(CLOSE_MESSAGE)
        .title(CLOSE_TITLE)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            CLOSE_CONFIRM.to_owned(),
            CLOSE_CANCEL.to_owned(),
        ))
        .parent(&window);
    confirmation.show(move |confirmed| {
        // Destroying closes the window without another CloseRequested event.
        if confirmed && let Err(error) = window.destroy() {
            eprintln!("partsmith-desktop: could not close the window: {error}");
        }
    });
}
