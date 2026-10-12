//! Registration checks of params schemas and manifests (features.md section 3.3).

use std::collections::{BTreeMap, BTreeSet};

use partsmith_model::{FeatureManifest, FeatureRecord};
use schemars::Schema;
use serde_json::{Map, Value};

use crate::field::{Segment, segments};
use crate::registry::RegistryError;

/// Deepest chain of `$ref`s and combinators followed when looking through a schema.
const MAX_SCHEMA_DEPTH: usize = 32;

/// Prefix of a reference to a definition of the same root schema.
const DEFINITION_PREFIX: &str = "#/$defs/";

/// The definition name schemars gives `partsmith_model::Expr`.
const EXPR_DEFINITION: &str = "Expr";

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
    check_manifest(manifest, schema.as_value(), root, properties).map_err(|(path, problem)| {
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
    schema_value: &Value,
    root: &Map<String, Value>,
    properties: Option<&Map<String, Value>>,
) -> Result<(), (String, String)> {
    let schema = SchemaView {
        root: schema_value,
        definitions: own_definitions(root),
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
        if object.get("$ref").and_then(Value::as_str)
            == Some(&format!("{DEFINITION_PREFIX}{EXPR_DEFINITION}"))
        {
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
