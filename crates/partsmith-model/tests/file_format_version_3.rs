//! Acceptance tests for format version 3 (file-format.md section 6, "Format version 3", tests 59 to 68), with tests
//! of the stored printer profile's rules of sections 3.2 and 3.9. Test 69 reads the generated schema file and lives in
//! `crates/partsmith-cli/tests/schema.rs`.

use crate::common;

use common::{
    E1, E2, E10, E11, differing_lines, e11_profile, keys_at, load_document, save_text,
    with_format_version,
};
use partsmith_model::canonical::{parse, to_string};
use partsmith_model::{
    LoadError, MIGRATIONS, MigrationStep, SchemaError, StructureError, load, migrate, migrate_with,
    ptsm_schema,
};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

/// The written form's member order of a stored profile (file-format.md section 3.2).
const WRITTEN_ORDER: [&str; 9] = [
    "id",
    "name",
    "technology",
    "bed",
    "nozzle_diameter",
    "layer_height",
    "max_overhang_angle",
    "max_bridge_length",
    "fit",
];

/// E11 with `old` replaced by `new` exactly once.
fn e11_with(old: &str, new: &str) -> String {
    assert!(E11.contains(old), "E11 has no {old:?}");
    E11.replacen(old, new, 1)
}

/// The canonical text of E11 with the members of its stored profile in reversed order.
fn e11_reversed() -> String {
    let mut document = parse(E11).expect("E11 is strict JSON");
    let profile = document["printer_profile"]
        .as_object()
        .expect("E11 stores a profile")
        .clone();
    let reversed: Map<String, Value> = profile.into_iter().rev().collect();
    document["printer_profile"] = Value::Object(reversed);
    to_string(&document)
}

/// The step `S` of test 66: from version 1, returns its input.
fn returns_its_input(document: Value) -> Result<Value, String> {
    Ok(document)
}

#[test]
fn t059_a_version_2_document_changes_only_its_version() {
    for (name, text, length) in [("E2", E2, 1614), ("E10", E10, 3081)] {
        let input = with_format_version(text, Some("2"));

        let loaded = load(&input).unwrap_or_else(|error| panic!("{name} at version 2: {error}"));

        assert_eq!(loaded.migrated_from, Some(2), "{name}");
        assert_eq!(loaded.document, load_document(text), "{name}");
        let saved = save_text(&loaded.document);
        assert_eq!(saved, text, "{name}");
        assert_eq!(saved.len(), length, "{name}");
        assert_eq!(differing_lines(&input, &saved), [3], "{name}");
    }
}

#[test]
fn t060_e11_round_trips_with_its_stored_profile() {
    let loaded = load(E11).expect("E11 loads");

    assert_eq!(loaded.migrated_from, None);
    let document = &loaded.document;
    assert_eq!(document.printer.as_deref(), Some("my-mk4"));
    let profile = document
        .printer_profile
        .as_ref()
        .expect("E11 stores a profile");
    assert_eq!(
        profile.keys().map(String::as_str).collect::<Vec<_>>(),
        WRITTEN_ORDER
    );
    assert_eq!(profile, &e11_profile());
    let saved = save_text(document);
    assert_eq!(saved, E11);
    assert_eq!(saved.len(), 588);
}

#[test]
fn t061_the_model_keeps_a_stored_profile_as_it_is() {
    let reversed = e11_reversed();
    let reversed_order: Vec<&str> = WRITTEN_ORDER.iter().rev().copied().collect();

    let document = load_document(&reversed);

    let profile = document
        .printer_profile
        .as_ref()
        .expect("the text stores a profile");
    assert_eq!(
        profile.keys().map(String::as_str).collect::<Vec<_>>(),
        reversed_order
    );
    let saved = save_text(&document);
    assert_eq!(saved, reversed);
    let saved_json = parse(&saved).expect("the saved text is strict JSON");
    assert_eq!(keys_at(&saved_json, "/printer_profile"), reversed_order);
}

#[test]
fn t062_a_stored_profile_needs_a_printer() {
    let result = load(&e11_with("  \"printer\": \"my-mk4\",\n", ""));

    assert_eq!(
        result,
        Err(LoadError::Structure(vec![
            StructureError::PrinterProfileWithoutPrinter
        ]))
    );
    assert_eq!(
        result.expect_err("the profile has no printer").to_string(),
        "This document has a problem: The document stores a printer profile but names no printer. Set \"printer\" to the profile's id, or remove \"printer_profile\"."
    );
}

#[test]
fn t063_a_stored_profile_of_another_printer_is_refused() {
    let result = load(&e11_with(
        "  \"printer\": \"my-mk4\",",
        "  \"printer\": \"my-mk3\",",
    ));

    assert_eq!(
        result,
        Err(LoadError::Structure(vec![
            StructureError::PrinterProfileIdMismatch {
                printer: "my-mk3".to_string(),
                found: Some("my-mk4".to_string())
            }
        ]))
    );
    assert_eq!(
        result.expect_err("the ids differ").to_string(),
        "This document has a problem: The stored printer profile has the id \"my-mk4\", but the document names the printer \"my-mk3\". Make them the same."
    );
}

