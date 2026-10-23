//! Acceptance tests for the generated protocol JSON Schema (protocol.md section 6, test 41 of "JSON shapes", "Schema", tests 43 to 46, and test 81).

use crate::common;

use std::collections::BTreeSet;
use std::path::Path;

use common::examples::{
    BLOCK_USES_RESULT, COMMAND_EXAMPLES, DELETED_POINT_MESSAGE, ELEMENT_REPAIR, ERROR_OUTCOMES,
    FULL_REFERENCE, LOST_FACE_REPAIR, MERGED_REFERENCE, MESSAGE_EXAMPLES, R16_DIAGNOSTIC,
    R26_CAUSE, REGION_REPAIR, RESULT_EXAMPLES, SOLVE_ON_FACE_IN_BODY_FRAME, STALE_FIX_MESSAGE,
    TYPE_EXAMPLES,
};
use common::json_schema::validate;
use common::{
    definition, parse_json, property, schema_json, string_set, string_values, variant, variant_tags,
};
use partsmith_protocol::{MAX_SAFE_INTEGER, protocol_schema};
use serde_json::{Value, json};

/// The names a schema object lists as `required`.
fn required_names(subschema: &Value) -> Vec<&str> {
    subschema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// The schema's `maximum` for property `name` of `subschema`.
fn maximum_of(schema: &Value, subschema: &Value, name: &str) -> Option<u64> {
    property(schema, subschema, name)
        .unwrap_or_else(|| panic!("the schema declares no property {name}"))
        .get("maximum")
        .and_then(Value::as_u64)
}

/// The `oneOf` entry of `Message` whose `kind` is `kind`.
fn message_variant<'a>(schema: &'a Value, kind: &str) -> &'a Value {
    variant(schema, "Message", "kind", kind)
}

#[test]
fn t041_spec_examples_validate_against_the_schema() {
    let schema = schema_json();
    let mut failures = Vec::new();

    // A message example validates as the only message of a frame header, against the schema root.
    for text in MESSAGE_EXAMPLES {
        let header = json!({"protocol_version": 1, "messages": [parse_json(text)], "buffers": []});
        if let Err(reason) = validate(&schema, "", &header) {
            failures.push(format!("{text}\n  {reason}"));
        }
    }
    // An example of a single type validates against its $defs entry.
    let single_types = COMMAND_EXAMPLES
        .into_iter()
        .map(|text| ("Command", text))
        .chain(
            RESULT_EXAMPLES
                .into_iter()
                .map(|text| ("CommandResult", text)),
        )
        .chain(TYPE_EXAMPLES);
    for (name, text) in single_types {
        if let Err(reason) = validate(&schema, &format!("/$defs/{name}"), &parse_json(text)) {
            failures.push(format!("{name}: {text}\n  {reason}"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} examples do not validate:\n{}",
        failures.len(),
        failures.join("\n")
    );

    // The validation is not vacuous: an unknown command does not validate.
    let unknown = json!({
        "protocol_version": 1,
        "messages": [{"kind": "request", "id": 1, "command": {"type": "no_such_command"}}],
        "buffers": []
    });
    assert!(validate(&schema, "", &unknown).is_err());
}

#[test]
fn t043_schema_has_the_draft_title_version_and_definitions() {
    let schema = schema_json();

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], "Partsmith protocol");
    assert_eq!(schema["x-protocol-version"], json!(1));
    for name in [
        "Command",
        "CommandResult",
        "Event",
        "Message",
        "ErrorCode",
        "BufferInfo",
        "SketchSolution",
        "MeshBuffers",
        "PrintReport",
        "FeatureManifest",
        "ElementRef",
        "Repair",
    ] {
        definition(&schema, name);
    }
}

#[test]
fn t044_schema_text_is_stable_and_ends_with_one_newline() {
    let first = protocol_schema();
    let second = protocol_schema();

    assert_eq!(first, second);
    assert!(first.ends_with('\n'), "the schema ends with a newline");
    assert!(
        !first.ends_with("\n\n"),
        "the schema ends with exactly one newline"
    );
}

