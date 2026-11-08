//! The engine behind the `call` command, and the flag that stops its running call (desktop.md
//! 2.3, 3.1 and 3.10).

use std::sync::{Mutex, PoisonError};

use partsmith_engine::{CancelFlag, Session, SessionOptions};

/// The engine behind the `call` command: one session, one call at a time.
pub struct Host {
    session: Mutex<Session>,
    /// The session's flag, kept outside the mutex so a stop never waits for the running call.
    cancel: CancelFlag,
}

impl Host {
    /// A host around a new session with these options, whose `cancel` the host replaces with its
    /// own `CancelFlag::word()`.
    pub fn new(options: SessionOptions) -> Host {
        let cancel = CancelFlag::word();
        Host {
            session: Mutex::new(Session::new(SessionOptions {
                cancel: cancel.clone(),
                ..options
            })),
            cancel,
        }
    }

    /// Handles one request frame and returns its response frame. Never panics.
    pub fn call(&self, frame: &[u8]) -> Vec<u8> {
        // Session::call contains its own panics, so a poisoned lock still guards a usable session.
        let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        session.call(frame)
    }

    /// Asks the running call to stop when its request id is `request`. Takes no lock: it stores
    /// the id in the host's flag and returns at once.
    pub fn cancel(&self, request: u64) {
        self.cancel.cancel(request);
    }
}

impl Default for Host {
    /// `Host::new(SessionOptions::default())`: display on, random ids, 1 GiB cache, 1000 undo entries, standard quality (engine.md 2).
    fn default() -> Host {
        Host::new(SessionOptions::default())
    }
}
