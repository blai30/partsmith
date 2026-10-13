//! Helpers shared by the unit tests of this crate.

use partsmith_model::Id;
use partsmith_protocol::DocHandle;

use crate::{Session, SessionOptions};

/// A test id.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("a valid test id")
}

/// A session with sequential ids and `box.ptsm` open, no events collected yet; it builds display
/// meshes when `display`.
pub(crate) fn box_session(display: bool) -> (Session, DocHandle) {
    let mut session = Session::new(SessionOptions {
        display,
        ..SessionOptions::headless()
    });
    let doc = session
        .open_document(include_str!("../tests/fixtures/box.ptsm"), Vec::new())
        .expect("box.ptsm opens")
        .doc;
    (session, doc)
}
