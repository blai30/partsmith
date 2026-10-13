//! Record ids and the generators that create new ones (model.md sections 3.1 and 3.2).

use std::borrow::{Borrow, Cow};
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

/// Regular expression every valid id matches.
pub(crate) const ID_PATTERN: &str = "^[A-Za-z0-9_-]{1,64}$";

/// Largest value a generated id can encode: the low 48 bits.
const MAX_GENERATED_VALUE: u64 = 0xFFFF_FFFF_FFFF;

/// Most candidate values `RandomIdGenerator` tries before giving up.
pub const MAX_ID_ATTEMPTS: usize = 64;

/// A validated record id: 1 to 64 characters, each one of `A-Z`, `a-z`, `0-9`, `-` or `_`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Id(String);

impl Id {
    /// Longest valid id, in characters.
    pub const MAX_LENGTH: usize = 64;

    /// Validates `value` and wraps it.
    pub fn new(value: impl Into<String>) -> Result<Id, IdError> {
        let value = value.into();
        check_id(&value)?;
        Ok(Id(value))
    }

    /// Builds the generated form: the kind prefix plus 12 lowercase hex digits of the low 48 bits of `value`.
    pub fn generated(kind: IdKind, value: u64) -> Id {
        Id(format!(
            "{}{:012x}",
            kind.prefix(),
            value & MAX_GENERATED_VALUE
        ))
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Checks the id rules in the order of model.md section 3.1.
fn check_id(text: &str) -> Result<(), IdError> {
    if text.is_empty() {
        return Err(IdError::Empty);
    }
    let length = text.chars().count();
    if length > Id::MAX_LENGTH {
        return Err(IdError::TooLong { length });
    }
    match text.chars().find(|character| !is_id_character(*character)) {
        Some(character) => Err(IdError::InvalidCharacter {
            id: text.to_string(),
            character,
        }),
        None => Ok(()),
    }
}

/// True for the characters an id may contain.
fn is_id_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '_'
}

impl FromStr for Id {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Id, IdError> {
        Id::new(text)
    }
}

impl TryFrom<String> for Id {
    type Error = IdError;

    fn try_from(value: String) -> Result<Id, IdError> {
        Id::new(value)
    }
}

impl TryFrom<&str> for Id {
    type Error = IdError;

    fn try_from(value: &str) -> Result<Id, IdError> {
        Id::new(value)
    }
}

impl From<Id> for String {
    fn from(id: Id) -> String {
        id.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Id {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl JsonSchema for Id {
    fn schema_name() -> Cow<'static, str> {
        "Id".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Id").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": ID_PATTERN})
    }
}

/// The kind of record a generated id is for; it selects the prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdKind {
    /// Prefix `p-`.
    Parameter,
    /// Prefix `b-`.
    Body,
    /// Prefix `f-`.
    Feature,
    /// Prefix `e-`.
    SketchEntity,
    /// Prefix `c-`.
    SketchConstraint,
}

impl IdKind {
    /// Every kind, in declaration order.
    pub const ALL: [IdKind; 5] = [
        IdKind::Parameter,
        IdKind::Body,
        IdKind::Feature,
        IdKind::SketchEntity,
        IdKind::SketchConstraint,
    ];

    /// The prefix, such as `"f-"`.
    pub fn prefix(self) -> &'static str {
        match self {
            IdKind::Parameter => "p-",
            IdKind::Body => "b-",
            IdKind::Feature => "f-",
            IdKind::SketchEntity => "e-",
            IdKind::SketchConstraint => "c-",
        }
    }

    /// Position of this kind in `IdKind::ALL`.
    fn position(self) -> usize {
        match self {
            IdKind::Parameter => 0,
            IdKind::Body => 1,
            IdKind::Feature => 2,
            IdKind::SketchEntity => 3,
            IdKind::SketchConstraint => 4,
        }
    }
}

