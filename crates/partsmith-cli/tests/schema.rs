//! Acceptance tests for `partsmith schema` (cli spec sections 3.5 and 6, tests 50 to 54).

use crate::common;

use std::fs;

use common::{Workspace, repository_root, run_in};
use pretty_assertions::assert_eq;

#[test]
fn t050_schema_protocol_prints_the_protocol_schema() {
    let workspace = Workspace::new();

    let run = workspace.run(&["schema", "protocol"]);

    run.assert_code(0);
    assert_eq!(run.stdout, partsmith_protocol::protocol_schema());
}

#[test]
fn t051_schema_write_creates_and_keeps_files() {
    let workspace = Workspace::new();

    let first = workspace.run(&["schema", "--write", "--out-dir", "s"]);
    first.assert_code(0);
    assert_eq!(
        first.stdout,
        "wrote s/ptsm.schema.json\nwrote s/protocol.schema.json\n"
    );
    assert!(workspace.exists("s/ptsm.schema.json"));
    assert!(workspace.exists("s/protocol.schema.json"));
    let ptsm_modified = workspace.pin_modified_time("s/ptsm.schema.json");
    let protocol_modified = workspace.pin_modified_time("s/protocol.schema.json");

    let second = workspace.run(&["schema", "--write", "--out-dir", "s"]);

    second.assert_code(0);
    assert_eq!(
        second.stdout,
        "unchanged s/ptsm.schema.json\nunchanged s/protocol.schema.json\n"
    );
    assert_eq!(workspace.modified_time("s/ptsm.schema.json"), ptsm_modified);
    assert_eq!(
        workspace.modified_time("s/protocol.schema.json"),
        protocol_modified
    );
}

#[test]
fn t052_schema_check_detects_drift() {
    let workspace = Workspace::new();
    workspace
        .run(&["schema", "--write", "--out-dir", "s"])
        .assert_code(0);

    let current = workspace.run(&["schema", "--check", "--out-dir", "s"]);
    let mut drifted = workspace.read("s/ptsm.schema.json");
    drifted.push(b' ');
    workspace.write("s/ptsm.schema.json", drifted);
    let stale = workspace.run(&["schema", "--check", "--out-dir", "s"]);

    current.assert_code(0);
    stale.assert_code(1);
    assert_eq!(
        stale.stdout,
        "s/ptsm.schema.json is out of date. Run: partsmith schema --write\n"
    );
}

#[test]
fn t053_committed_schema_files_are_up_to_date() {
    let root = fs::canonicalize(repository_root()).expect("the repository root exists");

    let run = run_in(&root, &["schema", "--check"], None);

    assert_eq!(
        run.code, 0,
        "docs/format is out of date; run `cargo run -p partsmith-cli -- schema --write` in the repository root\n{}",
        run.stdout
    );
}

#[test]
fn t054_schema_needs_exactly_one_action() {
    let workspace = Workspace::new();

    let bare = workspace.run(&["schema"]);
    let name_and_write = workspace.run(&["schema", "ptsm", "--write"]);

    bare.assert_code(2);
    name_and_write.assert_code(2);
}

// The checks of the committed `.ptsm` schema (file-format.md section 6, tests 58, 69, 71, 72 and 73), on
// `docs/format/ptsm.schema.json` as `partsmith schema --write` generates it with the built-in features.

/// The committed `.ptsm` schema.
fn committed_ptsm_schema() -> serde_json::Value {
    let path = repository_root().join("docs/format/ptsm.schema.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} cannot be read: {error}", path.display()));
    serde_json::from_str(&text).expect("the schema is JSON")
}

/// The value at a JSON pointer of the schema, which must exist.
fn at<'a>(schema: &'a serde_json::Value, pointer: &str) -> &'a serde_json::Value {
    schema
        .pointer(pointer)
        .unwrap_or_else(|| panic!("the schema has nothing at {pointer}"))
}

/// The definition a schema refers to with `$ref`, or the schema itself.
fn resolve<'a>(
    schema: &'a serde_json::Value,
    value: &'a serde_json::Value,
) -> &'a serde_json::Value {
    match value.get("$ref").and_then(serde_json::Value::as_str) {
        Some(reference) => at(
            schema,
            reference.strip_prefix('#').expect("references are local"),
        ),
        None => value,
    }
}

