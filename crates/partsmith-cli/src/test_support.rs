//! Fixtures shared by the crate's unit tests.

use std::io;

use partsmith_engine::{Session, SessionOptions};
use partsmith_model::Id;
use partsmith_protocol::{FixAction, FixKind, RepairCandidate};
use serde_json::json;

use crate::output::Output;

/// A document whose body id has a space, which the file format rejects at `/bodies/0/id`.
pub const BAD_ID: &str = r#"{"format": "partsmith", "format_version": 1, "units": "mm",
        "parameters": [], "bodies": [{"id": "b 1", "name": "Block"}], "features": []}"#;

/// A valid document without parameters, bodies or features, not in canonical form.
pub const EMPTY: &str = r#"{"format": "partsmith", "format_version": 1, "units": "mm",
        "parameters": [], "bodies": [], "features": []}"#;

/// A headless session, as the binary uses.
pub fn session() -> Session {
    Session::new(SessionOptions::headless())
}

/// The id `text`, which is valid.
pub fn id(text: &str) -> Id {
    Id::new(text).expect("a valid id")
}

/// Runs `write` on an `Output` over memory: what it returned, then the text it wrote to standard
/// output and to standard error.
pub fn capture<T>(write: impl FnOnce(&mut Output<'_>) -> io::Result<T>) -> (T, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut output = Output::new(&mut stdout, &mut stderr);
    let result = write(&mut output).expect("writing to memory succeeds");
    (
        result,
        String::from_utf8(stdout).expect("UTF-8"),
        String::from_utf8(stderr).expect("UTF-8"),
    )
}

/// The "Pick an edge again" action of the fillet `f-fil1` for its first edge.
pub fn pick_edge_again() -> FixAction {
    FixAction {
        label: "Pick an edge again".to_owned(),
        run: FixKind::Pick {
            feature: id("f-fil1"),
            path: "/edges/0".to_owned(),
        },
    }
}

/// A repair candidate of the front edge of the cut made by 'Extrude 1' with this query and
/// relation, as a headless session gives it: without a mesh element.
pub fn candidate(query: &str, relation: &str) -> RepairCandidate {
    serde_json::from_value(json!({
        "reference": {"q": query, "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"},
        "description": "the front edge of the cut made by 'Extrude 1'",
        "relation": relation,
        "meaning": true,
        "distance": 5.0,
        "score": 0.9
    }))
    .expect("a valid candidate")
}
