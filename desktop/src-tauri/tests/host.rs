//! Acceptance tests for `Host` (desktop.md section 6, tests 1 to 7): one engine session behind the `call` command, serialized calls and the display option.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use common::{
    EXAMPLE_A_VERSION_OFFSET, error_info, example_a, new_document_frame, ok_result, request_frame,
    response_frame,
};
use partsmith_desktop::Host;
use partsmith_engine::{ENGINE_VERSION, SessionOptions};
use partsmith_protocol::{
    Command, CommandResult, DocHandle, ErrorCode, Event, FrameError, ResponseFrame,
};

/// Threads sending requests at once in test 5.
const THREADS: u64 = 8;
/// Requests each thread sends in test 5.
const REQUESTS_PER_THREAD: u64 = 25;

fn headless_host() -> Host {
    Host::new(SessionOptions::headless())
}

/// The `box.ptsm` fixture of engine.md section 3.20.
fn box_document_text() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/partsmith-engine/tests/fixtures/box.ptsm");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {} failed: {error}", path.display()))
}

fn open_box_frame() -> Vec<u8> {
    request_frame(
        1,
        Command::OpenDocument {
            text: box_document_text(),
            assets: Vec::new(),
        },
    )
}

fn has_body_mesh(frame: &ResponseFrame) -> bool {
    frame
        .events
        .iter()
        .any(|event| matches!(event, Event::BodyMesh { .. }))
}

/// The handle of a successful `new_document` response.
fn new_document_handle(frame: &ResponseFrame) -> DocHandle {
    match ok_result(frame) {
        CommandResult::NewDocument { doc } => *doc,
        other => panic!("expected a new_document result, got {other:?}"),
    }
}

#[test]
fn t001_engine_info_reports_the_protocol_and_engine_versions() {
    let frame = response_frame(&headless_host().call(&example_a()));

    assert!(frame.events.is_empty(), "engine_info emits no events");
    assert_eq!(frame.response.id, 1);
    match ok_result(&frame) {
        CommandResult::EngineInfo {
            protocol_version,
            engine_version,
            ..
        } => {
            assert_eq!(*protocol_version, 1);
            assert_eq!(engine_version, ENGINE_VERSION);
        }
        other => panic!("expected an engine_info result, got {other:?}"),
    }
}

#[test]
fn t002_bytes_that_are_not_a_frame_answer_malformed_frame_with_id_0() {
    let frame = response_frame(&headless_host().call(b"hello"));

    assert_eq!(frame.response.id, 0);
    let error = error_info(&frame);
    assert_eq!(error.code, ErrorCode::MalformedFrame);
    // Five bytes are shorter than the 8-byte frame start, which protocol.md 3.2 checks before the magic.
    let expected = FrameError::Truncated {
        length: 5,
        expected: 8,
    }
    .to_string();
    assert_eq!(error.message, expected);
}

#[test]
fn t002_bytes_long_enough_but_without_psm1_answer_the_bad_magic_message() {
    let frame = response_frame(&headless_host().call(b"hello, frame"));

    assert_eq!(frame.response.id, 0);
    let error = error_info(&frame);
    assert_eq!(error.code, ErrorCode::MalformedFrame);
    assert_eq!(error.message, "The frame does not start with PSM1.");
}

#[test]
fn t003_protocol_version_2_answers_version_mismatch_with_the_request_id() {
    let mut request = example_a();
    assert_eq!(
        request[EXAMPLE_A_VERSION_OFFSET], b'1',
        "example A has protocol version 1 at this offset"
    );
    request[EXAMPLE_A_VERSION_OFFSET] = b'2';

    let frame = response_frame(&headless_host().call(&request));

    assert_eq!(frame.response.id, 1);
    assert_eq!(error_info(&frame).code, ErrorCode::ProtocolVersionMismatch);
}

#[test]
fn t004_the_session_keeps_a_new_document_for_the_next_call() {
    let host = headless_host();
    let created = response_frame(&host.call(&new_document_frame(1)));
    let doc = new_document_handle(&created);

    let saved = response_frame(&host.call(&request_frame(2, Command::SaveDocument { doc })));

    assert_eq!(saved.response.id, 2);
    assert!(
        matches!(ok_result(&saved), CommandResult::SaveDocument { .. }),
        "save_document of the new document succeeds"
    );
}

#[test]
fn t005_concurrent_calls_are_serialized_and_each_gets_its_own_response() {
    let host = Arc::new(headless_host());

    let workers: Vec<_> = (0..THREADS)
        .map(|thread_index| {
            let host = Arc::clone(&host);
            thread::spawn(move || {
                (0..REQUESTS_PER_THREAD)
                    .map(|request_index| {
                        let id = thread_index * REQUESTS_PER_THREAD + request_index + 1;
                        let frame = response_frame(&host.call(&new_document_frame(id)));
                        assert_eq!(frame.response.id, id, "the response echoes its request id");
                        new_document_handle(&frame)
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let handles: Vec<u32> = workers
        .into_iter()
        .flat_map(|worker| worker.join().expect("no worker thread panics"))
        .map(|handle| handle.0)
        .collect();

    let total = u32::try_from(THREADS * REQUESTS_PER_THREAD).expect("200 fits in u32");
    assert_eq!(handles.len(), 200);
    let distinct: BTreeSet<u32> = handles.iter().copied().collect();
    assert_eq!(
        distinct,
        (1..=total).collect::<BTreeSet<u32>>(),
        "the handles are 1 to 200, each once"
    );
}

#[test]
fn t006_host_is_send_and_sync() {
    fn assert_send_and_sync<T: Send + Sync>() {}
    assert_send_and_sync::<Host>();
}

#[test]
fn t007_the_default_host_produces_display_meshes() {
    let frame = response_frame(&Host::default().call(&open_box_frame()));

    assert!(
        matches!(ok_result(&frame), CommandResult::OpenDocument { .. }),
        "box.ptsm opens"
    );
    assert!(has_body_mesh(&frame), "a display session sends body_mesh");
}

#[test]
fn t007_a_headless_host_produces_no_display_meshes() {
    let frame = response_frame(&headless_host().call(&open_box_frame()));

    assert!(
        matches!(ok_result(&frame), CommandResult::OpenDocument { .. }),
        "box.ptsm opens"
    );
    assert!(
        !has_body_mesh(&frame),
        "a headless session sends no body_mesh"
    );
}
