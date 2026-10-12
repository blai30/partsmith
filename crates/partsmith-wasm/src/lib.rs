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
    use partsmith_protocol::{
        Command, ErrorCode, Event, Outcome, Request, ResponseFrame, decode_response_frame,
        encode_request_frame,
    };
    use serde_json::{Value, json};

    use super::*;

    /// A document with one body holding a 10 x 20 x 30 mm box (`examples/box/default.ptsm`).
    const BOX_DOCUMENT: &str = r#"{
  "format": "partsmith",
  "format_version": 1,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}
  ]
}
"#;

    /// Sends request `id` with `command`, a JSON command object, through `call` and decodes the response frame.
    fn send(id: u64, command: Value) -> ResponseFrame {
        let command: Command = serde_json::from_value(command).expect("the test command is valid");
        let request = encode_request_frame(&Request { id, command }, &[])
            .expect("a small request always encodes");
        decode_response_frame(&call(&request)).expect("call always returns a response frame")
    }

    /// The JSON form of a response's outcome.
    fn outcome(frame: &ResponseFrame) -> Value {
        serde_json::to_value(&frame.response.result).expect("an outcome always serializes")
    }

    /// The error code of a response that must have failed.
    fn error_code(frame: &ResponseFrame) -> &ErrorCode {
        match &frame.response.result {
            Outcome::Error(info) => &info.code,
            Outcome::Ok(_) => panic!("expected an error, got {}", outcome(frame)),
        }
    }

    /// Creates an empty document and returns its handle as JSON.
    fn new_document(id: u64) -> Value {
        let created = send(id, json!({"type": "new_document"}));
        outcome(&created)["doc"].clone()
    }

    #[test]
    fn session_options_turn_display_meshes_on() {
        assert!(session_options().display);
    }

    #[test]
    fn engine_info_answers_with_the_request_id() {
        let frame = send(41, json!({"type": "engine_info"}));

        assert_eq!(frame.response.id, 41);
        assert_eq!(outcome(&frame)["type"], "engine_info");
    }

    #[test]
    fn engine_info_reports_this_crate_version_as_the_engine_version() {
        let frame = send(1, json!({"type": "engine_info"}));

        assert_eq!(outcome(&frame)["engine_version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn bytes_that_are_not_a_frame_answer_malformed_frame_with_id_0() {
        let frame = decode_response_frame(&call(b"PSM2")).expect("a response frame");

        assert_eq!(frame.response.id, 0);
        assert_eq!(error_code(&frame), &ErrorCode::MalformedFrame);
    }

    #[test]
    fn an_error_response_frame_has_no_events_or_buffers() {
        let frame = decode_response_frame(&call(&[0; 16])).expect("a response frame");

        assert!(frame.events.is_empty());
        assert!(frame.buffers.is_empty());
    }

    #[test]
    fn a_document_created_by_one_call_is_open_for_the_next() {
        let doc = new_document(1);

        let closed = send(2, json!({"type": "close_document", "doc": doc}));

        assert_eq!(outcome(&closed)["kind"], "ok");
    }

    #[test]
    fn a_closed_document_is_unknown_to_later_calls() {
        let doc = new_document(1);
        send(2, json!({"type": "close_document", "doc": doc}));

        let saved = send(3, json!({"type": "save_document", "doc": doc}));

        assert_eq!(error_code(&saved), &ErrorCode::UnknownDocument);
    }

    #[test]
    fn initializing_again_keeps_the_open_documents() {
        initialize_session();
        let doc = new_document(1);

        initialize_session();
        let saved = send(2, json!({"type": "save_document", "doc": doc}));

        assert_eq!(outcome(&saved)["kind"], "ok");
    }

    #[test]
    fn opening_a_document_with_a_body_sends_its_display_mesh() {
        let opened = send(
            1,
            json!({"type": "open_document", "text": BOX_DOCUMENT, "assets": []}),
        );

        assert_eq!(outcome(&opened)["kind"], "ok", "{}", outcome(&opened));
        assert!(
            opened
                .events
                .iter()
                .any(|event| matches!(event, Event::BodyMesh { mesh: Some(_), .. })),
            "a display session sends body meshes"
        );
        assert!(!opened.buffers.is_empty(), "meshes travel as raw buffers");
    }
}