#[test]
fn t045_schema_equals_the_checked_in_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/format/protocol.schema.json");
    let checked_in = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} cannot be read ({error}). Generate it with `cargo run -p partsmith-cli -- schema --write`.",
            path.display()
        )
    });

    assert!(
        protocol_schema() == checked_in,
        "protocol_schema() differs from {}. Regenerate it with `cargo run -p partsmith-cli -- schema --write`.",
        path.display()
    );
}

#[test]
fn t046_ids_and_mesh_ids_declare_the_javascript_safe_maximum() {
    let schema = schema_json();
    let request = schema
        .pointer("/$defs/Request")
        .unwrap_or_else(|| message_variant(&schema, "request"));
    let response = schema
        .pointer("/$defs/Response")
        .unwrap_or_else(|| message_variant(&schema, "response"));
    let body_mesh = variant(&schema, "Event", "type", "body_mesh");

    assert_eq!(maximum_of(&schema, request, "id"), Some(MAX_SAFE_INTEGER));
    assert_eq!(maximum_of(&schema, response, "id"), Some(MAX_SAFE_INTEGER));
    assert_eq!(
        maximum_of(&schema, body_mesh, "mesh_id"),
        Some(MAX_SAFE_INTEGER)
    );
}

#[test]
fn t081_schema_defines_the_diagnostic_fix_repair_and_ghost_types() {
    let schema = schema_json();

    for name in [
        "Diagnostic",
        "Cause",
        "FixAction",
        "FixKind",
        "FixRef",
        "LossReason",
        "Removal",
        "Absence",
        "InstanceIndex",
        "Ghost",
        "GhostKind",
        "GhostFrame",
        "RepairCandidate",
        "RegionCandidate",
        "Use",
        "Connection",
        "HintFrame",
    ] {
        definition(&schema, name);
    }
    assert!(
        schema.pointer("/$defs/AxisRef").is_none(),
        "feature params travel inside FeatureRecord, so AxisRef has no $defs entry"
    );

    // `actions` is declared but optional in both diagnostics and errors.
    for name in ["Diagnostic", "ErrorInfo"] {
        let entry = definition(&schema, name);
        assert!(
            property(&schema, entry, "actions").is_some(),
            "{name} declares actions"
        );
        let required = required_names(entry);
        assert!(
            !required.contains(&"actions"),
            "{name} does not require actions: {required:?}"
        );
    }

    // FixRef, which requests carry, rejects unknown properties.
    let fix = json!({"feature": "f-fil1", "code": "x", "cause": false, "index": 0, "revision": 1});
    assert_eq!(validate(&schema, "/$defs/FixRef", &fix), Ok(()));
    let mut with_extra = fix;
    with_extra["extra"] = json!(1);
    assert!(validate(&schema, "/$defs/FixRef", &with_extra).is_err());
}

#[test]
fn fix_kind_has_exactly_the_five_kinds_of_section_2_4() {
    assert_eq!(
        variant_tags(&schema_json(), "FixKind", "kind"),
        string_set(&["apply", "pick", "show", "undo", "finish_transaction"])
    );
}

#[test]
fn loss_reasons_and_removals_have_exactly_the_kinds_of_section_2_4() {
    let schema = schema_json();

    assert_eq!(
        variant_tags(&schema, "LossReason", "kind"),
        string_set(&[
            "unknown",
            "not_in_document",
            "suppressed",
            "comes_later",
            "rolled_back",
            "upstream_failed",
            "other_body",
            "removed",
            "entity_deleted",
            "point_gone",
            "copy_gone",
            "not_in_file",
            "followed",
        ])
    );
    assert_eq!(
        variant_tags(&schema, "Removal", "kind"),
        string_set(&[
            "cut_away",
            "covered",
            "rounded",
            "beveled",
            "hollowed",
            "piece_gone",
            "no_longer_made",
            "no_longer_meet",
        ])
    );
}

