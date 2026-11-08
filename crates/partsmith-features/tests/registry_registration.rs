//! Acceptance tests for feature registration, built-in manifests and params schemas, with the
//! defaults the feature catalog carries (features.md section 6.2, tests 1 to 7 and 15), and the
//! manifest hints of M2 (features.md section 6.8, tests 46 to 63, ADR 0065).
//!
//! Tests of the 20 built-in feature types that check framework behavior each have a companion with
//! the same number that uses test features instead.
//!
//! The manifest test of each feature spec (features/<type>.md, for example extrude.md test 64) is
//! that type's case of features.md tests 46, 48, 50 and 51 (and 52 for extrude, revolve and hole).
//! Each has a test of its own, named after its type and its number, beside the tests of section 6.8
//! that run every type.

use crate::common;

use std::marker::PhantomData;

use common::{
    Probe, Slab, expression_hint, hint, manifest, no_dependencies, nothing, options_hint, record,
    selection_hint,
};
use partsmith_features::{
    BodyCreation, BodyOp, BuildContext, Canonical, Category, Dependencies, DynFeature, Feature,
    FeatureError, FeatureManifest, FeatureOutput, FieldHint, FieldUnit, PythonCall, PythonNames,
    Registry, RegistryError, SelectionKind, StartValue, register_builtins,
};
use partsmith_model::{Expr, Id};
use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Declares a unit struct implementing `Feature` with the given type name, params and manifest,
/// no dependencies and an empty output.
macro_rules! test_feature {
    ($(#[$attribute:meta])* $name:ident = $type_name:literal, $params:ty, $manifest:expr) => {
        $(#[$attribute])*
        struct $name;

        impl Feature for $name {
            const TYPE: &'static str = $type_name;
            type Params = $params;

            fn manifest() -> FeatureManifest {
                $manifest
            }

            fn dependencies(_params: &$params) -> Dependencies {
                no_dependencies()
            }

            fn build(
                _params: &$params,
                _context: &BuildContext<'_>,
            ) -> Result<FeatureOutput, FeatureError> {
                Ok(nothing())
            }
        }
    };
}

/// Params with only a target body.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct MinimalParams {
    /// Target body.
    body: Id,
}

/// The manifest of `MinimalParams` with the given label.
fn minimal_manifest(label: &str) -> FeatureManifest {
    manifest(label, vec![hint("body", "Body")])
}

/// Params with a field named like a record field.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct NamedParams {
    /// Target body.
    body: Id,
    /// Collides with the record's `name`.
    name: String,
}

/// Params that reject unknown keys.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClosedParams {
    /// Target body.
    body: Id,
}

/// Params with a size field.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct SizedParams {
    /// Target body.
    body: Id,
    /// Three lengths.
    size: [Expr; 3],
}

/// Holds the first of two different structs named `Extent`.
mod first {
    use partsmith_model::Expr;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    /// An extent with one distance.
    #[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
    pub struct Extent {
        /// The distance.
        pub expr: Expr,
    }
}

/// Holds the second of two different structs named `Extent`.
mod second {
    use partsmith_model::Expr;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    /// An extent with a distance and a flip flag.
    #[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
    pub struct Extent {
        /// The distance.
        pub expr: Expr,
        /// Whether the direction is flipped.
        pub flip: bool,
    }
}

/// Params using the first `Extent`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct FirstExtentParams {
    /// Target body.
    body: Id,
    /// The extent.
    extent: first::Extent,
}

/// Params using the second `Extent`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct SecondExtentParams {
    /// Target body.
    body: Id,
    /// The extent.
    extent: second::Extent,
}

/// The manifest of the two extent test features.
fn extent_manifest() -> FeatureManifest {
    manifest(
        "Extent test",
        vec![hint("body", "Body"), hint("extent", "Extent")],
    )
}

test_feature!(
    /// A second feature named like the built-in extrude.
    FakeExtrude = "extrude", MinimalParams, minimal_manifest("Fake extrude")
);
test_feature!(
    /// A second feature named like the probe test feature.
    ProbeAgain = "probe", MinimalParams, minimal_manifest("Probe again")
);
test_feature!(
    /// A type name with an uppercase letter and punctuation.
    Exclaimed = "Extrude!", MinimalParams, minimal_manifest("Exclaimed")
);
test_feature!(
    /// The shortest valid type name.
    SingleLetter = "a", MinimalParams, minimal_manifest("Single letter")
);
test_feature!(
    /// A type name of 64 characters, the longest allowed.
    SixtyFourCharacters = "abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcd",
    MinimalParams,
    minimal_manifest("Sixty-four characters")
);
test_feature!(
    /// A type name of 65 characters, one too many.
    SixtyFiveCharacters = "abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcde",
    MinimalParams,
    minimal_manifest("Sixty-five characters")
);
test_feature!(
    /// Params with a field `name`.
    Named = "named", NamedParams, manifest("Named", vec![hint("body", "Body"), hint("name", "Name")])
);
test_feature!(
    /// Params that use `deny_unknown_fields`.
    Closed = "closed", ClosedParams, minimal_manifest("Closed")
);
test_feature!(
    /// A manifest hint for a field the params do not have.
    UnknownHint = "unknown_hint",
    MinimalParams,
    manifest("Unknown hint", vec![hint("body", "Body"), hint("depth", "Depth")])
);
test_feature!(
    /// A top-level field without a manifest hint.
    UncoveredField = "uncovered_field", SizedParams, minimal_manifest("Uncovered field")
);
test_feature!(
    /// Params whose schema defines the first `Extent`.
    FirstExtent = "first_extent", FirstExtentParams, extent_manifest()
);
test_feature!(
    /// Params whose schema defines a different `Extent`.
    SecondExtent = "second_extent", SecondExtentParams, extent_manifest()
);

/// The built-in catalog of features.md section 3.13: type, label, category and icon.
const CATALOG: [(&str, &str, Category, &str); 20] = [
    ("sketch", "Sketch", Category::Sketch, "sketch"),
    ("extrude", "Extrude", Category::Create, "extrude"),
    ("revolve", "Revolve", Category::Create, "revolve"),
    ("hole", "Hole", Category::Create, "hole"),
    ("import", "Import", Category::Create, "import"),
    ("box", "Box", Category::Create, "box"),
    ("cylinder", "Cylinder", Category::Create, "cylinder"),
    ("sphere", "Sphere", Category::Create, "sphere"),
    ("cone", "Cone", Category::Create, "cone"),
    ("torus", "Torus", Category::Create, "torus"),
    ("fillet", "Fillet", Category::Modify, "fillet"),
    ("chamfer", "Chamfer", Category::Modify, "chamfer"),
    ("shell", "Shell", Category::Modify, "shell"),
    ("boolean", "Boolean", Category::Modify, "boolean"),
    ("move_body", "Move body", Category::Modify, "move-body"),
    ("split_body", "Split body", Category::Modify, "split-body"),
    ("mirror", "Mirror", Category::Pattern, "mirror"),
    (
        "linear_pattern",
        "Linear pattern",
        Category::Pattern,
        "linear-pattern",
    ),
    (
        "circular_pattern",
        "Circular pattern",
        Category::Pattern,
        "circular-pattern",
    ),
    (
        "datum_plane",
        "Datum plane",
        Category::Construct,
        "datum-plane",
    ),
];

