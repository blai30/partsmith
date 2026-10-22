//! Cache keys (recompute.md section 3.4): the exact hashed byte sequence and its consequences,
//! checked over test-only features. Recompute tests 6 to 16 and 35 check keys on the built-in
//! features (`runs_and_cache.rs`).

use crate::common;

use std::sync::Arc;

use common::{
    Harness, bodies, body_record, datum_height, document, document_json, error_code, hex, key,
    keys, load, owned, parameter, parameter_record, record, step, suppressed, test_block,
    test_datum, test_datum_on, test_node, test_probe, upstream, with_action,
};
use partsmith_model::Quantity;
use partsmith_recompute::{CacheKey, Plan};
use proptest::prelude::*;
use serde_json::{Value, json};

/// Item 2 of the key: the engine signature.
fn engine_signature() -> String {
    format!(
        "partsmith-features {} occt {}",
        partsmith_features::VERSION,
        partsmith_features::OCCT_VERSION
    )
}

/// Items 1 to 4 of the key of a feature with manifest version 1.
fn header(feature_type: &str, feature: &str) -> Vec<String> {
    vec![
        "partsmith-recompute 1".to_string(),
        engine_signature(),
        format!("type {feature_type}"),
        "version 1".to_string(),
        format!("id {feature}"),
    ]
}

/// The `value` item of an evaluated value. The values used here have a non-zero first hex digit,
/// so the text is the same with or without zero padding.
fn value_item(value: f64) -> String {
    format!("value {:016x}", value.to_bits())
}

/// The BLAKE3 hash of the items, each followed by one line feed.
fn hash(items: &[String]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for item in items {
        bytes.extend_from_slice(item.as_bytes());
        bytes.push(b'\n');
    }
    *blake3::hash(&bytes).as_bytes()
}

/// The key a run gives a feature.
fn run_key(harness: &mut Harness, document: &Value, feature: &str) -> CacheKey {
    let report = harness.run(&load(document));
    key(&report, feature).unwrap_or_else(|| panic!("{feature} has no key"))
}

/// The key a plan gives a feature.
fn plan_key(plan: &Plan, feature: &str) -> CacheKey {
    step(plan, feature)
        .key
        .unwrap_or_else(|| panic!("{feature} has no planned key"))
}

#[test]
fn key_is_the_blake3_hash_of_the_specified_items() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({"features": [test_datum("f-d1", "5")]}));

    let key = run_key(&mut harness, &document, "f-d1");

    let mut items = header("test_datum", "f-d1");
    items.push(r#"params {"offset":"5"}"#.to_string());
    items.push("expr offset length".to_string());
    items.push(value_item(5.0));
    assert_eq!(key.0, hash(&items));
}

#[test]
fn key_lists_values_then_feature_links_then_read_and_written_bodies() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({
        "bodies": bodies(2),
        "features": [
            test_datum("f-d1", "5"),
            test_block("f-a", "b-1", "2", "new"),
            test_node("f-n", &["f-d1"], &["b-2"], &["b-1"], &["3"])
        ]
    }));

    let report = harness.run(&load(&document));

    let datum_key = key(&report, "f-d1").expect("f-d1 has a key");
    let block_key = key(&report, "f-a").expect("f-a has a key");
    let mut items = header("test_node", "f-n");
    items.push(
        r#"params {"uses":["f-d1"],"reads":["b-2"],"writes":["b-1"],"values":["3"]}"#.to_string(),
    );
    items.push("expr values[0] length".to_string());
    items.push(value_item(3.0));
    items.push(format!("feature f-d1 {}", hex(&datum_key)));
    items.push("body b-2 empty".to_string());
    items.push(format!("body b-1 {}", hex(&block_key)));
    assert_eq!(key(&report, "f-n").expect("f-n has a key").0, hash(&items));
}

#[test]
fn key_of_a_first_writer_has_an_empty_body_item() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({
        "bodies": bodies(1),
        "features": [test_block("f-a", "b-1", "2", "new")]
    }));

    let key = run_key(&mut harness, &document, "f-a");

    let mut items = header("test_block", "f-a");
    items.push(r#"params {"body":"b-1","size":"2","op":"new"}"#.to_string());
    items.push("expr size length".to_string());
    items.push(value_item(2.0));
    items.push("body b-1 empty".to_string());
    assert_eq!(key.0, hash(&items));
}

#[test]
fn key_writes_none_for_a_feature_link_without_a_key() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({
        "features": [suppressed(test_datum("f-s", "1")), test_datum_on("f-d2", "f-s", "5")]
    }));

    let key = run_key(&mut harness, &document, "f-d2");

    let mut items = header("test_datum", "f-d2");
    items.push(r#"params {"base":"f-s","offset":"5"}"#.to_string());
    items.push("expr offset length".to_string());
    items.push(value_item(5.0));
    items.push("feature f-s none".to_string());
    assert_eq!(key.0, hash(&items));
}

