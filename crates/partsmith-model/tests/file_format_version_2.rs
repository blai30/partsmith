//! Acceptance tests for format version 2 (file-format.md section 6, "Format version 2", tests 54 to 56), with unit tests of the version 2 rules of section 3.9 and the messages that name the version.

mod common;

use common::{E1, E2, E3, E4, E5, E6, E7, E8_OUTPUT, E10, e8_input, with_format_version};
use partsmith_model::{
    Document, FORMAT_VERSION, LoadError, SaveError, StructureError, Units, load, load_bytes,
    migrate_with, save,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Every canonical example of section 3.6 that `load` and `save` reproduce, with its name.
const CANONICAL_EXAMPLES: [(&str, &str); 9] = [
    ("E1", E1),
    ("E2", E2),
    ("E3", E3),
    ("E4", E4),
    ("E5", E5),
    ("E6", E6),
    ("E7", E7),
    ("E8", E8_OUTPUT),
    ("E10", E10),
];

/// The document `text` loads to.
fn load_document(text: &str) -> Document {
    load(text)
        .unwrap_or_else(|error| panic!("the text does not load: {error}"))
        .document
}

/// The canonical text of `document`.
fn save_text(document: &Document) -> String {
    save(document).unwrap_or_else(|error| panic!("the document does not save: {error}"))
}

/// A canonical example written at format version 1.
fn at_version_one(text: &str) -> String {
    with_format_version(text, Some("1"))
}

/// The 1-based numbers of the lines that differ between two texts with the same number of lines.
fn differing_lines(before: &str, after: &str) -> Vec<usize> {
    let before: Vec<&str> = before.lines().collect();
    let after: Vec<&str> = after.lines().collect();
    assert_eq!(
        before.len(),
        after.len(),
        "the texts have different line counts"
    );
    before
        .iter()
        .zip(&after)
        .enumerate()
        .filter(|(_, (old, new))| old != new)
        .map(|(index, _)| index + 1)
        .collect()
}

/// The params of the feature with this id, as a JSON object.
fn params_of(document: &Document, id_text: &str) -> Value {
    let feature = document
        .features
        .iter()
        .find(|feature| feature.id.as_str() == id_text)
        .unwrap_or_else(|| panic!("the document has no feature {id_text}"));
    Value::Object(feature.params.clone())
}

/// Keys of the JSON object at a pointer, in order.
fn keys_at<'a>(value: &'a Value, pointer: &str) -> Vec<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("there is no object at {pointer}"))
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn t054_a_version_1_document_changes_only_its_version() {
    for (name, text) in [("E2", E2), ("E10", E10)] {
        let input = at_version_one(text);

        let loaded = load(&input).unwrap_or_else(|error| panic!("{name} at version 1: {error}"));

        assert_eq!(loaded.migrated_from, Some(1), "{name}");
        assert_eq!(loaded.document, load_document(text), "{name}");
        let saved = save_text(&loaded.document);
        assert_eq!(saved, text, "{name}");
        assert_eq!(differing_lines(&input, &saved), [3], "{name}");
    }
}

#[test]
fn t055_version_2_fields_round_trip_byte_for_byte() {
    let loaded = load(E10).expect("E10 loads");

    assert_eq!(loaded.migrated_from, None);
    let saved = save_text(&loaded.document);
    assert_eq!(saved, E10);
    assert_eq!(saved.len(), 3081);

    let document = &loaded.document;
    assert_eq!(
        keys_at(&params_of(document, "f-ext1"), "/profile/regions/0"),
        ["key", "fp", "outline"]
    );
    assert_eq!(
        keys_at(&params_of(document, "f-sk2"), "/plane"),
        ["kind", "body", "face", "fallback", "fallback_frame"]
    );
    assert_eq!(
        keys_at(&params_of(document, "f-ext2"), "/profile/regions/0"),
        ["key", "fp", "outline"]
    );
    assert_eq!(
        keys_at(&params_of(document, "f-fil1"), "/edges/0"),
        ["q", "fp", "nb"]
    );
    assert_eq!(
        keys_at(&params_of(document, "f-lp1"), "/direction/axis"),
        ["kind", "body", "edge", "direction"]
    );
    assert_eq!(
        keys_at(&params_of(document, "f-dp1"), "/base/face"),
        ["q", "fp", "merged"]
    );
}

#[test]
fn t056_a_build_that_reads_only_version_1_stops_cleanly() {
    let document: Value = serde_json::from_str(E10).expect("E10 is JSON");

    let result = migrate_with(document, 1, 1, &[]);

    assert_eq!(
        result,
        Err(LoadError::NewerFormatVersion {
            found: 2,
            supported: 1
        })
    );
    assert_eq!(
        result.expect_err("version 2 is newer").to_string(),
        "This file uses format version 2, which is newer than this version of Partsmith supports (up to 1). Update Partsmith to open it."
    );
}

#[test]
fn canonical_examples_are_at_version_2_and_load_without_migrating() {
    for (name, text) in CANONICAL_EXAMPLES {
        let loaded = load(text).unwrap_or_else(|error| panic!("{name}: {error}"));

        assert!(
            text.starts_with("{\n  \"format\": \"partsmith\",\n  \"format_version\": 2,\n"),
            "{name}"
        );
        assert_eq!(loaded.migrated_from, None, "{name}");
        assert_eq!(loaded.document.format_version, 2, "{name}");
    }
}

