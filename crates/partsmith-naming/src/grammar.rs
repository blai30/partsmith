//! Validation of element names against the grammar of naming.md section 3.1.
//!
//! The validator is linear in the name length. It walks a chain of nested face-name generators
//! iteratively and recurses only per `edge(` or `vertex(` level, which is bounded by
//! [`MAX_NAME_DEPTH`].

use std::fmt;

use partsmith_model::Id;

use crate::kind::ElementKind;

/// Longest valid element name, in bytes.
pub(crate) const MAX_NAME_LENGTH: usize = 16384;

/// Deepest valid nesting of `edge(` and `vertex(` groups.
pub(crate) const MAX_NAME_DEPTH: usize = 32;

/// The kinds of names that list faces, with the text each starts with.
const LIST_PREFIXES: [(ElementKind, &str); 2] = [
    (ElementKind::Edge, "edge("),
    (ElementKind::Vertex, "vertex("),
];

/// Most digits a number in a name can have: `u32::MAX` has 10.
const MAX_NUMBER_DIGITS: usize = 10;

/// Why a text is not a valid element name. Displays as the `reason` of `NamingError::InvalidName`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NameDefect {
    /// The text is empty.
    Empty,
    /// The text is longer than [`MAX_NAME_LENGTH`].
    TooLong,
    /// The text nests deeper than [`MAX_NAME_DEPTH`].
    TooDeep,
    /// A character the grammar does not allow at this position.
    UnexpectedCharacter {
        /// The character.
        character: char,
        /// Its byte offset.
        position: usize,
    },
    /// The text ends where the grammar needs more.
    EndsEarly,
    /// A feature or entity id is empty or longer than 64 characters.
    InvalidId,
    /// A role is missing or does not start with a lowercase letter.
    InvalidRole,
    /// A split number is missing, zero or has a leading zero.
    InvalidSplit,
    /// An instance number is missing, zero where not allowed, or has a leading zero.
    InvalidInstance,
    /// A number does not fit in `u32`.
    NumberTooLarge,
    /// The face names of an edge or vertex name are not strictly ascending.
    UnsortedFaces,
    /// A parenthesis is not closed or closes nothing.
    UnbalancedParentheses,
    /// A valid name of another kind than this kind, which the context needs.
    WrongKind(ElementKind),
}

impl NameDefect {
    /// True for the defects that only the length and nesting limits cause.
    pub(crate) fn is_limit(&self) -> bool {
        matches!(self, NameDefect::TooLong | NameDefect::TooDeep)
    }
}

impl fmt::Display for NameDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameDefect::Empty => formatter.write_str("it is empty"),
            NameDefect::TooLong => {
                write!(formatter, "it is longer than {MAX_NAME_LENGTH} bytes")
            }
            NameDefect::TooDeep => write!(formatter, "it nests more than {MAX_NAME_DEPTH} levels"),
            NameDefect::UnexpectedCharacter {
                character,
                position,
            } => write!(
                formatter,
                "unexpected character '{character}' at byte {position}"
            ),
            NameDefect::EndsEarly => formatter.write_str("it ends too early"),
            NameDefect::InvalidId => write!(
                formatter,
                "a feature id must be 1 to {} letters, digits, '_' or '-'",
                Id::MAX_LENGTH
            ),
            NameDefect::InvalidRole => {
                formatter.write_str("a role must start with a lowercase letter")
            }
            NameDefect::InvalidSplit => formatter.write_str("split numbers start at 1"),
            NameDefect::InvalidInstance => formatter.write_str(
                "instance numbers are n, n.m or 0.m, with n and m of 1 or more and no leading zeros",
            ),
            NameDefect::NumberTooLarge => formatter.write_str("a number is too large"),
            NameDefect::UnsortedFaces => {
                formatter.write_str("edge and vertex face lists must be sorted and distinct")
            }
            NameDefect::UnbalancedParentheses => formatter.write_str("unbalanced parentheses"),
            NameDefect::WrongKind(kind) => formatter.write_str(match kind {
                ElementKind::Face => "it is not a face name",
                ElementKind::Edge => "it is not an edge name",
                ElementKind::Vertex => "it is not a vertex name",
            }),
        }
    }
}