/// `ptsm_schema` of the registry's params schemas, with the stand-in printer profile schema
/// `{"type": "object"}` (file-format.md section 3.10).
fn ptsm_schema(registry: &Registry) -> Result<schemars::Schema, partsmith_model::SchemaError> {
    let profile = schemars::Schema::try_from(json!({"type": "object"})).expect("a schema");
    partsmith_model::ptsm_schema(&registry.params_schemas(), &profile)
}

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

/// The keys of a JSON object in order.
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .expect("the value is an object")
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn t001_with_builtins_lists_the_twenty_types_in_byte_order() {
    let registry = Registry::with_builtins();

    assert_eq!(
        registry.types().collect::<Vec<_>>(),
        [
            "boolean",
            "box",
            "chamfer",
            "circular_pattern",
            "cone",
            "cylinder",
            "datum_plane",
            "extrude",
            "fillet",
            "hole",
            "import",
            "linear_pattern",
            "mirror",
            "move_body",
            "revolve",
            "shell",
            "sketch",
            "sphere",
            "split_body",
            "torus",
        ]
    );
}

#[test]
fn t001_types_of_a_test_registry_are_in_byte_order() {
    let registry = test_registry();

    assert_eq!(registry.types().collect::<Vec<_>>(), ["probe", "slab"]);
}

#[test]
fn t001_a_new_registry_is_empty() {
    assert_eq!(Registry::new().types().count(), 0);
}

#[test]
fn t002_registering_a_second_extrude_is_a_duplicate_and_changes_nothing() {
    let mut registry = Registry::with_builtins();

    let result = registry.register::<FakeExtrude>();

    assert_eq!(
        result,
        Err(RegistryError::DuplicateType {
            type_name: "extrude".to_string()
        })
    );
    assert_eq!(registry.types().count(), 20);
    assert_eq!(
        registry
            .get("extrude")
            .expect("extrude is still registered")
            .manifest()
            .label,
        "Extrude"
    );
}

#[test]
fn t002_registering_a_type_twice_is_a_duplicate_and_changes_nothing() {
    let mut registry = test_registry();

    let result = registry.register::<ProbeAgain>();

    assert_eq!(
        result,
        Err(RegistryError::DuplicateType {
            type_name: "probe".to_string()
        })
    );
    assert_eq!(registry.types().collect::<Vec<_>>(), ["probe", "slab"]);
    assert_eq!(
        registry
            .get("probe")
            .expect("probe is still registered")
            .manifest()
            .label,
        "Probe"
    );
}

#[test]
fn t003_type_name_with_uppercase_and_punctuation_is_invalid() {
    let mut registry = Registry::new();

    assert_eq!(
        registry.register::<Exclaimed>(),
        Err(RegistryError::InvalidTypeName {
            type_name: "Extrude!".to_string()
        })
    );
    assert_eq!(registry.types().count(), 0);
}

#[test]
fn t003_single_letter_type_name_is_valid() {
    let mut registry = Registry::new();

    assert_eq!(registry.register::<SingleLetter>(), Ok(()));
    assert_eq!(registry.types().collect::<Vec<_>>(), ["a"]);
}

#[test]
fn t003_type_name_of_64_characters_is_valid() {
    let mut registry = Registry::new();
    assert_eq!(SixtyFourCharacters::TYPE.len(), 64);

    assert_eq!(registry.register::<SixtyFourCharacters>(), Ok(()));
}

#[test]
fn t003_type_name_of_65_characters_is_invalid() {
    let mut registry = Registry::new();
    assert_eq!(SixtyFiveCharacters::TYPE.len(), 65);

    assert_eq!(
        registry.register::<SixtyFiveCharacters>(),
        Err(RegistryError::InvalidTypeName {
            type_name: SixtyFiveCharacters::TYPE.to_string()
        })
    );
}

#[test]
fn t004_params_field_named_name_is_reserved() {
    let mut registry = Registry::new();

    assert_eq!(
        registry.register::<Named>(),
        Err(RegistryError::ReservedField {
            type_name: "named".to_string(),
            field: "name".to_string()
        })
    );
    assert_eq!(registry.types().count(), 0);
}

#[test]
fn t004_params_with_deny_unknown_fields_are_not_an_open_object() {
    let mut registry = Registry::new();

    assert_eq!(
        registry.register::<Closed>(),
        Err(RegistryError::NotOpenObject {
            type_name: "closed".to_string()
        })
    );
    assert_eq!(registry.types().count(), 0);
}

#[test]
fn t005_hint_for_a_missing_field_is_a_manifest_mismatch() {
    let mut registry = Registry::new();

    let result = registry.register::<UnknownHint>();

    assert!(
        matches!(
            &result,
            Err(RegistryError::ManifestMismatch { type_name, path, .. })
                if type_name == "unknown_hint" && path == "depth"
        ),
        "got {result:?}"
    );
    assert_eq!(registry.types().count(), 0);
}

#[test]
fn t005_top_level_field_without_a_hint_is_a_manifest_mismatch() {
    let mut registry = Registry::new();

    let result = registry.register::<UncoveredField>();

    assert!(
        matches!(
            &result,
            Err(RegistryError::ManifestMismatch { type_name, path, .. })
                if type_name == "uncovered_field" && path == "size"
        ),
        "got {result:?}"
    );
    assert_eq!(registry.types().count(), 0);
}

#[test]
fn t006_different_definitions_with_one_name_conflict() {
    let mut registry = Registry::new();
    registry
        .register::<FirstExtent>()
        .expect("the first Extent definition registers");

    let result = registry.register::<SecondExtent>();

    assert_eq!(
        result,
        Err(RegistryError::SchemaConflict {
            type_name: "second_extent".to_string(),
            definition: "Extent".to_string()
        })
    );
    assert_eq!(registry.types().collect::<Vec<_>>(), ["first_extent"]);
}

#[test]
fn t006_identical_definitions_with_one_name_are_shared() {
    let mut registry = Registry::new();

    // Both params schemas define `Id` the same way, so the second registration shares it.
    assert_eq!(registry.register::<SingleLetter>(), Ok(()));
    assert_eq!(registry.register::<FirstExtent>(), Ok(()));
    assert_eq!(registry.types().collect::<Vec<_>>(), ["a", "first_extent"]);
}

#[test]
fn t007_builtin_manifests_match_the_catalog() {
    let registry = Registry::with_builtins();

    for (type_name, label, category, icon) in CATALOG {
        let manifest = registry
            .get(type_name)
            .unwrap_or_else(|| panic!("{type_name} is registered"))
            .manifest();
        assert_eq!(manifest.version, 1, "version of {type_name}");
        assert_eq!(manifest.label, label, "label of {type_name}");
        assert_eq!(manifest.category, category, "category of {type_name}");
        assert_eq!(manifest.icon, icon, "icon of {type_name}");
    }
}

#[test]
fn t007_registered_manifest_is_the_feature_manifest() {
    let registry = test_registry();

    let registered = registry.get("slab").expect("slab is registered");

    assert_eq!(registered.type_name(), "slab");
    assert_eq!(*registered.manifest(), Slab::manifest());
}

