//! Acceptance tests for params validation and canonicalization, dependencies, the feature inspector and labels (features.md section 6.2, tests 8 to 14, 16 and 21), and the near matches of unknown keys (section 3.2, section 6.5 test 8).
//!
//! Tests of the built-in feature types each have a companion with the same number that checks the
//! same framework behavior with the `slab` test feature, whose params have extrude's shape.

use crate::common;

use std::collections::BTreeMap;

use common::{
    NoNames, Probe, Slab, evaluate, extrude_record, field, id, link_pairs, params_text, pick,
    record, record_text, rectangle_profile, rectangle_sketch_record, render_error, slab_manifest,
    slab_record, use_triples, with_render_context,
};
use partsmith_features::{
    Diagnosis, DocumentNames, FeatureError, FieldRef, FieldUnit, Fix, Registry, Validated,
};
use partsmith_model::{
    Document, Expr, ExprLink, FeatureInspector, FeatureLinks, FeatureRecord, Units,
};
use proptest::prelude::*;
use serde_json::{Map, Value, json};

/// A registry holding the probe and slab test features.
fn test_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register::<Probe>()
        .expect("the probe test feature registers");
    registry
        .register::<Slab>()
        .expect("the slab test feature registers");
    registry
}

/// Validates and canonicalizes a record that must be valid.
fn canonical(registry: &Registry, record: &FeatureRecord) -> Validated {
    registry
        .validate_and_canonicalize(record)
        .expect("the record is valid")
}

/// The `detail` of an `InvalidParams` error, or a panic naming what was returned instead.
fn invalid_params_detail(result: Result<Validated, FeatureError>) -> String {
    match result {
        Err(FeatureError::InvalidParams { detail, .. }) => detail,
        Err(other) => panic!("expected InvalidParams, got {other:?}"),
        Ok(validated) => panic!(
            "expected InvalidParams, got valid params {}",
            params_text(&validated.record)
        ),
    }
}

/// The non-canonical params of features.md section 6.2 test 8, as a record of type `kind`.
fn shuffled_record(id_text: &str, kind: &str) -> FeatureRecord {
    let mut shuffled = record(json!({"id": id_text, "type": kind}));
    shuffled.params = params(json!({
        "op": "new",
        "reverse": false,
        "extent": {"expr": "30 mm", "kind": "distance"},
        "taper": "2 deg",
        "profile": {"regions": [{"fp": "c=40,25|a=4000", "key": "e-1,e-2,e-3,e-4"}], "sketch": "f-sk1"},
        "body": "b-1"
    }));
    shuffled
}

/// The canonical form of the params of `shuffled_record`, as compact JSON text.
fn canonical_shuffled_params_text() -> String {
    json!({
        "body": "b-1",
        "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
        "extent": {"kind": "distance", "expr": "30 mm"},
        "op": "new",
        "taper": "2 deg"
    })
    .to_string()
}

/// The map of a JSON object literal, keeping its key order.
fn params(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("params must be a JSON object, got {other}"),
    }
}

/// A copy of `record` with one params key replaced or added.
fn with_param(mut record: FeatureRecord, key: &str, value: Value) -> FeatureRecord {
    record.params.insert(key.to_string(), value);
    record
}

/// A copy of `record` without one params key.
fn without_param(mut record: FeatureRecord, key: &str) -> FeatureRecord {
    record.params.shift_remove(key);
    record
}

/// A document holding `features` in timeline order.
fn document(features: Vec<FeatureRecord>) -> Document {
    let mut document = Document::new(Units::Mm);
    document.features = features;
    document
}

/// An unsuppressed record with empty params and an optional name.
fn bare(id_text: &str, kind: &str, name: Option<&str>) -> FeatureRecord {
    let mut bare = FeatureRecord::new(id(id_text), kind);
    bare.name = name.map(str::to_string);
    bare
}

/// The label of each feature of `document`, in timeline order.
fn labels(registry: &Registry, document: &Document) -> Vec<Option<String>> {
    document
        .features
        .iter()
        .map(|feature| registry.label(document, &feature.id))
        .collect()
}