#[test]
fn absence_and_ghost_enums_have_exactly_the_values_of_section_2_4() {
    let schema = schema_json();

    assert_eq!(
        string_values(&schema, "Absence"),
        string_set(&["through", "end_outside", "full_turn"])
    );
    assert_eq!(
        string_values(&schema, "GhostKind"),
        string_set(&["lines", "triangles", "points"])
    );
    assert_eq!(
        string_values(&schema, "GhostFrame"),
        string_set(&["world", "sketch"])
    );
}

#[test]
fn apply_fix_request_validates_and_rejects_an_unknown_fix_field() {
    let schema = schema_json();
    let request = json!({
        "protocol_version": 1,
        "messages": [{"kind": "request", "id": 5, "command": {"type": "apply_fix", "doc": 1, "fix": {"feature": "f-fil1", "code": "x", "cause": false, "index": 0, "revision": 1}}}],
        "buffers": []
    });
    assert_eq!(validate(&schema, "", &request), Ok(()));

    let mut with_extra = request;
    with_extra["messages"][0]["command"]["fix"]["extra"] = json!(1);
    assert!(validate(&schema, "", &with_extra).is_err());
}

#[test]
fn fix_ref_requires_every_field_but_field() {
    let schema = schema_json();
    let fix = json!({"feature": "f-fil1", "code": "x", "field": "/edges/0", "cause": false, "index": 0, "revision": 1});
    assert_eq!(validate(&schema, "/$defs/FixRef", &fix), Ok(()));

    for name in ["feature", "code", "field", "cause", "index", "revision"] {
        let mut without = fix.clone();
        without
            .as_object_mut()
            .expect("a fix reference is an object")
            .remove(name);
        let result = validate(&schema, "/$defs/FixRef", &without);
        if name == "field" {
            assert_eq!(result, Ok(()), "a fix reference without field");
        } else {
            assert!(
                result.is_err(),
                "a fix reference without {name} is rejected"
            );
        }
    }
}

#[test]
fn diagnostic_and_fix_ref_descriptions_name_sketch_profile_findings() {
    let schema = schema_json();
    let description = |name: &str, field: Option<&str>| {
        let entry = definition(&schema, name);
        let described = match field {
            Some(field) => {
                property(&schema, entry, field).unwrap_or_else(|| panic!("{name} declares {field}"))
            }
            None => entry,
        };
        described["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{name} {field:?} has a description"))
            .to_string()
    };

    // The texts of section 2.4, which the generated TypeScript types repeat.
    assert_eq!(
        description("Diagnostic", None),
        "A message about a document, feature, sketch or export."
    );
    assert_eq!(
        description("Diagnostic", Some("field")),
        "JSON Pointer of the feature param the message is about, relative to the feature record (for example `/radius`). Feature messages, and sketch profile findings, whose field is their position in `SketchSolution::profile_findings` (`/profile_findings/0`, section 2.6)."
    );
    assert_eq!(
        description("FixRef", Some("feature")),
        "The feature whose message offers the fix (the sketch, for a profile finding)."
    );
}

#[test]
fn both_repair_variants_validate_with_and_without_their_ghost() {
    let schema = schema_json();

    for text in [ELEMENT_REPAIR, REGION_REPAIR] {
        let repair = parse_json(text);
        assert_eq!(
            validate(&schema, "/$defs/Repair", &repair),
            Ok(()),
            "{text}"
        );

        let mut without_ghost = repair;
        without_ghost
            .as_object_mut()
            .expect("a repair is an object")
            .remove("ghost");
        without_ghost["candidates"] = json!([]);
        assert_eq!(
            validate(&schema, "/$defs/Repair", &without_ghost),
            Ok(()),
            "{without_ghost}"
        );
    }
}

#[test]
fn references_with_anchors_or_merged_names_validate() {
    let schema = schema_json();

    for text in [FULL_REFERENCE, MERGED_REFERENCE] {
        assert_eq!(
            validate(&schema, "/$defs/Reference", &parse_json(text)),
            Ok(()),
            "{text}"
        );
    }
}