#[test]
fn t015_builtin_params_schemas_are_open_objects_titled_by_label() {
    let registry = Registry::with_builtins();

    let schemas = registry.params_schemas();

    let kinds: Vec<&str> = schemas.iter().map(|schema| schema.kind.as_str()).collect();
    assert_eq!(kinds, registry.types().collect::<Vec<_>>());
    assert_eq!(kinds.len(), 20);
    for schema in &schemas {
        let root = schema.schema.as_value();
        let label = &registry
            .get(&schema.kind)
            .expect("each schema kind is registered")
            .manifest()
            .label;
        assert_eq!(root["type"], json!("object"), "type of {}", schema.kind);
        assert_ne!(
            root.get("additionalProperties"),
            Some(&json!(false)),
            "additionalProperties of {}",
            schema.kind
        );
        assert_eq!(root["title"], json!(label), "title of {}", schema.kind);
    }
}

#[test]
fn t015_builtin_params_schemas_compose_into_the_ptsm_schema() {
    let registry = Registry::with_builtins();

    let schema = ptsm_schema(&registry).expect("the built-in params schemas compose");

    let properties = schema
        .as_value()
        .pointer("/$defs/feature.extrude/properties")
        .expect("the extrude params have properties");
    for key in ["body", "profile", "extent", "op"] {
        assert!(
            properties.get(key).is_some(),
            "extrude properties lack {key}: {properties}"
        );
    }
}

#[test]
fn t015_params_schemas_are_sorted_by_kind() {
    let mut registry = Registry::new();
    registry.register::<Slab>().expect("slab registers");
    registry.register::<Probe>().expect("probe registers");

    let schemas = registry.params_schemas();

    let kinds: Vec<&str> = schemas.iter().map(|schema| schema.kind.as_str()).collect();
    assert_eq!(kinds, ["probe", "slab"]);
}

#[test]
fn t015_params_schema_is_an_open_object_titled_by_the_label() {
    let registry = test_registry();

    let schemas = registry.params_schemas();

    let slab = schemas[1].schema.as_value();
    assert_eq!(slab["type"], json!("object"));
    assert_ne!(slab.get("additionalProperties"), Some(&json!(false)));
    assert_ne!(slab.get("unevaluatedProperties"), Some(&json!(false)));
    assert_eq!(slab["title"], json!("Slab"));
}

#[test]
fn t015_params_schema_is_described_by_the_params_doc_comment() {
    let registry = test_registry();

    let schemas = registry.params_schemas();

    assert_eq!(
        schemas[1].schema.as_value()["description"],
        json!(
            "Params of the slab test feature: the shape of extrude's params (features/extrude.md) with two extents."
        )
    );
}

#[test]
fn t015_params_schema_lists_properties_in_struct_order() {
    let registry = test_registry();

    let schemas = registry.params_schemas();

    assert_eq!(
        keys(&schemas[1].schema.as_value()["properties"]),
        ["body", "profile", "extent", "reverse", "op"]
    );
}

#[test]
fn t015_params_schema_requires_the_fields_without_defaults() {
    let registry = test_registry();

    let schemas = registry.params_schemas();

    assert_eq!(
        schemas[1].schema.as_value()["required"],
        json!(["body", "profile", "extent", "op"])
    );
}

#[test]
fn t015_params_schemas_compose_into_the_ptsm_schema() {
    let registry = test_registry();

    let schema = ptsm_schema(&registry).expect("the test params schemas compose");

    let properties = schema
        .as_value()
        .pointer("/$defs/feature.slab/properties")
        .expect("the slab params have properties");
    assert_eq!(
        keys(properties),
        ["body", "profile", "extent", "reverse", "op"]
    );
}

