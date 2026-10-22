//! Acceptance tests for loading `.ptsm` text (file-format.md section 6, "Loading", tests 14 to 31).

use crate::common;

use common::{E1, id, load_document, save_text, with_format_version};
use partsmith_model::{
    Document, Expr, LoadError, NameError, StructureError, Units, load, load_bytes,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A document whose keys are in reverse declaration order at every level.
const SHUFFLED: &str = r##"{
  "rollback": "f-1",
  "features": [
    {"op": "new", "body": "b-1", "suppressed": true, "type": "extrude", "name": "Pad", "id": "f-1"}
  ],
  "bodies": [
    {"role": "support_blocker", "visible": false, "color": "#ff0000", "material": "PLA", "name": "Base", "id": "b-1"}
  ],
  "parameters": [
    {"comment": "Outer wall", "expr": "2 mm", "name": "wall", "id": "p-wall"}
  ],
  "printer": "generic-fdm",
  "units": "mm",
  "format_version": 2,
  "format": "partsmith"
}
"##;

/// `SHUFFLED` after loading and saving: keys in declaration order, params in file order.
const SHUFFLED_SAVED: &str = r##"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "printer": "generic-fdm",
  "parameters": [
    {"id": "p-wall", "name": "wall", "expr": "2 mm", "comment": "Outer wall"}
  ],
  "bodies": [
    {"id": "b-1", "name": "Base", "material": "PLA", "color": "#ff0000", "visible": false, "role": "support_blocker"}
  ],
  "features": [
    {"id": "f-1", "type": "extrude", "name": "Pad", "suppressed": true, "op": "new", "body": "b-1"}
  ],
  "rollback": "f-1"
}
"##;

/// E1 with its `"<name>": []` member replaced by `"<name>": <json>`.
fn e1_with_list(name: &str, json: &str) -> String {
    let empty = format!("\"{name}\": []");
    assert!(E1.contains(&empty), "E1 has no empty {name} list");
    E1.replace(&empty, &format!("\"{name}\": {json}"))
}

/// E1 with one feature whose params are `params_json` (a JSON object body without braces).
fn e1_with_feature_params(params_json: &str) -> String {
    e1_with_list(
        "features",
        &format!("[{{\"id\": \"f-1\", \"type\": \"box\", {params_json}}}]"),
    )
}

#[test]
fn t014_keys_are_written_in_declaration_order() {
    let document = load_document(SHUFFLED);

    assert_eq!(save_text(&document), SHUFFLED_SAVED);
}

