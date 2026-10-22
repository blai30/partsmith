//! Acceptance tests for format versions and the migration chain (file-format.md section 6, "Migration", tests 39 to 45), with a test of the built-in step (section 3.9).

use crate::common;

use common::E1;
use partsmith_model::{LoadError, MIGRATIONS, MigrationStep, migrate, migrate_with};
use serde_json::{Value, json};

/// Step S1: from version 1, adds the root key `"a": 1`.
const ADD_A: MigrationStep = MigrationStep {
    from: 1,
    migrate: add_a,
};

/// Step S2: from version 2, renames the root key `a` to `b`.
const RENAME_A_TO_B: MigrationStep = MigrationStep {
    from: 2,
    migrate: rename_a_to_b,
};

/// Adds the root key `"a": 1`.
fn add_a(mut document: Value) -> Result<Value, String> {
    document
        .as_object_mut()
        .ok_or("The document is not an object.")?
        .insert("a".to_string(), json!(1));
    Ok(document)
}

/// Renames the root key `a` to `b`, if it is present.
fn rename_a_to_b(mut document: Value) -> Result<Value, String> {
    let root = document
        .as_object_mut()
        .ok_or("The document is not an object.")?;
    if let Some(value) = root.remove("a") {
        root.insert("b".to_string(), value);
    }
    Ok(document)
}

/// Always fails with a message.
fn reject_yards(_: Value) -> Result<Value, String> {
    Err("Unknown units 'yd'.".to_string())
}

/// Writes a wrong `format_version` into the document.
fn write_version_99(mut document: Value) -> Result<Value, String> {
    document["format_version"] = json!(99);
    Ok(document)
}

/// A minimal raw document at `version`.
fn document_at(version: u64) -> Value {
    json!({
        "format": "partsmith",
        "format_version": version,
        "units": "mm",
        "parameters": [],
        "bodies": [],
        "features": []
    })
}

/// The `format_version` of a migrated document.
fn format_version(document: &Value) -> Option<u64> {
    document["format_version"].as_u64()
}

#[test]
fn t039_current_version_is_unchanged_and_version_1_is_brought_to_2() {
    let document: Value = serde_json::from_str(E1).expect("E1 is JSON");
    let mut at_version_1 = document.clone();
    at_version_1["format_version"] = json!(1);

    assert_eq!(migrate(document.clone()), Ok((document.clone(), None)));
    assert_eq!(migrate(at_version_1), Ok((document, Some(1))));
}

#[test]
fn the_built_in_step_from_version_1_keeps_params_it_does_not_know() {
    let [step] = MIGRATIONS else {
        panic!("the built-in chain has {} steps, not one", MIGRATIONS.len());
    };
    let document = json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "features": [
            {"id": "f-1", "type": "gear_demo", "nb": "not a map", "merged": [1, 2], "teeth": 12}
        ]
    });

    assert_eq!(step.from, 1);
    assert_eq!((step.migrate)(document.clone()), Ok(document));
}

#[test]
fn t040_steps_run_in_version_order_whatever_their_slice_order() {
    let (migrated, original) =
        migrate_with(document_at(1), 3, 1, &[RENAME_A_TO_B, ADD_A]).expect("both steps run");

    assert_eq!(migrated["b"], json!(1));
    assert_eq!(migrated.get("a"), None);
    assert_eq!(format_version(&migrated), Some(3));
    assert_eq!(original, Some(1));
}

#[test]
fn t041_only_later_steps_run() {
    let (migrated, original) =
        migrate_with(document_at(2), 3, 1, &[RENAME_A_TO_B, ADD_A]).expect("the second step runs");

    assert_eq!(migrated.get("a"), None);
    assert_eq!(migrated.get("b"), None);
    assert_eq!(format_version(&migrated), Some(3));
    assert_eq!(original, Some(2));
}

#[test]
fn t042_a_missing_step_is_reported() {
    assert_eq!(
        migrate_with(document_at(1), 3, 1, &[ADD_A]),
        Err(LoadError::MissingMigration { from: 2 })
    );
}

#[test]
fn t043_a_failing_step_is_reported() {
    let failing = MigrationStep {
        from: 1,
        migrate: reject_yards,
    };

    let result = migrate_with(document_at(1), 2, 1, &[failing]);

    assert_eq!(
        result,
        Err(LoadError::Migration {
            from: 1,
            to: 2,
            message: "Unknown units 'yd'.".to_string()
        })
    );
    assert_eq!(
        result.expect_err("the step fails").to_string(),
        "Could not upgrade the file from format version 1 to 2: Unknown units 'yd'."
    );
}

#[test]
fn t044_the_chain_sets_the_version() {
    let careless = MigrationStep {
        from: 1,
        migrate: write_version_99,
    };

    let (migrated, _) = migrate_with(document_at(1), 2, 1, &[careless]).expect("the step succeeds");

    assert_eq!(format_version(&migrated), Some(2));
}

#[test]
fn t045_versions_outside_the_supported_range() {
    assert_eq!(
        migrate_with(document_at(0), 3, 1, &[ADD_A, RENAME_A_TO_B]),
        Err(LoadError::UnsupportedFormatVersion {
            found: 0,
            oldest: 1
        })
    );
    assert_eq!(
        migrate_with(document_at(4), 3, 1, &[ADD_A, RENAME_A_TO_B]),
        Err(LoadError::NewerFormatVersion {
            found: 4,
            supported: 3
        })
    );
}