/// The fixed-length arrays of three lengths whose default is `["0", "0", "0"]`, by feature type
/// and property (move_body.md section 2, features.md section 3.14), with a minimal valid record
/// of each type that sets the property to its default.
fn zero_triples() -> [(&'static str, &'static str, Value); 6] {
    let zero = json!(["0", "0", "0"]);
    [
        (
            "move_body",
            "translation",
            json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": zero}),
        ),
        (
            "box",
            "position",
            json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": zero, "op": "new"}),
        ),
        (
            "cylinder",
            "position",
            json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "position": zero, "op": "new"}),
        ),
        (
            "sphere",
            "position",
            json!({"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "5", "position": zero, "op": "new"}),
        ),
        (
            "cone",
            "position",
            json!({"id": "f-con1", "type": "cone", "body": "b-1", "bottom_radius": "5", "top_radius": "2", "height": "10", "position": zero, "op": "new"}),
        ),
        (
            "torus",
            "position",
            json!({"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "10", "minor_radius": "2", "position": zero, "op": "new"}),
        ),
    ]
}

/// The root params schema of built-in type `kind`, as the feature catalog carries it.
fn builtin_params_schema(kind: &str) -> Value {
    Registry::with_builtins()
        .params_schemas()
        .into_iter()
        .find(|schema| schema.kind == kind)
        .unwrap_or_else(|| panic!("{kind} has a params schema"))
        .schema
        .as_value()
        .clone()
}

/// Asserts that property `property` of the params schema of `kind` is an optional array of
/// exactly three items whose `default` is `["0", "0", "0"]`, so a host starts each item at 0 and
/// never sends a shorter array (ui.md section 2.12, open issue 17).
fn assert_zero_triple_default(kind: &str, property: &str) {
    let root = builtin_params_schema(kind);

    let schema = &root["properties"][property];

    assert_eq!(
        schema["default"],
        json!(["0", "0", "0"]),
        "default of {kind} {property}: {schema}"
    );
    assert_eq!(
        schema["type"],
        json!("array"),
        "{kind} {property}: {schema}"
    );
    assert_eq!(schema["minItems"], json!(3), "{kind} {property}: {schema}");
    assert_eq!(schema["maxItems"], json!(3), "{kind} {property}: {schema}");
    let required = root["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{kind} lists its required properties: {root}"));
    assert!(
        !required.contains(&json!(property)),
        "{kind} {property} stays optional: {required:?}"
    );
}

#[test]
fn t015_move_body_translation_defaults_to_zero() {
    assert_zero_triple_default("move_body", "translation");
}

#[test]
fn t015_box_position_defaults_to_zero() {
    assert_zero_triple_default("box", "position");
}

#[test]
fn t015_cylinder_position_defaults_to_zero() {
    assert_zero_triple_default("cylinder", "position");
}

#[test]
fn t015_sphere_position_defaults_to_zero() {
    assert_zero_triple_default("sphere", "position");
}

#[test]
fn t015_cone_position_defaults_to_zero() {
    assert_zero_triple_default("cone", "position");
}

#[test]
fn t015_torus_position_defaults_to_zero() {
    assert_zero_triple_default("torus", "position");
}

#[test]
fn t015_zero_triples_stay_omitted_from_canonical_records() {
    let registry = Registry::with_builtins();

    for (kind, property, json) in zero_triples() {
        let validated = registry
            .validate_and_canonicalize(&record(json))
            .unwrap_or_else(|error| panic!("the {kind} record is valid: {error:?}"));

        assert!(
            !validated.record.params.contains_key(property),
            "a default {kind} {property} is omitted: {:?}",
            validated.record.params
        );
    }
}

#[test]
fn t015_zero_triple_defaults_compose_into_the_ptsm_schema() {
    let registry = Registry::with_builtins();

    let schema = ptsm_schema(&registry).expect("the built-in params schemas compose");

    for (kind, property, _) in zero_triples() {
        let pointer = format!("/$defs/feature.{kind}/properties/{property}/default");
        assert_eq!(
            schema.as_value().pointer(&pointer),
            Some(&json!(["0", "0", "0"])),
            "{pointer}"
        );
    }
}

/// The words no built-in description or help text uses (features.md section 2.3, tests 47 and 49).
const FORBIDDEN_WORDS: [&str; 5] = ["omitted", "default", "params", "None", "serde"];

/// The help of the `op` hint of extrude and revolve.
const OPERATION_HELP: &str = "New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap.";

/// The help of the `body` hint of extrude and revolve.
const CHANGED_BODY_HELP: &str = "The body that Join, Cut and Intersect change.";

/// One row of the table of help, start values and new bodies of a feature spec's Manifest section.
struct SpecHint {
    /// The hint path.
    path: &'static str,
    /// The help, or None when the row leaves it empty.
    help: Option<&'static str>,
    /// The start value, or None when the row leaves it empty.
    start: Option<StartValue>,
    /// The new body, or None when the row leaves it empty.
    creates_body: Option<BodyCreation>,
}

/// What the Manifest section of `docs/spec/features/<type>.md` lists beyond the fields table: the
/// description and the table of help, start values and new bodies. A hint the table leaves out has
/// none of the three.
struct SpecManifest {
    /// The feature type.
    type_name: &'static str,
    /// The feature spec's test of this manifest, for failure messages.
    spec_test: &'static str,
    /// The description.
    description: &'static str,
    /// The rows of the table, in the spec's order.
    hints: Vec<SpecHint>,
}

impl SpecManifest {
    /// The row of the table for `path`, if the table lists it.
    fn row(&self, path: &str) -> Option<&SpecHint> {
        self.hints.iter().find(|row| row.path == path)
    }
}

/// A row with only a help text.
fn help(path: &'static str, text: &'static str) -> SpecHint {
    SpecHint {
        path,
        help: Some(text),
        start: None,
        creates_body: None,
    }
}

/// A row with only a start value.
fn start(path: &'static str, value: StartValue) -> SpecHint {
    SpecHint {
        path,
        help: None,
        start: Some(value),
        creates_body: None,
    }
}

/// A row with a help text and a start value.
fn help_and_start(path: &'static str, text: &'static str, value: StartValue) -> SpecHint {
    SpecHint {
        start: Some(value),
        ..help(path, text)
    }
}

/// A row with only a new body.
fn new_body(path: &'static str, creation: BodyCreation) -> SpecHint {
    SpecHint {
        path,
        help: None,
        start: None,
        creates_body: Some(creation),
    }
}

/// The start value `{"kind": "expr", "expr": expr}`.
fn start_expr(expr: &str) -> StartValue {
    StartValue::Expr {
        expr: Expr::new(expr),
    }
}

/// The start value `{"kind": "choice", "value": value}`.
fn start_choice(value: &str) -> StartValue {
    StartValue::Choice {
        value: value.to_string(),
    }
}

/// The start value `{"kind": "multiple", "field": field, "factor": factor}`.
fn start_multiple(field: &str, factor: f64) -> StartValue {
    StartValue::Multiple {
        field: field.to_string(),
        factor,
    }
}

/// A feature spec's manifest with its description and table rows.
fn spec(
    type_name: &'static str,
    spec_test: &'static str,
    description: &'static str,
    hints: Vec<SpecHint>,
) -> SpecManifest {
    SpecManifest {
        type_name,
        spec_test,
        description,
        hints,
    }
}

/// The Manifest sections of the 20 feature specs, in the order of features.md section 3.13.
fn spec_manifests() -> Vec<SpecManifest> {
    vec![
        spec(
            "sketch",
            "sketch.md test 52",
            "Draws a 2D shape on a plane or a flat face, to extrude, revolve or place holes from.",
            Vec::new(),
        ),
        spec(
            "extrude",
            "extrude.md test 64",
            "Pulls a closed sketch shape straight out into a solid, or cuts it into one.",
            vec![
                help(
                    "profile",
                    "The closed sketch shapes to extrude. Click inside a shape to add it.",
                ),
                help(
                    "extent.kind",
                    "Distance goes one way, Symmetric the same distance both ways, Two sides sets each side, Through all goes through the whole body.",
                ),
                help("op", OPERATION_HELP),
                help("reverse", "Extrudes to the other side of the sketch."),
                help("body", CHANGED_BODY_HELP),
            ],
        ),
        spec(
            "revolve",
            "revolve.md test 37",
            "Spins a closed sketch shape around an axis into a round solid, or cuts with it.",
            vec![
                help(
                    "axis",
                    "The line to spin around: an origin axis, a sketch line or a straight edge.",
                ),
                help_and_start(
                    "extent.kind",
                    "Full makes a complete turn. Angle stops part way.",
                    start_choice("full"),
                ),
                help("op", OPERATION_HELP),
                help("reverse", "Turns the other way around the axis."),
                help("body", CHANGED_BODY_HELP),
            ],
        ),
        spec(
            "hole",
            "hole.md test 41",
            "Drills round holes where you click on a flat face or at sketch points.",
            vec![
                help(
                    "points",
                    "Where the holes go: click a flat face, or pick points of a sketch.",
                ),
                start("diameter", start_expr("3.2 mm")),
                help(
                    "hole_type.kind",
                    "Counterbore adds a wider flat recess for a screw head. Countersink adds a cone for a flat-head screw.",
                ),
                help_and_start(
                    "hole_type.diameter",
                    "Diameter of the wider recess at the top of the hole.",
                    start_multiple("diameter", 2.0),
                ),
                help_and_start(
                    "hole_type.depth",
                    "Depth of the recess.",
                    start_expr("3 mm"),
                ),
                help_and_start(
                    "hole_type.angle",
                    "Angle of the countersink cone. 90° fits most flat-head screws.",
                    start_expr("90 deg"),
                ),
                help_and_start(
                    "extent.kind",
                    "Depth stops at a set depth. Through all goes through the whole body.",
                    start_choice("through_all"),
                ),
                help(
                    "tip_angle",
                    "Gives the bottom a drill-point cone of this angle. Leave it empty for a flat bottom.",
                ),
                help("reverse", "Drills out of the face instead of into it."),
            ],
        ),
        spec(
            "import",
            "import.md test 18",
            "Brings the solids of a STEP file into a body.",
            Vec::new(),
        ),
        spec(
            "box",
            "box.md test 15",
            "Adds a box of the given length, width and height.",
            Vec::new(),
        ),
        spec(
            "cylinder",
            "cylinder.md test 13",
            "Adds a cylinder of the given radius and height.",
            Vec::new(),
        ),
        spec(
            "sphere",
            "sphere.md test 11",
            "Adds a ball of the given radius.",
            Vec::new(),
        ),
        spec(
            "cone",
            "cone.md test 17",
            "Adds a cone, or a tapered cylinder, from two radii and a height.",
            Vec::new(),
        ),
        spec(
            "torus",
            "torus.md test 14",
            "Adds a ring from the radius of its center line and the radius of its tube.",
            Vec::new(),
        ),
        spec(
            "fillet",
            "fillet.md test 23",
            "Rounds the selected edges.",
            vec![
                help(
                    "edges",
                    "The edges to round. Click a face to add all of its edges.",
                ),
                start("radius", start_expr("1 mm")),
            ],
        ),
        spec(
            "chamfer",
            "chamfer.md test 20",
            "Bevels the selected edges with a flat cut.",
            vec![
                help(
                    "edges",
                    "The edges to bevel. Click a face to add all of its edges.",
                ),
                help(
                    "distance.kind",
                    "Equal distance cuts the same amount from both faces. Two distances sets each side.",
                ),
                start("distance.expr", start_expr("1 mm")),
                start("distance.second_expr", start_expr("1 mm")),
            ],
        ),
        spec(
            "shell",
            "shell.md test 27",
            "Hollows out a body, leaving walls of the given thickness and the picked faces open.",
            vec![
                help(
                    "remove_faces",
                    "Faces to leave open. With none, the body becomes a closed hollow.",
                ),
                help_and_start(
                    "thickness",
                    "Wall thickness. For FDM printing, a multiple of the nozzle width prints best.",
                    start_expr("2 mm"),
                ),
            ],
        ),
        spec(
            "boolean",
            "boolean.md test 18",
            "Joins, cuts or intersects one body with other bodies.",
            vec![
                help("target", "The body that changes."),
                help(
                    "tools",
                    "The bodies that join, cut or intersect the target. They are used up unless Keep tools is on.",
                ),
                help(
                    "op",
                    "Join merges the bodies, Cut removes the tools from the target, and Intersect keeps only the overlap.",
                ),
                help(
                    "keep_tools",
                    "Keeps the tool bodies after the operation instead of using them up.",
                ),
            ],
        ),
        spec(
            "move_body",
            "move_body.md test 16",
            "Moves and rotates a whole body.",
            vec![help(
                "rotation.axis",
                "The axis to rotate around. Leave it empty to only move.",
            )],
        ),
        spec(
            "split_body",
            "split_body.md test 14",
            "Cuts a body in two along a plane, for example to fit a large part on the bed.",
            vec![
                SpecHint {
                    creates_body: Some(BodyCreation::AtStart),
                    ..help(
                        "target",
                        "The body that gets the other part. Leave it empty to discard that part.",
                    )
                },
                help(
                    "flip",
                    "Keeps the back part in Body instead of the front part.",
                ),
            ],
        ),
        spec(
            "mirror",
            "mirror.md test 17",
            "Mirrors a body across a plane, joined to it or as a new body.",
            vec![
                help(
                    "op",
                    "Join adds the mirrored copy to the body. New body puts it in a separate body.",
                ),
                new_body("target", BodyCreation::WhileNew),
            ],
        ),
        spec(
            "linear_pattern",
            "linear_pattern.md test 27",
            "Repeats a body in a row, or in a grid with a second direction.",
            vec![
                help(
                    "direction.count",
                    "How many instances along this direction, counting the original.",
                ),
                help(
                    "direction.spacing",
                    "Distance between neighboring instances.",
                ),
                new_body("target", BodyCreation::WhileNew),
            ],
        ),
        spec(
            "circular_pattern",
            "circular_pattern.md test 21",
            "Repeats a body around an axis.",
            vec![
                help(
                    "count",
                    "How many instances around the axis, counting the original.",
                ),
                help("angle", "The angle the instances spread over."),
                new_body("target", BodyCreation::WhileNew),
            ],
        ),
        spec(
            "datum_plane",
            "datum_plane.md test 20",
            "Adds a plane offset from another plane, to sketch on or mirror across.",
            vec![help_and_start(
                "offset",
                "Distance from the base plane along its normal.",
                start_expr("10 mm"),
            )],
        ),
    ]
}

/// The feature spec's manifest of built-in type `type_name`.
fn spec_manifest(type_name: &str) -> SpecManifest {
    spec_manifests()
        .into_iter()
        .find(|spec| spec.type_name == type_name)
        .unwrap_or_else(|| panic!("{type_name} has a feature spec manifest"))
}

/// The manifest of built-in type `type_name`, as `Registry::with_builtins()` returns it.
fn builtin_manifest(type_name: &str) -> FeatureManifest {
    Registry::with_builtins()
        .get(type_name)
        .unwrap_or_else(|| panic!("{type_name} is registered"))
        .manifest()
        .clone()
}

/// Asserts that every path of `spec`'s table names a hint of `manifest`, so no row goes unchecked.
fn assert_rows_name_hints(spec: &SpecManifest, manifest: &FeatureManifest) {
    for row in &spec.hints {
        assert!(
            manifest.fields.iter().any(|hint| hint.path == row.path),
            "{} lists the path {} that the {} manifest has no hint for",
            spec.spec_test,
            row.path,
            spec.type_name
        );
    }
}

/// Asserts features.md test 46 for one type: its manifest has the description of its feature spec.
fn assert_description(spec: &SpecManifest) {
    assert_eq!(
        builtin_manifest(spec.type_name).description,
        spec.description,
        "description of {} ({})",
        spec.type_name,
        spec.spec_test
    );
}

/// Asserts features.md test 48 for one type: each hint has the help its feature spec lists, and
/// none when the spec lists none.
fn assert_help(spec: &SpecManifest) {
    let manifest = builtin_manifest(spec.type_name);
    assert_rows_name_hints(spec, &manifest);
    for hint in &manifest.fields {
        assert_eq!(
            hint.help.as_deref(),
            spec.row(&hint.path).and_then(|row| row.help),
            "help of {} {} ({})",
            spec.type_name,
            hint.path,
            spec.spec_test
        );
    }
}

/// Asserts features.md test 50 for one type: each hint has the start value its feature spec lists,
/// and none when the spec lists none.
fn assert_start(spec: &SpecManifest) {
    let manifest = builtin_manifest(spec.type_name);
    assert_rows_name_hints(spec, &manifest);
    for hint in &manifest.fields {
        assert_eq!(
            hint.start.as_ref(),
            spec.row(&hint.path).and_then(|row| row.start.as_ref()),
            "start value of {} {} ({})",
            spec.type_name,
            hint.path,
            spec.spec_test
        );
    }
}

/// Asserts features.md test 51 for one type: each hint has the new body its feature spec lists, and
/// none when the spec lists none.
fn assert_creates_body(spec: &SpecManifest) {
    let manifest = builtin_manifest(spec.type_name);
    assert_rows_name_hints(spec, &manifest);
    for hint in &manifest.fields {
        assert_eq!(
            hint.creates_body,
            spec.row(&hint.path).and_then(|row| row.creates_body),
            "new body of {} {} ({})",
            spec.type_name,
            hint.path,
            spec.spec_test
        );
    }
}

/// Asserts the manifest test of one feature spec: the type's cases of features.md tests 46, 48, 50
/// and 51.
fn assert_matches_its_feature_spec(type_name: &str) {
    let spec = spec_manifest(type_name);
    assert_description(&spec);
    assert_help(&spec);
    assert_start(&spec);
    assert_creates_body(&spec);
}

/// Whether `text` holds one of `FORBIDDEN_WORDS` as a whole word with its case, as the regular
/// expression `\b(omitted|default|params|None|serde)\b` matches.
fn has_forbidden_word(text: &str) -> bool {
    text.split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .any(|word| FORBIDDEN_WORDS.contains(&word))
}

/// The `essential` flag of the hint at `path` of built-in type `type_name`.
fn essential(type_name: &str, path: &str) -> bool {
    builtin_manifest(type_name)
        .fields
        .iter()
        .find(|hint| hint.path == path)
        .unwrap_or_else(|| panic!("{type_name} has a hint {path}"))
        .essential
}

#[test]
fn t046_builtin_descriptions_are_their_feature_specs() {
    let specs = spec_manifests();
    let mut spec_types: Vec<&str> = specs.iter().map(|spec| spec.type_name).collect();
    spec_types.sort_unstable();

    assert_eq!(
        spec_types,
        Registry::with_builtins().types().collect::<Vec<_>>()
    );
    for spec in &specs {
        assert_description(spec);
    }
}

#[test]
fn t047_builtin_descriptions_follow_the_style() {
    let registry = Registry::with_builtins();

    for type_name in registry.types() {
        let description = &registry
            .get(type_name)
            .expect("each listed type is registered")
            .manifest()
            .description;
        let length = description.chars().count();
        assert!(
            (1..=120).contains(&length),
            "{type_name} has a description of {length} characters: {description}"
        );
        assert!(
            description.ends_with('.'),
            "{type_name} has a description without a final period: {description}"
        );
        assert!(
            !has_forbidden_word(description),
            "{type_name} has a description with a forbidden word: {description}"
        );
    }
}

#[test]
fn t047_forbidden_words_match_whole_words_with_their_case() {
    assert!(has_forbidden_word("Uses the default size."));
    assert!(has_forbidden_word("None of them."));
    assert!(!has_forbidden_word("Defaults and none of the paramsets."));
    assert!(!has_forbidden_word("The defaulted size."));
}

#[test]
fn t048_builtin_help_is_their_feature_specs() {
    for spec in &spec_manifests() {
        assert_help(spec);
    }
}

#[test]
fn t049_builtin_help_follows_the_style() {
    let registry = Registry::with_builtins();

    for type_name in registry.types() {
        let manifest = registry
            .get(type_name)
            .expect("each listed type is registered")
            .manifest();
        for hint in &manifest.fields {
            let Some(text) = &hint.help else {
                continue;
            };
            assert!(
                text.ends_with('.'),
                "the help of {type_name} {} has no final period: {text}",
                hint.path
            );
            assert!(
                text.matches(". ").count() <= 1,
                "the help of {type_name} {} has more than two sentences: {text}",
                hint.path
            );
            assert!(
                !has_forbidden_word(text),
                "the help of {type_name} {} has a forbidden word: {text}",
                hint.path
            );
        }
    }
}

#[test]
fn t050_builtin_start_values_are_their_feature_specs() {
    for spec in &spec_manifests() {
        assert_start(spec);
    }
}

#[test]
fn t050_fillet_revolve_and_hole_start_values() {
    let start_of = |type_name: &str, path: &str| {
        builtin_manifest(type_name)
            .fields
            .into_iter()
            .find(|hint| hint.path == path)
            .unwrap_or_else(|| panic!("{type_name} has a hint {path}"))
            .start
    };

    assert_eq!(
        start_of("fillet", "radius"),
        Some(StartValue::Expr {
            expr: Expr::new("1 mm")
        })
    );
    assert_eq!(
        start_of("revolve", "extent.kind"),
        Some(StartValue::Choice {
            value: "full".to_string()
        })
    );
    assert_eq!(
        start_of("hole", "hole_type.diameter"),
        Some(StartValue::Multiple {
            field: "diameter".to_string(),
            factor: 2.0
        })
    );
}

#[test]
fn t051_builtin_new_body_fields() {
    let registry = Registry::with_builtins();

    for type_name in registry.types() {
        let manifest = registry
            .get(type_name)
            .expect("each listed type is registered")
            .manifest();
        for hint in &manifest.fields {
            let expected = match (type_name, hint.path.as_str()) {
                ("mirror" | "linear_pattern" | "circular_pattern", "target") => {
                    Some(BodyCreation::WhileNew)
                }
                ("split_body", "target") => Some(BodyCreation::AtStart),
                _ => None,
            };
            assert_eq!(
                hint.creates_body, expected,
                "new body of {type_name} {}",
                hint.path
            );
        }
    }
}

#[test]
fn t051_builtin_new_body_fields_are_their_feature_specs() {
    for spec in &spec_manifests() {
        assert_creates_body(spec);
    }
}

#[test]
fn t052_reverse_is_essential_in_extrude_revolve_and_hole() {
    for type_name in ["extrude", "revolve", "hole"] {
        assert!(
            essential(type_name, "reverse"),
            "{type_name} reverse is essential"
        );
    }
}

#[test]
fn t052_the_reverse_flags_of_the_patterns_stay_not_essential() {
    for (type_name, path) in [
        ("circular_pattern", "reverse"),
        ("linear_pattern", "direction.reverse"),
        ("linear_pattern", "second_direction.reverse"),
    ] {
        assert!(
            !essential(type_name, path),
            "{type_name} {path} is not essential"
        );
    }
}

#[test]
fn sketch_t052_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("sketch");
}

