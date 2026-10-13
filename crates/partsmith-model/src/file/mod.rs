//! Loading and saving `.ptsm` text, versions and migration, and the JSON Schema (file-format.md).

mod migrate;
mod read;
mod schema;

use crate::canonical;
use crate::{Document, StructureError};

pub use migrate::{MIGRATIONS, MigrationStep, migrate, migrate_with};
pub use schema::{FeatureParamsSchema, SchemaError, ptsm_schema, ptsm_schema_text};

/// Value of the `format` field.
pub const FORMAT: &str = "partsmith";
/// Format version this build writes.
pub const FORMAT_VERSION: u32 = 1;
/// Oldest format version this build can migrate from.
pub const OLDEST_FORMAT_VERSION: u32 = 1;
/// File extension, without the dot.
pub const FILE_EXTENSION: &str = "ptsm";

/// The byte order mark, U+FEFF.
const BYTE_ORDER_MARK: char = '\u{feff}';
/// The byte order mark in UTF-8.
const BYTE_ORDER_MARK_BYTES: &[u8] = &[0xEF, 0xBB, 0xBF];

/// A loaded document.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// The document: structurally valid, numbers normalized, at FORMAT_VERSION.
    pub document: Document,
    /// The format version of the text when it was older and had to be migrated.
    pub migrated_from: Option<u32>,
}

/// Parses `.ptsm` text (file-format.md section 3.7).
pub fn load(text: &str) -> Result<Loaded, LoadError> {
    let text = text.strip_prefix(BYTE_ORDER_MARK).unwrap_or(text);
    let value = canonical::parse(text)?;
    let is_partsmith = value
        .get("format")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|format| format == FORMAT);
    if !is_partsmith {
        return Err(LoadError::NotPartsmith);
    }
    let (value, migrated_from) = migrate(value)?;
    let document = read::document(value)?;
    let problems = document.check_structure();
    if !problems.is_empty() {
        return Err(LoadError::Structure(problems));
    }
    Ok(Loaded {
        document,
        migrated_from,
    })
}

/// Checks UTF-8, drops a leading byte order mark, then loads.
pub fn load_bytes(bytes: &[u8]) -> Result<Loaded, LoadError> {
    let (skipped, content) = match bytes.strip_prefix(BYTE_ORDER_MARK_BYTES) {
        Some(content) => (BYTE_ORDER_MARK_BYTES.len(), content),
        None => (0, bytes),
    };
    let text = std::str::from_utf8(content).map_err(|error| LoadError::NotUtf8 {
        offset: skipped + error.valid_up_to(),
    })?;
    load(text)
}

/// Writes canonical `.ptsm` text (file-format.md sections 3.2 to 3.5); refuses structurally invalid documents.
pub fn save(document: &Document) -> Result<String, SaveError> {
    let problems = document.check_structure();
    if !problems.is_empty() {
        return Err(SaveError::Structure(problems));
    }
    // Every number of a document lives in a `serde_json::Value`, which cannot hold NaN or
    // infinity, and every map key is a string. The writer prints every number in its normalized
    // form, so the copy needs no normalization pass.
    let value =
        serde_json::to_value(document).expect("a document serializes to JSON with string keys");
    Ok(canonical::to_string(&value))
}

/// Why `.ptsm` text cannot be loaded (file-format.md section 4.1).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LoadError {
    /// The bytes are not UTF-8.
    #[error(
        "The file is not valid UTF-8 text (the first bad byte is at offset {offset}). Save it as UTF-8."
    )]
    NotUtf8 {
        /// Offset of the first invalid byte.
        offset: usize,
    },
    /// The text is not valid JSON.
    #[error("The file is not valid JSON at line {line}, column {column}: {message}.")]
    Syntax {
        /// 1-based line of the problem.
        line: usize,
        /// Column of the problem.
        column: usize,
        /// The JSON parser's description.
        message: String,
    },
    /// A key appears twice in one object.
    #[error(
        "The key \"{key}\" appears twice in the same object (line {line}). Remove one of them."
    )]
    DuplicateKey {
        /// The repeated key.
        key: String,
        /// 1-based line of the repeat.
        line: usize,
        /// Column of the repeat.
        column: usize,
    },
    /// The root is not an object whose `format` is "partsmith".
    #[error(
        "This is not a Partsmith document: the top level must be an object whose \"format\" is \"partsmith\"."
    )]
    NotPartsmith,
    /// The root has no `format_version`.
    #[error(
        "The document has no \"format_version\". Add \"format_version\": {version} after \"format\".",
        version = FORMAT_VERSION
    )]
    MissingFormatVersion,
    /// `format_version` is not a non-negative whole number.
    #[error(
        "\"format_version\" must be a whole number such as {version}, not {found}.",
        version = FORMAT_VERSION
    )]
    InvalidFormatVersion {
        /// The value's one-line canonical form.
        found: String,
    },
    /// The text is newer than this build supports.
    #[error(
        "This file uses format version {found}, which is newer than this version of Partsmith supports (up to {supported}). Update Partsmith to open it."
    )]
    NewerFormatVersion {
        /// The version of the text.
        found: u64,
        /// The newest version this build reads.
        supported: u32,
    },
    /// The text is older than this build can migrate.
    #[error(
        "Format version {found} is too old to open. The oldest version this Partsmith can open is {oldest}."
    )]
    UnsupportedFormatVersion {
        /// The version of the text.
        found: u64,
        /// The oldest version this build migrates from.
        oldest: u32,
    },
    /// No migration step upgrades from this version.
    #[error(
        "There is no upgrade from format version {from} to {}. This is a bug in Partsmith; please report it.",
        u64::from(*.from) + 1
    )]
    MissingMigration {
        /// The version without a step.
        from: u32,
    },
    /// A migration step failed.
    #[error("Could not upgrade the file from format version {from} to {to}: {message}")]
    Migration {
        /// The version the step upgrades from.
        from: u32,
        /// The version it upgrades to.
        to: u32,
        /// The step's message.
        message: String,
    },
    /// A field has the wrong type or value, is missing, or is unknown.
    #[error("{}", invalid_message(.path, .message))]
    Invalid {
        /// Where the problem is: object keys joined by `.`, array indices as `[i]`, the root as "".
        path: String,
        /// What is wrong.
        message: String,
    },
    /// The document breaks structural rules (model.md section 3.6).
    #[error("{}", load_structure_message(.0))]
    Structure(Vec<StructureError>),
}

