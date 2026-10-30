//! Helpers shared by the unit tests of this crate.

use partsmith_model::Id;
use partsmith_protocol::DocHandle;

use crate::{Session, SessionOptions};

/// A test id.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("a valid test id")
}

/// A session with `options` and `text` open, no events collected yet.
pub(crate) fn opened_with(options: SessionOptions, text: &str) -> (Session, DocHandle) {
    let mut session = Session::new(options);
    let doc = session
        .open_document(text, Vec::new())
        .expect("the document opens")
        .doc;
    (session, doc)
}

/// A session with sequential ids and `box.ptsm` open, no events collected yet; it builds display
/// meshes when `display`.
pub(crate) fn box_session(display: bool) -> (Session, DocHandle) {
    opened_with(
        SessionOptions {
            display,
            ..SessionOptions::headless()
        },
        include_str!("../tests/fixtures/box.ptsm"),
    )
}
