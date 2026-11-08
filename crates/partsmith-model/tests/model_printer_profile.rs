//! Acceptance tests for the stored printer profile (model.md section 6, "The stored printer profile", tests 98 to 105
//! and 115): `Edit::SetPrinter`'s `profile`, structural rules 9 and 10, and `Document::normalize`.

use crate::common;

use common::{d1, e11_profile};
use partsmith_model::{Document, Edit, EditError, StructureError};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

/// The `SetPrinter` edit with this printer and profile.
fn set_printer(printer: Option<&str>, profile: Option<Map<String, Value>>) -> Edit {
    Edit::SetPrinter {
        printer: printer.map(str::to_string),
        profile,
    }
}

/// `P` with `max_bridge_length` replaced by the `f64` 10.0.
fn profile_with_float_bridge() -> Map<String, Value> {
    let mut profile = e11_profile();
    profile.insert(
        "max_bridge_length".to_string(),
        Value::from(serde_json::Number::from_f64(10.0).expect("10.0 is finite")),
    );
    assert!(profile["max_bridge_length"].is_f64());
    profile
}

/// Asserts that `value` is the integer 10 stored as a `u64`.
#[track_caller]
fn assert_integer_ten(value: Option<&Value>) {
    let value = value.expect("the profile has max_bridge_length");
    assert_eq!(value, &json!(10));
    assert!(value.is_u64(), "{value:?} is not a u64");
}

#[test]
fn t098_set_printer_stores_a_profile() {
    let mut document = d1();

    let inverse = document.apply(set_printer(Some("my-mk4"), Some(e11_profile())));

    assert_eq!(inverse, Ok(set_printer(None, None)));
    assert_eq!(document.printer.as_deref(), Some("my-mk4"));
    assert_eq!(document.printer_profile, Some(e11_profile()));
    document
        .apply(inverse.expect("the edit applies"))
        .expect("the inverse applies");
    assert_eq!(document, d1());
}

#[test]
fn t099_set_printer_without_a_profile_clears_the_stored_profile() {
    let mut document = d1();
    document
        .apply(set_printer(Some("my-mk4"), Some(e11_profile())))
        .expect("test 98's edit applies");

    let inverse = document.apply(set_printer(Some("generic-fdm"), None));

    assert_eq!(
        inverse,
        Ok(set_printer(Some("my-mk4"), Some(e11_profile())))
    );
    assert_eq!(document.printer.as_deref(), Some("generic-fdm"));
    assert_eq!(document.printer_profile, None);
}

#[test]
fn t100_a_profile_without_a_printer_is_refused() {
    let mut document = d1();

    let result = document.apply(set_printer(None, Some(e11_profile())));

    assert_eq!(result, Err(EditError::ProfileWithoutPrinter));
    assert_eq!(
        result.expect_err("there is no printer").to_string(),
        "A printer profile can be stored only with the printer it belongs to. Name its id as the printer."
    );
    assert_eq!(document, d1());
}

#[test]
fn t101_a_profile_of_another_printer_is_refused() {
    let mut document = d1();

    let result = document.apply(set_printer(Some("my-mk3"), Some(e11_profile())));

    assert_eq!(
        result,
        Err(EditError::ProfileIdMismatch {
            printer: "my-mk3".to_string(),
            found: Some("my-mk4".to_string())
        })
    );
    assert_eq!(
        result.expect_err("the ids differ").to_string(),
        "The printer profile has the id \"my-mk4\", but the printer is \"my-mk3\". Store the profile of the printer you name."
    );
    assert_eq!(document, d1());
}

#[test]
fn t102_a_profile_without_an_id_is_refused() {
    let mut document = d1();
    let mut profile = e11_profile();
    profile.shift_remove("id");

    let result = document.apply(set_printer(Some("my-mk4"), Some(profile)));

    assert_eq!(
        result,
        Err(EditError::ProfileIdMismatch {
            printer: "my-mk4".to_string(),
            found: None
        })
    );
    assert_eq!(
        result.expect_err("the profile has no id").to_string(),
        "The printer profile needs the id \"my-mk4\", the printer you name."
    );
    assert_eq!(document, d1());
}

#[test]
fn a_profile_whose_id_is_not_a_string_is_refused_like_one_without_an_id() {
    let mut document = d1();
    let mut profile = e11_profile();
    profile.insert("id".to_string(), json!(7));

    let result = document.apply(set_printer(Some("my-mk4"), Some(profile)));

    assert_eq!(
        result,
        Err(EditError::ProfileIdMismatch {
            printer: "my-mk4".to_string(),
            found: None
        })
    );
    assert_eq!(document, d1());
}

#[test]
fn t103_apply_normalizes_the_numbers_of_a_stored_profile() {
    let mut document = d1();

    document
        .apply(set_printer(Some("my-mk4"), Some(profile_with_float_bridge())))
        .expect("the edit applies");

    let profile = document
        .printer_profile
        .as_ref()
        .expect("the document stores a profile");
    assert_integer_ten(profile.get("max_bridge_length"));
}

#[test]
fn t104_check_structure_reports_rules_9_and_10_after_the_printer_label() {
    let mut without_printer = d1();
    without_printer.format_version = 2;
    without_printer.printer_profile = Some(e11_profile());
    let mut other_printer = without_printer.clone();
    other_printer.printer = Some("my-mk3".to_string());

    assert_eq!(
        without_printer.check_structure(),
        vec![
            StructureError::WrongFormatVersion { found: 2 },
            StructureError::PrinterProfileWithoutPrinter,
        ]
    );
    assert_eq!(
        other_printer.check_structure(),
        vec![
            StructureError::WrongFormatVersion { found: 2 },
            StructureError::PrinterProfileIdMismatch {
                printer: "my-mk3".to_string(),
                found: Some("my-mk4".to_string())
            },
        ]
    );
}

#[test]
fn a_stored_profile_of_the_printer_it_names_is_structurally_valid() {
    let mut document = d1();
    document.printer = Some("my-mk4".to_string());
    document.printer_profile = Some(e11_profile());

    assert_eq!(document.check_structure(), Vec::<StructureError>::new());
}

#[test]
fn t105_set_printer_in_json() {
    let with_profile = json!({"type": "set_printer", "printer": "my-mk4", "profile": e11_profile()});
    let with_null = json!({"type": "set_printer", "printer": "my-mk4", "profile": null});
    let with_number = json!({"type": "set_printer", "printer": "my-mk4", "profile": 5});

    let edit: Edit =
        serde_json::from_value(with_profile.clone()).expect("the edit with a profile deserializes");
    let null_edit: Edit =
        serde_json::from_value(with_null).expect("the edit with a null profile deserializes");
    let number_edit = serde_json::from_value::<Edit>(with_number);

    assert_eq!(edit, set_printer(Some("my-mk4"), Some(e11_profile())));
    assert_eq!(
        serde_json::to_string(&edit).expect("the edit serializes"),
        serde_json::to_string(&with_profile).expect("the JSON serializes")
    );
    assert_eq!(null_edit, set_printer(Some("my-mk4"), None));
    assert!(number_edit.is_err(), "{number_edit:?}");
}

#[test]
fn t115_normalize_normalizes_a_stored_profile() {
    let mut document: Document = d1();
    document.printer = Some("my-mk4".to_string());
    document.printer_profile = Some(profile_with_float_bridge());

    document.normalize();

    let profile = document
        .printer_profile
        .as_ref()
        .expect("the document stores a profile");
    assert_integer_ten(profile.get("max_bridge_length"));
}