#[test]
fn extrude_t064_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("extrude");
    assert!(essential("extrude", "reverse"));
}

#[test]
fn revolve_t037_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("revolve");
    assert!(essential("revolve", "reverse"));
}

#[test]
fn hole_t041_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("hole");
    assert!(essential("hole", "reverse"));
}

#[test]
fn import_t018_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("import");
}

#[test]
fn box_t015_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("box");
}

#[test]
fn cylinder_t013_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("cylinder");
}

#[test]
fn sphere_t011_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("sphere");
}

#[test]
fn cone_t017_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("cone");
}

#[test]
fn torus_t014_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("torus");
}

#[test]
fn fillet_t023_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("fillet");
}

#[test]
fn chamfer_t020_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("chamfer");
}

#[test]
fn shell_t027_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("shell");
}

#[test]
fn boolean_t018_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("boolean");
}

#[test]
fn move_body_t016_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("move_body");
}

#[test]
fn split_body_t014_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("split_body");
}

#[test]
fn mirror_t017_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("mirror");
}

#[test]
fn linear_pattern_t027_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("linear_pattern");
}

#[test]
fn circular_pattern_t021_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("circular_pattern");
}

#[test]
fn datum_plane_t020_manifest_matches_its_feature_spec() {
    assert_matches_its_feature_spec("datum_plane");
}

