//! Acceptance tests for the canonical writer and the `.ptsm` layout (file-format.md section 6, "Writer and layout", tests 1 to 13).

mod common;

use common::{E1, E2, E3, E4, E5, E6, E7, E8_OUTPUT, e8_input, feature, id};
use partsmith_model::canonical::{format_number, format_string, to_string};
use partsmith_model::{
    Body, BodyRole, Color, Document, Expr, FeatureRecord, Parameter, Units, load, save,
};
use pretty_assertions::assert_eq;
use serde_json::{Number, Value, json};

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

/// Keys of a JSON object, in order.
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

#[test]
fn t001_empty_document_is_e1() {
    let text = save_text(&Document::new(Units::Mm));

    assert_eq!(text, E1);
    assert_eq!(text.len(), 122);
    assert_eq!(Document::new(Units::Mm), load_document(E1));
}

#[test]
fn t002_full_document_round_trips() {
    let loaded = load(E2).expect("E2 loads");

    assert_eq!(loaded.migrated_from, None);
    assert_eq!(loaded.document.parameters.len(), 2);
    assert_eq!(loaded.document.bodies.len(), 1);
    assert_eq!(loaded.document.features.len(), 3);
    let text = save_text(&loaded.document);
    assert_eq!(text, E2);
    assert_eq!(text.len(), 1614);
}

#[test]
fn t003_examples_e3_to_e7_round_trip() {
    for (name, text, size) in [
        ("E3", E3, 630),
        ("E4", E4, 399),
        ("E5", E5, 463),
        ("E6", E6, 337),
        ("E7", E7, 626),
    ] {
        let saved = save_text(&load_document(text));

        assert_eq!(saved, text, "{name}");
        assert_eq!(saved.len(), size, "{name}");
    }
}

#[test]
fn t004_document_built_in_code_saves_as_e3() {
    let mut keep_out = Body::new(id("b-2"), "Keep out");
    keep_out.color = Some(Color::new(0xff, 0x00, 0x00));
    keep_out.visible = false;
    keep_out.role = BodyRole::SupportBlocker;
    let mut base_block = feature(
        "f-box1",
        "box",
        json!({"body": "b-1", "size": ["40 mm", "30 mm", "10 mm"], "op": "new"}),
    );
    base_block.name = Some("Base block".to_string());
    let mut screw_hole = feature(
        "f-cyl1",
        "cylinder",
        json!({"body": "b-1", "radius": "3 mm", "height": "10 mm", "op": "cut"}),
    );
    screw_hole.suppressed = true;
    let mut document = Document::new(Units::Mm);
    document.bodies = vec![Body::new(id("b-1"), "Base"), keep_out];
    document.features = vec![base_block, screw_hole];
    document.rollback = Some(id("f-cyl1"));

    assert_eq!(save_text(&document), E3);
}

#[test]
fn t005_non_canonical_input_is_normalized() {
    let saved = save_text(&load_document(&e8_input()));

    assert_eq!(saved, E8_OUTPUT);
    assert_eq!(saved.len(), 282);
    assert_eq!(save_text(&load_document(&saved)), saved);
}

#[test]
fn t006_output_text_rules() {
    let text = save_text(&load_document(E2));

    assert!(!text.contains('\r'), "the text has a carriage return");
    assert!(!text.contains('\t'), "the text has a tab");
    assert!(
        !text.starts_with('\u{feff}'),
        "the text has a byte order mark"
    );
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    for line in text.lines() {
        assert!(!line.ends_with(' '), "line ends with a space: {line:?}");
        assert!(
            line.chars().count() <= 121,
            "line is longer than 121 characters: {line:?}"
        );
    }
}

#[test]
fn t007_number_table() {
    let floats: [(f64, &str); 18] = [
        (80.0, "80"),
        (-0.0, "0"),
        (1e2, "100"),
        (-40.0, "-40"),
        (0.1, "0.1"),
        (-2.5, "-2.5"),
        (12345678.9, "12345678.9"),
        (0.30000000000000004, "0.30000000000000004"),
        (0.000015, "0.000015"),
        (1e-5, "0.00001"),
        (1e-7, "1e-7"),
        (-1e-7, "-1e-7"),
        (9007199254740991.0, "9007199254740991"),
        (9007199254740992.0, "9007199254740992.0"),
        (1e16, "1e16"),
        (1.5e300, "1.5e300"),
        (5e-324, "5e-324"),
        (1.7976931348623157e308, "1.7976931348623157e308"),
    ];
    for (value, written) in floats {
        let number = Number::from_f64(value).expect("the value is finite");
        assert_eq!(format_number(&number), written, "{value:?}");
    }

    let integers = [
        (Number::from(-40_i64), "-40"),
        (Number::from(9007199254740991_u64), "9007199254740991"),
        (Number::from(9007199254740992_u64), "9007199254740992.0"),
    ];
    for (number, written) in integers {
        assert_eq!(format_number(&number), written, "{number:?}");
    }
}

