//! The feature registry: registration, params validation and canonicalization, schemas,
//! dependencies, builds, Python and labels (features.md sections 2.9, 3.2, 3.3, 3.11 and 3.12).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use partsmith_model::canonical;
use partsmith_model::{
    Document, ExprLink, FeatureInspector, FeatureLinks, FeatureManifest, FeatureParamsSchema,
    FeatureRecord, Id, SUGGESTION_DISTANCE, edit_distance, is_valid_feature_type,
};
use schemars::Schema;
use serde_json::{Map, Value};

use crate::builtin::BUILTINS;
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::deserializer::from_params;
use crate::error::field::pointer;
use crate::error::{FeatureError, FieldRef};
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::python::{PythonCall, PythonNames, PythonValue, is_keyword_name};
use crate::schema;

/// Maps feature type names to type-erased features. One per session; no global registry.
#[derive(Clone, Default)]
pub struct Registry {
    /// Registered features by type name.
    features: BTreeMap<String, Registered>,
    /// Every `$defs` entry of the registered params schemas, by name.
    definitions: BTreeMap<String, Value>,
}

/// One registered feature with its checked params schema.
#[derive(Clone)]
struct Registered {
    /// The feature.
    feature: Arc<dyn DynFeature>,
    /// Its params schema, titled with the manifest label.
    schema: Schema,
    /// The top-level params keys of the schema, which unknown keys are matched against (section 3.2).
    known_keys: Vec<String>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("types", &self.features.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Registry::default()
    }

    /// A registry with every built-in feature, registered through `register_builtins`.
    pub fn with_builtins() -> Self {
        let mut registry = Registry::new();
        register_builtins(&mut registry).expect(
            "the built-in features pass the registration checks, as the registry tests verify",
        );
        registry
    }

    /// Registers a typed feature. Fails on an invalid or duplicate type name, a reserved field, a schema conflict or a manifest mismatch.
    pub fn register<F: Feature>(&mut self) -> Result<(), RegistryError> {
        self.register_dyn(Box::new(Typed::<F>::new()))
    }

    /// Registers a type-erased feature (for future script and plugin hosts). Same checks as `register`.
    pub fn register_dyn(&mut self, feature: Box<dyn DynFeature>) -> Result<(), RegistryError> {
        let type_name = feature.type_name().to_string();
        if !is_valid_feature_type(&type_name) {
            return Err(RegistryError::InvalidTypeName { type_name });
        }
        if self.features.contains_key(&type_name) {
            return Err(RegistryError::DuplicateType { type_name });
        }
        let mut params_schema = feature.params_schema();
        schema::check(
            &type_name,
            feature.manifest(),
            &params_schema,
            &self.definitions,
        )?;
        let root = params_schema
            .as_object_mut()
            .expect("the schema check accepts only object schemas");
        root.insert(
            "title".to_string(),
            Value::String(feature.manifest().label.clone()),
        );
        for (name, definition) in schema::own_definitions(root).into_iter().flatten() {
            self.definitions
                .entry(name.clone())
                .or_insert_with(|| definition.clone());
        }
        let known_keys = schema::known_keys(params_schema.as_value());
        self.features.insert(
            type_name,
            Registered {
                feature: Arc::from(feature),
                schema: params_schema,
                known_keys,
            },
        );
        Ok(())
    }

    /// The feature registered under `type_name`.
    pub fn get(&self, type_name: &str) -> Option<&dyn DynFeature> {
        self.features
            .get(type_name)
            .map(|registered| registered.feature.as_ref())
    }