#[test]
fn t008_extrude_params_take_struct_order_without_defaults() {
    let registry = Registry::with_builtins();

    let validated = canonical(&registry, &shuffled_record("f-ext1", "extrude"));

    assert_eq!(
        params_text(&validated.record),
        canonical_shuffled_params_text()
    );
}

#[test]
fn t008_extrude_unknown_keys_are_reported() {
    let registry = Registry::with_builtins();

    let validated = canonical(&registry, &shuffled_record("f-ext1", "extrude"));

    assert_eq!(validated.unknown_keys, ["taper"]);
}

#[test]
fn t008_params_take_struct_order_without_defaults() {
    let registry = test_registry();

    let validated = canonical(&registry, &shuffled_record("f-slab1", "slab"));

    assert_eq!(
        params_text(&validated.record),
        canonical_shuffled_params_text()
    );
}

#[test]
fn t008_unknown_keys_are_reported() {
    let registry = test_registry();

    let validated = canonical(&registry, &shuffled_record("f-slab1", "slab"));

    assert_eq!(validated.unknown_keys, ["taper"]);
}

#[test]
fn t008_record_fields_other_than_params_are_unchanged() {
    let registry = test_registry();
    let mut input = shuffled_record("f-slab1", "slab");
    input.name = Some("Base".to_string());
    input.suppressed = true;

    let validated = canonical(&registry, &input);

    assert_eq!(validated.record.id, id("f-slab1"));
    assert_eq!(validated.record.kind, "slab");
    assert_eq!(validated.record.name.as_deref(), Some("Base"));
    assert!(validated.record.suppressed);
}

#[test]
fn t009_canonicalizing_canonical_extrude_params_changes_nothing() {
    let registry = Registry::with_builtins();
    let first = canonical(&registry, &shuffled_record("f-ext1", "extrude"));

    let second = canonical(&registry, &first.record);

    assert_eq!(record_text(&second.record), record_text(&first.record));
    assert_eq!(second.unknown_keys, first.unknown_keys);
}

#[test]
fn t009_canonicalizing_canonical_params_changes_nothing() {
    let registry = test_registry();
    let first = canonical(&registry, &shuffled_record("f-slab1", "slab"));

    let second = canonical(&registry, &first.record);

    assert_eq!(record_text(&second.record), record_text(&first.record));
    assert_eq!(second.unknown_keys, first.unknown_keys);
}

/// Known slab params with values, in struct order; `reverse` is left out when false.
fn known_slab_params(reverse: bool) -> Vec<(&'static str, Value)> {
    let mut known = vec![
        ("body", json!("b-1")),
        ("profile", rectangle_profile()),
        (
            "extent",
            json!({"kind": "two_sides", "expr": "20", "opposite_expr": "5 mm"}),
        ),
    ];
    if reverse {
        known.push(("reverse", json!(true)));
    }
    known.push(("op", json!("cut")));
    known
}

/// A slab record with the unknown entries first, then the known params in `order`.
fn arbitrary_slab_record(
    order: &[usize],
    reverse: bool,
    unknown: &[(String, String)],
) -> FeatureRecord {
    let known = known_slab_params(reverse);
    let mut input = FeatureRecord::new(id("f-slab1"), "slab");
    for (key, value) in unknown {
        input.params.insert(key.clone(), json!(value));
    }
    for &position in order.iter().filter(|&&position| position < known.len()) {
        let (key, value) = &known[position];
        input.params.insert(key.to_string(), value.clone());
    }
    input
}

/// Unknown keys that are not slab params or record fields, with string values.
fn unknown_entries() -> impl Strategy<Value = Vec<(String, String)>> {
    proptest::collection::vec(("x_[a-z]{1,8}", "[a-z0-9 ]{0,8}"), 0..4)
}