/// Why a number in a name is invalid.
enum NumberDefect {
    /// No digit, a zero, or a leading zero.
    NotPositive,
    /// More than `u32::MAX`.
    TooLarge,
}

/// Checks `text` against the element name grammar.
pub(crate) fn validate(text: &str) -> Result<(), NameDefect> {
    if text.is_empty() {
        return Err(NameDefect::Empty);
    }
    if text.len() > MAX_NAME_LENGTH {
        return Err(NameDefect::TooLong);
    }
    let mut parser = Parser::new(text);
    parser.element_name()?;
    match parser.peek() {
        None => Ok(()),
        Some(b')') => Err(NameDefect::UnbalancedParentheses),
        Some(_) => Err(parser.unexpected()),
    }
}

/// Checks only the length and nesting limits, for a name composed from valid parts.
pub(crate) fn check_limits(text: &str) -> Result<(), NameDefect> {
    if text.len() > MAX_NAME_LENGTH {
        return Err(NameDefect::TooLong);
    }
    let mut depth = 0_usize;
    for byte in text.bytes() {
        match byte {
            b'(' => {
                depth += 1;
                if depth > MAX_NAME_DEPTH {
                    return Err(NameDefect::TooDeep);
                }
            }
            b')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// True when `text` matches `word`: a lowercase letter, then lowercase letters, digits and hyphens.
pub(crate) fn is_word(text: &str) -> bool {
    let mut parser = Parser::new(text);
    parser.word().is_ok() && parser.at_end()
}

/// True when `text` matches `sketch-ref`: an id, optionally followed by a dot and a word.
pub(crate) fn is_sketch_reference(text: &str) -> bool {
    let mut parser = Parser::new(text);
    parser.sketch_reference().is_ok() && parser.at_end()
}

/// The kind and prefix of the edge or vertex name `text` starts with; `None` for a face name.
pub(crate) fn list_prefix(text: &[u8]) -> Option<(ElementKind, &'static str)> {
    LIST_PREFIXES
        .into_iter()
        .find(|(_, prefix)| text.starts_with(prefix.as_bytes()))
}

/// The prefix of edge or vertex names; `None` for faces, whose names list no faces.
pub(crate) fn list_prefix_of(kind: ElementKind) -> Option<&'static str> {
    LIST_PREFIXES
        .into_iter()
        .find_map(|(listed, prefix)| (listed == kind).then_some(prefix))
}

/// Number of bytes before the face list of an edge or vertex name; `None` for a face name.
pub(crate) fn list_prefix_length(text: &str) -> Option<usize> {
    list_prefix(text.as_bytes()).map(|(_, prefix)| prefix.len())
}

/// A cursor over the bytes of a candidate name.
struct Parser<'a> {
    /// The whole text.
    text: &'a str,
    /// Byte offset of the next unread byte. Only ASCII bytes are ever consumed, so it is always a
    /// character boundary.
    position: usize,
    /// Number of currently open `edge(` and `vertex(` groups.
    depth: usize,
}

impl<'a> Parser<'a> {
    /// A parser at the start of `text`.
    fn new(text: &'a str) -> Parser<'a> {
        Parser {
            text,
            position: 0,
            depth: 0,
        }
    }

    /// The unread bytes.
    fn rest(&self) -> &'a [u8] {
        self.text
            .as_bytes()
            .get(self.position..)
            .unwrap_or_default()
    }

    /// The next unread byte.
    fn peek(&self) -> Option<u8> {
        self.rest().first().copied()
    }

    /// True when every byte has been read.
    fn at_end(&self) -> bool {
        self.rest().is_empty()
    }

    /// Consumes `expected` if it is the next byte.
    fn eat(&mut self, expected: u8) -> bool {
        let found = self.peek() == Some(expected);
        if found {
            self.position += 1;
        }
        found
    }

    /// Consumes `expected` or fails.
    fn expect(&mut self, expected: u8) -> Result<(), NameDefect> {
        if self.eat(expected) {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    /// Consumes bytes while `accept` holds and returns how many it consumed.
    fn skip_while(&mut self, accept: impl Fn(u8) -> bool) -> usize {
        let count = self.rest().iter().take_while(|byte| accept(**byte)).count();
        self.position += count;
        count
    }

    /// The defect for the next character, which the grammar does not allow here.
    fn unexpected(&self) -> NameDefect {
        let next = self
            .text
            .get(self.position..)
            .and_then(|rest| rest.chars().next());
        match next {
            Some(character) => NameDefect::UnexpectedCharacter {
                character,
                position: self.position,
            },
            None if self.depth > 0 => NameDefect::UnbalancedParentheses,
            None => NameDefect::EndsEarly,
        }
    }

    /// True when an edge or vertex name starts here.
    fn at_list_name(&self) -> bool {
        list_prefix(self.rest()).is_some()
    }

    /// `element-name` with its trailing suffixes.
    fn element_name(&mut self) -> Result<(), NameDefect> {
        if self.at_list_name() {
            self.list_name()?;
            while self.peek() == Some(b'#') {
                self.split()?;
            }
            Ok(())
        } else {
            self.face_name()
        }
    }

    /// `face-name`, walking the generator chain iteratively.
    fn face_name(&mut self) -> Result<(), NameDefect> {
        self.id()?;
        loop {
            self.expect(b'/')?;
            self.word()?;
            if !self.eat(b'/') {
                break;
            }
            if self.at_list_name() {
                self.list_name()?;
                break;
            }
            // An id followed by `/` starts a nested face name; otherwise it is a sketch reference.
            self.id()?;
            if self.peek() == Some(b'/') {
                continue;
            }
            if self.eat(b'.') {
                self.word()?;
            }
            break;
        }
        while let Some(marker) = self.peek() {
            match marker {
                b'#' => self.split()?,
                b'@' => self.instance()?,
                _ => break,
            }
        }
        Ok(())
    }

    /// `"edge(" face-list ")"` or `"vertex(" face-list ")"`, checking that the faces ascend.
    fn list_name(&mut self) -> Result<(), NameDefect> {
        let Some((_, prefix)) = list_prefix(self.rest()) else {
            return Err(self.unexpected());
        };
        self.position += prefix.len();
        self.depth += 1;
        if self.depth > MAX_NAME_DEPTH {
            return Err(NameDefect::TooDeep);
        }
        let bytes = self.text.as_bytes();
        let mut previous: Option<&[u8]> = None;
        loop {
            let start = self.position;
            self.face_name()?;
            let term = bytes.get(start..self.position).unwrap_or_default();
            if previous.is_some_and(|previous| previous >= term) {
                return Err(NameDefect::UnsortedFaces);
            }
            previous = Some(term);
            if self.eat(b')') {
                self.depth -= 1;
                return Ok(());
            }
            self.expect(b'|')?;
        }
    }

    /// `id`: 1 to 64 letters, digits, `_` or `-`.
    fn id(&mut self) -> Result<(), NameDefect> {
        let length =
            self.skip_while(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        if (1..=Id::MAX_LENGTH).contains(&length) {
            Ok(())
        } else {
            Err(NameDefect::InvalidId)
        }
    }

    /// `word`: a lowercase letter, then lowercase letters, digits and hyphens.
    fn word(&mut self) -> Result<(), NameDefect> {
        if !self.peek().is_some_and(|byte| byte.is_ascii_lowercase()) {
            return Err(NameDefect::InvalidRole);
        }
        self.skip_while(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        Ok(())
    }

    /// `sketch-ref`: an id, optionally followed by a dot and a word.
    fn sketch_reference(&mut self) -> Result<(), NameDefect> {
        self.id()?;
        if self.eat(b'.') {
            self.word()?;
        }
        Ok(())
    }

    /// `split`: `#` and a positive number.
    fn split(&mut self) -> Result<(), NameDefect> {
        self.expect(b'#')?;
        self.positive().map_err(|defect| match defect {
            NumberDefect::NotPositive => NameDefect::InvalidSplit,
            NumberDefect::TooLarge => NameDefect::NumberTooLarge,
        })
    }

    /// `instance`: `@`, a feature id, `:` and an instance number (`n`, `n.m` or `0.m`).
    fn instance(&mut self) -> Result<(), NameDefect> {
        let instance_defect = |defect| match defect {
            NumberDefect::NotPositive => NameDefect::InvalidInstance,
            NumberDefect::TooLarge => NameDefect::NumberTooLarge,
        };
        self.expect(b'@')?;
        self.id()?;
        self.expect(b':')?;
        if self.rest().starts_with(b"0.") {
            self.position += 2;
            return self.positive().map_err(instance_defect);
        }
        self.positive().map_err(instance_defect)?;
        if self.eat(b'.') {
            self.positive().map_err(instance_defect)?;
        }
        Ok(())
    }

    /// `positive`: 1 to 4294967295 without a leading zero.
    fn positive(&mut self) -> Result<(), NumberDefect> {
        let start = self.position;
        let length = self.skip_while(|byte| byte.is_ascii_digit());
        let digits = self.text.get(start..self.position).unwrap_or_default();
        if length == 0 || digits.starts_with('0') {
            return Err(NumberDefect::NotPositive);
        }
        let fits = length <= MAX_NUMBER_DIGITS && digits.parse::<u32>().is_ok();
        if fits {
            Ok(())
        } else {
            Err(NumberDefect::TooLarge)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_face_name() {
        assert_eq!(validate("f-ext1/end-cap"), Ok(()));
    }

    #[test]
    fn accepts_face_name_with_sketch_generator_and_role() {
        assert_eq!(validate("f-hole1/bore/e-1.center"), Ok(()));
    }

    #[test]
    fn accepts_chain_of_face_name_generators() {
        assert_eq!(
            validate("f-sh1/inner/f-fil1/fillet/f-ext1/lateral/e-3#2"),
            Ok(())
        );
    }

    #[test]
    fn accepts_edge_name_generator_with_suffixes() {
        assert_eq!(validate("f-fil1/fillet/edge(f-a/x|f-b/y)#2@f-p:1"), Ok(()));
    }

    #[test]
    fn accepts_feature_named_edge() {
        assert_eq!(validate("edge/x"), Ok(()));
        assert_eq!(validate("f-a/x/edge"), Ok(()));
        assert_eq!(validate("f-a/x/vertex.start"), Ok(()));
    }

    #[test]
    fn accepts_instance_numbers() {
        assert_eq!(validate("f-a/x@f-p:1"), Ok(()));
        assert_eq!(validate("f-a/x@f-p:0.1"), Ok(()));
        assert_eq!(validate("f-a/x@f-p:4294967295.4294967295"), Ok(()));
    }

    #[test]
    fn rejects_empty_text() {
        assert_eq!(validate(""), Err(NameDefect::Empty));
    }

    #[test]
    fn rejects_text_over_length_limit() {
        let text = format!("f-a/{}", "x".repeat(MAX_NAME_LENGTH));

        assert_eq!(validate(&text), Err(NameDefect::TooLong));
    }

    #[test]
    fn rejects_feature_id_alone() {
        assert_eq!(validate("f-ext1"), Err(NameDefect::EndsEarly));
    }

    #[test]
    fn rejects_missing_role() {
        assert_eq!(validate("f-ext1/"), Err(NameDefect::InvalidRole));
    }

    #[test]
    fn rejects_uppercase_role() {
        assert_eq!(validate("f-ext1/End-cap"), Err(NameDefect::InvalidRole));
    }

    #[test]
    fn rejects_uppercase_sketch_role() {
        assert_eq!(validate("f-a/x/e-1.Center"), Err(NameDefect::InvalidRole));
    }

    #[test]
    fn rejects_space_with_its_position() {
        assert_eq!(
            validate("f ext1/end-cap"),
            Err(NameDefect::UnexpectedCharacter {
                character: ' ',
                position: 1
            })
        );
    }

    #[test]
    fn reports_non_ascii_character() {
        assert_eq!(
            validate("f-a/x\u{e9}"),
            Err(NameDefect::UnexpectedCharacter {
                character: '\u{e9}',
                position: 5
            })
        );
    }

    #[test]
    fn rejects_long_feature_id() {
        let text = format!("{}/x", "a".repeat(65));

        assert_eq!(validate(&text), Err(NameDefect::InvalidId));
    }

    #[test]
    fn accepts_feature_id_of_64_characters() {
        let text = format!("{}/x", "a".repeat(64));

        assert_eq!(validate(&text), Ok(()));
    }

    #[test]
    fn rejects_zero_and_leading_zero_splits() {
        assert_eq!(validate("f-a/x#0"), Err(NameDefect::InvalidSplit));
        assert_eq!(validate("f-a/x#01"), Err(NameDefect::InvalidSplit));
        assert_eq!(validate("f-a/x#"), Err(NameDefect::InvalidSplit));
    }

    #[test]
    fn accepts_largest_split() {
        assert_eq!(validate("f-a/x#4294967295"), Ok(()));
    }

    #[test]
    fn rejects_split_over_u32() {
        assert_eq!(
            validate("f-a/x#4294967296"),
            Err(NameDefect::NumberTooLarge)
        );
        assert_eq!(
            validate("f-a/x#99999999999"),
            Err(NameDefect::NumberTooLarge)
        );
    }

    #[test]
    fn rejects_invalid_instance_numbers() {
        for text in [
            "f-a/x@f-p:0",
            "f-a/x@f-p:01",
            "f-a/x@f-p:0.0",
            "f-a/x@f-p:1.0",
            "f-a/x@f-p:",
        ] {
            assert_eq!(validate(text), Err(NameDefect::InvalidInstance), "{text}");
        }
    }

    #[test]
    fn instance_reason_names_every_valid_form() {
        assert_eq!(
            NameDefect::InvalidInstance.to_string(),
            "instance numbers are n, n.m or 0.m, with n and m of 1 or more and no leading zeros"
        );
    }

    #[test]
    fn rejects_instance_without_colon() {
        assert_eq!(validate("f-a/x@f-p"), Err(NameDefect::EndsEarly));
    }

    #[test]
    fn rejects_empty_face_list() {
        assert_eq!(validate("edge()"), Err(NameDefect::InvalidId));
    }

    #[test]
    fn rejects_duplicate_and_unsorted_faces() {
        assert_eq!(
            validate("edge(f-a/x|f-a/x)"),
            Err(NameDefect::UnsortedFaces)
        );
        assert_eq!(
            validate("edge(f-b/x|f-a/x)"),
            Err(NameDefect::UnsortedFaces)
        );
    }

    #[test]
    fn rejects_unclosed_list() {
        assert_eq!(
            validate("edge(f-a/x"),
            Err(NameDefect::UnbalancedParentheses)
        );
        assert_eq!(
            validate("vertex(f-a"),
            Err(NameDefect::UnbalancedParentheses)
        );
    }

    #[test]
    fn rejects_stray_closing_parenthesis() {
        assert_eq!(validate("f-a/x)"), Err(NameDefect::UnbalancedParentheses));
    }

    #[test]
    fn rejects_instance_suffix_on_edge_name() {
        assert_eq!(
            validate("edge(f-a/x)@f-p:1"),
            Err(NameDefect::UnexpectedCharacter {
                character: '@',
                position: 11
            })
        );
    }

    #[test]
    fn rejects_edge_name_inside_face_list() {
        assert_eq!(
            validate("edge(edge(f-a/x))"),
            Err(NameDefect::UnexpectedCharacter {
                character: '(',
                position: 9
            })
        );
    }

    #[test]
    fn nesting_is_limited_to_32_levels() {
        let mut text = String::from("vertex(f-a/x)");
        for _ in 1..MAX_NAME_DEPTH {
            text = format!("vertex(f-a/r/{text})");
        }
        assert_eq!(validate(&text), Ok(()));

        let deeper = format!("vertex(f-a/r/{text})");
        assert_eq!(validate(&deeper), Err(NameDefect::TooDeep));
    }

    #[test]
    fn sorted_check_compares_whole_terms() {
        assert_eq!(validate("edge(f-a/x|f-a/x#1)"), Ok(()));
        assert_eq!(
            validate("edge(f-a/x#1|f-a/x)"),
            Err(NameDefect::UnsortedFaces)
        );
    }

    #[test]
    fn check_limits_accepts_names_within_limits() {
        assert_eq!(check_limits("vertex(f-a/r/edge(f-a/x))"), Ok(()));
    }

    #[test]
    fn check_limits_rejects_long_and_deep_names() {
        let long = "x".repeat(MAX_NAME_LENGTH + 1);
        let deep = format!(
            "{}f-a/x{}",
            "edge(f-a/r/".repeat(MAX_NAME_DEPTH + 1),
            ")".repeat(MAX_NAME_DEPTH + 1)
        );

        assert_eq!(check_limits(&long), Err(NameDefect::TooLong));
        assert_eq!(check_limits(&deep), Err(NameDefect::TooDeep));
    }

    #[test]
    fn limit_defects_are_length_and_depth() {
        assert!(NameDefect::TooLong.is_limit());
        assert!(NameDefect::TooDeep.is_limit());
        assert!(!NameDefect::InvalidRole.is_limit());
    }

    #[test]
    fn word_accepts_lowercase_words_only() {
        assert!(is_word("counterbore-floor"));
        assert!(is_word("a1"));
        assert!(!is_word("1a"));
        assert!(!is_word("Lateral"));
        assert!(!is_word(""));
        assert!(!is_word("end cap"));
    }

    #[test]
    fn sketch_reference_accepts_id_with_optional_role() {
        assert!(is_sketch_reference("e-9e12"));
        assert!(is_sketch_reference("e-1.center"));
        assert!(!is_sketch_reference("e 1"));
        assert!(!is_sketch_reference("e-1."));
        assert!(!is_sketch_reference(""));
    }

    #[test]
    fn list_prefix_names_the_kind_of_list_names() {
        assert_eq!(
            list_prefix(b"edge(f-a/x)"),
            Some((ElementKind::Edge, "edge("))
        );
        assert_eq!(
            list_prefix(b"vertex(f-a/x)"),
            Some((ElementKind::Vertex, "vertex("))
        );
        assert_eq!(list_prefix(b"edge/x"), None);
    }

    #[test]
    fn only_edges_and_vertices_have_a_list_prefix() {
        assert_eq!(list_prefix_of(ElementKind::Edge), Some("edge("));
        assert_eq!(list_prefix_of(ElementKind::Vertex), Some("vertex("));
        assert_eq!(list_prefix_of(ElementKind::Face), None);
    }

    #[test]
    fn list_prefix_length_detects_edge_and_vertex_names() {
        assert_eq!(list_prefix_length("edge(f-a/x)"), Some(5));
        assert_eq!(list_prefix_length("vertex(f-a/x)"), Some(7));
        assert_eq!(list_prefix_length("edge/x"), None);
    }

    #[test]
    fn reasons_match_the_spec_texts() {
        assert_eq!(
            NameDefect::TooLong.to_string(),
            "it is longer than 16384 bytes"
        );
        assert_eq!(
            NameDefect::TooDeep.to_string(),
            "it nests more than 32 levels"
        );
        assert_eq!(
            NameDefect::UnexpectedCharacter {
                character: ' ',
                position: 1
            }
            .to_string(),
            "unexpected character ' ' at byte 1"
        );
        assert_eq!(
            NameDefect::InvalidId.to_string(),
            "a feature id must be 1 to 64 letters, digits, '_' or '-'"
        );
    }

    #[test]
    fn invalid_id_message_names_the_id_length_limit() {
        let message = NameDefect::InvalidId.to_string();

        assert!(
            message.contains(&format!("1 to {} letters", Id::MAX_LENGTH)),
            "{message}"
        );
    }
}