/// Why a document cannot be saved (file-format.md section 4.2).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SaveError {
    /// The document breaks structural rules (model.md section 3.6).
    #[error("{}", save_structure_message(.0))]
    Structure(Vec<StructureError>),
}

/// `{path}: {message}`, or `{message}` alone at the root.
fn invalid_message(path: &str, message: &str) -> String {
    if path.is_empty() {
        message.to_string()
    } else {
        format!("{path}: {message}")
    }
}

/// The load message for structural problems: one problem inline, several as a list.
fn load_structure_message(problems: &[StructureError]) -> String {
    match problems {
        [problem] => format!("This document has a problem: {problem}"),
        _ => format!(
            "This document has {} problems:{}",
            problems.len(),
            problem_list(problems)
        ),
    }
}

/// The save message for structural problems: one problem inline, several as a list.
fn save_structure_message(problems: &[StructureError]) -> String {
    match problems {
        [problem] => format!("The document cannot be saved: {problem}"),
        _ => format!(
            "The document cannot be saved because of {} problems:{}",
            problems.len(),
            problem_list(problems)
        ),
    }
}

/// Each problem on its own line, after a line feed and `- `.
fn problem_list(problems: &[StructureError]) -> String {
    problems
        .iter()
        .map(|problem| format!("\n- {problem}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Id;

    fn duplicate(text: &str) -> StructureError {
        StructureError::DuplicateId {
            id: Id::new(text).expect("valid id"),
        }
    }

    #[test]
    fn save_writes_numbers_in_normalized_form() {
        let mut record = crate::FeatureRecord::new(Id::new("f-1").expect("valid id"), "box");
        record.params = match serde_json::json!({"a": 80.0, "b": -0.0, "c": [2.0, 0.5]}) {
            serde_json::Value::Object(params) => params,
            other => panic!("expected an object, got {other}"),
        };
        let document = Document {
            features: vec![record],
            ..Document::default()
        };

        let text = save(&document).expect("the document saves");

        assert!(
            text.contains(r#"{"id": "f-1", "type": "box", "a": 80, "b": 0, "c": [2, 0.5]}"#),
            "{text}"
        );
    }

    #[test]
    fn invalid_at_the_root_is_the_message_alone() {
        let error = LoadError::Invalid {
            path: String::new(),
            message: "missing field `units`".to_string(),
        };

        assert_eq!(error.to_string(), "missing field `units`");
    }

    #[test]
    fn invalid_message_leads_with_the_path() {
        let error = LoadError::Invalid {
            path: "bodies[0].color".to_string(),
            message: "bad".to_string(),
        };

        assert_eq!(error.to_string(), "bodies[0].color: bad");
    }

    #[test]
    fn save_messages_for_one_and_several_problems() {
        assert_eq!(
            SaveError::Structure(vec![duplicate("a")]).to_string(),
            "The document cannot be saved: The id a is used by more than one record. Give the later record a new id."
        );
        assert_eq!(
            SaveError::Structure(vec![duplicate("a"), duplicate("b")]).to_string(),
            "The document cannot be saved because of 2 problems:\n- The id a is used by more than one record. Give the later record a new id.\n- The id b is used by more than one record. Give the later record a new id."
        );
    }

    #[test]
    fn missing_migration_names_the_next_version() {
        assert_eq!(
            LoadError::MissingMigration { from: 2 }.to_string(),
            "There is no upgrade from format version 2 to 3. This is a bug in Partsmith; please report it."
        );
    }

    #[test]
    fn invalid_utf8_after_a_byte_order_mark_counts_the_mark() {
        assert_eq!(
            load_bytes(&[0xEF, 0xBB, 0xBF, b'{', 0xC3]),
            Err(LoadError::NotUtf8 { offset: 4 })
        );
    }

    #[test]
    fn non_object_roots_are_not_partsmith() {
        assert_eq!(load("\"partsmith\""), Err(LoadError::NotPartsmith));
    }

    #[test]
    fn missing_format_version_suggests_the_current_version() {
        let message = LoadError::MissingFormatVersion.to_string();

        assert!(
            message.contains(&format!("\"format_version\": {FORMAT_VERSION} after")),
            "{message}"
        );
    }

    #[test]
    fn invalid_format_version_suggests_the_current_version() {
        let error = LoadError::InvalidFormatVersion {
            found: "\"1\"".to_string(),
        };

        assert!(
            error
                .to_string()
                .contains(&format!("such as {FORMAT_VERSION}, not")),
            "{error}"
        );
    }
}