#[test]
fn key_records_an_evaluation_error_with_its_code_and_display_text() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({"features": [test_datum("f-e", "nope")]}));

    let key = run_key(&mut harness, &document, "f-e");

    let mut items = header("test_datum", "f-e");
    items.push(r#"params {"offset":"nope"}"#.to_string());
    items.push("expr offset length".to_string());
    items.push(
        "error expression Offset: Unknown name 'nope'. Check the spelling, or add a parameter named nope."
            .to_string(),
    );
    assert_eq!(key.0, hash(&items));
}

#[test]
fn key_hashes_params_in_canonical_order_whatever_the_stored_order() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({
        "bodies": bodies(1),
        "features": [{"op": "new", "size": "2", "type": "test_block", "body": "b-1", "id": "f-a"}]
    }));

    let key = run_key(&mut harness, &document, "f-a");

    let mut items = header("test_block", "f-a");
    items.push(r#"params {"body":"b-1","size":"2","op":"new"}"#.to_string());
    items.push("expr size length".to_string());
    items.push(value_item(2.0));
    items.push("body b-1 empty".to_string());
    assert_eq!(key.0, hash(&items));
}

#[test]
fn unknown_keys_are_not_in_the_key() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({"features": [test_datum("f-d1", "5")]}));
    record(&mut document, "f-d1")["taper"] = json!("2 deg");

    let key = run_key(&mut harness, &document, "f-d1");

    let mut items = header("test_datum", "f-d1");
    items.push(r#"params {"offset":"5"}"#.to_string());
    items.push("expr offset length".to_string());
    items.push(value_item(5.0));
    assert_eq!(key.0, hash(&items));
}

#[test]
fn only_active_nodes_have_keys() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [
            test_datum("f-d1", "1"),
            suppressed(test_datum("f-s", "2")),
            {"id": "f-g", "type": "gear_demo", "teeth": 12},
            test_datum("f-r", "3")
        ],
        "rollback": "f-r"
    }));

    let plan = harness.plan(&document);
    let report = harness.run(&document);

    assert!(key(&report, "f-d1").is_some());
    for feature in ["f-s", "f-g", "f-r"] {
        assert_eq!(key(&report, feature), None, "{feature} has a key");
        assert_eq!(
            step(&plan, feature).key,
            None,
            "{feature} has a planned key"
        );
    }
}

#[test]
fn a_node_that_fails_its_pre_build_checks_keeps_its_key() {
    let mut harness = Harness::test_features();
    let document = document(json!({"features": [test_datum_on("f-d1", "f-nope", "1")]}));

    let plan = harness.plan(&document);
    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-d1"), "unknown_feature");
    assert!(key(&report, "f-d1").is_some());
    assert_eq!(key(&report, "f-d1"), Some(plan_key(&plan, "f-d1")));
}

#[test]
fn plan_and_run_agree_on_every_key() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "parameters": [parameter("p-h", "h", "2 mm")],
        "bodies": bodies(2),
        "features": [
            test_datum("f-d1", "5"),
            test_datum_on("f-d2", "f-d1", "h"),
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-b", "b-1", "h", "join"),
            test_datum("f-e", "nope"),
            test_datum_on("f-f", "f-e", "1"),
            test_probe("f-p", "b-1")
        ]
    }));

    let plan = harness.plan(&document);
    let report = harness.run(&document);

    let planned: Vec<_> = plan
        .steps
        .iter()
        .map(|step| (step.feature.to_string(), step.key))
        .collect();
    assert_eq!(planned, keys(&report));
}

#[test]
fn the_feature_id_is_in_the_key() {
    let mut harness = Harness::test_features();

    let first = run_key(
        &mut harness,
        &document_json(json!({"features": [test_datum("f-d1", "5")]})),
        "f-d1",
    );
    let second = run_key(
        &mut harness,
        &document_json(json!({"features": [test_datum("f-d2", "5")]})),
        "f-d2",
    );

    assert_ne!(first, second);
}

#[test]
fn names_colors_and_materials_are_not_in_keys() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "bodies": bodies(1),
        "features": [test_block("f-a", "b-1", "2", "new"), test_probe("f-p", "b-1")]
    }));
    let before = keys(&harness.run(&load(&document)));

    body_record(&mut document, "b-1")["name"] = json!("Bracket");
    body_record(&mut document, "b-1")["color"] = json!("#3b82f6");
    body_record(&mut document, "b-1")["material"] = json!("PETG");
    record(&mut document, "f-a")["name"] = json!("Base");
    let after = keys(&harness.run(&load(&document)));

    assert_eq!(before, after);
}

