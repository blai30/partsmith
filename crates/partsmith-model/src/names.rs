//! Parameter names, labels and feature type names (model.md sections 3.3 to 3.5).

use std::fmt;

/// Longest parameter name, in characters.
pub const MAX_PARAMETER_NAME_LENGTH: usize = 64;
/// Longest label (body name, feature name, material, printer), in characters.
pub const MAX_LABEL_LENGTH: usize = 128;
/// Longest feature type name, in characters.
pub const MAX_FEATURE_TYPE_LENGTH: usize = 64;

/// Words that cannot be parameter names, in the order listed in model.md section 3.3: the constant,
/// the expression units, the expression functions and the printer namespace.
pub const RESERVED_NAMES: &[&str] = &[
    "pi", "um", "mm", "cm", "m", "in", "ft", "deg", "rad", "abs", "sqrt", "sin", "cos", "tan",
    "asin", "acos", "atan", "atan2", "min", "max", "floor", "ceil", "round", "printer",
];

/// Regular expression every valid parameter name matches, apart from the reserved words.
pub(crate) const PARAMETER_NAME_PATTERN: &str = "^[A-Za-z_][A-Za-z0-9_]{0,63}$";

/// Checks a parameter name against model.md section 3.3.
pub fn validate_parameter_name(name: &str) -> Result<(), NameError> {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return Err(NameError::Empty);
    };
    let length = name.chars().count();
    if length > MAX_PARAMETER_NAME_LENGTH {
        return Err(NameError::TooLong {
            name: name.to_string(),
            length,
        });
    }
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(NameError::InvalidStart {
            name: name.to_string(),
        });
    }
    if let Some(character) =
        characters.find(|character| !(character.is_ascii_alphanumeric() || *character == '_'))
    {
        return Err(NameError::InvalidCharacter {
            name: name.to_string(),
            character,
        });
    }
    if RESERVED_NAMES.contains(&name) {
        return Err(NameError::Reserved {
            name: name.to_string(),
        });
    }
    Ok(())
}

/// Checks a label against model.md section 3.4; `field` only selects the wording of the error.
pub fn validate_label(field: LabelField, text: &str) -> Result<(), LabelError> {
    if text.chars().all(char::is_whitespace) {
        return Err(LabelError::Empty { field });
    }
    let length = text.chars().count();
    if length > MAX_LABEL_LENGTH {
        return Err(LabelError::TooLong { field, length });
    }
    if text.chars().any(char::is_control) {
        return Err(LabelError::ControlCharacter { field });
    }
    Ok(())
}

/// Regular expression every valid feature type name matches.
pub(crate) const FEATURE_TYPE_PATTERN: &str = "^[a-z][a-z0-9_]{0,63}$";

/// True when `kind` is a valid feature type name (model.md section 3.5): `^[a-z][a-z0-9_]{0,63}$`.
pub fn is_valid_feature_type(kind: &str) -> bool {
    let mut characters = kind.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && kind.len() <= MAX_FEATURE_TYPE_LENGTH
}

/// Which label a `LabelError` is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelField {
    /// `Body::name`, worded "Body name".
    BodyName,
    /// `FeatureRecord::name`, worded "Feature name".
    FeatureName,
    /// `Body::material`, worded "Material".
    Material,
    /// `Document::printer`, worded "Printer".
    Printer,
}

impl fmt::Display for LabelField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wording = match self {
            LabelField::BodyName => "Body name",
            LabelField::FeatureName => "Feature name",
            LabelField::Material => "Material",
            LabelField::Printer => "Printer",
        };
        formatter.write_str(wording)
    }
}

/// Why a parameter name is invalid.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum NameError {
    /// The name is empty.
    #[error("A parameter name cannot be empty.")]
    Empty,
    /// The name has more than 64 characters.
    #[error(
        "The parameter name '{name}' is {length} characters long; the limit is {max}.",
        max = MAX_PARAMETER_NAME_LENGTH
    )]
    TooLong {
        /// The rejected name.
        name: String,
        /// Length in characters.
        length: usize,
    },
    /// The name starts with something other than a letter or `_`.
    #[error("The parameter name '{name}' must start with a letter or _.")]
    InvalidStart {
        /// The rejected name.
        name: String,
    },
    /// The name contains a character other than letters, digits and `_`.
    #[error(
        "The parameter name '{name}' contains '{}'. Use only letters, digits and _, as in wall_2.",
        .character.escape_debug()
    )]
    InvalidCharacter {
        /// The rejected name.
        name: String,
        /// The first character that is not allowed.
        character: char,
    },
    /// The name is a reserved word.
    #[error("'{name}' is a reserved word. Pick another name, such as {name}_1.")]
    Reserved {
        /// The rejected name.
        name: String,
    },
}