/// Why an id is invalid or could not be generated.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum IdError {
    /// The text is empty.
    #[error("An id cannot be empty.")]
    Empty,
    /// The text has more than 64 characters.
    #[error(
        "This id is {length} characters long; ids can be at most {max} characters.",
        max = Id::MAX_LENGTH
    )]
    TooLong {
        /// Length in characters.
        length: usize,
    },
    /// The text contains a character other than letters, digits, `-` and `_`.
    #[error(
        "The id '{id}' contains '{}'. Ids use only letters A to Z and a to z, digits, - and _.",
        .character.escape_debug()
    )]
    InvalidCharacter {
        /// The rejected text.
        id: String,
        /// The first character that is not allowed.
        character: char,
    },
    /// The operating system random source failed.
    #[error("Could not create a new id because the system random number source is unavailable.")]
    RandomUnavailable,
    /// Every candidate the generator tried was taken.
    #[error(
        "Could not find an unused id after {attempts} attempts. The id generator keeps producing ids that are already taken."
    )]
    Exhausted {
        /// Number of candidates tried.
        attempts: usize,
    },
}

/// Source of new ids. Each generator owns its state; there is no global generator.
pub trait IdGenerator: Send {
    /// Returns a new id of `kind`, starting with `kind.prefix()`, for which `is_taken` returns false (model.md section 3.2).
    fn generate(&mut self, kind: IdKind, is_taken: &dyn Fn(&Id) -> bool) -> Result<Id, IdError>;
}

/// Generator backed by the operating system random source; used for interactive edits.
#[derive(Clone, Copy, Debug, Default)]
pub struct RandomIdGenerator;

impl RandomIdGenerator {
    /// Creates the generator.
    pub fn new() -> RandomIdGenerator {
        RandomIdGenerator
    }
}

impl IdGenerator for RandomIdGenerator {
    fn generate(&mut self, kind: IdKind, is_taken: &dyn Fn(&Id) -> bool) -> Result<Id, IdError> {
        for _ in 0..MAX_ID_ATTEMPTS {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes).map_err(|_| IdError::RandomUnavailable)?;
            let candidate = Id::generated(kind, u64::from_le_bytes(bytes));
            if !is_taken(&candidate) {
                return Ok(candidate);
            }
        }
        Err(IdError::Exhausted {
            attempts: MAX_ID_ATTEMPTS,
        })
    }
}

/// Deterministic generator with one counter per kind (`f-000000000001`, `f-000000000002`, ...).
/// Used by script runs, the CLI and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SequentialIds {
    /// The value of the last id handed out for each kind, in `IdKind::ALL` order; 0 before the first.
    last_values: [u64; IdKind::ALL.len()],
}

impl SequentialIds {
    /// Creates a generator whose counters all start at 1.
    pub fn new() -> SequentialIds {
        SequentialIds::default()
    }
}