proptest! {
    #[test]
    fn t009_canonicalizing_is_idempotent_for_any_key_order(
        order in Just((0..5).collect::<Vec<usize>>()).prop_shuffle(),
        reverse in any::<bool>(),
        unknown in unknown_entries(),
    ) {
        let registry = test_registry();
        let input = arbitrary_slab_record(&order, reverse, &unknown);

        let first = canonical(&registry, &input);
        let second = canonical(&registry, &first.record);

        prop_assert_eq!(record_text(&second.record), record_text(&first.record));
        prop_assert_eq!(second.unknown_keys, first.unknown_keys);
    }

    #[test]
    fn t009_canonical_keys_are_known_keys_in_struct_order_then_unknown_keys_in_input_order(
        order in Just((0..5).collect::<Vec<usize>>()).prop_shuffle(),
        reverse in any::<bool>(),
        unknown in unknown_entries(),
    ) {
        let registry = test_registry();
        let input = arbitrary_slab_record(&order, reverse, &unknown);
        // Inserting a repeated key keeps its first position, so the input map holds the expected order.
        let unknown_keys: Vec<String> = input
            .params
            .keys()
            .filter(|key| key.starts_with("x_"))
            .cloned()
            .collect();

        let validated = canonical(&registry, &input);

        let mut expected: Vec<String> = known_slab_params(reverse)
            .into_iter()
            .map(|(key, _)| key.to_string())
            .collect();
        expected.extend(unknown_keys.iter().cloned());
        prop_assert_eq!(validated.record.params.keys().cloned().collect::<Vec<_>>(), expected);
        prop_assert_eq!(validated.unknown_keys, unknown_keys);
    }
}

#[test]
fn t010_unknown_type_is_reported() {
    let registry = Registry::with_builtins();
    let gear = record(json!({"id": "f-g1", "type": "gear", "teeth": 12}));

    let result = registry.validate_and_canonicalize(&gear);

    assert_eq!(
        result.err(),
        Some(FeatureError::UnknownType {
            type_name: "gear".to_string()
        })
    );
}

#[test]
fn t010_unknown_type_message() {
    let error = FeatureError::UnknownType {
        type_name: "gear".to_string(),
    };

    assert_eq!(
        render_error(&error, "f-ext1", &NoNames),
        "This document has a feature of type 'gear', which this version of Partsmith does not have. Update Partsmith, or delete the feature."
    );
}

/// The message and the fixes of `error` on the record `invalid`, rendered with `DocumentNames` for a millimeter
/// document holding only that record and the built-in registry, as recompute renders an `invalid_params` error.
fn rendered_in_document(error: &FeatureError, invalid: &FeatureRecord) -> (String, Vec<Fix>) {
    let registry = Registry::with_builtins();
    let document = document(vec![invalid.clone()]);
    let outputs = BTreeMap::new();
    let names = DocumentNames {
        document: &document,
        registry: &registry,
        outputs: &outputs,
    };
    with_render_context(
        &names,
        invalid.id.as_str(),
        Units::Mm,
        &Diagnosis::default(),
        |context| (error.render(context), error.fixes(context)),
    )
}

#[test]
fn t011_extrude_without_body_says_which_setting_cannot_be_read() {
    let registry = Registry::with_builtins();
    let invalid = without_param(extrude_record(), "body");
    let error = registry
        .validate_and_canonicalize(&invalid)
        .expect_err("an extrude without a body is invalid");

    let (text, fixes) = rendered_in_document(&error, &invalid);

    assert_eq!(
        text,
        "Body of this extrude cannot be read, so it was not built. Set Body again."
    );
    // The Body hint has a selection, so the field can be picked again.
    assert_eq!(fixes, [pick("/body")]);
    assert_eq!(
        error.detail().as_deref(),
        Some("the setting 'body' is missing")
    );
}

#[test]
fn t011_extrude_without_body_names_the_missing_setting() {
    let registry = Registry::with_builtins();

    let result = registry.validate_and_canonicalize(&without_param(extrude_record(), "body"));

    assert_eq!(
        invalid_params_detail(result),
        "the setting 'body' is missing"
    );
}

/// The path and label of the field of an `InvalidParams` error, or a panic naming what was returned instead.
fn invalid_params_field(result: Result<Validated, FeatureError>) -> Option<(String, String)> {
    match result {
        Err(FeatureError::InvalidParams { field, .. }) => {
            field.map(|field| (field.path, field.label))
        }
        Err(other) => panic!("expected InvalidParams, got {other:?}"),
        Ok(validated) => panic!(
            "expected InvalidParams, got valid params {}",
            params_text(&validated.record)
        ),
    }
}

