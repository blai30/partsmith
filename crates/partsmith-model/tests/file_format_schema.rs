//! Acceptance tests for the `.ptsm` JSON Schema (file-format.md section 6, "JSON Schema", tests 46 to 53).

use partsmith_model::canonical::{parse, to_string};
use partsmith_model::{FeatureParamsSchema, SchemaError, ptsm_schema, ptsm_schema_text};
use serde_json::{Value, json};

/// A registered params schema for `kind`.
fn params_schema(kind: &str, schema: Value) -> FeatureParamsSchema {
    FeatureParamsSchema {
        kind: kind.to_string(),
        schema: schemars::Schema::try_from(schema).expect("the schema is a JSON object"),
    }
}

/// A params schema with one string property `depth`.
fn depth_schema(kind: &str) -> FeatureParamsSchema {
    params_schema(
        kind,
        json!({"type": "object", "properties": {"depth": {"type": "string"}}}),
    )
}

/// A params schema that defines `Extent` as `extent` and refers to it.
fn extent_schema(kind: &str, extent: Value) -> FeatureParamsSchema {
    params_schema(
        kind,
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {"extent": {"$ref": "#/$defs/Extent"}},
            "$defs": {"Extent": extent}
        }),
    )
}

/// The generated schema as JSON.
fn schema_json(features: &[FeatureParamsSchema]) -> Value {
    ptsm_schema(features)
        .unwrap_or_else(|error| panic!("the schema builds: {error}"))
        .as_value()
        .clone()
}

/// The value at a JSON pointer, which must exist.
fn at<'a>(schema: &'a Value, pointer: &str) -> &'a Value {
    schema
        .pointer(pointer)
        .unwrap_or_else(|| panic!("the schema has nothing at {pointer}"))
}

/// Keys of the object at a JSON pointer, in order.
fn keys_at<'a>(schema: &'a Value, pointer: &str) -> Vec<&'a str> {
    at(schema, pointer)
        .as_object()
        .unwrap_or_else(|| panic!("{pointer} is not an object"))
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn t046_root_of_the_schema() {
    let schema = schema_json(&[]);

    assert_eq!(
        at(&schema, "/$schema"),
        &json!("https://json-schema.org/draft/2020-12/schema")
    );
    assert_eq!(at(&schema, "/title"), &json!("Partsmith document"));
    assert_eq!(at(&schema, "/type"), &json!("object"));
    assert_eq!(at(&schema, "/properties/format/const"), &json!("partsmith"));
    assert_eq!(at(&schema, "/properties/format_version/const"), &json!(2));
    assert_eq!(
        at(&schema, "/required"),
        &json!(["format", "format_version", "units"])
    );
    assert_eq!(at(&schema, "/additionalProperties"), &json!(false));
    assert_eq!(
        at(&schema, "/properties/rollback/$ref"),
        &json!("#/$defs/Id")
    );
}

#[test]
fn t047_model_definitions() {
    let schema = schema_json(&[]);

    assert_eq!(
        at(&schema, "/$defs/Id/pattern"),
        &json!("^[A-Za-z0-9_-]{1,64}$")
    );
    assert_eq!(
        at(&schema, "/$defs/Color/pattern"),
        &json!("^#[0-9A-Fa-f]{6}$")
    );
    assert_eq!(
        at(&schema, "/$defs/Units/enum"),
        &json!(["mm", "cm", "m", "in"])
    );
    assert_eq!(
        at(&schema, "/$defs/BodyRole/enum"),
        &json!([
            "part",
            "negative_volume",
            "support_blocker",
            "support_enforcer",
            "settings_modifier"
        ])
    );
    assert_eq!(
        at(&schema, "/$defs/Parameter/required"),
        &json!(["id", "name", "expr"])
    );
    assert_eq!(at(&schema, "/$defs/Body/required"), &json!(["id", "name"]));
    assert_eq!(
        at(&schema, "/$defs/FeatureRecord/required"),
        &json!(["id", "type"])
    );
    assert_eq!(schema.pointer("/$defs/FeatureRecord/allOf"), None);
}

#[test]
fn t048_definition_and_root_key_order() {
    let schema = schema_json(&[]);

    assert_eq!(
        keys_at(&schema, "/$defs"),
        [
            "Body",
            "BodyRole",
            "Color",
            "Expr",
            "FeatureRecord",
            "Id",
            "Parameter",
            "Units"
        ]
    );
    assert_eq!(
        keys_at(&schema, ""),
        [
            "$schema",
            "title",
            "description",
            "type",
            "properties",
            "required",
            "additionalProperties",
            "$defs"
        ]
    );
}

