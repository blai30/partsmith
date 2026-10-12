//! WebAssembly entry point that runs the Partsmith engine in a browser worker (protocol.md section 2.10, ADR 0009).
//!
//! The crate builds the `partsmith_core` Emscripten module. Its runtime stays alive after `main`, which creates the module's one engine [`Session`]. JavaScript then trades protocol frames with that session through three C ABI exports: [`ps_alloc`], [`ps_free`] and [`ps_call`]. The exports only move bytes between WebAssembly memory and Rust; [`call`] is their safe core, which native tests drive directly.
//!
//! `ps_call` has no session parameter, so the session lives in a `thread_local!` cell owned by this host crate, like the `Session` a desktop or Python host owns. The core crates stay free of global state (ADR 0010).

mod exports;

use std::cell::RefCell;

use partsmith_engine::{Session, SessionOptions};

pub use exports::{ps_alloc, ps_call, ps_free};

thread_local! {
    /// The session of the current thread, created on first use. The web module runs on one thread, so this is its one session.
    static SESSION: RefCell<Session> = RefCell::new(Session::new(session_options()));
}

/// Options of the module's session: display meshes on and random ids, as an interactive host needs (protocol.md section 2.10).
fn session_options() -> SessionOptions {
    SessionOptions {
        display: true,
        ..SessionOptions::default()
    }
}

/// Creates the current thread's session if it does not exist yet; an existing session and its documents stay as they are.
///
/// The module's `main` calls this, so the setup cost is paid while the module is instantiated rather than on the first request.
pub fn initialize_session() {
    SESSION.with(|_session| ());
}

/// Handles one request frame with the current thread's session and returns one response frame.
///
/// Never panics: bytes that are not a valid request frame get a response frame with one error response, and a panic inside the engine becomes an `internal_error` response (engine.md section 3.15). This is the safe core of [`ps_call`].
pub fn call(request: &[u8]) -> Vec<u8> {
    SESSION.with_borrow_mut(|session| session.call(request))
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Id, IdKind};

    use super::*;

    #[test]
    fn session_options_turn_display_meshes_on() {
        assert!(session_options().display);
    }

    #[test]
    fn session_options_generate_random_ids() {
        let mut options = session_options();

        let first = options
            .id_generator
            .generate(IdKind::Body, &|_: &Id| false)
            .expect("a random id is free");

        // `SequentialIds` starts at 1; a random 48-bit id is 1 with probability 2^-48.
        assert_ne!(first.as_str(), "b-000000000001");
    }
}