#[test]
fn equal_values_give_equal_keys() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "parameters": [parameter("p-h", "h", "2 mm")],
        "features": [test_datum("f-d1", "h")]
    }));
    let before = run_key(&mut harness, &document, "f-d1");

    parameter_record(&mut document, "p-h")["expr"] = json!("1 mm + 1 mm");
    let after = run_key(&mut harness, &document, "f-d1");

    assert_eq!(before, after);
}

#[test]
fn a_units_change_changes_only_keys_whose_converted_values_change() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "features": [test_datum("f-d1", "10"), test_datum("f-d2", "10 mm")]
    }));
    let before = harness.run(&load(&document));

    document["units"] = json!("in");
    let after = harness.run(&load(&document));

    assert_ne!(key(&before, "f-d1"), key(&after, "f-d1"));
    assert_eq!(key(&before, "f-d2"), key(&after, "f-d2"));
}

#[test]
fn context_values_are_in_keys_through_evaluated_values() {
    let mut harness = Harness::test_features();
    let document = document_json(json!({
        "features": [test_datum("f-d1", "printer.fit.slip * 100")]
    }));

    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let first = run_key(&mut harness, &document, "f-d1");
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.3));
    let second = run_key(&mut harness, &document, "f-d1");
    harness
        .context
        .insert("printer.fit.slip".to_string(), Quantity::length(0.2));
    let third = run_key(&mut harness, &document, "f-d1");

    assert_ne!(first, second);
    assert_eq!(first, third);
}

#[test]
fn asset_bytes_are_keyed_by_the_asset_name_only() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [{"id": "f-a1", "type": "test_asset", "asset": "a.step"}]
    }));
    harness
        .assets
        .insert("a.step".to_string(), Arc::from(&b"abcd"[..]));
    let first = harness.run(&document);

    // The host names assets by the hash of their bytes, so new bytes under the same name never
    // happen in practice; the key does not see them.
    harness
        .assets
        .insert("a.step".to_string(), Arc::from(&b"abcdef"[..]));
    let plan = harness.plan(&document);
    let second = harness.run(&document);

    assert_eq!(with_action(&plan, "reuse"), owned(&["f-a1"]));
    assert_eq!(key(&first, "f-a1"), key(&second, "f-a1"));
    assert_eq!(datum_height(&second, "f-a1"), 4.0);
}

#[test]
fn different_asset_names_give_different_keys() {
    let mut harness = Harness::test_features();
    harness
        .assets
        .insert("a.step".to_string(), Arc::from(&b"abcd"[..]));
    harness
        .assets
        .insert("b.step".to_string(), Arc::from(&b"abcd"[..]));

    let first = run_key(
        &mut harness,
        &document_json(
            json!({"features": [{"id": "f-a1", "type": "test_asset", "asset": "a.step"}]}),
        ),
        "f-a1",
    );
    let second = run_key(
        &mut harness,
        &document_json(
            json!({"features": [{"id": "f-a1", "type": "test_asset", "asset": "b.step"}]}),
        ),
        "f-a1",
    );

    assert_ne!(first, second);
}

#[test]
fn a_failed_writer_stays_the_provider_of_its_bodies() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-b", "b-1", "3", "new"),
            test_probe("f-p", "b-1")
        ]
    }));
    let report = harness.run(&load(&document));
    assert_eq!(error_code(&report, "f-b"), "body_not_empty");

    record(&mut document, "f-b")["size"] = json!("4");
    let graph = harness.graph(&load(&document));
    let plan = harness.plan(&load(&document));

    assert_eq!(upstream(&graph, "f-p"), owned(&["body b-1 f-b"]));
    assert_eq!(with_action(&plan, "build"), owned(&["f-b", "f-p"]));
}

#[test]
fn units_of_the_document_do_not_change_keys_of_unit_free_features() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "bodies": bodies(1),
        "features": [test_probe("f-p", "b-1")]
    }));
    let before = run_key(&mut harness, &document, "f-p");

    document["units"] = json!("cm");
    let after = run_key(&mut harness, &document, "f-p");

    assert_eq!(before, after);
}

#[test]
fn cache_key_displays_as_64_lowercase_hex_characters() {
    let key = CacheKey([0xab; 32]);

    assert_eq!(key.to_string(), "ab".repeat(32));
}

proptest! {
    #[test]
    fn cache_key_display_is_the_lowercase_hex_of_its_bytes(bytes in proptest::array::uniform32(any::<u8>())) {
        let key = CacheKey(bytes);

        let text = key.to_string();

        prop_assert_eq!(text.len(), 64);
        prop_assert!(text.chars().all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character)));
        prop_assert_eq!(text, hex(&key));
    }
}