    /// Registered type names in byte order.
    pub fn types(&self) -> impl Iterator<Item = &str> + '_ {
        self.features.keys().map(String::as_str)
    }

    /// Validates the record's params and returns the record with canonical params plus the unknown keys and their
    /// near matches (section 3.2).
    pub fn validate_and_canonicalize(
        &self,
        record: &FeatureRecord,
    ) -> Result<Validated, FeatureError> {
        let registered = self.registered(record)?;
        let canonical = registered.feature.canonicalize(&record.params)?;
        let suggestions = near_matches(
            &canonical.unknown_keys,
            &registered.known_keys,
            &record.params,
            registered.feature.manifest(),
        );
        Ok(Validated {
            record: FeatureRecord {
                id: record.id.clone(),
                kind: record.kind.clone(),
                name: record.name.clone(),
                suppressed: record.suppressed,
                params: canonical.params,
            },
            unknown_keys: canonical.unknown_keys,
            suggestions,
        })
    }

    /// The params schema of every registered type, sorted by type name, for the `.ptsm` schema and the protocol catalog.
    pub fn params_schemas(&self) -> Vec<FeatureParamsSchema> {
        self.features
            .iter()
            .map(|(kind, registered)| FeatureParamsSchema {
                kind: kind.clone(),
                schema: registered.schema.clone(),
            })
            .collect()
    }

    /// Declared dependencies of a valid record.
    pub fn dependencies(&self, record: &FeatureRecord) -> Result<Dependencies, FeatureError> {
        self.feature(record)?.dependencies(&record.params)
    }

    /// Builds a valid record.
    pub fn build(
        &self,
        record: &FeatureRecord,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        self.feature(record)?.build(&record.params, context)
    }

    /// The Python call that recreates the record (section 3.11): the dedicated form with the record keywords appended when the type has one for these params, else the generic form. Never fails.
    pub fn python(&self, record: &FeatureRecord, names: &dyn PythonNames) -> PythonCall {
        self.python_dedicated(record, names)
            .unwrap_or_else(|| self.python_generic(record))
    }

    /// The dedicated form with the record keywords appended, when the type is registered, the record validates and its hook has one for these params (section 3.11); None otherwise.
    pub fn python_dedicated(
        &self,
        record: &FeatureRecord,
        names: &dyn PythonNames,
    ) -> Option<PythonCall> {
        let feature = self.get(&record.kind)?;
        let validated = feature.canonicalize(&record.params).ok()?;
        let mut call = feature.python(&validated.params, names)?;
        call.keywords.extend(record_keywords(record));
        Some(call)
    }

    /// The generic `feature(...)` call that recreates any record, registered or not, valid or not (section 3.11). Never fails.
    pub fn python_generic(&self, record: &FeatureRecord) -> PythonCall {
        let mut call = PythonCall::new("feature");
        call.positional.push(PythonValue::Str(record.kind.clone()));
        call.keywords.extend(record_keywords(record));
        for (key, value) in &record.params {
            let argument = (key.clone(), PythonValue::from_json(value));
            if is_keyword_name(key) {
                call.keywords.push(argument);
            } else {
                call.extra.push(argument);
            }
        }
        call
    }

    /// The label of a feature in a document (section 3.12): its name, or the manifest label (the
    /// type name for an unknown type) followed by the 1-based count of records of its type up to
    /// and including it in timeline order.
    pub fn label(&self, document: &Document, feature: &Id) -> Option<String> {
        let position = document
            .features
            .iter()
            .position(|record| record.id == *feature)?;
        let record = &document.features[position];
        if let Some(name) = &record.name {
            return Some(name.clone());
        }
        let count = document.features[..=position]
            .iter()
            .filter(|earlier| earlier.kind == record.kind)
            .count();
        let label = self
            .get(&record.kind)
            .map_or(record.kind.as_str(), |registered| {
                registered.manifest().label.as_str()
            });
        Some(format!("{label} {count}"))
    }

    /// The feature registered for the record's type.
    fn feature(&self, record: &FeatureRecord) -> Result<&dyn DynFeature, FeatureError> {
        self.registered(record)
            .map(|registered| registered.feature.as_ref())
    }

    /// The registration of the record's type; `UnknownType` when it has none.
    fn registered(&self, record: &FeatureRecord) -> Result<&Registered, FeatureError> {
        self.features
            .get(&record.kind)
            .ok_or_else(|| FeatureError::UnknownType {
                type_name: record.kind.clone(),
            })
    }
}

