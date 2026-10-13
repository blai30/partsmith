//! Fixtures shared by the unit tests of the commands.

use partsmith_engine::{Session, SessionOptions};
use partsmith_model::Id;

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
