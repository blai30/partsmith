//! Acceptance test 86 of `docs/spec/protocol.md` ("Printer profiles"): results list profiles in print.md's written
//! form and requests carry the input form (sections 2.5 commands 3 and 4, and 2.9).

use crate::common;

use common::examples::{LIST_PRINTER_PROFILES_RESULT, PUT_PRINTER_PROFILE_REQUEST};
use common::json_schema::validate;
use common::{
    assert_accepts_but_rejects, assert_requires_all_but, dangling_references, definition,
    parse_json, property, schema_json, variant, with_member,
};
use partsmith_print::{PrinterProfile, ProfileInput, generic_fdm, generic_resin};
use partsmith_protocol::{CommandResult, Message, Outcome, Response};
use serde_json::{Value, json};

/// The members of `P` that the input form leaves optional: all but `id`, `name` and `technology`.
const INPUT_OPTIONAL: [&str; 6] = [
    "bed",
    "nozzle_diameter",
    "layer_height",
    "max_overhang_angle",
    "max_bridge_length",
    "fit",
];

/// `P`: the `generic-fdm` profile of the `list_printer_profiles` result example of section 2.5.
fn listed_profile() -> Value {
    parse_json(LIST_PRINTER_PROFILES_RESULT)["profiles"][0].clone()
}

/// The profile of the `put_printer_profile` request example of section 2.5.
fn put_profile() -> Value {
    parse_json(PUT_PRINTER_PROFILE_REQUEST)["profile"].clone()
}

/// The profiles of test 86 (c), as the engine lists them: `generic_fdm()`, `generic_resin()` and the profile that
/// `PrinterProfile::try_from` makes of the `put_printer_profile` example's input.
fn engine_profiles() -> Vec<PrinterProfile> {
    let input: ProfileInput =
        serde_json::from_value(put_profile()).expect("the put example is a profile input");
    let converted =
        PrinterProfile::try_from(input).expect("the put example converts to a valid profile");
    vec![generic_fdm(), generic_resin(), converted]
}

/// The `$ref` of the property `name` of `subschema`.
fn property_reference<'a>(schema: &'a Value, subschema: &'a Value, name: &str) -> &'a Value {
    &property(schema, subschema, name)
        .unwrap_or_else(|| panic!("the schema declares no property {name}"))["$ref"]
}

#[test]
fn t086_listed_profiles_refer_to_the_written_form() {
    let schema = schema_json();
    let result = variant(&schema, "CommandResult", "type", "list_printer_profiles");

    let profiles =
        property(&schema, result, "profiles").expect("the list result declares profiles");
    assert_eq!(profiles["type"], "array");
    assert_eq!(profiles["items"]["$ref"], "#/$defs/PrinterProfile");
}

#[test]
fn t086_the_put_request_refers_to_the_input_form() {
    let schema = schema_json();
    let command = variant(&schema, "Command", "type", "put_printer_profile");

    assert_eq!(
        property_reference(&schema, command, "profile"),
        "#/$defs/ProfileInput"
    );
}

#[test]
fn t086_the_written_form_takes_its_fit_as_fit_classes() {
    let schema = schema_json();

    assert_eq!(
        property_reference(&schema, definition(&schema, "PrinterProfile"), "fit"),
        "#/$defs/FitClasses"
    );
}

#[test]
fn t086_a_listed_profile_needs_every_member_but_its_nozzle() {
    let schema = schema_json();

    assert_requires_all_but(
        &schema,
        "PrinterProfile",
        &listed_profile(),
        &["nozzle_diameter"],
    );
}

#[test]
fn t086_listed_fit_classes_need_all_four_classes() {
    let schema = schema_json();

    assert_requires_all_but(&schema, "FitClasses", &listed_profile()["fit"], &[]);
}

#[test]
fn t086_a_listed_profile_with_an_unknown_member_fails() {
    let schema = schema_json();
    let profile = listed_profile();
    let with_extra = with_member(&profile, "extra", json!(1));

    assert_accepts_but_rejects(&schema, "PrinterProfile", &profile, &with_extra);
}