#[test]
fn hint_frame_allows_exactly_world_and_body() {
    assert_eq!(
        string_values(&schema_json(), "HintFrame"),
        string_set(&["world", "body"])
    );
}

#[test]
fn face_plane_with_a_body_fallback_frame_validates_and_an_unknown_frame_does_not() {
    let schema = schema_json();
    let command = parse_json(SOLVE_ON_FACE_IN_BODY_FRAME);
    assert_eq!(validate(&schema, "/$defs/Command", &command), Ok(()));

    let mut unknown = command;
    unknown["candidate"]["plane"]["fallback_frame"] = json!("local");
    assert!(validate(&schema, "/$defs/Command", &unknown).is_err());
}

#[test]
fn error_outcomes_of_section_2_8_validate_in_responses() {
    let schema = schema_json();
    let stale_fix = json!({"kind": "error", "code": "stale_fix", "message": STALE_FIX_MESSAGE});
    let outcomes = ERROR_OUTCOMES
        .into_iter()
        .map(parse_json)
        .chain([stale_fix]);

    for outcome in outcomes {
        let header = json!({
            "protocol_version": 1,
            "messages": [{"kind": "response", "id": 8, "result": outcome}],
            "buffers": []
        });
        assert_eq!(validate(&schema, "", &header), Ok(()), "{outcome}");
    }
}

