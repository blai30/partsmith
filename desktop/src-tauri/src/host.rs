//! The engine behind the `call` command (desktop.md 2.3 and 3.1).

use std::sync::{Mutex, PoisonError};

use partsmith_engine::{Session, SessionOptions};

/// The engine behind the `call` command: one session, one call at a time.
pub struct Host {
    session: Mutex<Session>,
}

impl Host {
    /// A host around a new session with these options.
    pub fn new(options: SessionOptions) -> Host {
        Host {
            session: Mutex::new(Session::new(options)),
        }
    }

    /// Handles one request frame and returns its response frame. Never panics.
    pub fn call(&self, frame: &[u8]) -> Vec<u8> {
        // Session::call contains its own panics, so a poisoned lock still guards a usable session.
        let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        session.call(frame)
    }
}

impl Default for Host {
    /// `Host::new(SessionOptions::default())`: display on, random ids, 1 GiB cache, 1000 undo entries, standard quality (engine.md 2).
    fn default() -> Host {
        Host::new(SessionOptions::default())
    }
}
