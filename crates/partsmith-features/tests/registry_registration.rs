//! Acceptance tests for feature registration, built-in manifests and params schemas, with the
//! defaults the feature catalog carries (features.md section 6.2, tests 1 to 7 and 15).
//!
//! Tests of the 20 built-in feature types that check framework behavior each have a companion with
//! the same number that uses test features instead.

use crate::common;

use common::{Probe, Slab, hint, manifest, no_dependencies, nothing, record};
use partsmith_features::{
    BuildContext, Category, Dependencies, Feature, FeatureError, FeatureManifest, FeatureOutput,
    Registry, RegistryError,
};
use partsmith_model::{Expr, Id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

    let schema = partsmith_model::ptsm_schema(&registry.params_schemas())
        .expect("the built-in params schemas compose");

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

    let schema = partsmith_model::ptsm_schema(&registry.params_schemas())
        .expect("the test params schemas compose");

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

    let schema = partsmith_model::ptsm_schema(&registry.params_schemas())
        .expect("the built-in params schemas compose");

    for (kind, property, _) in zero_triples() {
        let pointer = format!("/$defs/feature.{kind}/properties/{property}/default");
        assert_eq!(
            schema.as_value().pointer(&pointer),
            Some(&json!(["0", "0", "0"])),
            "{pointer}"
        );
    }
}