#[test]
fn loss_reasons_added_by_the_review_validate_and_other_body_requires_present() {
    let schema = schema_json();
    let end_outside = json!({"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "end_outside"}});
    let left_the_body = json!({"kind": "other_body", "body": "b-2", "by": "f-ext1", "by_suppressed": false, "present": false});

    assert_eq!(validate(&schema, "/$defs/LossReason", &end_outside), Ok(()));
    assert_eq!(
        validate(&schema, "/$defs/LossReason", &left_the_body),
        Ok(())
    );
    let mut without_present = left_the_body;
    without_present
        .as_object_mut()
        .expect("a loss reason is an object")
        .remove("present");
    assert!(validate(&schema, "/$defs/LossReason", &without_present).is_err());
}

#[test]
fn headless_pick_reference_request_validates_and_its_pick_stays_closed() {
    let schema = schema_json();
    let command = json!({"type": "pick_reference", "doc": 1, "pick": {"mesh_id": 0, "kind": "edge", "index": 4, "body": "b-1"}});
    assert_eq!(validate(&schema, "/$defs/Command", &command), Ok(()));

    let mut with_extra = command;
    with_extra["pick"]["feature"] = json!("f-box1");
    assert!(validate(&schema, "/$defs/Command", &with_extra).is_err());
}

#[test]
fn region_references_with_and_without_an_outline_validate() {
    let schema = schema_json();
    let with_outline = json!({"reference": {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}, "area": 4000.0, "centroid": [40.0, 25.0]});
    let mut without_outline = with_outline.clone();
    without_outline["reference"]
        .as_object_mut()
        .expect("a region reference is an object")
        .remove("outline");

    assert_eq!(
        validate(&schema, "/$defs/RegionInfo", &with_outline),
        Ok(())
    );
    assert_eq!(
        validate(&schema, "/$defs/RegionInfo", &without_outline),
        Ok(())
    );
}

#[test]
fn diagnostic_with_a_suggestion_and_a_root_validates() {
    let schema = schema_json();
    let mut diagnostic = parse_json(R16_DIAGNOSTIC);
    diagnostic["suggestion"] = json!("2.3 mm");
    diagnostic["root"] = json!("f-dp1");

    assert_eq!(validate(&schema, "/$defs/Diagnostic", &diagnostic), Ok(()));
}

#[test]
fn set_document_units_needs_its_units_and_may_leave_out_keep_sizes() {
    let schema = schema_json();
    let command = variant(&schema, "Command", "type", "set_document_units");

    let required = required_names(command);

    assert!(
        required.contains(&"doc") && required.contains(&"units"),
        "{required:?}"
    );
    assert!(!required.contains(&"keep_sizes"), "{required:?}");
    assert!(property(&schema, command, "keep_sizes").is_some());
}

#[test]
fn document_changed_declares_bare_numbers() {
    let schema = schema_json();
    let event = variant(&schema, "Event", "type", "document_changed");

    assert!(property(&schema, event, "bare_numbers").is_some());
}

#[test]
fn use_requires_its_user_field_and_feature_but_not_its_entity() {
    let schema = schema_json();
    let entry = definition(&schema, "Use");

    let required: BTreeSet<String> = required_names(entry)
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(required, string_set(&["user", "field", "feature"]));
    assert!(property(&schema, entry, "entity").is_some());

    let complete =
        json!({"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1", "entity": "e-3"});
    assert_eq!(validate(&schema, "/$defs/Use", &complete), Ok(()));
    let mut null_entity = complete.clone();
    null_entity["entity"] = Value::Null;
    assert_eq!(validate(&schema, "/$defs/Use", &null_entity), Ok(()));
    for name in ["entity", "user", "field", "feature"] {
        let mut without = complete.clone();
        without
            .as_object_mut()
            .expect("a use is an object")
            .remove(name);
        let result = validate(&schema, "/$defs/Use", &without);
        if name == "entity" {
            assert_eq!(result, Ok(()), "a use without entity");
        } else {
            assert!(result.is_err(), "a use without {name} is rejected");
        }
    }
}

#[test]
fn list_uses_request_names_only_its_document() {
    let schema = schema_json();
    let command = json!({"type": "list_uses", "doc": 1});
    assert_eq!(validate(&schema, "/$defs/Command", &command), Ok(()));

    let mut with_extra = command;
    with_extra["feature"] = json!("f-fil1");
    assert!(validate(&schema, "/$defs/Command", &with_extra).is_err());
    let without_doc = json!({"type": "list_uses"});
    assert!(validate(&schema, "/$defs/Command", &without_doc).is_err());
}

#[test]
fn list_uses_result_requires_its_uses_as_a_list_of_use() {
    let schema = schema_json();
    let result = variant(&schema, "CommandResult", "type", "list_uses");

    assert!(
        required_names(result).contains(&"uses"),
        "{:?}",
        required_names(result)
    );
    let uses = property(&schema, result, "uses").expect("the list_uses result declares uses");
    assert_eq!(uses["type"], "array");
    assert_eq!(uses["items"]["$ref"], "#/$defs/Use");
    assert_eq!(
        validate(
            &schema,
            "/$defs/CommandResult",
            &json!({"type": "list_uses", "uses": []})
        ),
        Ok(())
    );
    assert!(
        validate(
            &schema,
            "/$defs/CommandResult",
            &json!({"type": "list_uses"})
        )
        .is_err()
    );
}

#[test]
fn uses_causes_and_ghosts_the_engine_fills_validate() {
    let schema = schema_json();
    let shapes = [
        ("CommandResult", BLOCK_USES_RESULT),
        ("Cause", R26_CAUSE),
        ("Diagnostic", DELETED_POINT_MESSAGE),
        ("Repair", LOST_FACE_REPAIR),
    ];

    for (name, text) in shapes {
        assert_eq!(
            validate(&schema, &format!("/$defs/{name}"), &parse_json(text)),
            Ok(()),
            "{name}: {text}"
        );
    }

    // A ghost names its kind and frame from the lists of section 2.4 only.
    let mut unknown_kind = parse_json(LOST_FACE_REPAIR);
    unknown_kind["ghost"]["kind"] = json!("polygons");
    assert!(validate(&schema, "/$defs/Repair", &unknown_kind).is_err());
    let mut unknown_frame = parse_json(DELETED_POINT_MESSAGE);
    unknown_frame["cause"]["ghost"]["frame"] = json!("body");
    assert!(validate(&schema, "/$defs/Diagnostic", &unknown_frame).is_err());
}