#[test]
fn t011_extrude_without_body_is_about_its_body_field() {
    let registry = Registry::with_builtins();

    let result = registry.validate_and_canonicalize(&without_param(extrude_record(), "body"));

    assert_eq!(
        invalid_params_field(result),
        Some(("body".to_owned(), "Body".to_owned()))
    );
}

#[test]
fn t011_missing_field_names_the_missing_setting() {
    let registry = test_registry();

    let result = registry.validate_and_canonicalize(&without_param(slab_record(), "body"));

    assert_eq!(
        invalid_params_detail(result),
        "the setting 'body' is missing"
    );
}

#[test]
fn t012_extrude_with_unknown_op_names_the_invalid_choice() {
    let registry = Registry::with_builtins();

    let result =
        registry.validate_and_canonicalize(&with_param(extrude_record(), "op", json!("merge")));

    assert_eq!(
        invalid_params_detail(result),
        "'merge' is not a valid choice for 'op'"
    );
}

#[test]
fn t012_extrude_with_unknown_op_is_about_its_operation_field() {
    let registry = Registry::with_builtins();

    let result =
        registry.validate_and_canonicalize(&with_param(extrude_record(), "op", json!("merge")));

    assert_eq!(
        invalid_params_field(result),
        Some(("op".to_owned(), "Operation".to_owned()))
    );
}

#[test]
fn t012_unknown_variant_names_the_invalid_choice() {
    let registry = test_registry();

    let result =
        registry.validate_and_canonicalize(&with_param(slab_record(), "op", json!("merge")));

    assert_eq!(
        invalid_params_detail(result),
        "'merge' is not a valid choice for 'op'"
    );
}

#[test]
fn t012_value_of_the_wrong_type_names_the_field() {
    let registry = test_registry();

    let result = registry.validate_and_canonicalize(&with_param(slab_record(), "body", json!(5)));

    let detail = invalid_params_detail(result);
    assert!(
        detail.starts_with("'body' has the wrong form (") && detail.ends_with(')'),
        "got {detail:?}"
    );
}

#[test]
fn t012_extrude_with_unknown_op_says_which_setting_cannot_be_read() {
    let registry = Registry::with_builtins();
    let invalid = with_param(extrude_record(), "op", json!("merge"));
    let error = registry
        .validate_and_canonicalize(&invalid)
        .expect_err("'merge' is not an operation");

    let (text, fixes) = rendered_in_document(&error, &invalid);

    assert_eq!(
        text,
        "Operation of this extrude cannot be read, so it was not built. Set Operation again."
    );
    // The Operation hint has options, not a selection, so there is nothing to pick.
    assert_eq!(fixes, []);
    assert_eq!(
        error.detail().as_deref(),
        Some("'merge' is not a valid choice for 'op'")
    );
}

#[test]
fn t012_invalid_params_without_a_field_ask_to_set_the_settings_again() {
    let error = FeatureError::InvalidParams {
        field: None,
        detail: "the settings are not a JSON object".to_string(),
    };

    let (text, fixes) = rendered_in_document(&error, &extrude_record());

    assert_eq!(
        text,
        "The settings of this extrude cannot be read, so it was not built. Edit it to set them again, or delete it."
    );
    assert_eq!(fixes, []);
}

#[test]
fn t012_invalid_params_with_a_field_render_with_the_field_label() {
    let error = FeatureError::InvalidParams {
        field: Some(field("radius", "Radius")),
        detail: "'radius' has the wrong form (invalid type: map, expected a string)".to_string(),
    };

    assert_eq!(
        render_error(&error, "f-fil1", &NoNames),
        "Radius of this feature cannot be read, so it was not built. Set Radius again."
    );
}

/// A shell record on b-1 with `extra` added after its own params (features.md section 6.5 test 8).
fn shell_with(extra: &[(&str, Value)]) -> FeatureRecord {
    let mut shell = record(json!({
        "id": "f-sh1",
        "type": "shell",
        "body": "b-1",
        "thickness": "2",
        "remove_faces": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"}]
    }));
    for (key, value) in extra {
        shell.params.insert((*key).to_string(), value.clone());
    }
    shell
}

/// A shell record on b-1 with `extra` in place of `remove_faces`, as when a typo replaced that key (features.md
/// section 6.5 test 8).
fn misspelled_shell(extra: &[(&str, Value)]) -> FeatureRecord {
    let mut shell =
        record(json!({"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2"}));
    for (key, value) in extra {
        shell.params.insert((*key).to_string(), value.clone());
    }
    shell
}