#[test]
fn t064_a_stored_profile_without_a_string_id_is_refused() {
    let without_id = e11_with("    \"id\": \"my-mk4\",\n", "");
    let numeric_id = e11_with("    \"id\": \"my-mk4\",", "    \"id\": 7,");
    let expected = Err(LoadError::Structure(vec![
        StructureError::PrinterProfileIdMismatch {
            printer: "my-mk4".to_string(),
            found: None,
        },
    ]));
    let message = "This document has a problem: The stored printer profile needs the id \"my-mk4\", the printer the document names. Set its \"id\" to \"my-mk4\".";

    for (case, text) in [("without an id", without_id), ("with the id 7", numeric_id)] {
        let result = load(&text);

        assert_eq!(result, expected, "{case}");
        assert_eq!(
            result.expect_err("the id is not the printer").to_string(),
            message,
            "{case}"
        );
    }
}

#[test]
fn t065_a_stored_profile_is_an_object_and_null_means_absent() {
    let open_brace = "  \"printer_profile\": {\n";
    let start = E11.find(open_brace).expect("E11 stores a profile");
    let end = start + E11[start..].find("\n  },\n").expect("the profile closes") + "\n  },\n".len();
    let with_value = |value: &str| {
        format!(
            "{}  \"printer_profile\": {value},\n{}",
            &E11[..start],
            &E11[end..]
        )
    };

    let array = load(&with_value("[]"));
    let null = load(&with_value("null"));

    let Err(LoadError::Invalid { path, .. }) = array else {
        panic!("an array is not a stored profile: {array:?}");
    };
    assert_eq!(path, "printer_profile");
    let loaded = null.expect("null means no stored profile");
    assert_eq!(loaded.document.printer_profile, None);
    assert_eq!(loaded.document.printer.as_deref(), Some("my-mk4"));
}

#[test]
fn t066_a_build_that_reads_up_to_version_2_stops_cleanly() {
    let document: Value = serde_json::from_str(E11).expect("E11 is JSON");
    let step = MigrationStep {
        from: 1,
        migrate: returns_its_input,
    };

    let result = migrate_with(document, 2, 1, &[step]);

    assert_eq!(
        result,
        Err(LoadError::NewerFormatVersion {
            found: 3,
            supported: 2
        })
    );
    assert_eq!(
        result.expect_err("version 3 is newer").to_string(),
        "This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it."
    );
}

#[test]
fn t067_the_built_in_chain_has_two_steps_and_keeps_every_member() {
    let froms: Vec<u32> = MIGRATIONS.iter().map(|step| step.from).collect();
    let text = with_format_version(E1, Some("2")).replacen(
        "  \"units\": \"mm\",\n",
        "  \"units\": \"mm\",\n  \"x\": 1,\n",
        1,
    );
    let document: Value = serde_json::from_str(&text).expect("the text is JSON");

    let (migrated, original) = migrate(document).expect("version 2 migrates");

    assert_eq!(froms.len(), 2);
    assert!(froms.contains(&1) && froms.contains(&2), "{froms:?}");
    assert_eq!(migrated["format_version"], json!(3));
    assert_eq!(migrated["x"], json!(1));
    assert_eq!(original, Some(2));
    let Err(LoadError::Invalid { path, .. }) = load(&text) else {
        panic!("x is not a document key");
    };
    assert_eq!(path, "x");
}

#[test]
fn t068_the_printer_profile_schema_is_hoisted() {
    let profile_schema = schemars::Schema::try_from(json!({
        "type": "object",
        "title": "PrinterProfile",
        "properties": {"bed": {"$ref": "#/$defs/Bed"}},
        "$defs": {"Bed": {"type": "object"}}
    }))
    .expect("the schema is a JSON object");
    let clashing_box = partsmith_model::FeatureParamsSchema {
        kind: "box".to_string(),
        schema: schemars::Schema::try_from(json!({
            "type": "object",
            "$defs": {"Bed": {"type": "string"}}
        }))
        .expect("the schema is a JSON object"),
    };

    let schema = ptsm_schema(&[], &profile_schema)
        .expect("the schema builds")
        .as_value()
        .clone();
    let clash = ptsm_schema(&[clashing_box], &profile_schema);

    assert_eq!(
        schema.pointer("/$defs/Bed"),
        Some(&json!({"type": "object"}))
    );
    assert_eq!(
        schema.pointer("/$defs/PrinterProfile"),
        Some(&json!({"type": "object", "properties": {"bed": {"$ref": "#/$defs/Bed"}}}))
    );
    assert_eq!(
        clash.map(|schema| schema.as_value().clone()),
        Err(SchemaError::ConflictingDefinition {
            name: "Bed".to_string()
        })
    );
}

#[test]
fn a_document_with_a_stored_profile_round_trips_through_load_and_save() {
    let document = load_document(E11);

    assert_eq!(load_document(&save_text(&document)), document);
}

#[test]
fn the_loader_keeps_a_stored_profile_it_would_not_accept_in_print() {
    let text = e11_with("    \"layer_height\": 0.15,", "    \"layer_height\": 0.5,").replacen(
        "    \"name\": \"My MK4\",",
        "    \"name\": \"My MK4\",\n    \"speed\": 100,",
        1,
    );

    let document = load_document(&text);

    let profile = document
        .printer_profile
        .as_ref()
        .expect("the text stores a profile");
    assert_eq!(profile.get("layer_height"), Some(&json!(0.5)));
    assert_eq!(profile.get("speed"), Some(&json!(100)));
}
