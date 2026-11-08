//! Interrupts: how a running facade call learns that its host asked it to stop (kernel spec
//! sections 2.14 and 3.13).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;

/// The largest request id a double holds exactly, the most the page's cancel word can name
/// (section 3.13 item 7).
#[cfg(target_os = "emscripten")]
const LARGEST_PAGE_REQUEST: u64 = (1 << 53) - 1;

/// How a running facade call learns that its host asked it to stop (section 3.13). Clones share
/// their state. During a call only the shim's progress indicator reads it; the facade passes it
/// on.
#[derive(Clone, Debug)]
pub struct Interrupt {
    /// The word that holds the id of the request to stop.
    word: Arc<AtomicU64>,
    /// The request id it waits for; 0 is never requested.
    request: u64,
    /// True when it also reads the web page's cancel word.
    #[cfg(target_os = "emscripten")]
    page: bool,
}

impl Interrupt {
    /// An interrupt with a word of its own, requested only once `request` is called on it or a
    /// clone.
    pub fn new() -> Interrupt {
        Interrupt::word(Arc::new(AtomicU64::new(0)), 1)
    }

    /// Requested while `word` holds `request` (the desktop host's flag, engine.md
    /// `CancelFlag::word`). Never requested when `request` is 0.
    pub fn word(word: Arc<AtomicU64>, request: u64) -> Interrupt {
        Interrupt {
            word,
            request,
            #[cfg(target_os = "emscripten")]
            page: false,
        }
    }

    /// Requested while the web page's cancel word holds `request`, read through the import
    /// `partsmith_cancel_word`, or once `request` was called on it or a clone. Never requested
    /// when `request` is 0.
    #[cfg(target_os = "emscripten")]
    pub fn page(request: u64) -> Interrupt {
        Interrupt {
            word: Arc::new(AtomicU64::new(0)),
            request,
            page: true,
        }
    }

    /// Makes it requested: stores its request in its word. Does nothing when its request is 0.
    pub fn request(&self) {
        if self.request != 0 {
            self.word.store(self.request, Ordering::Relaxed);
        }
    }

    /// True while it is requested: one atomic load, plus one call of the import for `page`.
    /// Never panics. The load is relaxed: the word guards no other data.
    pub fn is_requested(&self) -> bool {
        self.request != 0
            && (self.word.load(Ordering::Relaxed) == self.request || self.page_holds_request())
    }

    /// True when it reads the page's word and that word holds its request.
    #[cfg(target_os = "emscripten")]
    fn page_holds_request(&self) -> bool {
        // A double holds every request id up to LARGEST_PAGE_REQUEST exactly. The cast and the
        // comparison are exact.
        self.page
            && self.request <= LARGEST_PAGE_REQUEST
            && sys::page_cancel_word() == self.request as f64
    }

    /// Native builds have no page word.
    #[cfg(not(target_os = "emscripten"))]
    fn page_holds_request(&self) -> bool {
        false
    }
}

impl Default for Interrupt {
    /// `Interrupt::new()`.
    fn default() -> Interrupt {
        Interrupt::new()
    }
}

/// For the tests of callers: polls `interrupt` through OCCT's progress range in a C++ loop about
/// once per millisecond. Fails with kind `Canceled` once it is requested, and returns `Ok(())`
/// once `limit` has passed (section 3.13 item 8).
pub fn wait_for_interrupt(interrupt: &Interrupt, limit: Duration) -> Result<(), KernelError> {
    let milliseconds = limit.as_secs_f64() * 1000.0;
    call(Operation::WaitForInterrupt, || {
        sys::wait_for_interrupt(interrupt, milliseconds)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_interrupt_waits_for_request_1() {
        let interrupt = Interrupt::new();

        assert_eq!(interrupt.request, 1);
        assert_eq!(interrupt.word.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn interrupts_made_apart_share_no_word() {
        let first = Interrupt::new();
        let second = Interrupt::new();

        first.request();

        assert!(first.is_requested());
        assert!(!second.is_requested());
    }

    #[test]
    fn a_requested_word_stays_requested_until_it_changes() {
        let word = Arc::new(AtomicU64::new(0));
        let interrupt = Interrupt::word(word.clone(), 9);

        interrupt.request();
        assert!(interrupt.is_requested());
        assert!(interrupt.is_requested());

        word.store(0, Ordering::Relaxed);
        assert!(!interrupt.is_requested());
    }
}
