//! Element names and the operations on them (naming.md sections 2.2, 3.1, 3.2 and 3.5.1).

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use partsmith_model::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize, Serializer};

use crate::error::NamingError;
use crate::grammar::{self, NameDefect};
use crate::kind::ElementKind;

/// A validated element name such as `f-ext1/end-cap#2` or `edge(f-ext1/end-cap|f-ext1/lateral/e-9e12)`.
/// Ordering, equality and hashing are those of the underlying string (byte order).
///
/// The text is shared, so cloning a name (as naming maps, element infos and resolutions do for
/// every element) does not copy it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct ElementName(Arc<str>);

/// What a generated face was made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generator<'a> {
    /// A sketch reference: an entity id, optionally with a role, such as `e-9e12` or `e-1.center`.
    Sketch(&'a str),
    /// An element of an input shape, such as the edge a fillet face rounds.
    Element(&'a ElementName),
}

/// An instance number of a pattern or mirror copy. `(0, 0)` is the original and is never written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instance {
    /// Index along the first pattern direction (or the only one); 0 to 4294967295.
    pub first: u32,
    /// Index along the second direction of a two-direction pattern; 0 for one-direction patterns, circular patterns and mirrors.
    pub second: u32,
}

impl Instance {
    /// The suffix `@<feature>:<instance>` this instance appends to a face name.
    pub(crate) fn suffix(self, feature: &Id) -> Result<String, NamingError> {
        match (self.first, self.second) {
            (0, 0) => Err(NamingError::InvalidInstance),
            (first, 0) => Ok(format!("@{feature}:{first}")),
            (first, second) => Ok(format!("@{feature}:{first}.{second}")),
        }
    }
}