/// For each unknown key within `SUGGESTION_DISTANCE` of exactly one known top-level key that `params` does not set,
/// the key and that known field, in the order of `unknown` (section 3.2). Renaming onto a key that is set would
/// overwrite the user's value, so such a key gets no suggestion.
fn near_matches(
    unknown: &[String],
    known: &[String],
    params: &Map<String, Value>,
    manifest: &FeatureManifest,
) -> Vec<(String, FieldRef)> {
    unknown
        .iter()
        .filter_map(|key| {
            let mut near = known
                .iter()
                .filter(|known| edit_distance(key, known) <= SUGGESTION_DISTANCE);
            let only = near.next()?;
            (near.next().is_none() && !params.contains_key(only))
                .then(|| (key.clone(), FieldRef::new(only, manifest)))
        })
        .collect()
}

/// Registers every built-in feature type through `Registry::register`, exactly as a plugin would.
pub fn register_builtins(registry: &mut Registry) -> Result<(), RegistryError> {
    for register in BUILTINS {
        register(registry)?;
    }
    Ok(())
}

/// Lets the model validate documents without reading params (model.md section 2.7).
impl FeatureInspector for Registry {
    /// Validates and canonicalizes the record, then reports its declared ids, its written then
    /// read bodies, its features and its expressions with JSON Pointers. On failure, the error's
    /// `Display` text.
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String> {
        let links = || -> Result<FeatureLinks, FeatureError> {
            let validated = self.validate_and_canonicalize(feature)?;
            let dependencies = self.dependencies(&validated.record)?;
            Ok(FeatureLinks {
                declared_ids: dependencies.declared_ids,
                bodies: dependencies
                    .bodies_written
                    .into_iter()
                    .chain(dependencies.bodies_read)
                    .map(|link| link.id)
                    .collect(),
                features: dependencies
                    .features
                    .into_iter()
                    .map(|link| link.id)
                    .collect(),
                expressions: dependencies
                    .expressions
                    .into_iter()
                    .map(|expression| ExprLink {
                        pointer: pointer(&expression.field),
                        expr: expression.expr,
                        unit: expression.kind,
                    })
                    .collect(),
            })
        };
        links().map_err(|error| error.to_string())
    }
}

/// The `name=`, `suppressed=True` and `id=` keywords of a record, in that order, each only when set.
fn record_keywords(record: &FeatureRecord) -> Vec<(String, PythonValue)> {
    let mut keywords = Vec::new();
    if let Some(name) = &record.name {
        keywords.push(("name".to_string(), PythonValue::Str(name.clone())));
    }
    if record.suppressed {
        keywords.push(("suppressed".to_string(), PythonValue::Bool(true)));
    }
    keywords.push(("id".to_string(), PythonValue::Str(record.id.to_string())));
    keywords
}