#[test]
fn t086_a_listed_profile_whose_bed_has_an_unknown_member_fails() {
    let schema = schema_json();
    let profile = listed_profile();
    let mut bed_with_extra = profile.clone();
    bed_with_extra["bed"]["extra"] = json!(1);

    assert_accepts_but_rejects(&schema, "PrinterProfile", &profile, &bed_with_extra);
}

#[test]
fn t086_listed_fit_classes_with_an_unknown_member_fail() {
    let schema = schema_json();
    let fit = listed_profile()["fit"].clone();
    let fit_with_extra = with_member(&fit, "extra", json!(1));

    assert_accepts_but_rejects(&schema, "FitClasses", &fit, &fit_with_extra);
}

#[test]
fn t086_the_input_form_needs_only_id_name_and_technology() {
    let schema = schema_json();

    assert_requires_all_but(&schema, "ProfileInput", &listed_profile(), &INPUT_OPTIONAL);
}

#[test]
fn t086_an_input_with_an_unknown_member_fails() {
    let schema = schema_json();
    let profile = listed_profile();
    let with_extra = with_member(&profile, "extra", json!(1));

    assert_accepts_but_rejects(&schema, "ProfileInput", &profile, &with_extra);
}

#[test]
fn t086_the_put_example_is_an_input_but_not_a_listed_profile() {
    let schema = schema_json();
    let profile = put_profile();

    assert_eq!(validate(&schema, "/$defs/ProfileInput", &profile), Ok(()));
    assert_accepts_but_rejects(&schema, "PrinterProfile", &listed_profile(), &profile);
}

#[test]
fn t086_a_fit_naming_one_class_is_an_input_but_not_fit_classes() {
    let schema = schema_json();
    let fit = json!({"slip": 0.3});

    assert_eq!(validate(&schema, "/$defs/FitInput", &fit), Ok(()));
    assert_accepts_but_rejects(&schema, "FitClasses", &listed_profile()["fit"], &fit);
}

#[test]
fn t086_every_profile_the_engine_lists_validates_against_the_written_form() {
    let schema = schema_json();

    for profile in engine_profiles() {
        let value = serde_json::to_value(&profile).expect("profiles serialize");
        assert_eq!(
            validate(&schema, "/$defs/PrinterProfile", &value),
            Ok(()),
            "{value}"
        );
    }
}

#[test]
fn t086_a_list_result_of_those_profiles_validates_in_a_frame_header() {
    let schema = schema_json();
    let response = Message::Response(Response {
        id: 1,
        result: Outcome::Ok(CommandResult::ListPrinterProfiles {
            profiles: engine_profiles(),
        }),
    });
    let message = serde_json::to_value(&response).expect("messages serialize");
    let header = json!({"protocol_version": 1, "messages": [message], "buffers": []});

    assert_eq!(validate(&schema, "", &header), Ok(()), "{header}");
}

// The profile routes of ADR 0062 (section 2.5 commands 4 and 36, section 2.9; tests 89, 92, 93, 95 and 122). The parts
// of tests 89 to 91, 94 and 95 that go through `Session::call` are in partsmith-engine's `tests/stored_profiles.rs`.

/// `E11P`: the `printer_profile` object of file-format.md's E11.
fn e11p() -> Value {
    json!({
        "id": "my-mk4",
        "name": "My MK4",
        "technology": "fdm",
        "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
        "nozzle_diameter": 0.4,
        "layer_height": 0.15,
        "max_overhang_angle": std::f64::consts::FRAC_PI_4,
        "max_bridge_length": 10,
        "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3}
    })
}

/// Whether `value` is the schema `anyOf` of `{"$ref": "#/$defs/PrinterProfile"}` and `{"type": "null"}`.
fn is_optional_profile(value: &Value) -> bool {
    let Some(branches) = value.get("anyOf").and_then(Value::as_array) else {
        return false;
    };
    branches.len() == 2
        && branches.contains(&json!({"$ref": "#/$defs/PrinterProfile"}))
        && branches.contains(&json!({"type": "null"}))
}

