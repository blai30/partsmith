//! Helpers shared by the acceptance tests of `docs/spec/desktop.md` section 6: protocol frames, a fake for the native dialogs, and file grants made through it.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use partsmith_desktop::{Dialogs, FileError, FileKind, Files, Granted};
use partsmith_protocol::{
    Command, CommandResult, ErrorInfo, Outcome, Request, ResponseFrame, decode_response_frame,
    encode_request_frame,
};

/// Example A of protocol.md section 2.2: request id 1, `engine_info`, no buffers. 120 bytes, copied from the spec's hex dump.
pub const EXAMPLE_A_HEX: &str = "
    50 53 4d 31 6b 00 00 00 7b 22 70 72 6f 74 6f 63
    6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
    65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
    22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
    3a 31 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
    79 70 65 22 3a 22 65 6e 67 69 6e 65 5f 69 6e 66
    6f 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73 22 3a
    5b 5d 7d 00 00 00 00 00
";

/// Offset of the digit of `"protocol_version":1` in example A: 8 bytes of magic and header length, then 20 bytes of `{"protocol_version":`.
pub const EXAMPLE_A_VERSION_OFFSET: usize = 28;

/// The bytes of example A.
pub fn example_a() -> Vec<u8> {
    EXAMPLE_A_HEX
        .split_whitespace()
        .map(|pair| {
            u8::from_str_radix(pair, 16).unwrap_or_else(|_| panic!("{pair:?} is not a hex byte"))
        })
        .collect()
}

/// A request frame without buffers for `command` with request `id`, built with the protocol encoder.
pub fn request_frame(id: u64, command: Command) -> Vec<u8> {
    encode_request_frame(&Request { id, command }, &[])
        .expect("a request without buffers always encodes")
}

/// A `new_document` request frame with default units and no printer.
pub fn new_document_frame(id: u64) -> Vec<u8> {
    request_frame(
        id,
        Command::NewDocument {
            units: None,
            printer: None,
        },
    )
}

/// Decodes bytes the host returned as a response frame, failing the test with the decoder's message when they are not one.
pub fn response_frame(bytes: &[u8]) -> ResponseFrame {
    decode_response_frame(bytes)
        .unwrap_or_else(|error| panic!("the host returned a valid response frame: {error}"))
}

/// The result of a response that must have succeeded.
pub fn ok_result(frame: &ResponseFrame) -> &CommandResult {
    match &frame.response.result {
        Outcome::Ok(result) => result,
        Outcome::Error(info) => panic!("expected a successful response, got {info:?}"),
    }
}

/// The error of a response that must have failed.
pub fn error_info(frame: &ResponseFrame) -> &ErrorInfo {
    match &frame.response.result {
        Outcome::Error(info) => info,
        Outcome::Ok(result) => panic!("expected an error response, got {result:?}"),
    }
}

/// One question the host asked through the native dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogCall {
    /// `Dialogs::pick_open`.
    Open,
    /// `Dialogs::pick_save` with its arguments.
    Save {
        /// The proposed file name.
        suggested_name: String,
        /// The file kind of the filter.
        kind: FileKind,
    },
}

/// Native dialogs for tests: each pick returns the next preset answer, and every call is recorded.
pub struct FakeDialogs {
    answers: RefCell<VecDeque<Option<PathBuf>>>,
    calls: RefCell<Vec<DialogCall>>,
}

impl FakeDialogs {
    /// Dialogs that answer the picks with `answers` in order; a pick beyond them fails the test.
    pub fn answering(answers: impl IntoIterator<Item = Option<PathBuf>>) -> FakeDialogs {
        FakeDialogs {
            answers: RefCell::new(answers.into_iter().collect()),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// Dialogs where the user chooses `path` once.
    pub fn choosing(path: &Path) -> FakeDialogs {
        FakeDialogs::answering([Some(path.to_path_buf())])
    }

    /// Dialogs where the user cancels once.
    pub fn canceling() -> FakeDialogs {
        FakeDialogs::answering([None])
    }

    /// The calls made so far, in order.
    pub fn calls(&self) -> Vec<DialogCall> {
        self.calls.borrow().clone()
    }

    fn next_answer(&self) -> Option<PathBuf> {
        self.answers
            .borrow_mut()
            .pop_front()
            .expect("the test preset an answer for every dialog the host shows")
    }
}

impl Dialogs for FakeDialogs {
    fn pick_open(&self) -> Option<PathBuf> {
        self.calls.borrow_mut().push(DialogCall::Open);
        self.next_answer()
    }

    fn pick_save(&self, suggested_name: &str, kind: FileKind) -> Option<PathBuf> {
        self.calls.borrow_mut().push(DialogCall::Save {
            suggested_name: suggested_name.to_owned(),
            kind,
        });
        self.next_answer()
    }
}

/// The UTF-8 text of a test path, which the temporary folders always have.
pub fn path_text(path: &Path) -> &str {
    path.to_str().expect("temporary test paths are valid UTF-8")
}

/// Grants `path` as if the user had chosen it in the open dialog.
pub fn grant_by_open(files: &Files, path: &Path) -> Granted {
    files
        .pick_open(&FakeDialogs::choosing(path))
        .expect("picking a valid UTF-8 path succeeds")
        .expect("the fake dialog chose a path")
}

/// Asks the save dialog for a file of `kind` and answers it with `path`.
pub fn pick_save_path(
    files: &Files,
    path: &Path,
    kind: FileKind,
) -> Result<Option<Granted>, FileError> {
    files.pick_save(&FakeDialogs::choosing(path), "suggested.ptsm", kind)
}
