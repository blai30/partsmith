//! Registration checks of params schemas and manifests (features.md section 3.3).

use std::collections::{BTreeMap, BTreeSet};

use partsmith_model::{FeatureManifest, FeatureRecord};
use schemars::Schema;
use serde_json::{Map, Value};

use crate::error::field::{Segment, segments};
use crate::registry::RegistryError;

/// Deepest chain of `$ref`s and combinators followed when looking through a schema.
const MAX_SCHEMA_DEPTH: usize = 32;

/// Prefix of a reference to a definition of the same root schema.
const DEFINITION_PREFIX: &str = "#/$defs/";

/// A reference to the definition schemars gives `partsmith_model::Expr`.
const EXPR_REFERENCE: &str = "#/$defs/Expr";

/// Checks a params schema and its manifest before registration: an open object root, no reserved
/// property, no definition that conflicts with `definitions`, and a manifest that matches the
/// schema (section 3.3).
pub(crate) fn check(
    type_name: &str,
    manifest: &FeatureManifest,
    schema: &Schema,
    definitions: &BTreeMap<String, Value>,
) -> Result<(), RegistryError> {
    let root = open_object(schema).ok_or_else(|| RegistryError::NotOpenObject {
        type_name: type_name.to_string(),
    })?;
    let properties = root.get("properties").and_then(Value::as_object);
    if let Some(field) = properties
        .into_iter()
        .flat_map(Map::keys)
        .find(|key| FeatureRecord::RESERVED_KEYS.contains(&key.as_str()))
    {
        return Err(RegistryError::ReservedField {
            type_name: type_name.to_string(),
            field: field.clone(),
        });
    }
    for (name, definition) in own_definitions(root).into_iter().flatten() {
        if definitions
            .get(name)
            .is_some_and(|registered| registered != definition)
        {
            return Err(RegistryError::SchemaConflict {
                type_name: type_name.to_string(),
                definition: name.clone(),
            });
        }
    }
    check_manifest(manifest, schema.as_value(), properties).map_err(|(path, problem)| {
        RegistryError::ManifestMismatch {
            type_name: type_name.to_string(),
            path,
            problem,
        }
    })
}

/// The `$defs` of a root schema.
pub(crate) fn own_definitions(root: &Map<String, Value>) -> Option<&Map<String, Value>> {
    root.get("$defs").and_then(Value::as_object)
}

/// The root object of a schema whose type is `object` and that does not close it with
/// `additionalProperties: false` or `unevaluatedProperties: false`.
fn open_object(schema: &Schema) -> Option<&Map<String, Value>> {
    let root = schema.as_object()?;
    let closes = |key: &str| root.get(key) == Some(&Value::Bool(false));
    (root.get("type") == Some(&Value::String("object".to_string()))
        && !closes("additionalProperties")
        && !closes("unevaluatedProperties"))
    .then_some(root)
}