/// The field `remove_faces` of the shell manifest.
fn faces_to_remove() -> FieldRef {
    field("remove_faces", "Faces to remove")
}

#[test]
fn an_unknown_key_that_nearly_matches_one_setting_suggests_it() {
    let registry = Registry::with_builtins();

    let validated = canonical(&registry, &misspelled_shell(&[("remove_face", json!([]))]));

    assert_eq!(validated.unknown_keys, ["remove_face"]);
    assert_eq!(
        validated.suggestions,
        [("remove_face".to_string(), faces_to_remove())]
    );
}

#[test]
fn a_near_match_the_record_sets_as_well_is_not_suggested() {
    // Renaming 'remove_face' would overwrite the faces 'remove_faces' already holds (section 3.2).
    let registry = Registry::with_builtins();

    let validated = canonical(&registry, &shell_with(&[("remove_face", json!([]))]));

    assert_eq!(validated.unknown_keys, ["remove_face"]);
    assert_eq!(validated.suggestions, []);
}

#[test]
fn an_unknown_key_far_from_every_setting_suggests_nothing() {
    let registry = Registry::with_builtins();

    let validated = canonical(&registry, &shell_with(&[("zzz", json!(1))]));

    assert_eq!(validated.unknown_keys, ["zzz"]);
    assert_eq!(validated.suggestions, []);
}

#[test]
fn suggestions_follow_the_order_of_the_unknown_keys() {
    let registry = Registry::with_builtins();

    let validated = canonical(
        &registry,
        &misspelled_shell(&[("zzz", json!(1)), ("remove_face", json!([]))]),
    );

    assert_eq!(validated.unknown_keys, ["zzz", "remove_face"]);
    assert_eq!(
        validated.suggestions,
        [("remove_face".to_string(), faces_to_remove())]
    );
}

#[test]
fn an_unknown_key_near_two_settings_suggests_neither() {
    // 'maor_radius' is within edit distance 2 of both 'major_radius' and 'minor_radius'.
    let registry = Registry::with_builtins();
    let torus = record(json!({
        "id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "5",
        "maor_radius": "3", "op": "new"
    }));

    let validated = canonical(&registry, &torus);

    assert_eq!(validated.unknown_keys, ["maor_radius"]);
    assert_eq!(validated.suggestions, []);
}

#[test]
fn an_unknown_key_near_a_setting_the_record_already_sets_suggests_nothing() {
    // Renaming 'raduis' to 'radius' would replace the radius 2 with 5, more than Rename says (section 3.2).
    let registry = Registry::with_builtins();
    let fillet = record(json!({
        "id": "f-fil1", "type": "fillet", "body": "b-1",
        "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}],
        "radius": "2", "raduis": "5"
    }));

    let validated = canonical(&registry, &fillet);

    assert_eq!(validated.unknown_keys, ["raduis"]);
    assert_eq!(validated.suggestions, []);
}

#[test]
fn a_known_key_is_never_suggested_for_itself() {
    let registry = Registry::with_builtins();

    let validated = canonical(&registry, &shell_with(&[]));

    assert_eq!(validated.unknown_keys, Vec::<String>::new());
    assert_eq!(validated.suggestions, []);
}

#[test]
fn t013_fillet_without_edges_is_an_empty_selection() {
    let registry = Registry::with_builtins();
    let fillet = record(json!({
        "id": "f-fil1",
        "type": "fillet",
        "body": "b-1",
        "edges": [],
        "radius": "1"
    }));

    let error = registry
        .validate_and_canonicalize(&fillet)
        .expect_err("an empty edge list is invalid");

    assert_eq!(error.code(), "empty_selection");
    assert_eq!(
        error.field().map(|field| field.path.as_str()),
        Some("edges")
    );
    assert_eq!(
        render_error(&error, "f-fil1", &NoNames),
        "Select at least one edge for Edges."
    );
}

