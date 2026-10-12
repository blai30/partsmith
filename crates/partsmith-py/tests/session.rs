//! Acceptance tests for the `partsmith._core.Session` class (python-api.md section 2.2 and section 6, tests 73 and 74), driven through an embedded Python interpreter.
//!
//! The rest of section 6 exercises the pure-Python package and lives in `python/tests/` (pytest), as the spec places it.

use _core::PySession;
use partsmith_protocol::decode_response_frame;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde_json::{Value, json};

/// Example A of protocol.md section 2.2: request id 1, `engine_info`, no buffers. 120 bytes, copied from the spec's hex dump.
const EXAMPLE_A_HEX: &str = "
    50 53 4d 31 6b 00 00 00 7b 22 70 72 6f 74 6f 63
    6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
    65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
    22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
    3a 31 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
    79 70 65 22 3a 22 65 6e 67 69 6e 65 5f 69 6e 66
    6f 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73 22 3a
    5b 5d 7d 00 00 00 00 00
";

/// Calls per thread in test 74.
const CALLS_PER_THREAD: u64 = 50;

/// The bytes of a whitespace-separated hex dump.
fn hex_bytes(dump: &str) -> Vec<u8> {
    dump.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).expect("the dump holds hex byte pairs"))
        .collect()
}

/// Example A as bytes.
fn example_a() -> Vec<u8> {
    let bytes = hex_bytes(EXAMPLE_A_HEX);
    assert_eq!(bytes.len(), 120, "example A is 120 bytes");
    bytes
}

/// A request frame (protocol.md section 2.2) holding one `engine_info` request with `id` and no buffers.
fn engine_info_frame(id: u64) -> Vec<u8> {
    let header = format!(
        r#"{{"protocol_version":1,"messages":[{{"kind":"request","id":{id},"command":{{"type":"engine_info"}}}}],"buffers":[]}}"#
    );
    let header_length = u32::try_from(header.len()).expect("the header is short");
    let padding = (8 - header.len() % 8) % 8;
    let mut frame = b"PSM1".to_vec();
    frame.extend_from_slice(&header_length.to_le_bytes());
    frame.extend_from_slice(header.as_bytes());
    frame.extend(std::iter::repeat_n(0_u8, padding));
    frame
}

/// The response of a response frame as JSON (`{"id", "result"}`), after the protocol decoder accepted the frame.
fn response_json(frame: &[u8]) -> Value {
    let decoded = decode_response_frame(frame).expect("the session returns a valid response frame");
    serde_json::to_value(&decoded.response).expect("a response serializes to JSON")
}

/// A new `Session()` created through Python with its default options.
fn new_session(py: Python<'_>) -> Bound<'_, PyAny> {
    py.get_type::<PySession>()
        .call0()
        .expect("Session() takes no required arguments")
}

/// `session.call(frame)` from Python, returning the response bytes.
fn call(session: &Bound<'_, PyAny>, frame: &[u8]) -> Vec<u8> {
    let response = session
        .call_method1("call", (PyBytes::new(session.py(), frame),))
        .expect("call answers every bytes frame without raising");
    response
        .cast::<PyBytes>()
        .expect("call returns bytes")
        .as_bytes()
        .to_vec()
}

#[test]
fn t073_example_a_returns_an_engine_info_response_with_id_1() {
    Python::initialize();
    let response = Python::attach(|py| call(&new_session(py), &example_a()));

    let response = response_json(&response);
    assert_eq!(response["id"], json!(1));
    assert_eq!(response["result"]["kind"], json!("ok"));
    assert_eq!(response["result"]["type"], json!("engine_info"));
}

#[test]
fn t073_garbage_bytes_return_a_malformed_frame_error_without_raising() {
    Python::initialize();
    let response = Python::attach(|py| call(&new_session(py), b"hello"));

    let response = response_json(&response);
    assert_eq!(response["result"]["kind"], json!("error"));
    assert_eq!(response["result"]["code"], json!("malformed_frame"));
}

#[test]
fn t073_a_str_frame_raises_type_error() {
    Python::initialize();
    Python::attach(|py| {
        let session = new_session(py);

        let error = session
            .call_method1("call", ("text",))
            .expect_err("a str is not a frame");

        assert!(
            error.is_instance_of::<PyTypeError>(py),
            "expected TypeError, got {error}"
        );
    });
}

#[test]
fn t074_two_threads_share_one_session() {
    Python::initialize();
    let session: Py<PyAny> = Python::attach(|py| new_session(py).unbind());

    let responses: Vec<Vec<(u64, Value)>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..2_u64)
            .map(|thread_index| {
                let session = &session;
                scope.spawn(move || {
                    (1..=CALLS_PER_THREAD)
                        .map(|call_index| {
                            let id = thread_index * 1000 + call_index;
                            let frame = engine_info_frame(id);
                            let response =
                                Python::attach(|py| call(session.bind(py), &frame));
                            (id, response_json(&response))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("a worker thread finished"))
            .collect()
    });

    assert_eq!(responses.len(), 2);
    for thread_responses in &responses {
        assert_eq!(thread_responses.len(), 50);
        for (id, response) in thread_responses {
            assert_eq!(response["id"], json!(id));
            assert_eq!(response["result"]["kind"], json!("ok"));
            assert_eq!(response["result"]["type"], json!("engine_info"));
        }
    }
}