/// Object-safe form of `Feature`, working on generic JSON params.
pub trait DynFeature: Send + Sync {
    /// The type name.
    fn type_name(&self) -> &str;
    /// The manifest.
    fn manifest(&self) -> &FeatureManifest;
    /// The root JSON Schema of the params, as schemars generates it (with its own `$defs`).
    fn params_schema(&self) -> Schema;
    /// Parses, validates and re-serializes params in canonical order.
    fn canonicalize(&self, params: &Map<String, Value>) -> Result<Canonical, FeatureError>;
    /// Declared dependencies.
    fn dependencies(&self, params: &Map<String, Value>) -> Result<Dependencies, FeatureError>;
    /// Builds.
    fn build(
        &self,
        params: &Map<String, Value>,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError>;
    /// The dedicated Python call (`Feature::python`). None when the type has none or the params do not deserialize.
    fn python(&self, params: &Map<String, Value>, names: &dyn PythonNames) -> Option<PythonCall>;
}

/// Canonical params plus the keys the type does not know.
#[derive(Clone, Debug, PartialEq)]
pub struct Canonical {
    /// Known keys in struct order with defaults omitted, then unknown keys in their original order.
    pub params: Map<String, Value>,
    /// The unknown keys, in their original order.
    pub unknown_keys: Vec<String>,
}

/// A record with canonical params.
#[derive(Clone, Debug, PartialEq)]
pub struct Validated {
    /// The record with `params` replaced by the canonical params. Other fields unchanged.
    pub record: FeatureRecord,
    /// Keys the feature type does not know. They are kept in `record.params` and ignored by `build`.
    pub unknown_keys: Vec<String>,
    /// For each unknown key that nearly matches exactly one known key (section 3.2), the key and that known field, in
    /// the order of `unknown_keys`.
    pub suggestions: Vec<(String, FieldRef)>,
}

/// A registration failure. These are programming errors in a feature, not user errors.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegistryError {
    /// The type name does not match `[a-z][a-z0-9_]{0,63}`.
    #[error(
        "Feature type name '{type_name}' is invalid. Use lowercase letters, digits and underscores, starting with a letter, at most 64 characters."
    )]
    InvalidTypeName {
        /// The rejected type name.
        type_name: String,
    },
    /// The type name is already registered.
    #[error(
        "A feature type named '{type_name}' is already registered. Pick a different type name."
    )]
    DuplicateType {
        /// The type name.
        type_name: String,
    },
    /// A params field uses a record field name.
    #[error(
        "Feature type '{type_name}' has a params field named '{field}', which is reserved for the feature record. Rename the field."
    )]
    ReservedField {
        /// The type name.
        type_name: String,
        /// The reserved field.
        field: String,
    },
    /// The params schema is not an open object schema.
    #[error(
        "The params schema of '{type_name}' must be an object that allows extra keys. Do not use deny_unknown_fields on the params struct."
    )]
    NotOpenObject {
        /// The type name.
        type_name: String,
    },
    /// A schema definition name is already registered with a different schema.
    #[error(
        "Feature type '{type_name}' defines schema '{definition}' differently from an already registered feature. Rename one of the types."
    )]
    SchemaConflict {
        /// The type name.
        type_name: String,
        /// The conflicting definition name.
        definition: String,
    },
    /// A manifest hint does not match the params schema.
    #[error(
        "The manifest of '{type_name}' has a problem with field '{path}': {problem}. Fix the manifest or the params struct."
    )]
    ManifestMismatch {
        /// The type name.
        type_name: String,
        /// The hint path or top-level field.
        path: String,
        /// What does not match.
        problem: String,
    },
}

/// A typed feature as a `DynFeature`.
struct Typed<F> {
    /// `F::manifest()`, computed once.
    manifest: FeatureManifest,
    /// The schemars root schema of `F::Params`, computed once.
    schema: Schema,
    /// The top-level params keys the type knows: the properties of its schema's root and of its
    /// root branches.
    known_keys: BTreeSet<String>,
    /// The feature type.
    feature: PhantomData<fn() -> F>,
}

impl<F: Feature> Typed<F> {
    /// The adapter of `F`, with its manifest, schema and known keys computed once.
    fn new() -> Self {
        let schema = schemars::schema_for!(F::Params);
        let known_keys = schema::known_keys(schema.as_value()).into_iter().collect();
        Typed {
            manifest: F::manifest(),
            schema,
            known_keys,
            feature: PhantomData,
        }
    }

    /// Deserializes and validates params.
    fn parse(&self, params: &Map<String, Value>) -> Result<F::Params, FeatureError> {
        let typed = from_params(params).map_err(|problem| problem.into_error(&self.manifest))?;
        F::validate(&typed)?;
        Ok(typed)
    }
}