#[test]
fn every_canonical_example_at_version_1_saves_back_at_version_2() {
    for (name, text) in CANONICAL_EXAMPLES {
        let input = at_version_one(text);

        let loaded = load(&input).unwrap_or_else(|error| panic!("{name} at version 1: {error}"));

        assert_eq!(loaded.migrated_from, Some(1), "{name}");
        assert_eq!(loaded.document.format_version, FORMAT_VERSION, "{name}");
        assert_eq!(save_text(&loaded.document), text, "{name}");
    }
}

#[test]
fn a_migrated_document_is_structurally_valid() {
    let document = load_document(&at_version_one(E10));

    assert_eq!(document.check_structure(), Vec::<StructureError>::new());
}

#[test]
fn version_1_bytes_migrate_like_version_1_text() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(at_version_one(E2).as_bytes());

    let loaded = load_bytes(&bytes).expect("E2 at version 1 loads from bytes");

    assert_eq!(loaded.migrated_from, Some(1));
    assert_eq!(save_text(&loaded.document), E2);
}

#[test]
fn the_non_canonical_e8_input_at_version_1_saves_as_e8() {
    let input = e8_input().replacen("\"format_version\":2.0", "\"format_version\":1.0", 1);
    assert_ne!(
        input,
        e8_input(),
        "the E8 input has its version written as 2.0"
    );

    let loaded = load(&input).expect("the E8 input at version 1 loads");

    assert_eq!(loaded.migrated_from, Some(1));
    assert_eq!(save_text(&loaded.document), E8_OUTPUT);
}

#[test]
fn a_new_document_saves_at_version_2() {
    let text = save_text(&Document::new(Units::Mm));

    assert_eq!(text.lines().nth(2), Some("  \"format_version\": 2,"));
}

#[test]
fn a_document_in_memory_at_version_1_cannot_be_saved() {
    let mut document = Document::new(Units::Mm);
    document.format_version = 1;

    let result = save(&document);

    assert_eq!(
        result,
        Err(SaveError::Structure(vec![
            StructureError::WrongFormatVersion { found: 1 }
        ]))
    );
    assert_eq!(
        result
            .expect_err("version 1 is not the version in memory")
            .to_string(),
        "The document cannot be saved: The document is at format version 1, but documents in memory must be at version 2. Load it from text so it is migrated."
    );
}

#[test]
fn the_loader_keeps_version_2_fields_without_checking_them() {
    let text = E10
        .replacen(
            "\"fallback_frame\": \"body\"",
            "\"fallback_frame\": \"sideways\"",
            1,
        )
        .replacen(
            "\"nb\": {\"f-ext1/end-cap#2\": [\"f-ext1/lateral/e-2\", \"f-ext2/lateral/e-6\"]}",
            "\"nb\": 7",
            1,
        );
    assert_eq!(
        differing_lines(E10, &text).len(),
        2,
        "both replacements apply"
    );

    let document = load_document(&text);

    assert_eq!(
        params_of(&document, "f-sk2").pointer("/plane/fallback_frame"),
        Some(&json!("sideways"))
    );
    assert_eq!(
        params_of(&document, "f-fil1").pointer("/edges/0/nb"),
        Some(&json!(7))
    );
    assert_eq!(load_document(&save_text(&document)), document);
}

#[test]
fn version_2_fields_are_not_added_to_a_version_1_document() {
    let saved = save_text(&load_document(&at_version_one(E2)));

    for key in ["nb", "merged", "outline", "fallback_frame", "direction"] {
        assert!(
            !saved.contains(&format!("\"{key}\":")),
            "the saved E2 has {key}"
        );
    }
}

#[test]
fn the_version_2_fields_load_as_feature_params() {
    let document = load_document(E10);

    assert_eq!(
        params_of(&document, "f-ext1").pointer("/profile/regions/0/outline"),
        Some(&json!("e-1,e-2,e-3,e-4"))
    );
    assert_eq!(
        params_of(&document, "f-sk2").pointer("/plane/fallback_frame"),
        Some(&json!("body"))
    );
    assert_eq!(
        params_of(&document, "f-fil1").pointer("/edges/0/nb"),
        Some(&json!({"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}))
    );
    assert_eq!(
        params_of(&document, "f-lp1").pointer("/direction/axis/direction"),
        Some(&json!([1, 0, 0]))
    );
    assert_eq!(
        params_of(&document, "f-dp1").pointer("/base/face/merged"),
        Some(&json!({"f-ext1/start-cap": ["f-ext1/start-cap@f-lp1:*"]}))
    );
}

#[test]
fn version_messages_name_version_2() {
    assert_eq!(
        LoadError::MissingFormatVersion.to_string(),
        "The document has no \"format_version\". Add \"format_version\": 2 after \"format\"."
    );
    assert_eq!(
        LoadError::InvalidFormatVersion {
            found: "\"1\"".to_string()
        }
        .to_string(),
        "\"format_version\" must be a whole number such as 2, not \"1\"."
    );
    assert_eq!(
        StructureError::WrongFormatVersion { found: 1 }.to_string(),
        "The document is at format version 1, but documents in memory must be at version 2. Load it from text so it is migrated."
    );
}
