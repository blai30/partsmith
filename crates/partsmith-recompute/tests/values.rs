//! Values, units and the evaluation context (recompute.md sections 3.1 step 4 and 3.4): tests 31
//! to 33 of section 6, plus runner tests over test-only features.

mod common;

use common::{
    Harness, assert_absolute, assert_bounding_box, assert_volume, bodies, datum_height, document,
    document_json, error_code, final_state, load, message_texts, owned, parameter,
    parameter_value, reused, shape, status, test_datum, with_action,
};
use partsmith_model::{Expr, ExprErrorKind, Quantity};
use serde_json::json;

/// The parse error of an expression source.
fn parse_error(source: &str) -> partsmith_model::ExprError {
    Expr::new(source)
        .parse()
        .err()
        .unwrap_or_else(|| panic!("'{source}' should not parse"))
}

#[test]
fn t031_context_values_drive_expressions_and_cache_keys() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [{
            "id": "f-box1", "type": "box", "body": "b-1",
            "size": ["printer.fit.slip * 100", "10", "10"], "op": "new"
        }]
    }));

    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let first = harness.run(&document);
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.3));
    let second = harness.run(&document);
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let third = harness.run(&document);
    harness.context.clear();
    let missing = harness.run(&document);

    assert_volume(shape(final_state(&first, "b-1")), 2000.0);
    assert_bounding_box(shape(final_state(&first, "b-1")), [-10.0, -5.0, 0.0], [10.0, 5.0, 10.0]);
    assert!(reused(&second).is_empty());
    assert_volume(shape(final_state(&second, "b-1")), 3000.0);
    assert_eq!(reused(&third), owned(&["f-box1"]));
    assert_volume(shape(final_state(&third, "b-1")), 2000.0);
    assert_eq!(error_code(&missing, "f-box1"), "expression");
    assert_eq!(
        message_texts(&missing, "f-box1"),
        owned(&[
            "Length: Unknown value 'printer.fit.slip'. Check the spelling; printer values look like printer.fit.slip."
        ])
    );
}

#[test]
fn t032_a_units_change_rebuilds_only_features_with_bare_numbers() {
    let mut harness = Harness::builtins();
    let mut document = document_json(json!({
        "bodies": bodies(2),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {"id": "f-box2", "type": "box", "body": "b-2", "size": ["10 mm", "10 mm", "10 mm"], "op": "new"}
        ]
    }));
    harness.run(&load(&document));
    document["units"] = json!("in");

    let plan = harness.plan(&load(&document));

    assert_eq!(with_action(&plan, "build"), owned(&["f-box1"]));
    assert_eq!(with_action(&plan, "reuse"), owned(&["f-box2"]));
}

#[test]
fn t033_a_parameter_that_does_not_parse_reports_its_error() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "parameters": [parameter("p-bad", "bad", "2 +")],
        "bodies": bodies(1),
        "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["bad", "1", "1"], "op": "new"}]
    }));

    let report = harness.run(&document);

    assert_eq!(parameter_value(&report, "p-bad"), &Err(parse_error("2 +")));
    assert_eq!(error_code(&report, "f-box1"), "expression");
}

// Runner tests over test-only features.

#[test]
fn context_values_reach_feature_expressions() {
    let mut harness = Harness::test_features();
    let document = document(json!({"features": [test_datum("f-d1", "printer.fit.slip * 100")]}));

    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let first = harness.run(&document);
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.3));
    let second = harness.run(&document);
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let third = harness.run(&document);

    assert_absolute("height with 0.2 mm", datum_height(&first, "f-d1"), 20.0, 1e-9);
    assert!(reused(&second).is_empty());
    assert_absolute("height with 0.3 mm", datum_height(&second, "f-d1"), 30.0, 1e-9);
    assert_eq!(reused(&third), owned(&["f-d1"]));
}

#[test]
fn a_missing_context_value_is_an_expression_error_naming_the_field() {
    let mut harness = Harness::test_features();
    let document = document(json!({"features": [test_datum("f-d1", "printer.fit.slip * 100")]}));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-d1"), "expression");
    assert_eq!(
        message_texts(&report, "f-d1"),
        owned(&[
            "Offset: Unknown value 'printer.fit.slip'. Check the spelling; printer values look like printer.fit.slip."
        ])
    );
    let message = common::first_message(&report, "f-d1");
    assert_eq!(message.pointer.as_deref(), Some("/offset"));
}

#[test]
fn bare_numbers_take_the_document_units() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "units": "in",
        "features": [test_datum("f-d1", "2"), test_datum("f-d2", "2 mm")]
    }));

    let report = harness.run(&document);

    assert_absolute("bare 2 in an inch document", datum_height(&report, "f-d1"), 50.8, 1e-9);
    assert_absolute("2 mm in an inch document", datum_height(&report, "f-d2"), 2.0, 1e-9);
}

#[test]
fn a_units_change_rebuilds_only_features_whose_values_change() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "features": [test_datum("f-d1", "10"), test_datum("f-d2", "10 mm")]
    }));
    harness.run(&load(&document));
    document["units"] = json!("in");

    let plan = harness.plan(&load(&document));
    let report = harness.run(&load(&document));

    assert_eq!(with_action(&plan, "build"), owned(&["f-d1"]));
    assert_eq!(with_action(&plan, "reuse"), owned(&["f-d2"]));
    assert_absolute("f-d1 height", datum_height(&report, "f-d1"), 254.0, 1e-9);
    assert_absolute("f-d2 height", datum_height(&report, "f-d2"), 10.0, 1e-9);
}

#[test]
fn parameter_reports_list_every_parameter_in_document_order_in_internal_units() {
    let mut harness = Harness::test_features();
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let document = document(json!({
        "parameters": [
            parameter("p-l", "l", "1 in"),
            parameter("p-n", "n", "3"),
            parameter("p-s", "s", "printer.fit.slip * 2"),
            parameter("p-bad", "bad", "2 +"),
            parameter("p-dep", "dep", "bad + 1 mm")
        ]
    }));

    let report = harness.run(&document);

    let order: Vec<&str> = report
        .parameters
        .iter()
        .map(|entry| entry.parameter.as_str())
        .collect();
    assert_eq!(order, vec!["p-l", "p-n", "p-s", "p-bad", "p-dep"]);
    assert_eq!(parameter_value(&report, "p-l"), &Ok(Quantity::length(25.4)));
    assert_eq!(parameter_value(&report, "p-n"), &Ok(Quantity::plain(3.0)));
    assert_eq!(parameter_value(&report, "p-s"), &Ok(Quantity::length(0.4)));
    assert_eq!(parameter_value(&report, "p-bad"), &Err(parse_error("2 +")));
    match parameter_value(&report, "p-dep") {
        Err(error) => assert!(
            matches!(error.kind, ExprErrorKind::DependsOnFailed { .. }),
            "p-dep fails with {error:?}"
        ),
        Ok(value) => panic!("p-dep evaluated to {value:?}"),
    }
}

#[test]
fn a_feature_that_uses_a_failed_parameter_reports_an_expression_error() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "parameters": [parameter("p-bad", "bad", "2 +")],
        "features": [test_datum("f-d1", "bad"), test_datum("f-d2", "1")]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-d1"), "expression");
    assert_eq!(status(&report, "f-d2"), "ok");
}