impl ElementName {
    /// Parses and validates a name against the grammar in section 3.1.
    pub fn parse(text: &str) -> Result<ElementName, NamingError> {
        grammar::validate(text).map_err(|defect| invalid_name(text, &defect))?;
        Ok(ElementName(Arc::from(text)))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Face, edge or vertex, from the name's form.
    pub fn kind(&self) -> ElementKind {
        grammar::list_prefix(self.0.as_bytes()).map_or(ElementKind::Face, |(kind, _)| kind)
    }

    /// True when the name contains a `#` anywhere, so its meaning depends on a geometric order.
    pub fn is_order_dependent(&self) -> bool {
        self.0.contains('#')
    }

    /// The name with every trailing split suffix (`#n`) removed.
    pub fn split_base(&self) -> ElementName {
        let base = split_base_of(&self.0);
        if base.len() == self.0.len() {
            self.clone()
        } else {
            ElementName(Arc::from(base))
        }
    }

    /// True when `self` is `ancestor` followed by one or more split suffixes.
    pub fn is_split_descendant_of(&self, ancestor: &ElementName) -> bool {
        is_split_descendant(&self.0, &ancestor.0)
    }

    /// For an edge or vertex name, its face names in written (sorted) order; empty for a face name.
    pub fn face_terms(&self) -> Vec<ElementName> {
        list_parts(&self.0).map_or_else(Vec::new, |parts| {
            parts
                .terms
                .into_iter()
                .map(|term| ElementName(Arc::from(term)))
                .collect()
        })
    }

    /// The name with one more split suffix `#index` appended (`index >= 1`).
    pub fn with_split(&self, index: u32) -> Result<ElementName, NamingError> {
        ElementName::assembled(format!("{self}#{index}"))
    }

    /// The name of the same element in an instance copy made by a pattern or mirror feature (section 3.5).
    pub fn with_instance(
        &self,
        feature: &Id,
        instance: Instance,
    ) -> Result<ElementName, NamingError> {
        self.with_instance_suffix(&instance.suffix(feature)?)
    }

    /// A new face name `<feature>/<role>[/<generator>]` for a face a feature created.
    pub fn generated(
        feature: &Id,
        role: &str,
        generator: Option<Generator<'_>>,
    ) -> Result<ElementName, NamingError> {
        if !grammar::is_word(role) {
            return Err(NamingError::InvalidRole {
                role: role.to_owned(),
            });
        }
        let text = match generator {
            None => format!("{feature}/{role}"),
            Some(Generator::Sketch(reference)) => {
                if !grammar::is_sketch_reference(reference) {
                    return Err(NamingError::InvalidGenerator {
                        text: reference.to_owned(),
                    });
                }
                format!("{feature}/{role}/{reference}")
            }
            Some(Generator::Element(name)) => format!("{feature}/{role}/{name}"),
        };
        ElementName::composed(text)
    }

    /// The edge name for the given adjacent face names (sorted and deduplicated here).
    pub fn edge(faces: &[ElementName]) -> Result<ElementName, NamingError> {
        ElementName::from_faces(ElementKind::Edge, faces.iter().collect())
    }

    /// The vertex name for the given adjacent face names (sorted and deduplicated here).
    pub fn vertex(faces: &[ElementName]) -> Result<ElementName, NamingError> {
        ElementName::from_faces(ElementKind::Vertex, faces.iter().collect())
    }

    /// The edge or vertex name, as `kind` says, of these face names, sorted and deduplicated here.
    pub(crate) fn from_faces(
        kind: ElementKind,
        mut faces: Vec<&ElementName>,
    ) -> Result<ElementName, NamingError> {
        let prefix =
            grammar::list_prefix_of(kind).expect("only edge and vertex names list face names");
        faces.sort_unstable();
        faces.dedup();
        let length = prefix.len() + faces.iter().map(|face| face.0.len() + 1).sum::<usize>();
        let mut text = String::with_capacity(length);
        text.push_str(prefix);
        for (position, face) in faces.iter().enumerate() {
            if position > 0 {
                text.push('|');
            }
            text.push_str(&face.0);
        }
        text.push(')');
        if faces.is_empty() || faces.iter().any(|face| face.kind() != ElementKind::Face) {
            // Not a list of face names: the grammar reports what is wrong.
            return ElementName::assembled(text);
        }
        ElementName::composed(text)
    }

    /// The name with an instance suffix from `Instance::suffix` applied: appended to a face name,
    /// or to every face term of an edge or vertex name, which are then sorted again.
    pub(crate) fn with_instance_suffix(&self, suffix: &str) -> Result<ElementName, NamingError> {
        let Some(parts) = list_parts(&self.0) else {
            return ElementName::composed(format!("{self}{suffix}"));
        };
        let mut terms: Vec<String> = parts
            .terms
            .iter()
            .map(|term| format!("{term}{suffix}"))
            .collect();
        terms.sort_unstable();
        ElementName::composed(format!(
            "{}{}{}",
            parts.prefix,
            terms.join("|"),
            parts.closing
        ))
    }

    /// The name with every trailing split suffix removed, borrowed from this name.
    pub(crate) fn split_base_str(&self) -> &str {
        split_base_of(&self.0)
    }

    /// Validates a name this crate assembled. Exceeding the length or nesting limit is
    /// `NameTooLong`; any other defect is `InvalidName`.
    fn assembled(text: String) -> Result<ElementName, NamingError> {
        match grammar::validate(&text) {
            Ok(()) => Ok(ElementName(Arc::from(text))),
            Err(defect) if defect.is_limit() => {
                Err(NamingError::NameTooLong { length: text.len() })
            }
            Err(defect) => Err(invalid_name(&text, &defect)),
        }
    }

    /// Wraps a name this crate composed from valid parts in a way the grammar always accepts,
    /// checking only the length and nesting limits a composition can exceed (`NameTooLong`).
    fn composed(text: String) -> Result<ElementName, NamingError> {
        if grammar::check_limits(&text).is_err() {
            return Err(NamingError::NameTooLong { length: text.len() });
        }
        debug_assert_eq!(
            grammar::validate(&text),
            Ok(()),
            "{text} is composed from valid parts"
        );
        Ok(ElementName(Arc::from(text)))
    }
}

/// The `InvalidName` error for `text`.
pub(crate) fn invalid_name(text: &str, defect: &NameDefect) -> NamingError {
    NamingError::InvalidName {
        text: text.to_owned(),
        reason: defect.to_string(),
    }
}

/// `text` with every trailing split suffix (`#` and digits) removed.
fn split_base_of(text: &str) -> &str {
    let mut base = text;
    while let Some((head, number)) = base.rsplit_once('#') {
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            break;
        }
        base = head;
    }
    base
}

/// True when `text` is `ancestor` followed by one or more split suffixes.
fn is_split_descendant(text: &str, ancestor: &str) -> bool {
    text.strip_prefix(ancestor).is_some_and(|suffixes| {
        suffixes.starts_with('#')
            && suffixes
                .bytes()
                .all(|byte| byte == b'#' || byte.is_ascii_digit())
    })
}

/// An edge or vertex name taken apart.
struct ListParts<'a> {
    /// `edge(` or `vertex(`.
    prefix: &'a str,
    /// The face names of the face list.
    terms: Vec<&'a str>,
    /// The closing parenthesis and any split suffixes after it.
    closing: &'a str,
}