impl<F: Feature> DynFeature for Typed<F> {
    fn type_name(&self) -> &str {
        F::TYPE
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn params_schema(&self) -> Schema {
        self.schema.clone()
    }

    fn canonicalize(&self, params: &Map<String, Value>) -> Result<Canonical, FeatureError> {
        let typed = self.parse(params)?;
        let serialized = canonical::to_value(&typed).map_err(|error| {
            FeatureError::internal(format!("the params do not serialize: {error}"))
        })?;
        let Value::Object(mut canonical_params) = serialized else {
            return Err(FeatureError::internal(
                "the params do not serialize as an object",
            ));
        };
        let mut unknown_keys = Vec::new();
        for (key, value) in params {
            if !self.known_keys.contains(key) {
                canonical_params.insert(key.clone(), value.clone());
                unknown_keys.push(key.clone());
            }
        }
        Ok(Canonical {
            params: canonical_params,
            unknown_keys,
        })
    }

    fn dependencies(&self, params: &Map<String, Value>) -> Result<Dependencies, FeatureError> {
        Ok(F::dependencies(&self.parse(params)?))
    }

    fn build(
        &self,
        params: &Map<String, Value>,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        F::build(&self.parse(params)?, context)
    }

    fn python(&self, params: &Map<String, Value>, names: &dyn PythonNames) -> Option<PythonCall> {
        F::python(&from_params(params).ok()?, names)
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Expr, FieldHint, FieldUnit, OptionHint};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::*;
    use crate::output::OutputData;
    use crate::test_support::{id, manifest};

    /// Params that flatten a tagged enum into the params root.
    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct FlatParams {
        /// The body.
        body: Id,
        /// The extent, flattened.
        #[serde(flatten)]
        extent: FlatExtent,
    }

    /// A tagged extent.
    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum FlatExtent {
        /// One distance.
        Distance {
            /// The distance.
            expr: Expr,
        },
        /// Through everything.
        ThroughAll,
    }

    /// The hints of every key `FlatParams` knows.
    fn flat_hints() -> Vec<FieldHint> {
        let hint = |path: &str| FieldHint {
            path: path.to_string(),
            label: path.to_string(),
            essential: false,
            unit: None,
            selection: None,
            options: Vec::new(),
        };
        let option = |value: &str| OptionHint {
            value: value.to_string(),
            label: value.to_string(),
        };
        vec![
            hint("body"),
            FieldHint {
                options: vec![option("distance"), option("through_all")],
                ..hint("kind")
            },
            FieldHint {
                unit: Some(FieldUnit::Length),
                ..hint("expr")
            },
        ]
    }

    /// A feature whose params flatten an enum, with hints for every key.
    struct Flat;

    impl Feature for Flat {
        const TYPE: &'static str = "flat";
        type Params = FlatParams;

        fn manifest() -> FeatureManifest {
            manifest(flat_hints())
        }

        fn dependencies(_params: &FlatParams) -> Dependencies {
            Dependencies::default()
        }

        fn build(
            _params: &FlatParams,
            _context: &BuildContext<'_>,
        ) -> Result<FeatureOutput, FeatureError> {
            Ok(FeatureOutput::new(OutputData::Nothing, Vec::new()))
        }
    }

    /// `Flat` without a hint for the flattened tag `kind`.
    struct FlatWithoutKindHint;

    impl Feature for FlatWithoutKindHint {
        const TYPE: &'static str = "flat_without_kind_hint";
        type Params = FlatParams;

        fn manifest() -> FeatureManifest {
            let mut hints = flat_hints();
            hints.retain(|hint| hint.path != "kind");
            manifest(hints)
        }

        fn dependencies(params: &FlatParams) -> Dependencies {
            Flat::dependencies(params)
        }

        fn build(
            params: &FlatParams,
            context: &BuildContext<'_>,
        ) -> Result<FeatureOutput, FeatureError> {
            Flat::build(params, context)
        }
    }

    fn flat_record(params: serde_json::Value) -> FeatureRecord {
        let mut record = FeatureRecord::new(id("f-flat1"), "flat");
        record.params = params
            .as_object()
            .expect("the params are an object")
            .clone();
        record
    }

    #[test]
    fn flattened_fields_are_known_keys() {
        let mut registry = Registry::new();
        registry.register::<Flat>().expect("flat registers");

        let validated = registry
            .validate_and_canonicalize(&flat_record(
                json!({"body": "b-1", "kind": "distance", "expr": "5", "taper": 1}),
            ))
            .expect("the params are valid");

        assert_eq!(validated.unknown_keys, ["taper"]);
        assert_eq!(
            serde_json::to_string(&validated.record.params).expect("params serialize"),
            r#"{"body":"b-1","kind":"distance","expr":"5","taper":1}"#
        );
    }

    #[test]
    fn flattened_fields_need_hints() {
        let mut registry = Registry::new();

        let result = registry.register::<FlatWithoutKindHint>();

        assert!(
            matches!(&result, Err(RegistryError::ManifestMismatch { path, .. }) if path == "kind"),
            "got {result:?}"
        );
    }
}
