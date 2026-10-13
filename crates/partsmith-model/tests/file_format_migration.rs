//! Acceptance tests for format versions and the migration chain (file-format.md section 6, "Migration", tests 39 to 45), with unit tests of the built-in chain (section 3.9).

mod common;

use common::{E1, E2, E10};
use partsmith_model::{
    FORMAT_VERSION, LoadError, MIGRATIONS, MigrationStep, OLDEST_FORMAT_VERSION, migrate,
    migrate_with,
};
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

/// The raw JSON of a canonical example.
fn example_json(text: &str) -> Value {
    serde_json::from_str(text).expect("the example is JSON")
}

/// `document` with its root `format_version` set to `version`.
fn at_version(mut document: Value, version: u64) -> Value {
    document["format_version"] = json!(version);
    document
}

#[test]
fn t039_current_version_is_unchanged_and_version_one_is_brought_to_two() {
    let document = example_json(E1);

    assert_eq!(migrate(document.clone()), Ok((document.clone(), None)));
    assert_eq!(
        migrate(at_version(document.clone(), 1)),
        Ok((document, Some(1)))
    );
}

#[test]
fn this_build_writes_version_two_and_opens_version_one() {
    assert_eq!(FORMAT_VERSION, 2);
    assert_eq!(OLDEST_FORMAT_VERSION, 1);
}

#[test]
fn the_built_in_chain_has_one_step_from_version_one() {
    let froms: Vec<u32> = MIGRATIONS.iter().map(|step| step.from).collect();

    assert_eq!(froms, [1]);
}

#[test]
fn the_step_from_version_one_returns_its_input_unchanged() {
    let step = MIGRATIONS[0];
    let document = at_version(example_json(E10), 1);

    assert_eq!((step.migrate)(document.clone()), Ok(document));
}

#[test]
fn the_step_from_version_one_keeps_params_it_does_not_know() {
    let step = MIGRATIONS[0];
    let document = json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "features": [
            {"id": "f-1", "type": "gear_demo", "nb": "not a map", "merged": [1, 2], "teeth": 12}
        ]
    });

    assert_eq!((step.migrate)(document.clone()), Ok(document));
}

#[test]
fn version_one_documents_are_brought_to_version_two_unchanged() {
    for (name, text) in [("E1", E1), ("E2", E2), ("E10", E10)] {
        let current = example_json(text);

        assert_eq!(
            migrate(at_version(current.clone(), 1)),
            Ok((current, Some(1))),
            "{name}"
        );
    }
}

#[test]
fn version_two_documents_are_returned_unchanged() {
    let document = example_json(E10);

    assert_eq!(migrate(document.clone()), Ok((document, None)));
}

#[test]
fn the_built_in_chain_rejects_versions_it_cannot_open() {
    let document = example_json(E1);

    assert_eq!(
        migrate(at_version(document.clone(), 3)),
        Err(LoadError::NewerFormatVersion {
            found: 3,
            supported: 2
        })
    );
    assert_eq!(
        migrate(at_version(document, 0)),
        Err(LoadError::UnsupportedFormatVersion {
            found: 0,
            oldest: 1
        })
    );
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
