//! Acceptance tests for the JSON form of edits, records and transactions (model.md section 6, "Serialization", tests 68 to 73).

use crate::common;

use common::{feature, id};
use partsmith_model::{
    Body, BodyRole, Color, Edit, Expr, FeatureRecord, Parameter, Step, Transaction, Units,
};
use serde_json::{Value, json};

/// The `Transaction` example of model.md section 2.11.
const TRANSACTION_EXAMPLE: &str = r#"{
  "label": "Add parameter wall",
  "steps": [
    {
      "edit": {"type": "add_parameter", "index": 0, "parameter": {"id": "p-wall", "name": "wall", "expr": "2 mm"}},
      "inverse": {"type": "remove_parameter", "id": "p-wall"}
    },
    {"edit": {"type": "set_rollback"}, "inverse": {"type": "set_rollback", "rollback": "f-ext1"}}
  ]
}
"#;

/// The visible part body `b-1` named `Enclosure`, with no material and no color.
fn enclosure() -> Body {
    Body::new(id("b-1"), "Enclosure")
}

#[test]
fn t068_every_edit_example_round_trips() {
    let mut colored_enclosure = enclosure();
    colored_enclosure.material = Some("PETG".to_string());
    colored_enclosure.color = Some(Color::new(0x3b, 0x82, 0xf6));
    let mut hidden_enclosure = enclosure();
    hidden_enclosure.visible = false;
    let mut suppressed_extrude = feature("f-ext1", "extrude", json!({"body": "b-1", "op": "new"}));
    suppressed_extrude.suppressed = true;

    let examples: Vec<(&str, Edit)> = vec![
        (
            r#"{"type": "add_parameter", "index": 0, "parameter": {"id": "p-wall", "name": "wall", "expr": "2 mm"}}"#,
            Edit::AddParameter {
                index: 0,
                parameter: Parameter::new(id("p-wall"), "wall", "2 mm"),
            },
        ),
        (
            r#"{"type": "update_parameter", "parameter": {"id": "p-wall", "name": "wall", "expr": "2.4 mm", "comment": "Outer wall"}}"#,
            Edit::UpdateParameter {
                parameter: Parameter {
                    id: id("p-wall"),
                    name: "wall".to_string(),
                    expr: Expr::new("2.4 mm"),
                    comment: Some("Outer wall".to_string()),
                },
            },
        ),
        (
            r#"{"type": "remove_parameter", "id": "p-wall"}"#,
            Edit::RemoveParameter { id: id("p-wall") },
        ),
        (
            r#"{"type": "move_parameter", "id": "p-wall", "index": 1}"#,
            Edit::MoveParameter {
                id: id("p-wall"),
                index: 1,
            },
        ),
        (
            r##"{"type": "add_body", "index": 0, "body": {"id": "b-1", "name": "Enclosure", "material": "PETG", "color": "#3b82f6"}}"##,
            Edit::AddBody {
                index: 0,
                body: colored_enclosure,
            },
        ),
        (
            r#"{"type": "update_body", "body": {"id": "b-1", "name": "Enclosure", "visible": false}}"#,
            Edit::UpdateBody {
                body: hidden_enclosure,
            },
        ),
        (
            r#"{"type": "remove_body", "id": "b-1"}"#,
            Edit::RemoveBody { id: id("b-1") },
        ),
        (
            r#"{"type": "move_body", "id": "b-1", "index": 0}"#,
            Edit::MoveBody {
                id: id("b-1"),
                index: 0,
            },
        ),
        (
            r#"{"type": "add_feature", "index": 1, "feature": {"id": "f-ext1", "type": "extrude", "body": "b-1", "op": "new"}}"#,
            Edit::AddFeature {
                index: 1,
                feature: feature("f-ext1", "extrude", json!({"body": "b-1", "op": "new"})),
            },
        ),
        (
            r#"{"type": "update_feature", "feature": {"id": "f-ext1", "type": "extrude", "suppressed": true, "body": "b-1", "op": "new"}}"#,
            Edit::UpdateFeature {
                feature: suppressed_extrude,
            },
        ),
        (
            r#"{"type": "remove_feature", "id": "f-ext1"}"#,
            Edit::RemoveFeature { id: id("f-ext1") },
        ),
        (
            r#"{"type": "move_feature", "id": "f-ext1", "index": 0}"#,
            Edit::MoveFeature {
                id: id("f-ext1"),
                index: 0,
            },
        ),
        (
            r#"{"type": "set_rollback", "rollback": "f-sk1"}"#,
            Edit::SetRollback {
                rollback: Some(id("f-sk1")),
            },
        ),
        (
            r#"{"type": "set_rollback"}"#,
            Edit::SetRollback { rollback: None },
        ),
        (
            r#"{"type": "set_units", "units": "in"}"#,
            Edit::SetUnits { units: Units::In },
        ),
        (
            r#"{"type": "set_printer", "printer": "generic-fdm"}"#,
            Edit::SetPrinter {
                printer: Some("generic-fdm".to_string()),
            },
        ),
        (
            r#"{"type": "set_printer"}"#,
            Edit::SetPrinter { printer: None },
        ),
    ];

    for (line, expected) in examples {
        let edit: Edit = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("{line} does not deserialize: {error}"));
        let written = serde_json::to_string(&edit).expect("an edit serializes");
        let reference: Value = serde_json::from_str(line).expect("the example is JSON");

        assert_eq!(edit, expected, "{line}");
        assert_eq!(
            written,
            serde_json::to_string(&reference).expect("a value serializes"),
            "{line}"
        );
    }
}