/// Splits a valid edge or vertex name into its parts; `None` for a face name.
fn list_parts(text: &str) -> Option<ListParts<'_>> {
    let prefix_length = grammar::list_prefix_length(text)?;
    let mut terms = Vec::new();
    let mut depth = 0_usize;
    let mut start = prefix_length;
    // Delimiters are ASCII, so every slice boundary below is a character boundary.
    for (position, byte) in text.bytes().enumerate().skip(prefix_length) {
        match byte {
            b'(' => depth += 1,
            b')' if depth == 0 => {
                terms.push(&text[start..position]);
                return Some(ListParts {
                    prefix: &text[..prefix_length],
                    terms,
                    closing: &text[position..],
                });
            }
            b')' => depth -= 1,
            b'|' if depth == 0 => {
                terms.push(&text[start..position]);
                start = position + 1;
            }
            _ => {}
        }
    }
    None
}

impl fmt::Display for ElementName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ElementName {
    type Err = NamingError;

    fn from_str(text: &str) -> Result<ElementName, NamingError> {
        ElementName::parse(text)
    }
}

impl TryFrom<String> for ElementName {
    type Error = NamingError;

    fn try_from(text: String) -> Result<ElementName, NamingError> {
        ElementName::parse(&text)
    }
}

impl From<ElementName> for String {
    fn from(name: ElementName) -> String {
        name.0.to_string()
    }
}

impl Serialize for ElementName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl JsonSchema for ElementName {
    fn schema_name() -> Cow<'static, str> {
        "ElementName".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::ElementName").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "minLength": 3,
            "maxLength": grammar::MAX_NAME_LENGTH,
            "pattern": "^[A-Za-z0-9_./()|#@:-]+$"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn id(text: &str) -> Id {
        Id::new(text).expect("the test id is valid")
    }

    #[test]
    fn parse_keeps_the_text() {
        assert_eq!(name("f-a/x#1").as_str(), "f-a/x#1");
    }

    #[test]
    fn parse_error_carries_text_and_reason() {
        assert_eq!(
            ElementName::parse("f-a/X"),
            Err(NamingError::InvalidName {
                text: "f-a/X".to_owned(),
                reason: "a role must start with a lowercase letter".to_owned()
            })
        );
    }

    #[test]
    fn try_from_string_validates() {
        assert!(ElementName::try_from("f-a/x".to_owned()).is_ok());
        assert!(ElementName::try_from("f-a".to_owned()).is_err());
    }

    #[test]
    fn from_str_parses() {
        assert_eq!("f-a/x".parse::<ElementName>(), Ok(name("f-a/x")));
    }

    #[test]
    fn into_string_returns_the_text() {
        assert_eq!(String::from(name("edge(f-a/x)")), "edge(f-a/x)");
    }

    #[test]
    fn display_writes_the_text() {
        assert_eq!(name("vertex(f-a/x)").to_string(), "vertex(f-a/x)");
    }

    #[test]
    fn kind_of_face_name_with_edge_generator_is_face() {
        assert_eq!(name("f-f/fillet/edge(f-a/x)").kind(), ElementKind::Face);
    }

    #[test]
    fn instance_suffix_alone_is_not_order_dependent() {
        assert!(!name("f-a/x@f-p:2").is_order_dependent());
        assert!(name("f-f/fillet/edge(f-a/x#1)").is_order_dependent());
    }

    #[test]
    fn split_base_keeps_inner_splits() {
        assert_eq!(
            name("edge(f-a/x#1)#2#3").split_base(),
            name("edge(f-a/x#1)")
        );
    }

    #[test]
    fn split_base_of_stops_at_instance_suffix() {
        assert_eq!(split_base_of("f-a/x#1@f-p:2"), "f-a/x#1@f-p:2");
    }

    #[test]
    fn split_base_str_borrows_the_base() {
        assert_eq!(name("f-a/x#4").split_base_str(), "f-a/x");
    }

    #[test]
    fn split_descendant_needs_a_suffix_boundary() {
        assert!(!is_split_descendant("f-a/x#12", "f-a/x#1"));
        assert!(is_split_descendant("f-a/x#1#2", "f-a/x#1"));
        assert!(!is_split_descendant("f-a/x-y", "f-a/x"));
    }

    #[test]
    fn edge_name_is_split_descendant_of_its_base() {
        assert!(name("edge(f-a/x)#1").is_split_descendant_of(&name("edge(f-a/x)")));
    }

    #[test]
    fn face_terms_of_vertex_name() {
        assert_eq!(
            name("vertex(f-a/x|f-b/y|f-c/z)#2").face_terms(),
            vec![name("f-a/x"), name("f-b/y"), name("f-c/z")]
        );
    }

    #[test]
    fn face_terms_keep_nested_lists_whole() {
        assert_eq!(
            name("edge(f-f/fillet/edge(f-a/x|f-b/y)#1|f-g/z)").face_terms(),
            vec![name("f-f/fillet/edge(f-a/x|f-b/y)#1"), name("f-g/z")]
        );
    }