#[test]
fn t008_large_and_negative_integers() {
    assert_eq!(
        format_number(&Number::from(9007199254740993_u64)),
        "9007199254740992.0"
    );
    assert_eq!(format_number(&Number::from(-5_i64)), "-5");
}

#[test]
fn t009_string_escapes() {
    assert_eq!(
        format_string("Lid \"v2\" \\ draft\tAé°/x\u{1f}"),
        r#""Lid \"v2\" \\ draft\tAé°/x\u001f""#
    );
    assert_eq!(format_string("\u{7f}"), "\"\u{7f}\"");
    assert_eq!(format_string("\u{8}\u{c}\n\r"), r#""\b\f\n\r""#);
}

#[test]
fn t010_small_values_layout() {
    let rows = [
        (json!(5), "5\n"),
        (json!("a"), "\"a\"\n"),
        (json!({}), "{}\n"),
        (json!([]), "[]\n"),
        (json!({"a": 1}), "{\n  \"a\": 1\n}\n"),
        (json!([1, 2]), "[\n  1,\n  2\n]\n"),
        (
            json!({"a": {"b": [{"c": 1}]}}),
            "{\n  \"a\": {\n    \"b\": [\n      {\"c\": 1}\n    ]\n  }\n}\n",
        ),
        (
            json!({"m": [1, {"x": 1}]}),
            "{\n  \"m\": [\n    1,\n    {\"x\": 1}\n  ]\n}\n",
        ),
        (
            json!({"g": [[{"x": 1}]]}),
            "{\n  \"g\": [\n    [\n      {\"x\": 1}\n    ]\n  ]\n}\n",
        ),
        (
            json!({"k\"ey": [1, 2], "n": null, "t": true}),
            "{\n  \"k\\\"ey\": [1, 2],\n  \"n\": null,\n  \"t\": true\n}\n",
        ),
    ];

    for (value, output) in rows {
        assert_eq!(to_string(&value), output, "{value}");
    }
}

#[test]
fn t011_saving_is_deterministic() {
    let document = load_document(E2);
    let copy = document.clone();

    let first = save_text(&document);
    let second = save_text(&document);
    let from_copy = save_text(&copy);

    assert_eq!(first, second);
    assert_eq!(first, from_copy);
}

#[test]
fn t012_default_fields_are_omitted() {
    let mut document = Document::new(Units::Mm);
    document.parameters = vec![Parameter::new(id("p-wall"), "wall", "2 mm")];
    document.bodies = vec![Body::new(id("b-1"), "Base")];
    document.features = vec![FeatureRecord::new(id("f-1"), "box")];

    let text = save_text(&document);
    let saved: Value = serde_json::from_str(&text).expect("the saved text is JSON");

    assert_eq!(
        keys(&saved),
        [
            "format",
            "format_version",
            "units",
            "parameters",
            "bodies",
            "features"
        ]
    );
    assert_eq!(keys(&saved["parameters"][0]), ["id", "name", "expr"]);
    assert_eq!(keys(&saved["bodies"][0]), ["id", "name"]);
    assert_eq!(keys(&saved["features"][0]), ["id", "type"]);
}

#[test]
fn t013_one_expression_change_is_a_one_line_diff() {
    let mut document = load_document(E2);
    document
        .parameters
        .iter_mut()
        .find(|parameter| parameter.id == id("p-wall"))
        .expect("E2 has p-wall")
        .expr = Expr::new("3 mm");

    let changed = save_text(&document);

    let before: Vec<&str> = E2.lines().collect();
    let after: Vec<&str> = changed.lines().collect();
    assert_eq!(before.len(), after.len());
    let differing = before
        .iter()
        .zip(&after)
        .filter(|(old, new)| old != new)
        .count();
    assert_eq!(differing, 1);
}
