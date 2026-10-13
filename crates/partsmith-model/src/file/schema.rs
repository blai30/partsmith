//! The `.ptsm` JSON Schema (file-format.md section 3.10).

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use schemars::Schema;
use serde_json::{Map, Value, json};

use super::{FORMAT, FORMAT_VERSION};
use crate::id::ID_PATTERN;
use crate::names::{FEATURE_TYPE_PATTERN, PARAMETER_NAME_PATTERN};
use crate::units::COLOR_PATTERN;
use crate::{BodyRole, FeatureRecord, Units, canonical, is_valid_feature_type};

/// A registered feature type and the JSON Schema of its params.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureParamsSchema {
    /// Feature type name, such as "extrude".
    pub kind: String,
    /// Root schema of the typed params struct, as schemars generates it.
    pub schema: Schema,
}

/// Why the `.ptsm` schema cannot be built (file-format.md section 4.4).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SchemaError {
    /// A kind is not a valid feature type name.
    #[error("'{kind}' is not a valid feature type name.")]
    InvalidKind {
        /// The kind.
        kind: String,
    },
    /// Two params schemas have the same kind.
    #[error("Two params schemas are registered for the feature type '{kind}'.")]
    DuplicateKind {
        /// The kind.
        kind: String,
    },
    /// A params schema does not describe an object.
    #[error("The params schema of '{kind}' must describe an object (\"type\": \"object\").")]
    NotAnObject {
        /// The kind.
        kind: String,
    },
    /// A params schema forbids keys it does not list.
    #[error(
        "The params schema of '{kind}' forbids extra keys, but feature records keep unknown keys. Do not set additionalProperties or unevaluatedProperties to false."
    )]
    ClosedParams {
        /// The kind.
        kind: String,
    },
    /// A params schema declares a key the record itself uses.
    #[error(
        "The params schema of '{kind}' declares '{key}', which the feature record itself uses."
    )]
    ReservedParamKey {
        /// The kind.
        kind: String,
        /// The reserved key.
        key: String,
    },
    /// Two schemas define one definition name differently.
    #[error("Two schemas define '{name}' differently. Give one of the types a different name.")]
    ConflictingDefinition {
        /// The definition name.
        name: String,
    },
}