/// The two values of the `mode` field of the test features of tests 53 to 62.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Mode {
    /// The first value.
    Fast,
    /// The second value.
    Fine,
}

/// Params of the `start_demo` test feature of features.md tests 53 to 61: all required but
/// `target` and `other`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct StartDemoParams {
    /// A length.
    distance: Expr,
    /// A second length.
    depth: Expr,
    /// An angle.
    angle: Expr,
    /// A whole number.
    count: Expr,
    /// A text.
    label: String,
    /// One of two values.
    mode: Mode,
    /// A body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<Id>,
    /// A second body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    other: Option<Id>,
    /// A list of bodies.
    tools: Vec<Id>,
    /// What the feature does with its body.
    op: BodyOp,
}

/// Params of the `copy_demo` test feature of features.md test 62, which has no `op`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
struct CopyDemoParams {
    /// The body to copy.
    body: Id,
    /// The body that gets the copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<Id>,
    /// One of two values.
    mode: Mode,
}

/// A test feature with any manifest over the params of `P`, registered with
/// `Registry::register_dyn`. Tests 53 to 62 change its hints and only register it.
struct HintsDemo<P> {
    /// The type name.
    type_name: &'static str,
    /// The manifest the test gives it.
    manifest: FeatureManifest,
    /// The params type.
    params: PhantomData<fn() -> P>,
}