/// The values a string enumeration allows: its `enum`, or the `const` of each branch of its `oneOf`, as schemars
/// writes an enum with documented variants.
fn enum_values(value: &serde_json::Value) -> Vec<&str> {
    let values = match value.get("enum") {
        Some(values) => values
            .as_array()
            .expect("an enum is an array")
            .iter()
            .collect(),
        None => at(value, "/oneOf")
            .as_array()
            .expect("oneOf is an array")
            .iter()
            .map(|branch| at(branch, "/const"))
            .collect::<Vec<_>>(),
    };
    values
        .into_iter()
        .map(|value| value.as_str().expect("the values are strings"))
        .collect()
}

/// The branch of the `oneOf` at `pointer` whose `kind` property is the constant `kind`.
fn branch<'a>(schema: &'a serde_json::Value, pointer: &str, kind: &str) -> &'a serde_json::Value {
    at(schema, pointer)
        .as_array()
        .unwrap_or_else(|| panic!("{pointer} is an array"))
        .iter()
        .find(|branch| branch.pointer("/properties/kind/const") == Some(&serde_json::json!(kind)))
        .unwrap_or_else(|| panic!("{pointer} has no {kind} branch"))
}

/// Calls `visit` with the key of every member of every `properties` object, at any depth.
fn visit_property_keys(value: &serde_json::Value, visit: &mut dyn FnMut(&str)) {
    match value {
        serde_json::Value::Object(members) => {
            for (key, member) in members {
                if key == "properties"
                    && let Some(properties) = member.as_object()
                {
                    properties.keys().for_each(|name| visit(name));
                }
                visit_property_keys(member, visit);
            }
        }
        serde_json::Value::Array(items) => {
            items
                .iter()
                .for_each(|item| visit_property_keys(item, visit));
        }
        _ => {}
    }
}

/// The JSON pointers of every schema whose `type` is `integer` or a list that holds it (such as
/// `["integer", "null"]`), at any depth and whatever holds it: a property, the `items` of an array, a
/// branch or a definition.
fn integer_schemas(value: &serde_json::Value, pointer: &str, found: &mut Vec<String>) {
    let integer = serde_json::json!("integer");
    match value {
        serde_json::Value::Object(members) => {
            let is_integer = match members.get("type") {
                Some(serde_json::Value::Array(types)) => types.contains(&integer),
                Some(single) => single == &integer,
                None => false,
            };
            if is_integer {
                found.push(pointer.to_owned());
            }
            for (key, member) in members {
                let token = key.replace('~', "~0").replace('/', "~1");
                integer_schemas(member, &format!("{pointer}/{token}"), found);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                integer_schemas(item, &format!("{pointer}/{index}"), found);
            }
        }
        _ => {}
    }
}

#[test]
fn integer_schemas_finds_items_type_lists_and_definitions() {
    let schema = serde_json::json!({
        "properties": {
            "edges": {"type": "array", "items": {"type": "integer"}},
            "count": {"type": ["integer", "null"]},
            "width": {"type": "number"},
            "type": {"type": "string"}
        },
        "$defs": {"Index": {"type": "integer"}, "a/b": {"oneOf": [{"type": "integer"}]}}
    });
    let mut found = Vec::new();

    integer_schemas(&schema, "", &mut found);

    found.sort();
    assert_eq!(
        found,
        [
            "/$defs/Index",
            "/$defs/a~1b/oneOf/0",
            "/properties/count",
            "/properties/edges/items"
        ]
    );
}