#[test]
fn t015_missing_lists_are_empty() {
    let document = load_document(r#"{"format": "partsmith", "format_version": 2, "units": "mm"}"#);

    assert!(document.parameters.is_empty());
    assert!(document.bodies.is_empty());
    assert!(document.features.is_empty());
    assert_eq!(save_text(&document), E1);
}

#[test]
fn t016_a_byte_order_mark_is_dropped() {
    let with_mark = format!("\u{feff}{E1}");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(E1.as_bytes());

    let from_text = load(&with_mark).expect("text with a byte order mark loads");
    let from_bytes = load_bytes(&bytes).expect("bytes with a byte order mark load");

    assert_eq!(from_text.document, Document::new(Units::Mm));
    assert_eq!(from_bytes.document, Document::new(Units::Mm));
    assert!(!save_text(&from_text.document).starts_with('\u{feff}'));
}

#[test]
fn t017_invalid_utf8_is_reported_with_its_offset() {
    assert_eq!(
        load_bytes(&[0x7B, 0xFF, 0x7D]),
        Err(LoadError::NotUtf8 { offset: 1 })
    );
}

#[test]
fn t018_syntax_errors() {
    let trailing_comma = E1.replace("\"features\": []\n", "\"features\": [],\n");
    let comment = E1.replacen("{\n", "{\n  // comment\n", 1);
    let not_a_number = e1_with_feature_params("\"x\": NaN");
    let out_of_range = e1_with_feature_params("\"x\": 1e400");
    let single_quotes = e1_with_feature_params("\"x\": 'a'");

    let trailing_comma_result = load(&trailing_comma);
    assert!(
        matches!(
            trailing_comma_result,
            Err(LoadError::Syntax { line: 8, .. })
        ),
        "{trailing_comma_result:?}"
    );
    for (name, text) in [
        ("empty text", String::new()),
        ("comment", comment),
        ("NaN", not_a_number),
        ("1e400", out_of_range),
        ("single quotes", single_quotes),
    ] {
        let result = load(&text);
        assert!(
            matches!(result, Err(LoadError::Syntax { .. })),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn t019_duplicate_keys_are_rejected_at_any_depth() {
    let duplicate_units = E1.replace(
        "  \"units\": \"mm\",\n",
        "  \"units\": \"mm\",\n  \"units\": \"cm\",\n",
    );
    let duplicate_param = e1_with_feature_params("\"x\": 1, \"x\": 2");

    let units_result = load(&duplicate_units);
    let param_result = load(&duplicate_param);

    assert!(
        matches!(&units_result, Err(LoadError::DuplicateKey { key, .. }) if key == "units"),
        "{units_result:?}"
    );
    assert!(
        matches!(&param_result, Err(LoadError::DuplicateKey { key, .. }) if key == "x"),
        "{param_result:?}"
    );
}

#[test]
fn t020_nesting_limit() {
    let nested = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let shallow = e1_with_feature_params(&format!("\"x\": {}", nested(100)));
    let deep = e1_with_feature_params(&format!("\"x\": {}", nested(200)));

    let shallow_result = load(&shallow);
    let deep_result = load(&deep);

    assert!(shallow_result.is_ok(), "{shallow_result:?}");
    assert!(
        matches!(deep_result, Err(LoadError::Syntax { .. })),
        "{deep_result:?}"
    );
}

#[test]
fn t021_not_a_partsmith_document() {
    let other_format = E1.replace("\"format\": \"partsmith\"", "\"format\": \"other\"");
    let numeric_format = E1.replace("\"format\": \"partsmith\"", "\"format\": 1");

    for text in ["[]", "{}", other_format.as_str(), numeric_format.as_str()] {
        assert_eq!(load(text), Err(LoadError::NotPartsmith), "{text}");
    }
}

#[test]
fn t022_format_version_must_be_a_whole_number() {
    assert_eq!(
        load(&with_format_version(E1, None)),
        Err(LoadError::MissingFormatVersion)
    );
    assert_eq!(
        load(&with_format_version(E1, Some("\"1\""))),
        Err(LoadError::InvalidFormatVersion {
            found: "\"1\"".to_string()
        })
    );
    assert_eq!(
        load(&with_format_version(E1, Some("1.5"))),
        Err(LoadError::InvalidFormatVersion {
            found: "1.5".to_string()
        })
    );
    assert_eq!(
        load(&with_format_version(E1, Some("-1"))),
        Err(LoadError::InvalidFormatVersion {
            found: "-1".to_string()
        })
    );
    let whole_float = load(&with_format_version(E1, Some("1.0")));
    let migrated_from = whole_float.map(|loaded| loaded.migrated_from);
    assert_eq!(migrated_from, Ok(Some(1)));
}

#[test]
fn t023_newer_and_older_format_versions() {
    let newer = load(&with_format_version(E1, Some("3")));
    let older = load(&with_format_version(E1, Some("0")));

    assert_eq!(
        newer,
        Err(LoadError::NewerFormatVersion {
            found: 3,
            supported: 2
        })
    );
    assert_eq!(
        newer.expect_err("version 3 is newer").to_string(),
        "This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it."
    );
    assert_eq!(
        older,
        Err(LoadError::UnsupportedFormatVersion {
            found: 0,
            oldest: 1
        })
    );
}

#[test]
fn t024_invalid_fields_are_located_by_path() {
    let cases: [(String, &str, Option<&str>); 8] = [
        (
            E1.replace("\"features\": []\n", "\"features\": [],\n  \"colour\": 1\n"),
            "colour",
            None,
        ),
        (
            E1.replace("\"units\": \"mm\"", "\"units\": \"mmm\""),
            "units",
            Some("'mmm' is not a document unit"),
        ),
        (
            e1_with_list(
                "bodies",
                r##"[{"id": "b-1", "name": "A", "colour": "#000000"}]"##,
            ),
            "bodies[0].colour",
            None,
        ),
        (
            e1_with_list("parameters", r#"[{"id": "p 1", "name": "a", "expr": "1"}]"#),
            "parameters[0].id",
            Some("The id 'p 1' contains ' '"),
        ),
        (
            e1_with_list(
                "bodies",
                r##"[{"id": "b-1", "name": "A", "color": "#12345"}]"##,
            ),
            "bodies[0].color",
            None,
        ),
        (
            e1_with_list("parameters", r#"[{"id": "p-1", "name": "a", "expr": 2}]"#),
            "parameters[0].expr",
            None,
        ),
        (
            e1_with_list("bodies", r#"[{"id": "b-1", "name": "A", "visible": null}]"#),
            "bodies[0].visible",
            None,
        ),
        (
            e1_with_list(
                "features",
                r#"[{"id": "f-1", "type": "box"}, {"id": "f-2", "type": "box"}, {"id": "f-3"}]"#,
            ),
            "features[2]",
            Some("type"),
        ),
    ];

    for (text, expected_path, message_part) in cases {
        match load(&text) {
            Err(LoadError::Invalid { path, message }) => {
                assert_eq!(path, expected_path, "{text}");
                if let Some(part) = message_part {
                    assert!(message.contains(part), "message {message:?} lacks {part:?}");
                }
            }
            other => panic!("expected Invalid at {expected_path}, got {other:?}"),
        }
    }
}

#[test]
fn t025_missing_units_is_reported_at_the_root() {
    let text = E1.replace("  \"units\": \"mm\",\n", "");

    match load(&text) {
        Err(LoadError::Invalid { path, message }) => {
            assert_eq!(path, "");
            assert!(message.contains("units"), "message {message:?}");
        }
        other => panic!("expected Invalid at the root, got {other:?}"),
    }
}

#[test]
fn t026_one_structural_problem() {
    let text = e1_with_list(
        "bodies",
        r#"[{"id": "b-1", "name": "A"}, {"id": "b-1", "name": "B"}]"#,
    );

    let result = load(&text);

    assert_eq!(
        result,
        Err(LoadError::Structure(vec![StructureError::DuplicateId {
            id: id("b-1")
        }]))
    );
    assert_eq!(
        result.expect_err("b-1 is used twice").to_string(),
        "This document has a problem: The id b-1 is used by more than one record. Give the later record a new id."
    );
}

#[test]
fn t027_several_structural_problems() {
    let text = e1_with_list(
        "parameters",
        r#"[{"id": "p-1", "name": "pi", "expr": "1"}, {"id": "p-2", "name": "x", "expr": "1"}]"#,
    )
    .replace(
        "\"features\": []\n",
        "\"features\": [],\n  \"rollback\": \"f-9\"\n",
    );

    let result = load(&text);

    assert_eq!(
        result,
        Err(LoadError::Structure(vec![
            StructureError::InvalidParameterName {
                id: id("p-1"),
                error: NameError::Reserved {
                    name: "pi".to_string()
                }
            },
            StructureError::UnknownRollback { id: id("f-9") },
        ]))
    );
    assert_eq!(
        result
            .expect_err("the document has two problems")
            .to_string(),
        "This document has 2 problems:\n- Parameter p-1: 'pi' is a reserved word. Pick another name, such as pi_1.\n- The rollback marker points at f-9, which is not a feature in this document. Move the marker to an existing feature or to the end."
    );
}

#[test]
fn t028_numbers_are_normalized_on_load() {
    let text = e1_with_feature_params(
        r#""a": 80.0, "b": 1e2, "c": -0.0, "d": 0.5, "e": 9007199254740993"#,
    );

    let document = load_document(&text);
    let params = &document.features[0].params;

    assert_eq!(params["a"], json!(80));
    assert!(params["a"].is_u64(), "a is {:?}", params["a"]);
    assert_eq!(params["b"], json!(100));
    assert_eq!(params["c"], json!(0));
    assert!(
        params["c"].is_u64() || params["c"].is_i64(),
        "c is {:?}",
        params["c"]
    );
    assert!(params["d"].is_f64(), "d is {:?}", params["d"]);
    assert_eq!(params["d"].as_f64(), Some(0.5));
    assert!(params["e"].is_f64(), "e is {:?}", params["e"]);
    assert_eq!(params["e"].as_f64(), Some(9007199254740992.0));
}

#[test]
fn t029_unknown_params_keep_their_order() {
    let text = e1_with_feature_params(r#""zeta": 1, "alpha": 2, "mid": 3"#);

    let document = load_document(&text);
    let saved: Value = serde_json::from_str(&save_text(&document)).expect("the saved text is JSON");

    assert_eq!(
        document.features[0]
            .params
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["zeta", "alpha", "mid"]
    );
    assert_eq!(
        saved["features"][0]
            .as_object()
            .expect("a feature is an object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["id", "type", "zeta", "alpha", "mid"]
    );
}

#[test]
fn t030_expression_spacing_round_trips() {
    let text = e1_with_list(
        "parameters",
        r#"[{"id": "p-1", "name": "a", "expr": " 2 mm "}]"#,
    );

    let document = load_document(&save_text(&load_document(&text)));

    assert_eq!(document.parameters[0].expr, Expr::new(" 2 mm "));
}

#[test]
fn t031_unregistered_feature_types_load() {
    let text = e1_with_list(
        "features",
        r#"[{"id": "f-1", "type": "gear_demo", "teeth": 12}]"#,
    );

    let document = load_document(&text);

    assert_eq!(document.features[0].kind, "gear_demo");
}