#[test]
fn t013_validate_rejects_an_empty_selection() {
    let registry = test_registry();
    let empty = with_param(
        slab_record(),
        "profile",
        json!({"sketch": "f-sk1", "regions": []}),
    );

    let error = registry
        .validate_and_canonicalize(&empty)
        .expect_err("an empty region list is invalid");

    assert_eq!(error.code(), "empty_selection");
    assert_eq!(
        error.field().map(|field| field.path.as_str()),
        Some("profile")
    );
    assert_eq!(
        render_error(&error, "f-ext1", &NoNames),
        "Select at least one region for Profile."
    );
}

#[test]
fn t014_extrude_with_a_misspelled_unit_canonicalizes() {
    let registry = Registry::with_builtins();
    let typo = with_param(
        extrude_record(),
        "extent",
        json!({"kind": "distance", "expr": "30 mmm"}),
    );

    let validated = canonical(&registry, &typo);

    assert_eq!(params_text(&validated.record), params_text(&typo));
}

#[test]
fn t014_extrude_with_a_misspelled_unit_fails_when_evaluated() {
    let registry = Registry::with_builtins();
    let typo = with_param(
        extrude_record(),
        "extent",
        json!({"kind": "distance", "expr": "30 mmm"}),
    );
    let dependencies = registry
        .dependencies(&typo)
        .expect("dependencies of valid params");
    let manifest = registry
        .get("extrude")
        .expect("extrude is registered")
        .manifest()
        .clone();

    let values = evaluate(&dependencies.expressions, Units::Mm, &manifest);

    let (declared, result) = values.iter().next().expect("one declared expression");
    assert_eq!(declared.field, "extent.expr");
    assert!(
        matches!(result, Err(FeatureError::Expression { field, .. }) if field.path == "extent.expr"),
        "got {result:?}"
    );
}

#[test]
fn t014_misspelled_unit_canonicalizes() {
    let registry = test_registry();
    let typo = with_param(
        slab_record(),
        "extent",
        json!({"kind": "distance", "expr": "30 mmm"}),
    );

    let validated = canonical(&registry, &typo);

    assert_eq!(params_text(&validated.record), params_text(&typo));
}

#[test]
fn t014_misspelled_unit_fails_when_evaluated() {
    let registry = test_registry();
    let typo = with_param(
        slab_record(),
        "extent",
        json!({"kind": "distance", "expr": "30 mmm"}),
    );
    let dependencies = registry
        .dependencies(&typo)
        .expect("dependencies of valid params");

    let values = evaluate(&dependencies.expressions, Units::Mm, &slab_manifest());

    let (declared, result) = values.iter().next().expect("one declared expression");
    assert_eq!(declared.field, "extent.expr");
    assert!(
        matches!(result, Err(FeatureError::Expression { field, .. }) if field.path == "extent.expr"),
        "got {result:?}"
    );
}

#[test]
fn t016_extrude_dependencies() {
    let registry = Registry::with_builtins();

    let dependencies = registry
        .dependencies(&extrude_record())
        .expect("dependencies of valid params");

    assert_eq!(
        link_pairs(&dependencies.features),
        [("f-sk1".to_string(), "profile.sketch".to_string())]
    );
    assert!(dependencies.bodies_read.is_empty());
    assert_eq!(
        link_pairs(&dependencies.bodies_written),
        [("b-1".to_string(), "body".to_string())]
    );
    assert_eq!(
        use_triples(&dependencies.expressions),
        [(
            "extent.expr".to_string(),
            "30 mm".to_string(),
            FieldUnit::Length
        )]
    );
    assert!(dependencies.declared_ids.is_empty());
    assert!(dependencies.assets.is_empty());
}

#[test]
fn t016_extrude_inspect() {
    let registry = Registry::with_builtins();

    let links = registry.inspect(&extrude_record());

    assert_eq!(
        links,
        Ok(FeatureLinks {
            declared_ids: Vec::new(),
            bodies: vec![id("b-1")],
            features: vec![id("f-sk1")],
            expressions: vec![ExprLink {
                pointer: "/extent/expr".to_string(),
                expr: Expr::new("30 mm"),
                unit: FieldUnit::Length
            }],
        })
    );
}

#[test]
fn t016_sketch_declares_its_entity_ids() {
    let registry = Registry::with_builtins();

    let dependencies = registry
        .dependencies(&rectangle_sketch_record())
        .expect("dependencies of valid params");

    assert_eq!(
        dependencies.declared_ids,
        [id("e-1"), id("e-2"), id("e-3"), id("e-4")]
    );
}