#[test]
fn t089_a_profile_as_text_deserializes_with_text_and_no_profile() {
    let command: partsmith_protocol::Command = serde_json::from_str(
        r#"{"type":"put_printer_profile","text":"{\"id\": \"my-mk4\", \"name\": \"My MK4\", \"technology\": \"fdm\", \"layer_height\": 0.15}"}"#,
    )
    .expect("the request deserializes");

    let partsmith_protocol::Command::PutPrinterProfile { profile, text } = command else {
        panic!("a put_printer_profile command, got {command:?}");
    };
    assert_eq!(profile, None);
    assert_eq!(
        text.as_deref(),
        Some(r#"{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}"#)
    );
}

#[test]
fn t092_remove_printer_profile_round_trips_and_requires_its_id() {
    let text = r#"{"type":"remove_printer_profile","id":"my-mk4"}"#;
    let command: partsmith_protocol::Command =
        serde_json::from_str(text).expect("the request deserializes");
    let result =
        serde_json::to_string(&CommandResult::RemovePrinterProfile).expect("the result serializes");
    let schema = schema_json();

    assert_eq!(
        serde_json::to_string(&command).expect("the request serializes"),
        text
    );
    assert_eq!(result, r#"{"type":"remove_printer_profile"}"#);
    let branch = variant(&schema, "Command", "type", "remove_printer_profile");
    let required = branch
        .get("required")
        .and_then(Value::as_array)
        .expect("the branch lists required members");
    assert!(required.contains(&json!("id")), "{required:?}");
}

#[test]
fn t093_the_schema_of_the_stored_profile() {
    let schema = schema_json();
    let set_printer = variant(&schema, "Edit", "type", "set_printer");
    let put_result = variant(&schema, "CommandResult", "type", "put_printer_profile");

    assert!(
        is_optional_profile(
            schema
                .pointer("/$defs/Document/properties/printer_profile")
                .expect("Document has printer_profile")
        ),
        "{}",
        schema["$defs"]["Document"]["properties"]["printer_profile"]
    );
    assert!(
        is_optional_profile(
            set_printer
                .pointer("/properties/profile")
                .expect("set_printer has profile")
        ),
        "{set_printer}"
    );
    assert_eq!(
        put_result.pointer("/properties/profile"),
        Some(&json!({"$ref": "#/$defs/PrinterProfile"}))
    );
    assert_eq!(dangling_references(&schema), Vec::<String>::new());
}

#[test]
fn t095_a_set_printer_edit_with_a_profile_round_trips() {
    let request = json!({"type": "apply_edits", "doc": 1, "label": "Change printer", "edits": [{"type": "set_printer", "printer": "my-mk4", "profile": e11p()}]});
    let mut extra = e11p();
    extra["speed"] = json!(1);
    let with_extra = json!({"type": "apply_edits", "doc": 1, "label": "Change printer", "edits": [{"type": "set_printer", "printer": "my-mk4", "profile": extra}]});

    let command: partsmith_protocol::Command =
        serde_json::from_value(request.clone()).expect("the request deserializes");
    let extra_command = serde_json::from_value::<partsmith_protocol::Command>(with_extra);

    assert_eq!(
        serde_json::to_string(&command).expect("the request serializes"),
        serde_json::to_string(&request).expect("the JSON serializes")
    );
    assert!(extra_command.is_ok(), "{extra_command:?}");
}

#[test]
fn t122_the_protocol_schema_leaves_occurrence_free() {
    let generated = schema_json();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/format/protocol.schema.json");
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} cannot be read: {error}", path.display())),
    )
    .expect("the committed schema is JSON");

    for (name, schema) in [
        ("protocol_schema()", &generated),
        ("protocol.schema.json", &committed),
    ] {
        let mut keys = Vec::new();
        property_keys(schema, &mut keys);
        assert!(
            !keys.iter().any(|key| key == "occurrence"),
            "{name} has a property named occurrence"
        );
    }
}

/// The key of every member of every `properties` object in `value`, at any depth.
fn property_keys(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(members) => {
            for (key, member) in members {
                if key == "properties"
                    && let Some(properties) = member.as_object()
                {
                    found.extend(properties.keys().cloned());
                }
                property_keys(member, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| property_keys(item, found)),
        _ => {}
    }
}