#[test]
fn t069_null_rollback_clears_the_marker() {
    let edit: Edit = serde_json::from_str("{\"type\": \"set_rollback\", \"rollback\": null}")
        .expect("an explicit null is accepted");

    assert_eq!(edit, Edit::SetRollback { rollback: None });
}

#[test]
fn t070_body_defaults_are_omitted() {
    let plain = Body::new(id("b-1"), "Base");
    let mut hidden_blocker = Body::new(id("b-1"), "Base");
    hidden_blocker.visible = false;
    hidden_blocker.role = BodyRole::SupportBlocker;

    assert_eq!(
        serde_json::to_value(&plain).expect("a body serializes"),
        json!({"id": "b-1", "name": "Base"})
    );
    assert_eq!(
        serde_json::to_string(&plain).expect("a body serializes"),
        r#"{"id":"b-1","name":"Base"}"#
    );
    assert_eq!(
        serde_json::to_value(&hidden_blocker).expect("a body serializes"),
        json!({"id": "b-1", "name": "Base", "visible": false, "role": "support_blocker"})
    );
    assert_eq!(
        serde_json::to_string(&hidden_blocker).expect("a body serializes"),
        r#"{"id":"b-1","name":"Base","visible":false,"role":"support_blocker"}"#
    );
}

#[test]
fn t071_feature_record_keys_other_than_the_record_keys_are_params() {
    let record: FeatureRecord = serde_json::from_str(
        r#"{"type": "box", "size": [1, 2, 3], "id": "f-1", "suppressed": false, "label": "x"}"#,
    )
    .expect("the record deserializes");

    assert_eq!(record.id, id("f-1"));
    assert_eq!(record.kind, "box");
    assert!(!record.suppressed);
    assert_eq!(record.name, None);
    assert_eq!(
        record.params.keys().map(String::as_str).collect::<Vec<_>>(),
        ["size", "label"]
    );
}

#[test]
fn t072_parameters_reject_unknown_keys_and_numeric_expressions() {
    let unknown_key = serde_json::from_str::<Parameter>(
        r#"{"id": "p-1", "name": "a", "expr": "1", "unit": "mm"}"#,
    )
    .expect_err("the key unit is unknown");
    let numeric_expr =
        serde_json::from_str::<Parameter>(r#"{"id": "p-1", "name": "a", "expr": 2}"#);

    assert!(
        unknown_key.to_string().contains("unit"),
        "unexpected message: {unknown_key}"
    );
    assert!(
        numeric_expr.is_err(),
        "a number is not an expression: {numeric_expr:?}"
    );
}

#[test]
fn t073_transaction_example_deserializes() {
    let transaction: Transaction =
        serde_json::from_str(TRANSACTION_EXAMPLE).expect("the example deserializes");

    assert_eq!(transaction.label, "Add parameter wall");
    assert_eq!(transaction.steps.len(), 2);
    assert_eq!(
        transaction.steps[1],
        Step {
            edit: Edit::SetRollback { rollback: None },
            inverse: Edit::SetRollback {
                rollback: Some(id("f-ext1"))
            },
        }
    );
}