impl<P> HintsDemo<P> {
    /// The test feature `type_name` with these hints.
    fn new(type_name: &'static str, hints: Vec<FieldHint>) -> Self {
        HintsDemo {
            type_name,
            manifest: manifest("Hints demo", hints),
            params: PhantomData,
        }
    }
}

impl<P: JsonSchema> DynFeature for HintsDemo<P> {
    fn type_name(&self) -> &str {
        self.type_name
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn params_schema(&self) -> Schema {
        schemars::schema_for!(P)
    }

    fn canonicalize(&self, params: &Map<String, Value>) -> Result<Canonical, FeatureError> {
        Ok(Canonical {
            params: params.clone(),
            unknown_keys: Vec::new(),
        })
    }

    fn dependencies(&self, _params: &Map<String, Value>) -> Result<Dependencies, FeatureError> {
        Ok(no_dependencies())
    }

    fn build(
        &self,
        _params: &Map<String, Value>,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        Ok(nothing())
    }

    fn python(&self, _params: &Map<String, Value>, _names: &dyn PythonNames) -> Option<PythonCall> {
        None
    }
}

/// The options of `mode`.
const MODE_OPTIONS: [(&str, &str); 2] = [("fast", "Fast"), ("fine", "Fine")];

/// The hints of `start_demo` as tests 53 to 61 write it: one per field, in field order.
fn start_demo_hints() -> Vec<FieldHint> {
    vec![
        expression_hint("distance", "Distance", FieldUnit::Length),
        expression_hint("depth", "Depth", FieldUnit::Length),
        expression_hint("angle", "Angle", FieldUnit::Angle),
        expression_hint("count", "Count", FieldUnit::Count),
        hint("label", "Label"),
        options_hint("mode", "Mode", &MODE_OPTIONS),
        selection_hint("target", "Target body", SelectionKind::Body),
        selection_hint("other", "Other body", SelectionKind::Body),
        selection_hint("tools", "Tool bodies", SelectionKind::Body),
        options_hint(
            "op",
            "Operation",
            &[
                ("new", "New body"),
                ("join", "Join"),
                ("cut", "Cut"),
                ("intersect", "Intersect"),
            ],
        ),
    ]
}

/// The hint at `path`.
fn hint_at<'a>(hints: &'a mut [FieldHint], path: &str) -> &'a mut FieldHint {
    hints
        .iter_mut()
        .find(|hint| hint.path == path)
        .unwrap_or_else(|| panic!("the test feature has a hint {path}"))
}

/// Registers `start_demo` on `Registry::new()` with the hints of tests 53 to 61 changed by
/// `change`.
fn register_start_demo(change: impl FnOnce(&mut Vec<FieldHint>)) -> Result<(), RegistryError> {
    let mut hints = start_demo_hints();
    change(&mut hints);
    Registry::new().register_dyn(Box::new(HintsDemo::<StartDemoParams>::new(
        "start_demo",
        hints,
    )))
}

/// Registers `start_demo` with the start value `start` on the hint at `path`.
fn register_start_on(path: &str, start: StartValue) -> Result<(), RegistryError> {
    register_start_demo(|hints| hint_at(hints, path).start = Some(start))
}

/// `ManifestMismatch` of `start_demo` at `path` with `problem`.
fn start_demo_mismatch(path: &str, problem: &str) -> Result<(), RegistryError> {
    Err(RegistryError::ManifestMismatch {
        type_name: "start_demo".to_string(),
        path: path.to_string(),
        problem: problem.to_string(),
    })
}

#[test]
fn t053_the_start_demo_registers_as_written() {
    assert_eq!(register_start_demo(|_| {}), Ok(()));
}

#[test]
fn t053_a_start_expression_on_a_hint_without_a_unit_is_a_mismatch() {
    assert_eq!(
        register_start_on("label", start_expr("2 mm")),
        start_demo_mismatch(
            "label",
            "its start value is an expression, but it has no unit"
        )
    );
}

#[test]
fn t053_the_mismatch_says_which_field_and_why() {
    let error = register_start_on("label", start_expr("2 mm"))
        .expect_err("a start expression without a unit is refused");

    assert_eq!(
        error.to_string(),
        "The manifest of 'start_demo' has a problem with field 'label': its start value is an expression, but it has no unit. Fix the manifest or the params struct."
    );
}

#[test]
fn t053_a_start_multiple_on_a_hint_without_a_unit_is_a_mismatch() {
    assert_eq!(
        register_start_on("label", start_multiple("distance", 2.0)),
        start_demo_mismatch(
            "label",
            "its start value follows another field, but it has no unit"
        )
    );
}

#[test]
fn t054_a_start_length_that_is_not_a_fixed_length_is_a_mismatch() {
    for expr in ["2 mmm", "wall", "printer.nozzle", "3 deg"] {
        assert_eq!(
            register_start_on("distance", start_expr(expr)),
            start_demo_mismatch(
                "distance",
                &format!("its start value '{expr}' is not a fixed length")
            ),
            "{expr}"
        );
    }
}

#[test]
fn t054_a_start_count_that_is_not_a_whole_number_is_a_mismatch() {
    for expr in ["2.5", "-1"] {
        assert_eq!(
            register_start_on("count", start_expr(expr)),
            start_demo_mismatch(
                "count",
                &format!("its start value '{expr}' is not a fixed whole number")
            ),
            "{expr}"
        );
    }
}

#[test]
fn t055_a_bare_start_length_depends_on_the_document_units() {
    assert_eq!(
        register_start_on("distance", start_expr("2")),
        start_demo_mismatch(
            "distance",
            "its start value '2' depends on the document units"
        )
    );
}

#[test]
fn t055_fixed_counts_angles_and_lengths_register() {
    let result = register_start_demo(|hints| {
        hint_at(hints, "count").start = Some(start_expr("2"));
        hint_at(hints, "angle").start = Some(start_expr("90"));
        hint_at(hints, "depth").start = Some(start_expr("2.5 mm"));
    });

    assert_eq!(result, Ok(()));
}