/// Builds the `.ptsm` JSON Schema (file-format.md section 3.10).
pub fn ptsm_schema(features: &[FeatureParamsSchema]) -> Result<Schema, SchemaError> {
    let mut kinds = BTreeSet::new();
    for feature in features {
        if !is_valid_feature_type(&feature.kind) {
            return Err(SchemaError::InvalidKind {
                kind: feature.kind.clone(),
            });
        }
        if !kinds.insert(feature.kind.as_str()) {
            return Err(SchemaError::DuplicateKind {
                kind: feature.kind.clone(),
            });
        }
    }
    let mut sorted: Vec<&FeatureParamsSchema> = features.iter().collect();
    sorted.sort_unstable_by(|first, second| first.kind.cmp(&second.kind));
    let params_schemas = sorted
        .iter()
        .map(|feature| checked_params(feature).map(|params| (feature.kind.as_str(), params)))
        .collect::<Result<Vec<_>, _>>()?;

    let mut definitions = model_definitions(&sorted);
    for (kind, mut params) in params_schemas {
        // `shift_remove` keeps the order of the remaining keys; `remove` would move the last key
        // into the removed slot.
        params.shift_remove("$schema");
        if let Some(Value::Object(nested)) = params.shift_remove("$defs") {
            for (name, definition) in nested {
                add_definition(&mut definitions, name, definition)?;
            }
        }
        add_definition(
            &mut definitions,
            format!("feature.{kind}"),
            Value::Object(params),
        )?;
    }

    let mut root = Map::new();
    root.insert(
        "$schema".to_string(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert("title".to_string(), json!("Partsmith document"));
    root.insert(
        "description".to_string(),
        json!(format!(
            "A Partsmith .ptsm document, format version {FORMAT_VERSION}."
        )),
    );
    root.insert("type".to_string(), json!("object"));
    root.insert(
        "properties".to_string(),
        json!({
            "format": {"const": FORMAT},
            "format_version": {"const": FORMAT_VERSION},
            "units": {"$ref": "#/$defs/Units"},
            "printer": {"type": "string"},
            "parameters": {"type": "array", "items": {"$ref": "#/$defs/Parameter"}},
            "bodies": {"type": "array", "items": {"$ref": "#/$defs/Body"}},
            "features": {"type": "array", "items": {"$ref": "#/$defs/FeatureRecord"}},
            "rollback": {"$ref": "#/$defs/Id"}
        }),
    );
    root.insert(
        "required".to_string(),
        json!(["format", "format_version", "units"]),
    );
    root.insert("additionalProperties".to_string(), json!(false));
    root.insert(
        "$defs".to_string(),
        Value::Object(definitions.into_iter().collect()),
    );
    Ok(Schema::from(root))
}

/// The same schema as canonical text, ready to write to docs/format/ptsm.schema.json.
pub fn ptsm_schema_text(features: &[FeatureParamsSchema]) -> Result<String, SchemaError> {
    let schema = ptsm_schema(features)?;
    Ok(canonical::to_string(schema.as_value()))
}

/// The root object of a params schema after the checks of file-format.md section 3.10, step 2.
fn checked_params(feature: &FeatureParamsSchema) -> Result<Map<String, Value>, SchemaError> {
    let kind = || feature.kind.clone();
    let root = feature
        .schema
        .as_object()
        .filter(|root| root.get("type") == Some(&json!("object")))
        .ok_or_else(|| SchemaError::NotAnObject { kind: kind() })?;
    let closes = |key: &str| root.get(key) == Some(&Value::Bool(false));
    if closes("additionalProperties") || closes("unevaluatedProperties") {
        return Err(SchemaError::ClosedParams { kind: kind() });
    }
    if let Some(Value::Object(properties)) = root.get("properties")
        && let Some(key) = properties
            .keys()
            .find(|key| FeatureRecord::RESERVED_KEYS.contains(&key.as_str()))
    {
        return Err(SchemaError::ReservedParamKey {
            kind: kind(),
            key: key.clone(),
        });
    }
    Ok(root.clone())
}

/// Adds a definition, sharing an identical one of the same name and rejecting a different one.
fn add_definition(
    definitions: &mut BTreeMap<String, Value>,
    name: String,
    definition: Value,
) -> Result<(), SchemaError> {
    match definitions.entry(name) {
        Entry::Vacant(entry) => {
            entry.insert(definition);
            Ok(())
        }
        Entry::Occupied(entry) if *entry.get() == definition => Ok(()),
        Entry::Occupied(entry) => Err(SchemaError::ConflictingDefinition {
            name: entry.key().clone(),
        }),
    }
}

/// The definitions of the model types; `FeatureRecord` selects each feature's params by type.
fn model_definitions(features: &[&FeatureParamsSchema]) -> BTreeMap<String, Value> {
    let units: Vec<&str> = Units::ALL.iter().map(|units| units.symbol()).collect();
    let roles: Vec<Value> = BodyRole::ALL
        .iter()
        .map(|role| serde_json::to_value(role).expect("a body role serializes as a string"))
        .collect();
    let mut feature_record = json!({
        "type": "object",
        "properties": {
            "id": {"$ref": "#/$defs/Id"},
            "type": {"type": "string", "pattern": FEATURE_TYPE_PATTERN},
            "name": {"type": "string"},
            "suppressed": {"type": "boolean"}
        },
        "required": ["id", "type"]
    });
    if !features.is_empty() {
        let branches: Vec<Value> = features
            .iter()
            .map(|feature| {
                json!({
                    "if": {"properties": {"type": {"const": feature.kind}}, "required": ["type"]},
                    "then": {"$ref": format!("#/$defs/feature.{}", feature.kind)}
                })
            })
            .collect();
        feature_record["allOf"] = Value::Array(branches);
    }
    BTreeMap::from([
        (
            "Id".to_string(),
            json!({"type": "string", "pattern": ID_PATTERN}),
        ),
        ("Expr".to_string(), json!({"type": "string"})),
        (
            "Color".to_string(),
            json!({"type": "string", "pattern": COLOR_PATTERN}),
        ),
        (
            "Units".to_string(),
            json!({"type": "string", "enum": units}),
        ),
        (
            "BodyRole".to_string(),
            json!({"type": "string", "enum": roles}),
        ),
        (
            "Parameter".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "id": {"$ref": "#/$defs/Id"},
                    "name": {"type": "string", "pattern": PARAMETER_NAME_PATTERN},
                    "expr": {"$ref": "#/$defs/Expr"},
                    "comment": {"type": "string"}
                },
                "required": ["id", "name", "expr"],
                "additionalProperties": false
            }),
        ),
        (
            "Body".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "id": {"$ref": "#/$defs/Id"},
                    "name": {"type": "string"},
                    "material": {"type": "string"},
                    "color": {"$ref": "#/$defs/Color"},
                    "visible": {"type": "boolean"},
                    "role": {"$ref": "#/$defs/BodyRole"}
                },
                "required": ["id", "name"],
                "additionalProperties": false
            }),
        ),
        ("FeatureRecord".to_string(), feature_record),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(kind: &str, schema: Value) -> FeatureParamsSchema {
        FeatureParamsSchema {
            kind: kind.to_string(),
            schema: Schema::try_from(schema).expect("a schema"),
        }
    }

    #[test]
    fn boolean_schemas_are_not_objects() {
        assert_eq!(
            ptsm_schema(&[params("box", json!(true))]),
            Err(SchemaError::NotAnObject {
                kind: "box".to_string()
            })
        );
    }

    #[test]
    fn closed_unevaluated_properties_are_rejected() {
        assert_eq!(
            ptsm_schema(&[params(
                "box",
                json!({"type": "object", "unevaluatedProperties": false})
            )]),
            Err(SchemaError::ClosedParams {
                kind: "box".to_string()
            })
        );
    }

    #[test]
    fn open_additional_properties_are_fine() {
        assert!(
            ptsm_schema(&[params(
                "box",
                json!({"type": "object", "additionalProperties": true})
            )])
            .is_ok()
        );
    }

    #[test]
    fn input_checks_come_before_schema_checks() {
        assert_eq!(
            ptsm_schema(&[params("box", json!(true)), params("Box", json!(true))]),
            Err(SchemaError::InvalidKind {
                kind: "Box".to_string()
            })
        );
    }

    #[test]
    fn schema_checks_run_in_kind_order() {
        assert_eq!(
            ptsm_schema(&[params("zeta", json!(true)), params("alpha", json!(true))]),
            Err(SchemaError::NotAnObject {
                kind: "alpha".to_string()
            })
        );
    }

    #[test]
    fn model_definition_key_order() {
        let schema = ptsm_schema(&[params("box", json!({"type": "object"}))]).expect("builds");
        let record = &schema.as_value()["$defs"]["FeatureRecord"];

        assert_eq!(
            record
                .as_object()
                .expect("an object")
                .keys()
                .collect::<Vec<_>>(),
            ["type", "properties", "required", "allOf"]
        );
        assert_eq!(
            schema.as_value()["$defs"]["Parameter"]
                .as_object()
                .expect("an object")
                .keys()
                .collect::<Vec<_>>(),
            ["type", "properties", "required", "additionalProperties"]
        );
    }

    /// Property names of an object schema, in order.
    fn property_names(schema: &Value) -> Vec<&str> {
        schema["properties"]
            .as_object()
            .expect("an object schema with properties")
            .keys()
            .map(String::as_str)
            .collect()
    }

    /// Every `const` and `enum` string in `schema`, at any depth, in order.
    fn enumerated_strings(schema: &Value) -> Vec<Value> {
        match schema {
            Value::Object(members) => members
                .iter()
                .flat_map(|(key, value)| match (key.as_str(), value) {
                    ("const", Value::String(_)) => vec![value.clone()],
                    ("enum", Value::Array(values)) => values.clone(),
                    _ => enumerated_strings(value),
                })
                .collect(),
            Value::Array(values) => values.iter().flat_map(enumerated_strings).collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn model_definitions_match_the_derived_schemas() {
        let schema = ptsm_schema(&[]).expect("builds");
        let root = schema.as_value();
        let derived = [
            ("Parameter", schemars::schema_for!(crate::Parameter)),
            ("Body", schemars::schema_for!(crate::Body)),
            ("FeatureRecord", schemars::schema_for!(FeatureRecord)),
        ];

        for (name, derived) in &derived {
            let written = &root["$defs"][*name];
            assert_eq!(
                property_names(written),
                property_names(derived.as_value()),
                "{name}"
            );
            assert_eq!(
                written["required"],
                derived.as_value()["required"],
                "{name}"
            );
        }
        let document = schemars::schema_for!(crate::Document);
        assert_eq!(property_names(root), property_names(document.as_value()));
        assert_eq!(root["required"], document.as_value()["required"]);
    }

    #[test]
    fn value_definitions_list_every_serialized_value() {
        let schema = ptsm_schema(&[]).expect("builds");
        let definitions = &schema.as_value()["$defs"];

        assert_eq!(
            definitions["BodyRole"]["enum"],
            Value::Array(enumerated_strings(
                schemars::schema_for!(BodyRole).as_value()
            ))
        );
        assert_eq!(
            definitions["Units"]["enum"],
            Value::Array(enumerated_strings(schemars::schema_for!(Units).as_value()))
        );
    }

    #[test]
    fn every_body_role_serializes_to_a_listed_role() {
        let schema = ptsm_schema(&[]).expect("builds");
        let listed = &schema.as_value()["$defs"]["BodyRole"]["enum"];
        let listed = listed.as_array().expect("an enum list");

        assert_eq!(listed.len(), BodyRole::ALL.len());
        for role in BodyRole::ALL {
            let serialized = serde_json::to_value(role).expect("a body role serializes");
            assert!(listed.contains(&serialized), "{serialized}");
        }
    }

    #[test]
    fn feature_params_keep_their_key_order() {
        let schema = ptsm_schema(&[params(
            "extrude",
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "ExtrudeParams",
                "type": "object",
                "properties": {"extent": {"$ref": "#/$defs/Extent"}},
                "required": ["extent"],
                "$defs": {"Extent": {"type": "object"}}
            }),
        )])
        .expect("builds");

        assert_eq!(
            schema.as_value()["$defs"]["feature.extrude"]
                .as_object()
                .expect("an object")
                .keys()
                .collect::<Vec<_>>(),
            ["title", "type", "properties", "required"]
        );
    }

    #[test]
    fn the_description_names_the_format_version() {
        let schema = ptsm_schema(&[]).expect("builds");

        assert_eq!(
            schema.as_value()["description"],
            json!("A Partsmith .ptsm document, format version 1.")
        );
    }
}