impl IdGenerator for SequentialIds {
    fn generate(&mut self, kind: IdKind, is_taken: &dyn Fn(&Id) -> bool) -> Result<Id, IdError> {
        let last_value = &mut self.last_values[kind.position()];
        let mut attempts = 0;
        for value in (*last_value + 1)..=MAX_GENERATED_VALUE {
            attempts += 1;
            let candidate = Id::generated(kind, value);
            if !is_taken(&candidate) {
                *last_value = value;
                return Ok(candidate);
            }
        }
        Err(IdError::Exhausted { attempts })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never_taken(_: &Id) -> bool {
        false
    }

    #[test]
    fn digits_underscores_and_dashes_are_valid() {
        assert_eq!(Id::new("a-Z_9").map(String::from), Ok("a-Z_9".to_string()));
    }

    #[test]
    fn length_is_counted_in_characters() {
        assert_eq!(
            Id::new("é".repeat(65)),
            Err(IdError::TooLong { length: 65 })
        );
    }

    #[test]
    fn length_is_checked_before_characters() {
        assert_eq!(
            Id::new(" ".repeat(65)),
            Err(IdError::TooLong { length: 65 })
        );
    }

    #[test]
    fn the_first_invalid_character_is_reported() {
        assert_eq!(
            Id::new("a.b c"),
            Err(IdError::InvalidCharacter {
                id: "a.b c".to_string(),
                character: '.'
            })
        );
    }

    #[test]
    fn invalid_character_message_escapes_control_characters() {
        let error = Id::new("a\tb").expect_err("a tab is not allowed");

        assert_eq!(
            error.to_string(),
            "The id 'a\tb' contains '\\t'. Ids use only letters A to Z and a to z, digits, - and _."
        );
    }

    #[test]
    fn ids_parse_from_strings() {
        assert_eq!("f-1".parse::<Id>(), Id::new("f-1"));
        assert_eq!(Id::try_from("f 1"), Id::new("f 1"));
    }

    #[test]
    fn ids_display_and_borrow_their_text() {
        let id = Id::new("b-1").expect("valid");

        assert_eq!(id.to_string(), "b-1");
        assert_eq!(<Id as Borrow<str>>::borrow(&id), "b-1");
        assert_eq!(id.as_ref(), "b-1");
    }

    #[test]
    fn generated_ids_are_zero_padded() {
        assert_eq!(
            Id::generated(IdKind::SketchConstraint, 0xab).as_str(),
            "c-0000000000ab"
        );
    }

    #[test]
    fn id_schema_is_a_string_pattern() {
        let schema = schemars::schema_for!(Id);

        assert_eq!(schema.get("type"), Some(&serde_json::json!("string")));
        assert_eq!(schema.get("pattern"), Some(&serde_json::json!(ID_PATTERN)));
        assert_eq!(Id::schema_name(), "Id");
    }

    #[test]
    fn kind_positions_follow_all() {
        for (position, kind) in IdKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.position(), position);
        }
    }

    #[test]
    fn sequential_default_equals_new() {
        assert_eq!(SequentialIds::default(), SequentialIds::new());
    }

    #[test]
    fn sequential_counters_are_independent_per_kind() {
        let mut generator = SequentialIds::new();
        generator
            .generate(IdKind::Feature, &never_taken)
            .expect("an id");

        assert_eq!(
            generator.generate(IdKind::Parameter, &never_taken),
            Ok(Id::generated(IdKind::Parameter, 1))
        );
    }

    #[test]
    fn sequential_counter_moves_past_the_returned_id() {
        let mut generator = SequentialIds::new();
        let taken = Id::generated(IdKind::Body, 1);
        generator
            .generate(IdKind::Body, &|candidate| *candidate == taken)
            .expect("an id");

        assert_eq!(
            generator.generate(IdKind::Body, &never_taken),
            Ok(Id::generated(IdKind::Body, 3))
        );
    }

    #[test]
    fn sequential_generator_reports_exhaustion() {
        let mut generator = SequentialIds::new();
        generator.last_values[IdKind::Feature.position()] = MAX_GENERATED_VALUE - 2;

        let result = generator.generate(IdKind::Feature, &|_| true);

        assert_eq!(result, Err(IdError::Exhausted { attempts: 2 }));
    }

    #[test]
    fn sequential_generator_at_the_last_value_has_nothing_left() {
        let mut generator = SequentialIds::new();
        generator.last_values[IdKind::Feature.position()] = MAX_GENERATED_VALUE;

        let result = generator.generate(IdKind::Feature, &never_taken);

        assert_eq!(result, Err(IdError::Exhausted { attempts: 0 }));
    }

    #[test]
    fn random_ids_have_the_kind_prefix() {
        let generated = RandomIdGenerator::new()
            .generate(IdKind::SketchEntity, &never_taken)
            .expect("the random source is available");

        assert!(generated.as_str().starts_with("e-"));
        assert_eq!(generated.as_str().len(), 14);
    }

    #[test]
    fn too_long_names_the_maximum_length() {
        let error = IdError::TooLong { length: 65 };

        assert!(
            error
                .to_string()
                .contains(&format!("at most {} characters", Id::MAX_LENGTH)),
            "{error}"
        );
    }
}