#[test]
fn t056_a_start_choice_that_is_not_an_option_is_a_mismatch() {
    assert_eq!(
        register_start_on("mode", start_choice("slow")),
        start_demo_mismatch("mode", "its start value 'slow' is not one of its options")
    );
}

#[test]
fn t056_a_start_choice_on_a_hint_without_options_is_a_mismatch() {
    assert_eq!(
        register_start_on("distance", start_choice("fine")),
        start_demo_mismatch(
            "distance",
            "its start value 'fine' is not one of its options"
        )
    );
}

#[test]
fn t056_a_start_choice_of_an_option_registers() {
    assert_eq!(register_start_on("mode", start_choice("fine")), Ok(()));
}

#[test]
fn t057_a_multiple_of_a_field_that_does_not_fit_is_a_mismatch() {
    for field in ["angle", "depth", "width"] {
        assert_eq!(
            register_start_on("depth", start_multiple(field, 2.0)),
            start_demo_mismatch(
                "depth",
                &format!(
                    "its start value follows '{field}', which is not another field with the same unit"
                )
            ),
            "{field}"
        );
    }
}

#[test]
fn t057_a_multiple_of_another_length_registers() {
    assert_eq!(
        register_start_on("depth", start_multiple("distance", 2.0)),
        Ok(())
    );
}

#[test]
fn t058_a_multiple_of_a_multiple_is_a_mismatch_at_the_first_hint() {
    let result = register_start_demo(|hints| {
        hint_at(hints, "distance").start = Some(start_multiple("depth", 2.0));
        hint_at(hints, "depth").start = Some(start_multiple("distance", 2.0));
    });

    assert_eq!(
        result,
        start_demo_mismatch(
            "distance",
            "its start value follows 'depth', whose own start value follows another field"
        )
    );
}

#[test]
fn t059_a_factor_that_is_not_above_zero_is_a_mismatch() {
    for (factor, written) in [(0.0, "0"), (f64::NAN, "NaN"), (-1.0, "-1")] {
        assert_eq!(
            register_start_on("depth", start_multiple("distance", factor)),
            start_demo_mismatch(
                "depth",
                &format!("its start value's factor {written} is not a finite number above 0")
            ),
            "{written}"
        );
    }
}

#[test]
fn t059_an_infinite_factor_is_a_mismatch() {
    assert_eq!(
        register_start_on("depth", start_multiple("distance", f64::INFINITY)),
        start_demo_mismatch(
            "depth",
            "its start value's factor inf is not a finite number above 0"
        )
    );
}

#[test]
fn t060_a_new_body_in_a_field_that_is_not_a_body_is_a_mismatch() {
    let result = register_start_demo(|hints| {
        hint_at(hints, "label").creates_body = Some(BodyCreation::AtStart);
    });

    assert_eq!(
        result,
        start_demo_mismatch(
            "label",
            "it receives a new body, but its selection is not a body"
        )
    );
}

#[test]
fn t060_a_new_body_in_a_list_of_bodies_is_a_mismatch() {
    let result = register_start_demo(|hints| {
        hint_at(hints, "tools").creates_body = Some(BodyCreation::AtStart);
    });

    assert_eq!(
        result,
        start_demo_mismatch(
            "tools",
            "it receives a new body, but the field holds a list"
        )
    );
}

#[test]
fn t060_a_new_body_in_an_item_of_a_list_is_a_mismatch() {
    let result = register_start_demo(|hints| {
        *hint_at(hints, "tools") = FieldHint {
            creates_body: Some(BodyCreation::AtStart),
            ..selection_hint("tools[*]", "Tool body", SelectionKind::Body)
        };
    });

    assert_eq!(
        result,
        start_demo_mismatch(
            "tools[*]",
            "it receives a new body, but the field holds a list"
        )
    );
}

#[test]
fn t061_a_second_field_that_receives_the_new_body_is_a_mismatch() {
    let result = register_start_demo(|hints| {
        hint_at(hints, "target").creates_body = Some(BodyCreation::AtStart);
        hint_at(hints, "other").creates_body = Some(BodyCreation::AtStart);
    });

    assert_eq!(
        result,
        start_demo_mismatch("other", "another field already receives the new body")
    );
}

#[test]
fn t061_one_field_that_receives_the_new_body_while_op_is_new_registers() {
    let result = register_start_demo(|hints| {
        hint_at(hints, "target").creates_body = Some(BodyCreation::WhileNew);
    });

    assert_eq!(result, Ok(()));
}

/// Registers `copy_demo` on `Registry::new()` with `creation` on its `target` hint.
fn register_copy_demo(creation: BodyCreation) -> Result<(), RegistryError> {
    let hints = vec![
        selection_hint("body", "Body", SelectionKind::Body),
        FieldHint {
            creates_body: Some(creation),
            ..selection_hint("target", "Target body", SelectionKind::Body)
        },
        options_hint("mode", "Mode", &MODE_OPTIONS),
    ];
    Registry::new().register_dyn(Box::new(HintsDemo::<CopyDemoParams>::new(
        "copy_demo",
        hints,
    )))
}

#[test]
fn t062_a_new_body_while_op_is_new_without_an_op_is_a_mismatch() {
    assert_eq!(
        register_copy_demo(BodyCreation::WhileNew),
        Err(RegistryError::ManifestMismatch {
            type_name: "copy_demo".to_string(),
            path: "target".to_string(),
            problem: "it receives a new body while op is new, but no op field has the option new"
                .to_string(),
        })
    );
}

#[test]
fn t062_a_new_body_at_start_without_an_op_registers() {
    assert_eq!(register_copy_demo(BodyCreation::AtStart), Ok(()));
}

#[test]
fn t063_the_builtins_pass_the_new_checks() {
    let mut registry = Registry::new();

    assert_eq!(register_builtins(&mut registry), Ok(()));
    assert_eq!(
        Registry::with_builtins().types().collect::<Vec<_>>(),
        [
            "boolean",
            "box",
            "chamfer",
            "circular_pattern",
            "cone",
            "cylinder",
            "datum_plane",
            "extrude",
            "fillet",
            "hole",
            "import",
            "linear_pattern",
            "mirror",
            "move_body",
            "revolve",
            "shell",
            "sketch",
            "sphere",
            "split_body",
            "torus",
        ]
    );
}

#[test]
fn a_start_problem_comes_before_a_new_body_problem_of_the_same_hint() {
    let result = register_start_demo(|hints| {
        let label = hint_at(hints, "label");
        label.start = Some(start_expr("2 mm"));
        label.creates_body = Some(BodyCreation::AtStart);
    });

    assert_eq!(
        result,
        start_demo_mismatch(
            "label",
            "its start value is an expression, but it has no unit"
        )
    );
}

#[test]
fn a_refused_manifest_registers_nothing() {
    let mut registry = Registry::new();
    let mut hints = start_demo_hints();
    hint_at(&mut hints, "distance").start = Some(start_expr("2"));

    let result = registry.register_dyn(Box::new(HintsDemo::<StartDemoParams>::new(
        "start_demo",
        hints,
    )));

    assert!(result.is_err(), "got {result:?}");
    assert_eq!(registry.types().count(), 0);
}
