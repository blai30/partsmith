//! The `.ptsm` JSON Schema (file-format.md section 3.10).

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use schemars::Schema;
use serde_json::{Map, Value, json};

use super::{FORMAT, FORMAT_VERSION};
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
        params.remove("$schema");
        if let Some(Value::Object(nested)) = params.remove("$defs") {
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
    let roles: Vec<Value> = [
        BodyRole::Part,
        BodyRole::NegativeVolume,
        BodyRole::SupportBlocker,
        BodyRole::SupportEnforcer,
        BodyRole::SettingsModifier,
    ]
    .iter()
    .map(|role| serde_json::to_value(role).expect("a body role serializes as a string"))
    .collect();
    let mut feature_record = json!({
        "type": "object",
        "properties": {
            "id": {"$ref": "#/$defs/Id"},
            "type": {"type": "string", "pattern": "^[a-z][a-z0-9_]{0,63}$"},
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
            json!({"type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}$"}),
        ),
        ("Expr".to_string(), json!({"type": "string"})),
        (
            "Color".to_string(),
            json!({"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"}),
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
                    "name": {"type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]{0,63}$"},
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

    #[test]
    fn the_description_names_the_format_version() {
        let schema = ptsm_schema(&[]).expect("builds");

        assert_eq!(
            schema.as_value()["description"],
            json!("A Partsmith .ptsm document, format version 1.")
        );
    }
}