/// Checks every hint against the schema, then that every top-level property has a hint. A
/// problem is the path it is about and a description.
fn check_manifest(
    manifest: &FeatureManifest,
    root: &Value,
    properties: Option<&Map<String, Value>>,
) -> Result<(), (String, String)> {
    let schema = SchemaView {
        root,
        definitions: root.as_object().and_then(own_definitions),
    };
    for hint in &manifest.fields {
        let problem = |text: &str| (hint.path.clone(), text.to_string());
        let steps = segments(&hint.path).ok_or_else(|| problem("it is not a valid field path"))?;
        let nodes = schema.resolve(&steps);
        if nodes.is_empty() {
            return Err(problem("no params field has this path"));
        }
        if hint.unit.is_some() && !nodes.iter().all(|node| schema.is_expr(node)) {
            return Err(problem(
                "it has a unit, but the field is not an expression (Expr)",
            ));
        }
        if !hint.options.is_empty() {
            let allowed = schema.string_values(&nodes);
            if allowed.is_empty() {
                return Err(problem(
                    "it has options, but the field is not a string enumeration or tag",
                ));
            }
            let listed: BTreeSet<&str> = hint
                .options
                .iter()
                .map(|option| option.value.as_str())
                .collect();
            if listed.len() != hint.options.len() {
                return Err(problem("its options list a value more than once"));
            }
            if listed != allowed {
                let join =
                    |values: &BTreeSet<&str>| values.iter().copied().collect::<Vec<_>>().join(", ");
                return Err((
                    hint.path.clone(),
                    format!(
                        "its options are {}, but the field allows {}",
                        join(&listed),
                        join(&allowed)
                    ),
                ));
            }
        }
    }
    for property in properties.into_iter().flat_map(Map::keys) {
        let covered = manifest.fields.iter().any(|hint| {
            hint.path
                .strip_prefix(property.as_str())
                .is_some_and(|rest| {
                    rest.is_empty() || rest.starts_with('.') || rest.starts_with('[')
                })
        });
        if !covered {
            return Err((property.clone(), "the field has no hint".to_string()));
        }
    }
    Ok(())
}

/// A root schema, for looking up the schemas of field paths.
struct SchemaView<'a> {
    /// The root schema.
    root: &'a Value,
    /// Its `$defs`.
    definitions: Option<&'a Map<String, Value>>,
}

impl<'a> SchemaView<'a> {
    /// The schemas a field path can reach: through `$ref`s and every branch of `anyOf`, `oneOf`
    /// and `allOf`, so a field of any variant of a tagged union counts.
    fn resolve(&self, steps: &[Segment<'_>]) -> Vec<&'a Value> {
        let mut nodes = self.expand(self.root);
        let mut reached = Vec::new();
        for (position, step) in steps.iter().enumerate() {
            reached = nodes
                .iter()
                .flat_map(|node| self.children(node, *step))
                .collect();
            if position + 1 < steps.len() {
                nodes = reached.iter().flat_map(|node| self.expand(node)).collect();
            }
        }
        reached
    }

    /// The schemas one step below an object schema: a property, or the items of an array.
    fn children(&self, node: &'a Map<String, Value>, step: Segment<'_>) -> Vec<&'a Value> {
        match step {
            Segment::Name(name) => node
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(name))
                .into_iter()
                .collect(),
            Segment::Index(index) => {
                let prefix = node.get("prefixItems").and_then(Value::as_array);
                match prefix.and_then(|items| items.get(index)) {
                    Some(item) => vec![item],
                    None => node
                        .get("items")
                        .filter(|items| items.is_object())
                        .into_iter()
                        .collect(),
                }
            }
            Segment::AnyIndex => node
                .get("prefixItems")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(node.get("items").filter(|items| items.is_object()))
                .collect(),
        }
    }

    /// The object schemas `node` stands for: itself, its `$ref` target, and every branch of its
    /// combinators, recursively.
    fn expand(&self, node: &'a Value) -> Vec<&'a Map<String, Value>> {
        let mut expanded = Vec::new();
        self.expand_into(node, 0, &mut expanded);
        expanded
    }

    /// Adds the object schemas `node` stands for to `expanded`, at most `MAX_SCHEMA_DEPTH` deep.
    fn expand_into(
        &self,
        node: &'a Value,
        depth: usize,
        expanded: &mut Vec<&'a Map<String, Value>>,
    ) {
        let Some(object) = node.as_object() else {
            return;
        };
        if depth > MAX_SCHEMA_DEPTH {
            return;
        }
        expanded.push(object);
        if let Some(target) = self.reference(object) {
            self.expand_into(target, depth + 1, expanded);
        }
        for combinator in ["anyOf", "oneOf", "allOf"] {
            for branch in object
                .get(combinator)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                self.expand_into(branch, depth + 1, expanded);
            }
        }
    }

