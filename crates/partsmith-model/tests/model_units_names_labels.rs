//! Acceptance tests for document units, colors, body roles, parameter names, labels and feature types (model.md section 6, "Units and colors" and "Names and labels", tests 17 to 29).

use std::collections::BTreeSet;

use partsmith_model::{
    BodyRole, Color, ColorError, LabelError, LabelField, NameError, RESERVED_NAMES, Units,
    UnitsError, is_valid_feature_type, validate_label, validate_parameter_name,
};

#[test]
fn t017_units_serialize_as_lowercase_symbols() {
    let texts: Vec<String> = [Units::Mm, Units::Cm, Units::M, Units::In]
        .iter()
        .map(|units| serde_json::to_string(units).expect("units serialize"))
        .collect();

    assert_eq!(texts, ["\"mm\"", "\"cm\"", "\"m\"", "\"in\""]);
    assert!(serde_json::from_str::<Units>("\"MM\"").is_err());
    assert!(serde_json::from_str::<Units>("\"inch\"").is_err());
    assert_eq!(Units::default(), Units::Mm);
}

#[test]
fn t018_units_scale_and_parse() {
    assert_eq!(
        [Units::Mm, Units::Cm, Units::M, Units::In].map(Units::millimeters),
        [1.0, 10.0, 1000.0, 25.4]
    );
    assert_eq!("in".parse::<Units>(), Ok(Units::In));
    assert_eq!(
        "ft".parse::<Units>(),
        Err(UnitsError::Unknown {
            text: "ft".to_string()
        })
    );
}

#[test]
fn t019_colors_parse_any_case_and_write_lowercase() {
    let parsed = "#3B82F6".parse::<Color>();
    let deserialized: Color =
        serde_json::from_str("\"#3B82F6\"").expect("an uppercase color deserializes");

    assert_eq!(parsed, Ok(Color::new(0x3b, 0x82, 0xf6)));
    assert_eq!(Color::new(0x3b, 0x82, 0xf6).to_string(), "#3b82f6");
    assert_eq!(
        serde_json::to_string(&deserialized).expect("a color serializes"),
        "\"#3b82f6\""
    );
}

#[test]
fn t020_malformed_colors_are_rejected() {
    for text in ["#12345", "3b82f6", "#3b82fg", "#3b82f6ff", ""] {
        assert_eq!(
            text.parse::<Color>(),
            Err(ColorError::Invalid {
                text: text.to_string()
            }),
            "{text:?}"
        );
    }
}

#[test]
fn t021_body_roles_serialize_in_snake_case() {
    assert_eq!(
        serde_json::to_value(BodyRole::SettingsModifier).expect("a role serializes"),
        serde_json::json!("settings_modifier")
    );
    assert_eq!(BodyRole::default(), BodyRole::Part);
}

#[test]
fn t022_valid_parameter_names_are_accepted() {
    let longest = "a".repeat(64);
    for name in [
        "wall", "_x", "wall_2", "W", "Pi", "MM", "mm2", "in_1", "class", "None", &longest,
    ] {
        assert_eq!(validate_parameter_name(name), Ok(()), "{name:?}");
    }
}

#[test]
fn t023_empty_and_long_parameter_names_are_rejected() {
    assert_eq!(validate_parameter_name(""), Err(NameError::Empty));
    assert!(
        matches!(
            validate_parameter_name(&"a".repeat(65)),
            Err(NameError::TooLong { length: 65, .. })
        ),
        "{:?}",
        validate_parameter_name(&"a".repeat(65))
    );
}

#[test]
fn t024_parameter_names_use_letters_digits_and_underscores() {
    assert!(
        matches!(
            validate_parameter_name("2x"),
            Err(NameError::InvalidStart { .. })
        ),
        "{:?}",
        validate_parameter_name("2x")
    );
    assert!(
        matches!(
            validate_parameter_name("wall-2"),
            Err(NameError::InvalidCharacter { character: '-', .. })
        ),
        "{:?}",
        validate_parameter_name("wall-2")
    );
    assert!(
        matches!(
            validate_parameter_name("wäll"),
            Err(NameError::InvalidCharacter {
                character: 'ä', ..
            })
        ),
        "{:?}",
        validate_parameter_name("wäll")
    );
}

#[test]
fn t025_reserved_words_are_not_parameter_names() {
    for name in [
        "pi", "mm", "in", "deg", "rad", "um", "ft", "sqrt", "atan2", "round", "printer",
    ] {
        assert!(
            matches!(
                validate_parameter_name(name),
                Err(NameError::Reserved { .. })
            ),
            "{name}: {:?}",
            validate_parameter_name(name)
        );
    }
    let distinct: BTreeSet<&str> = RESERVED_NAMES.iter().copied().collect();
    assert_eq!(RESERVED_NAMES.len(), 24);
    assert_eq!(distinct.len(), 24, "RESERVED_NAMES has duplicates");
}

#[test]
fn t026_reserved_name_message_suggests_a_name() {
    let error = NameError::Reserved { name: "pi".into() };

    assert_eq!(
        error.to_string(),
        "'pi' is a reserved word. Pick another name, such as pi_1."
    );
}

#[test]
fn t027_label_rules() {
    let field = LabelField::BodyName;

    assert_eq!(validate_label(field, "Enclosure"), Ok(()));
    assert_eq!(validate_label(field, " Lid "), Ok(()));
    assert_eq!(validate_label(field, ""), Err(LabelError::Empty { field }));
    assert_eq!(
        validate_label(field, "   "),
        Err(LabelError::Empty { field })
    );
    assert_eq!(validate_label(field, &"x".repeat(128)), Ok(()));
    assert!(
        matches!(
            validate_label(field, &"x".repeat(129)),
            Err(LabelError::TooLong { length: 129, .. })
        ),
        "{:?}",
        validate_label(field, &"x".repeat(129))
    );
    assert_eq!(
        validate_label(field, "Lid\n"),
        Err(LabelError::ControlCharacter { field })
    );
    assert_eq!(
        validate_label(field, "a\tb"),
        Err(LabelError::ControlCharacter { field })
    );
}

#[test]
fn t028_empty_label_message_names_the_field() {
    let error = LabelError::Empty {
        field: LabelField::BodyName,
    };

    assert_eq!(error.to_string(), "Body name cannot be empty.");
}

#[test]
fn t029_feature_type_rules() {
    for kind in ["extrude", "linear_pattern", "a1"] {
        assert!(is_valid_feature_type(kind), "{kind:?} should be valid");
    }
    let too_long = "a".repeat(65);
    for kind in ["", "Extrude", "1box", "my-feature", "_box", &too_long] {
        assert!(!is_valid_feature_type(kind), "{kind:?} should be invalid");
    }
}