#[test]
fn t049_feature_params_are_selected_by_type() {
    let schema = schema_json(&[depth_schema("extrude"), depth_schema("box")]);

    assert_eq!(
        at(
            &schema,
            "/$defs/FeatureRecord/allOf/0/if/properties/type/const"
        ),
        &json!("box")
    );
    assert_eq!(
        at(&schema, "/$defs/FeatureRecord/allOf/0/then/$ref"),
        &json!("#/$defs/feature.box")
    );
    assert_eq!(
        at(
            &schema,
            "/$defs/FeatureRecord/allOf/1/if/properties/type/const"
        ),
        &json!("extrude")
    );
    assert_eq!(
        at(&schema, "/$defs/FeatureRecord/allOf/1/then/$ref"),
        &json!("#/$defs/feature.extrude")
    );
    assert_eq!(
        at(&schema, "/$defs/feature.box/properties/depth/type"),
        &json!("string")
    );
}

#[test]
fn t050_params_definitions_are_hoisted() {
    let schema = schema_json(&[extent_schema("extrude", json!({"type": "object"}))]);

    assert_eq!(at(&schema, "/$defs/Extent"), &json!({"type": "object"}));
    assert_eq!(schema.pointer("/$defs/feature.extrude/$schema"), None);
    assert_eq!(schema.pointer("/$defs/feature.extrude/$defs"), None);
}

#[test]
fn t051_hoisted_definitions_are_shared_or_conflict() {
    let shared = schema_json(&[
        extent_schema("extrude", json!({"type": "object"})),
        extent_schema("revolve", json!({"type": "object"})),
    ]);
    let conflicting = ptsm_schema(&[
        extent_schema("extrude", json!({"type": "object"})),
        extent_schema("revolve", json!({"type": "string"})),
    ]);
    let model_clash = ptsm_schema(&[params_schema(
        "box",
        json!({"type": "object", "$defs": {"Id": {"type": "integer"}}}),
    )]);

    assert_eq!(at(&shared, "/$defs/Extent"), &json!({"type": "object"}));
    assert_eq!(
        conflicting.map(|schema| schema.as_value().clone()),
        Err(SchemaError::ConflictingDefinition {
            name: "Extent".to_string()
        })
    );
    assert_eq!(
        model_clash.map(|schema| schema.as_value().clone()),
        Err(SchemaError::ConflictingDefinition {
            name: "Id".to_string()
        })
    );
}

#[test]
fn t052_invalid_params_schemas() {
    let error_of = |features: Vec<FeatureParamsSchema>| {
        ptsm_schema(&features)
            .map(|schema| schema.as_value().clone())
            .expect_err("the inputs are invalid")
    };

    assert_eq!(
        error_of(vec![depth_schema("box"), depth_schema("box")]),
        SchemaError::DuplicateKind {
            kind: "box".to_string()
        }
    );
    assert_eq!(
        error_of(vec![depth_schema("Box")]),
        SchemaError::InvalidKind {
            kind: "Box".to_string()
        }
    );
    assert!(
        matches!(
            error_of(vec![params_schema("box", json!({"type": "array"}))]),
            SchemaError::NotAnObject { .. }
        ),
        "an array schema is not an object"
    );
    assert!(
        matches!(
            error_of(vec![params_schema(
                "box",
                json!({"type": "object", "additionalProperties": false})
            )]),
            SchemaError::ClosedParams { .. }
        ),
        "closed params are rejected"
    );
    assert_eq!(
        error_of(vec![params_schema(
            "box",
            json!({"type": "object", "properties": {"name": {"type": "string"}}})
        )]),
        SchemaError::ReservedParamKey {
            kind: "box".to_string(),
            key: "name".to_string()
        }
    );
}

#[test]
fn t053_schema_text_is_canonical_and_order_independent() {
    let forward = ptsm_schema_text(&[depth_schema("extrude"), depth_schema("box")])
        .expect("the schema builds");
    let again = ptsm_schema_text(&[depth_schema("extrude"), depth_schema("box")])
        .expect("the schema builds");
    let reversed = ptsm_schema_text(&[depth_schema("box"), depth_schema("extrude")])
        .expect("the schema builds");

    assert_eq!(forward, again);
    assert_eq!(forward, reversed);
    assert!(forward.ends_with('\n') && !forward.ends_with("\n\n"));
    let reparsed = parse(&forward).expect("the schema text is strict JSON");
    assert_eq!(to_string(&reparsed), forward);
}