    /// The definition a `$ref` of this root names.
    fn reference(&self, object: &Map<String, Value>) -> Option<&'a Value> {
        let name = object
            .get("$ref")
            .and_then(Value::as_str)?
            .strip_prefix(DEFINITION_PREFIX)?;
        self.definitions?.get(name)
    }

    /// True when `node` is an expression: a reference to `Expr`, or an optional one (every
    /// branch of its `anyOf` or `oneOf` is `Expr` or null).
    fn is_expr(&self, node: &Value) -> bool {
        let Some(object) = node.as_object() else {
            return false;
        };
        if object.get("$ref").and_then(Value::as_str) == Some(EXPR_REFERENCE) {
            return true;
        }
        ["anyOf", "oneOf"].iter().any(|combinator| {
            object
                .get(*combinator)
                .and_then(Value::as_array)
                .is_some_and(|branches| {
                    branches.iter().any(|branch| self.is_expr(branch))
                        && branches
                            .iter()
                            .all(|branch| self.is_expr(branch) || is_null(branch))
                })
        })
    }

    /// The string values the schemas allow through `enum` and `const`, in any branch.
    fn string_values(&self, nodes: &[&'a Value]) -> BTreeSet<&'a str> {
        nodes
            .iter()
            .flat_map(|node| self.expand(node))
            .flat_map(|object| {
                let listed = object
                    .get("enum")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten();
                listed.chain(object.get("const")).filter_map(Value::as_str)
            })
            .collect()
    }
}

/// True for a schema that only allows null.
fn is_null(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("null")
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Expr, FieldHint, FieldUnit, Id, OptionHint};
    use schemars::{JsonSchema, schema_for};
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::params::BodyOp;
    use crate::test_support::manifest;

    /// Params exercising every kind of field the checks look at.
    #[derive(Serialize, Deserialize, JsonSchema)]
    struct Params {
        /// A body.
        body: Id,
        /// A tagged union.
        extent: Extent,
        /// An enumeration with documented values.
        op: BodyOp,
        /// An optional expression.
        #[serde(default)]
        taper: Option<Expr>,
        /// Fixed-size expressions.
        size: [Expr; 2],
        /// A plain string.
        label: String,
    }

    /// A tagged union.
    #[derive(Serialize, Deserialize, JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Extent {
        /// One distance.
        Distance {
            /// The distance.
            expr: Expr,
        },
        /// Through everything.
        ThroughAll,
    }

    fn hint(path: &str) -> FieldHint {
        FieldHint {
            path: path.to_string(),
            label: path.to_string(),
            essential: false,
            unit: None,
            selection: None,
            options: Vec::new(),
        }
    }

    fn with_unit(path: &str) -> FieldHint {
        FieldHint {
            unit: Some(FieldUnit::Length),
            ..hint(path)
        }
    }

    fn with_options(path: &str, values: &[&str]) -> FieldHint {
        FieldHint {
            options: values
                .iter()
                .map(|value| OptionHint {
                    value: value.to_string(),
                    label: value.to_string(),
                })
                .collect(),
            ..hint(path)
        }
    }

    /// Hints that cover every field correctly.
    fn complete_hints() -> Vec<FieldHint> {
        vec![
            hint("body"),
            with_options("extent.kind", &["distance", "through_all"]),
            with_unit("extent.expr"),
            with_options("op", &["new", "join", "cut", "intersect"]),
            with_unit("taper"),
            with_unit("size[0]"),
            with_unit("size[*]"),
            hint("label"),
        ]
    }

    fn check_hints(hints: Vec<FieldHint>) -> Result<(), RegistryError> {
        check(
            "params",
            &manifest(hints),
            &schema_for!(Params),
            &BTreeMap::new(),
        )
    }

    fn mismatch_path(result: Result<(), RegistryError>) -> String {
        match result {
            Err(RegistryError::ManifestMismatch { path, .. }) => path,
            other => panic!("expected ManifestMismatch, got {other:?}"),
        }
    }

    #[test]
    fn complete_hints_pass() {
        assert_eq!(check_hints(complete_hints()), Ok(()));
    }

    #[test]
    fn a_unit_on_a_string_is_a_mismatch() {
        let mut hints = complete_hints();
        hints.push(with_unit("label"));

        assert_eq!(mismatch_path(check_hints(hints)), "label");
    }

    #[test]
    fn options_must_list_exactly_the_allowed_values() {
        let mut missing = complete_hints();
        missing[3] = with_options("op", &["new", "join", "cut"]);
        let mut repeated = complete_hints();
        repeated[1] = with_options("extent.kind", &["distance", "distance", "through_all"]);

        assert_eq!(mismatch_path(check_hints(missing)), "op");
        assert_eq!(mismatch_path(check_hints(repeated)), "extent.kind");
    }

    #[test]
    fn options_on_a_field_without_values_are_a_mismatch() {
        let mut hints = complete_hints();
        hints[0] = with_options("body", &["b-1"]);

        assert_eq!(mismatch_path(check_hints(hints)), "body");
    }

    #[test]
    fn hints_beyond_a_fixed_size_list_or_inside_a_string_do_not_exist() {
        let mut beyond = complete_hints();
        beyond.push(with_unit("size[2].x"));
        let mut malformed = complete_hints();
        malformed.push(hint("size[x]"));

        assert_eq!(mismatch_path(check_hints(beyond)), "size[2].x");
        assert_eq!(mismatch_path(check_hints(malformed)), "size[x]");
    }

    #[test]
    fn a_field_covered_only_by_a_sub_path_hint_is_covered() {
        let hints: Vec<FieldHint> = complete_hints()
            .into_iter()
            .filter(|hint| hint.path != "body")
            .chain([hint("body")])
            .collect();

        assert_eq!(check_hints(hints), Ok(()));
    }

    #[test]
    fn a_reserved_root_property_is_rejected() {
        let schema = Schema::try_from(serde_json::json!({
            "type": "object",
            "properties": {"suppressed": {"type": "boolean"}}
        }))
        .expect("an object schema");

        let result = check(
            "closed",
            &manifest(vec![hint("suppressed")]),
            &schema,
            &BTreeMap::new(),
        );

        assert_eq!(
            result,
            Err(RegistryError::ReservedField {
                type_name: "closed".to_string(),
                field: "suppressed".to_string()
            })
        );
    }

    #[test]
    fn unevaluated_properties_false_closes_the_object() {
        let schema = Schema::try_from(serde_json::json!({
            "type": "object",
            "unevaluatedProperties": false
        }))
        .expect("an object schema");

        let result = check("closed", &manifest(Vec::new()), &schema, &BTreeMap::new());

        assert_eq!(
            result,
            Err(RegistryError::NotOpenObject {
                type_name: "closed".to_string()
            })
        );
    }

    #[test]
    fn identical_registered_definitions_do_not_conflict() {
        let schema = schema_for!(Params);
        let definitions: BTreeMap<String, Value> =
            own_definitions(schema.as_object().expect("an object schema"))
                .expect("the params have definitions")
                .iter()
                .map(|(name, definition)| (name.clone(), definition.clone()))
                .collect();

        assert_eq!(
            check("params", &manifest(complete_hints()), &schema, &definitions),
            Ok(())
        );
    }

    #[test]
    fn recursive_definitions_terminate() {
        let schema = Schema::try_from(serde_json::json!({
            "type": "object",
            "properties": {"node": {"$ref": "#/$defs/Node"}},
            "$defs": {"Node": {"anyOf": [{"$ref": "#/$defs/Node"}, {"type": "null"}]}}
        }))
        .expect("an object schema");

        let result = check(
            "recursive",
            &manifest(vec![hint("node"), with_unit("node")]),
            &schema,
            &BTreeMap::new(),
        );

        assert_eq!(mismatch_path(result), "node");
    }
}