#[test]
fn t058_the_schema_describes_the_version_2_fields() {
    let schema = committed_ptsm_schema();

    assert_eq!(
        at(&schema, "/properties/format_version/const"),
        &serde_json::json!(3)
    );
    let reference = at(&schema, "/$defs/Reference/properties");
    assert!(reference.get("nb").is_some() && reference.get("merged").is_some());
    assert_eq!(
        at(&schema, "/$defs/Reference/required"),
        &serde_json::json!(["q", "fp"])
    );
    assert!(
        at(&schema, "/$defs/RegionRef/properties")
            .get("outline")
            .is_some()
    );
    assert_eq!(
        at(&schema, "/$defs/RegionRef/required"),
        &serde_json::json!(["key", "fp"])
    );
    let face_plane = branch(&schema, "/$defs/SketchPlane/oneOf", "face");
    let fallback_frame = resolve(&schema, at(face_plane, "/properties/fallback_frame"));
    assert_eq!(enum_values(fallback_frame), ["world", "body"]);
    let not_required = |branch: &serde_json::Value, name: &str| {
        branch
            .get("required")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|required| !required.contains(&serde_json::json!(name)))
    };
    assert!(not_required(face_plane, "fallback_frame"));
    for kind in ["edge", "face"] {
        let axis = branch(&schema, "/$defs/AxisRef/oneOf", kind);
        let direction = axis
            .pointer("/properties/direction")
            .unwrap_or_else(|| panic!("the {kind} axis has a direction"));
        let types = direction.get("type").expect("the direction has a type");
        assert!(
            types == &serde_json::json!("array")
                || types
                    .as_array()
                    .is_some_and(|types| types.contains(&serde_json::json!("array"))),
            "{kind}: {types}"
        );
        assert_eq!(
            direction.get("minItems"),
            Some(&serde_json::json!(3)),
            "{kind}"
        );
        assert_eq!(
            direction.get("maxItems"),
            Some(&serde_json::json!(3)),
            "{kind}"
        );
        assert_eq!(
            direction.pointer("/items/type"),
            Some(&serde_json::json!("number")),
            "{kind}"
        );
        assert!(not_required(axis, "direction"), "{kind}");
    }
}

#[test]
fn t069_the_generated_schema_describes_the_stored_profile() {
    let schema = committed_ptsm_schema();

    assert_eq!(
        at(&schema, "/properties/printer_profile"),
        &serde_json::json!({"$ref": "#/$defs/PrinterProfile"})
    );
    assert_eq!(
        at(&schema, "/required"),
        &serde_json::json!(["format", "format_version", "units"])
    );
    assert_eq!(
        at(&schema, "/$defs/PrinterProfile/required"),
        &serde_json::json!([
            "id",
            "name",
            "technology",
            "bed",
            "layer_height",
            "max_overhang_angle",
            "max_bridge_length",
            "fit"
        ])
    );
    assert_eq!(
        at(&schema, "/$defs/PrinterProfile/additionalProperties"),
        &serde_json::json!(false)
    );
    assert_eq!(
        at(&schema, "/$defs/PrinterProfile/properties/id/$ref"),
        &serde_json::json!("#/$defs/Id")
    );
    assert_eq!(
        at(&schema, "/$defs/FitClasses/required"),
        &serde_json::json!(["press", "snug", "slip", "loose"])
    );
    let definitions = at(&schema, "/$defs");
    assert!(definitions.get("Bed").is_some() && definitions.get("Technology").is_some());
}

#[test]
fn t071_the_generated_schema_leaves_occurrence_free() {
    let schema = committed_ptsm_schema();
    let mut found = 0;

    visit_property_keys(&schema, &mut |key| {
        found += usize::from(key == "occurrence")
    });

    assert_eq!(found, 0, "a properties object has the key occurrence");
}

#[test]
fn t072_the_generated_schema_leaves_the_component_keys_free() {
    let schema = committed_ptsm_schema();
    let root = at(&schema, "/properties");

    for key in ["components", "occurrences", "joints"] {
        assert!(root.get(key).is_none(), "the root has {key}");
    }
}

#[test]
fn t073_no_params_field_indexes_elements_glyphs_or_characters() {
    let schema = committed_ptsm_schema();
    let forbidden = |key: &str| {
        matches!(
            key,
            "index" | "indices" | "glyph" | "glyphs" | "character" | "characters"
        ) || key.ends_with("_index")
            || key.ends_with("_indices")
    };
    let mut keys = Vec::new();
    let mut integers = Vec::new();

    visit_property_keys(&schema, &mut |key| {
        if forbidden(key) {
            keys.push(key.to_owned());
        }
    });
    integer_schemas(&schema, "", &mut integers);

    assert_eq!(keys, Vec::<String>::new());
    // None at a3f4573. From M2 item 14 on, the `degree` of the `bspline` and `fixed_spline` entity records
    // (sketch.md section 2.12) join this list, and the test lists their pointers.
    assert_eq!(integers, Vec::<String>::new());
}