#[test]
fn t016_registry_returns_the_feature_dependencies() {
    let registry = test_registry();

    let dependencies = registry
        .dependencies(&slab_record())
        .expect("dependencies of valid params");

    assert_eq!(
        link_pairs(&dependencies.features),
        [("f-sk1".to_string(), "profile.sketch".to_string())]
    );
    assert_eq!(
        link_pairs(&dependencies.bodies_written),
        [("b-1".to_string(), "body".to_string())]
    );
    assert_eq!(
        use_triples(&dependencies.expressions),
        [(
            "extent.expr".to_string(),
            "30 mm".to_string(),
            FieldUnit::Length
        )]
    );
}

#[test]
fn t016_inspect_lists_written_then_read_bodies_features_and_expression_pointers() {
    let registry = test_registry();
    let two_sides = with_param(
        slab_record(),
        "extent",
        json!({"kind": "two_sides", "expr": "20 mm", "opposite_expr": "width"}),
    );

    let links = registry.inspect(&two_sides);

    assert_eq!(
        links,
        Ok(FeatureLinks {
            declared_ids: Vec::new(),
            bodies: vec![id("b-1")],
            features: vec![id("f-sk1")],
            expressions: vec![
                ExprLink {
                    pointer: "/extent/expr".to_string(),
                    expr: Expr::new("20 mm"),
                    unit: FieldUnit::Length
                },
                ExprLink {
                    pointer: "/extent/opposite_expr".to_string(),
                    expr: Expr::new("width"),
                    unit: FieldUnit::Length
                },
            ],
        })
    );
}

#[test]
fn t016_inspect_of_invalid_params_returns_the_error_text() {
    let registry = test_registry();

    let links = registry.inspect(&without_param(slab_record(), "body"));

    assert_eq!(
        links,
        Err(
            "Body of this feature cannot be read, so it was not built. Set Body again.".to_string()
        )
    );
}

#[test]
fn t021_labels_count_records_of_each_type_in_timeline_order() {
    let registry = Registry::with_builtins();
    let document = document(vec![
        bare("f-sk1", "sketch", None),
        bare("f-ext1", "extrude", None),
        bare("f-ext2", "extrude", Some("Lid")),
        bare("f-ext3", "extrude", None),
    ]);

    assert_eq!(
        labels(&registry, &document),
        [
            Some("Sketch 1".to_string()),
            Some("Extrude 1".to_string()),
            Some("Lid".to_string()),
            Some("Extrude 3".to_string()),
        ]
    );
}

#[test]
fn t021_unnamed_features_are_numbered_per_type_and_named_ones_count() {
    let registry = test_registry();
    let document = document(vec![
        bare("f-probe1", "probe", None),
        bare("f-slab1", "slab", None),
        bare("f-slab2", "slab", Some("Lid")),
        bare("f-slab3", "slab", None),
    ]);

    assert_eq!(
        labels(&registry, &document),
        [
            Some("Probe 1".to_string()),
            Some("Slab 1".to_string()),
            Some("Lid".to_string()),
            Some("Slab 3".to_string()),
        ]
    );
}

#[test]
fn t021_suppressed_features_count() {
    let registry = test_registry();
    let mut suppressed = bare("f-slab1", "slab", None);
    suppressed.suppressed = true;
    let document = document(vec![suppressed, bare("f-slab2", "slab", None)]);

    assert_eq!(
        labels(&registry, &document),
        [Some("Slab 1".to_string()), Some("Slab 2".to_string())]
    );
}

#[test]
fn t021_unknown_types_use_the_type_name() {
    let registry = test_registry();
    let document = document(vec![bare("f-g1", "gear", None), bare("f-g2", "gear", None)]);

    assert_eq!(
        labels(&registry, &document),
        [Some("gear 1".to_string()), Some("gear 2".to_string())]
    );
}

#[test]
fn t021_feature_not_in_the_document_has_no_label() {
    let registry = test_registry();
    let document = document(vec![bare("f-slab1", "slab", None)]);

    assert_eq!(registry.label(&document, &id("f-slab9")), None);
}
