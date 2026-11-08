//! The host's flag that stops a running request, and the commands that answer `canceled` when
//! one stops (engine.md section 3.26).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use partsmith_features::CancelToken;
use partsmith_io::IoError;
use partsmith_occt_sys::{Interrupt, KernelErrorKind};

use crate::display::DisplayError;
use crate::error::EngineError;

/// A host's way to stop the request a session runs (section 3.26). Clones share the flag.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag {
    /// Where the id of the request to stop is.
    source: Source,
}

/// Where a flag keeps the id of the request to stop.
#[derive(Clone, Debug, Default)]
enum Source {
    /// Nowhere: nothing stops.
    #[default]
    Never,
    /// A word of the host's, shared by the flag's clones.
    Word(Arc<AtomicU64>),
    /// The web page's word, read through partsmith-occt-sys's import.
    #[cfg(target_os = "emscripten")]
    Page,
}

impl CancelFlag {
    /// Stops nothing: the default, which the CLI, Python and most tests keep.
    pub fn never() -> CancelFlag {
        CancelFlag::default()
    }

    /// A word of its own, 0 at first, that `cancel` sets: the desktop host's flag and native
    /// tests'.
    pub fn word() -> CancelFlag {
        CancelFlag {
            source: Source::Word(Arc::new(AtomicU64::new(0))),
        }
    }

    /// The web page's word, which partsmith-occt-sys reads through its JavaScript import (the
    /// web core's `session_options()`).
    #[cfg(target_os = "emscripten")]
    pub fn page() -> CancelFlag {
        CancelFlag {
            source: Source::Page,
        }
    }

    /// Asks the session to stop request `request` while it runs it, or as soon as it starts it.
    /// Never blocks and takes no lock of the session. Does nothing for `never` and `page`.
    pub fn cancel(&self, request: u64) {
        if let Source::Word(word) = &self.source {
            // The word guards no other data.
            word.store(request, Ordering::Relaxed);
        }
    }

    /// The token of request `request`: canceled while the flag holds `request`. Request 0 and
    /// `never` give a token that is never canceled.
    pub(crate) fn token(&self, request: u64) -> CancelToken {
        match &self.source {
            Source::Never => CancelToken::new(),
            Source::Word(word) => {
                CancelToken::from_interrupt(Interrupt::word(Arc::clone(word), request))
            }
            #[cfg(target_os = "emscripten")]
            Source::Page => CancelToken::from_interrupt(Interrupt::page(request)),
        }
    }
}

/// A token that is canceled from the start: what every later check of a request reads once one
/// check found it stopped, whatever the host's flag holds by then.
pub(crate) fn stopped_token() -> CancelToken {
    let token = CancelToken::new();
    token.cancel();
    token
}

/// For tests: a step after which a test stops the request (`Session::stop_after`).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StopPoint {
    /// `import_step` has read its STEP file.
    StepRead,
    /// `analyze_print` has every body's mesh.
    Meshes,
}

/// The commands that fail with `canceled` when they stop (section 3.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stoppable {
    /// `export`.
    Export,
    /// `import_step`, stopped before its edit.
    ImportStep,
    /// `analyze_print`.
    AnalyzePrint,
}

impl Stoppable {
    /// The message of the command's `canceled` error.
    pub(crate) fn message(self) -> &'static str {
        match self {
            Stoppable::Export => "The export was stopped before it finished. No file was made.",
            Stoppable::ImportStep => {
                "The STEP import was stopped before it finished. Nothing was added to the document."
            }
            Stoppable::AnalyzePrint => {
                "The print check was stopped before it finished. Run it again to see its findings."
            }
        }
    }

    /// The command's `canceled` error.
    pub(crate) fn error(self) -> EngineError {
        EngineError::Canceled {
            message: self.message().to_string(),
        }
    }

    /// The command's `canceled` error for a mesh that stopped; the internal error of any other
    /// mesh failure.
    pub(crate) fn mesh_error(self, error: DisplayError) -> EngineError {
        if error.stopped {
            self.error()
        } else {
            error.into()
        }
    }

    /// The command's `canceled` error for a STEP write or read that stopped; `failed` of any
    /// other io error.
    pub(crate) fn io_error(
        self,
        error: IoError,
        failed: impl FnOnce(IoError) -> EngineError,
    ) -> EngineError {
        match &error {
            IoError::Kernel(kernel) if kernel.kind == KernelErrorKind::Canceled => self.error(),
            _ => failed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_flag_stops_only_the_request_it_holds() {
        let flag = CancelFlag::word();
        let token = flag.token(3);

        flag.cancel(2);
        assert!(!token.is_canceled());

        flag.cancel(3);
        assert!(token.is_canceled());
    }

    #[test]
    fn request_0_and_never_stop_nothing() {
        let word = CancelFlag::word();
        let never = CancelFlag::never();

        word.cancel(0);
        never.cancel(1);

        assert!(!word.token(0).is_canceled());
        assert!(!never.token(1).is_canceled());
    }

    #[test]
    fn a_stopped_token_is_canceled() {
        assert!(stopped_token().is_canceled());
        assert!(stopped_token().interrupt().is_requested());
    }
}