    #[test]
    fn with_split_on_edge_name() {
        assert_eq!(
            name("edge(f-a/x|f-b/y)").with_split(3),
            Ok(name("edge(f-a/x|f-b/y)#3"))
        );
    }

    #[test]
    fn with_split_over_length_limit_is_name_too_long() {
        let long = name(&format!("f-a/{}", "x".repeat(16380)));

        assert_eq!(
            long.with_split(10),
            Err(NamingError::NameTooLong { length: 16387 })
        );
    }

    #[test]
    fn instance_suffix_formats() {
        let feature = id("f-p");

        assert_eq!(
            Instance {
                first: 3,
                second: 0
            }
            .suffix(&feature),
            Ok("@f-p:3".to_owned())
        );
        assert_eq!(
            Instance {
                first: 0,
                second: 2
            }
            .suffix(&feature),
            Ok("@f-p:0.2".to_owned())
        );
        assert_eq!(
            Instance {
                first: 0,
                second: 0
            }
            .suffix(&feature),
            Err(NamingError::InvalidInstance)
        );
    }

    #[test]
    fn with_instance_keeps_vertex_split_suffix() {
        let renamed = name("vertex(f-a/x|f-b/y)#2").with_instance(
            &id("f-m"),
            Instance {
                first: 1,
                second: 0,
            },
        );

        assert_eq!(renamed, Ok(name("vertex(f-a/x@f-m:1|f-b/y@f-m:1)#2")));
    }

    #[test]
    fn with_instance_on_sketch_role_name() {
        let renamed = name("f-h/bore/e-1.center").with_instance(
            &id("f-p"),
            Instance {
                first: 4,
                second: 1,
            },
        );

        assert_eq!(renamed, Ok(name("f-h/bore/e-1.center@f-p:4.1")));
    }

    #[test]
    fn generated_with_face_generator() {
        let source = name("f-ext1/lateral/e-3");

        let generated =
            ElementName::generated(&id("f-sh1"), "inner", Some(Generator::Element(&source)));

        assert_eq!(generated, Ok(name("f-sh1/inner/f-ext1/lateral/e-3")));
    }

    #[test]
    fn generated_rejects_empty_role() {
        assert_eq!(
            ElementName::generated(&id("f-a"), "", None),
            Err(NamingError::InvalidRole {
                role: String::new()
            })
        );
    }

    #[test]
    fn generated_rejects_sketch_reference_with_uppercase_role() {
        assert_eq!(
            ElementName::generated(&id("f-a"), "bore", Some(Generator::Sketch("e-1.Center"))),
            Err(NamingError::InvalidGenerator {
                text: "e-1.Center".to_owned()
            })
        );
    }

    #[test]
    fn generated_over_nesting_limit_is_name_too_long() {
        let mut nested = name("vertex(f-a/x)");
        for _ in 1..grammar::MAX_NAME_DEPTH {
            let face = ElementName::generated(&id("f-a"), "r", Some(Generator::Element(&nested)))
                .expect("within the limits");
            nested = ElementName::vertex(&[face]).expect("within the limits");
        }
        let face = ElementName::generated(&id("f-a"), "r", Some(Generator::Element(&nested)))
            .expect("a face name adds no nesting");

        let result = ElementName::edge(&[face]);

        assert!(
            matches!(result, Err(NamingError::NameTooLong { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn edge_of_single_face_is_seam_name() {
        assert_eq!(
            ElementName::edge(&[name("f-c/side")]),
            Ok(name("edge(f-c/side)"))
        );
    }

    #[test]
    fn edge_of_no_faces_is_invalid() {
        assert!(matches!(
            ElementName::edge(&[]),
            Err(NamingError::InvalidName { .. })
        ));
    }

    #[test]
    fn edge_of_an_edge_name_is_invalid() {
        assert!(matches!(
            ElementName::edge(&[name("edge(f-a/x)")]),
            Err(NamingError::InvalidName { .. })
        ));
    }

    #[test]
    fn serializes_as_plain_string() {
        let json = serde_json::to_string(&name("f-a/x@f-p:1")).expect("a name serializes");

        assert_eq!(json, "\"f-a/x@f-p:1\"");
    }

    #[test]
    fn json_schema_is_a_bounded_string() {
        let schema = schemars::schema_for!(ElementName);
        let value = schema.as_value();

        assert_eq!(value["type"], "string");
        assert_eq!(value["minLength"], 3);
        assert_eq!(value["maxLength"], 16384);
        assert_eq!(value["pattern"], "^[A-Za-z0-9_./()|#@:-]+$");
    }
}