/// Why a label is invalid.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LabelError {
    /// The label is empty or only whitespace.
    #[error("{field} cannot be empty.")]
    Empty {
        /// The label being checked.
        field: LabelField,
    },
    /// The label has more than 128 characters.
    #[error("{field} is {length} characters long; the limit is {max}.", max = MAX_LABEL_LENGTH)]
    TooLong {
        /// The label being checked.
        field: LabelField,
        /// Length in characters.
        length: usize,
    },
    /// The label contains a control character.
    #[error("{field} contains a control character such as a tab or line break. Remove it.")]
    ControlCharacter {
        /// The label being checked.
        field: LabelField,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{Function, Unit};

    #[test]
    fn reserved_names_are_pi_units_functions_and_printer() {
        let mut expected = vec!["pi"];
        expected.extend(Unit::ALL.map(Unit::symbol));
        expected.extend(Function::ALL.map(Function::name));
        expected.push("printer");

        assert_eq!(RESERVED_NAMES, expected.as_slice());
    }

    #[test]
    fn name_length_counts_characters() {
        let name = format!("_{}", "a".repeat(63));

        assert_eq!(validate_parameter_name(&name), Ok(()));
    }

    #[test]
    fn non_ascii_start_is_an_invalid_start() {
        assert_eq!(
            validate_parameter_name("é"),
            Err(NameError::InvalidStart {
                name: "é".to_string()
            })
        );
    }

    #[test]
    fn length_is_checked_before_the_start() {
        assert!(matches!(
            validate_parameter_name(&"1".repeat(65)),
            Err(NameError::TooLong { length: 65, .. })
        ));
    }

    #[test]
    fn reserved_matching_is_case_sensitive() {
        assert_eq!(validate_parameter_name("Printer"), Ok(()));
        assert!(validate_parameter_name("printer").is_err());
    }

    #[test]
    fn invalid_character_message_names_the_character() {
        let error = validate_parameter_name("a b").expect_err("a space is not allowed");

        assert_eq!(
            error.to_string(),
            "The parameter name 'a b' contains ' '. Use only letters, digits and _, as in wall_2."
        );
    }

    #[test]
    fn whitespace_only_labels_are_empty_before_control_characters() {
        assert_eq!(
            validate_label(LabelField::Material, "\t\n"),
            Err(LabelError::Empty {
                field: LabelField::Material
            })
        );
    }

    #[test]
    fn label_length_counts_characters() {
        assert_eq!(
            validate_label(LabelField::FeatureName, &"é".repeat(128)),
            Ok(())
        );
    }

    #[test]
    fn label_messages_name_the_field() {
        assert_eq!(
            LabelError::TooLong {
                field: LabelField::Printer,
                length: 129
            }
            .to_string(),
            "Printer is 129 characters long; the limit is 128."
        );
        assert_eq!(
            LabelError::ControlCharacter {
                field: LabelField::FeatureName
            }
            .to_string(),
            "Feature name contains a control character such as a tab or line break. Remove it."
        );
    }

    #[test]
    fn feature_types_may_hold_digits_and_underscores_after_the_first_letter() {
        assert!(is_valid_feature_type("a_1"));
        assert!(is_valid_feature_type(&"a".repeat(64)));
        assert!(!is_valid_feature_type("aé"));
    }

    #[test]
    fn too_long_parameter_name_names_the_limit() {
        let error = NameError::TooLong {
            name: "a".repeat(65),
            length: 65,
        };

        assert!(
            error
                .to_string()
                .contains(&format!("the limit is {MAX_PARAMETER_NAME_LENGTH}.")),
            "{error}"
        );
    }

    #[test]
    fn too_long_label_names_the_limit() {
        let error = LabelError::TooLong {
            field: LabelField::BodyName,
            length: 129,
        };

        assert!(
            error
                .to_string()
                .contains(&format!("the limit is {MAX_LABEL_LENGTH}.")),
            "{error}"
        );
    }

    #[test]
    fn patterns_allow_the_longest_valid_names() {
        assert!(
            PARAMETER_NAME_PATTERN.ends_with(&format!(",{}}}$", MAX_PARAMETER_NAME_LENGTH - 1))
        );
        assert!(FEATURE_TYPE_PATTERN.ends_with(&format!(",{}}}$", MAX_FEATURE_TYPE_LENGTH - 1)));
    }
}
