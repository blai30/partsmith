//! Deserializing typed params from their JSON map, describing every failure with the field it is
//! about (features.md section 3.2).
//!
//! The deserializer reads `serde_json` values directly and keeps the field path of the value it is
//! reading, so a failure names the deepest field it happened in. Its own error type records what
//! serde reported (a missing field, an unknown variant or another problem) without parsing
//! message text.

use std::fmt;

use serde::de::value::BorrowedStrDeserializer;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess,
    Unexpected, VariantAccess, Visitor,
};
use serde::forward_to_deserialize_any;
use serde_json::{Map, Number, Value};

use crate::error::FeatureError;

/// Deserializes typed params from their JSON map. A failure is `InvalidParams` whose `detail`
/// says what is wrong and where (section 3.2).
pub(crate) fn from_params<T: DeserializeOwned>(
    params: &Map<String, Value>,
) -> Result<T, FeatureError> {
    T::deserialize(Root(params)).map_err(|problem| FeatureError::InvalidParams {
        detail: problem.detail(),
    })
}

/// Where a value is inside the params: a borrowed chain from the value up to the root.
#[derive(Clone, Copy)]
enum Path<'a> {
    /// The params map itself.
    Root,
    /// A key of an object.
    Key(&'a Path<'a>, &'a str),
    /// An index of an array.
    Index(&'a Path<'a>, usize),
}

impl fmt::Display for Path<'_> {
    /// The field path grammar of section 3.2: `name ( "." name | "[" index "]" )*`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Path::Root => Ok(()),
            Path::Key(Path::Root, key) => formatter.write_str(key),
            Path::Key(parent, key) => write!(formatter, "{parent}.{key}"),
            Path::Index(parent, index) => write!(formatter, "{parent}[{index}]"),
        }
    }
}

/// A deserialization failure and the field it happened in.
#[derive(Debug)]
pub(crate) struct Problem {
    /// What went wrong.
    kind: ProblemKind,
    /// The field path of the deepest value being read, or None at the params root.
    path: Option<String>,
}

/// What went wrong while deserializing params.
#[derive(Debug)]
enum ProblemKind {
    /// A required field is missing.
    MissingField(String),
    /// A string names no variant of an enumeration or tag.
    UnknownVariant(String),
    /// Anything else, with serde's message.
    Other(String),
}

impl Problem {
    /// The problem located at `path`, unless a deeper value already located it.
    fn at(mut self, path: &Path<'_>) -> Problem {
        if self.path.is_none() && !matches!(path, Path::Root) {
            self.path = Some(path.to_string());
        }
        self
    }

    /// The `InvalidParams` detail (section 3.2).
    fn detail(&self) -> String {
        match (&self.kind, &self.path) {
            (ProblemKind::MissingField(field), _) => format!("the setting '{field}' is missing"),
            (ProblemKind::UnknownVariant(variant), Some(path)) => {
                format!("'{variant}' is not a valid choice for '{path}'")
            }
            (ProblemKind::UnknownVariant(variant), None) => {
                format!("'{variant}' is not a valid choice")
            }
            (ProblemKind::Other(message), Some(path)) => {
                format!("'{path}' has the wrong form ({message})")
            }
            (ProblemKind::Other(message), None) => message.clone(),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail())
    }
}

impl std::error::Error for Problem {}

impl de::Error for Problem {
    fn custom<T: fmt::Display>(message: T) -> Problem {
        Problem {
            kind: ProblemKind::Other(message.to_string()),
            path: None,
        }
    }

    fn missing_field(field: &'static str) -> Problem {
        Problem {
            kind: ProblemKind::MissingField(field.to_string()),
            path: None,
        }
    }

    fn unknown_variant(variant: &str, _expected: &'static [&'static str]) -> Problem {
        Problem {
            kind: ProblemKind::UnknownVariant(variant.to_string()),
            path: None,
        }
    }
}

/// The params map as a deserializer: always an object.
struct Root<'de>(&'de Map<String, Value>);

impl<'de> Deserializer<'de> for Root<'de> {
    type Error = Problem;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Problem> {
        visit_object(self.0, &Path::Root, visitor)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

/// One JSON value inside the params, with its path.
struct Node<'de, 'p> {
    /// The value.
    value: &'de Value,
    /// Where it is.
    path: Path<'p>,
}

impl<'de> Deserializer<'de> for Node<'de, '_> {
    type Error = Problem;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Problem> {
        let path = self.path;
        let result = match self.value {
            Value::Null => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(*value),
            Value::Number(number) => visit_number(number, visitor),
            Value::String(text) => visitor.visit_borrowed_str(text),
            Value::Array(items) => visit_array(items, &path, visitor),
            Value::Object(entries) => visit_object(entries, &path, visitor),
        };
        result.map_err(|problem| problem.at(&path))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Problem> {
        let path = self.path;
        let result = match self.value {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        };
        result.map_err(|problem| problem.at(&path))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Problem> {
        let path = self.path;
        visitor
            .visit_newtype_struct(self)
            .map_err(|problem| problem.at(&path))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Problem> {
        let path = self.path;
        let result = match self.value {
            Value::String(variant) => {
                visitor.visit_enum(BorrowedStrDeserializer::<Problem>::new(variant))
            }
            Value::Object(entries) if entries.len() == 1 => {
                let (variant, value) = entries
                    .iter()
                    .next()
                    .expect("an object with one entry has a first entry");
                visitor.visit_enum(Variant {
                    name: variant,
                    value,
                    path: &path,
                })
            }
            other => Err(de::Error::invalid_type(
                unexpected(other),
                &"a string or an object with one key",
            )),
        };
        result.map_err(|problem| problem.at(&path))
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Problem> {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        unit unit_struct seq tuple tuple_struct map struct identifier
    }
}

/// Visits a JSON number as the widest exact Rust number.
fn visit_number<'de, V: Visitor<'de>>(number: &Number, visitor: V) -> Result<V::Value, Problem> {
    if let Some(value) = number.as_u64() {
        visitor.visit_u64(value)
    } else if let Some(value) = number.as_i64() {
        visitor.visit_i64(value)
    } else if let Some(value) = number.as_f64() {
        visitor.visit_f64(value)
    } else {
        Err(de::Error::custom(format!(
            "the number {number} is out of range"
        )))
    }
}

/// Visits an array; every item must be consumed.
fn visit_array<'de, V: Visitor<'de>>(
    items: &'de [Value],
    path: &Path<'_>,
    visitor: V,
) -> Result<V::Value, Problem> {
    let mut access = Items {
        items: items.iter().enumerate(),
        path,
    };
    let value = visitor.visit_seq(&mut access)?;
    if access.items.len() > 0 {
        return Err(de::Error::invalid_length(
            items.len(),
            &"fewer elements in the list",
        ));
    }
    Ok(value)
}

/// Visits an object; every entry must be consumed.
fn visit_object<'de, V: Visitor<'de>>(
    entries: &'de Map<String, Value>,
    path: &Path<'_>,
    visitor: V,
) -> Result<V::Value, Problem> {
    let mut access = Entries {
        entries: entries.iter(),
        pending: None,
        path,
    };
    let value = visitor.visit_map(&mut access)?;
    if access.entries.len() > 0 {
        return Err(de::Error::invalid_length(
            entries.len(),
            &"fewer entries in the object",
        ));
    }
    Ok(value)
}

/// The items of an array with their paths.
struct Items<'de, 'p> {
    /// Items not read yet, with their indices.
    items: std::iter::Enumerate<std::slice::Iter<'de, Value>>,
    /// The array's path.
    path: &'p Path<'p>,
}

impl<'de> SeqAccess<'de> for Items<'de, '_> {
    type Error = Problem;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Problem> {
        let Some((index, value)) = self.items.next() else {
            return Ok(None);
        };
        let path = Path::Index(self.path, index);
        seed.deserialize(Node { value, path })
            .map(Some)
            .map_err(|problem| problem.at(&path))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.items.len())
    }
}

/// The entries of an object with their paths.
struct Entries<'de, 'p> {
    /// Entries not read yet.
    entries: serde_json::map::Iter<'de>,
    /// The entry whose key was read and whose value comes next.
    pending: Option<(&'de str, &'de Value)>,
    /// The object's path.
    path: &'p Path<'p>,
}

impl<'de> MapAccess<'de> for Entries<'de, '_> {
    type Error = Problem;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Problem> {
        let Some((key, value)) = self.entries.next() else {
            return Ok(None);
        };
        self.pending = Some((key, value));
        seed.deserialize(BorrowedStrDeserializer::<Problem>::new(key))
            .map(Some)
            .map_err(|problem| problem.at(self.path))
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Problem> {
        let (key, value) = self
            .pending
            .take()
            .ok_or_else(|| de::Error::custom("a value was read before its key"))?;
        let path = Path::Key(self.path, key);
        seed.deserialize(Node { value, path })
            .map_err(|problem| problem.at(&path))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

/// An externally tagged enum value `{"variant": value}`.
struct Variant<'de, 'p> {
    /// The variant name.
    name: &'de str,
    /// Its content.
    value: &'de Value,
    /// The enum value's path.
    path: &'p Path<'p>,
}

impl<'de: 'p, 'p> EnumAccess<'de> for Variant<'de, 'p> {
    type Error = Problem;
    type Variant = VariantContent<'de, 'p>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, VariantContent<'de, 'p>), Problem> {
        let variant = seed.deserialize(BorrowedStrDeserializer::<Problem>::new(self.name))?;
        Ok((
            variant,
            VariantContent {
                value: self.value,
                path: Path::Key(self.path, self.name),
            },
        ))
    }
}

/// The content of an externally tagged enum variant.
struct VariantContent<'de, 'p> {
    /// The content.
    value: &'de Value,
    /// Its path.
    path: Path<'p>,
}

impl<'de> VariantAccess<'de> for VariantContent<'de, '_> {
    type Error = Problem;

    fn unit_variant(self) -> Result<(), Problem> {
        match self.value {
            Value::Null => Ok(()),
            other => Err(de::Error::invalid_type(unexpected(other), &"no content")),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Problem> {
        let path = self.path;
        seed.deserialize(Node {
            value: self.value,
            path,
        })
        .map_err(|problem| problem.at(&path))
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _length: usize,
        visitor: V,
    ) -> Result<V::Value, Problem> {
        Node {
            value: self.value,
            path: self.path,
        }
        .deserialize_seq(visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Problem> {
        Node {
            value: self.value,
            path: self.path,
        }
        .deserialize_map(visitor)
    }
}

/// What a value is, for type errors.
fn unexpected(value: &Value) -> Unexpected<'_> {
    match value {
        Value::Null => Unexpected::Unit,
        Value::Bool(value) => Unexpected::Bool(*value),
        Value::Number(number) => match (number.as_u64(), number.as_i64(), number.as_f64()) {
            (Some(value), _, _) => Unexpected::Unsigned(value),
            (None, Some(value), _) => Unexpected::Signed(value),
            (None, None, Some(value)) => Unexpected::Float(value),
            (None, None, None) => Unexpected::Other("a number"),
        },
        Value::String(text) => Unexpected::Str(text),
        Value::Array(_) => Unexpected::Seq,
        Value::Object(_) => Unexpected::Map,
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Params {
        body: String,
        #[serde(default)]
        size: Vec<u32>,
        #[serde(default)]
        extent: Option<Extent>,
        #[serde(default)]
        shape: Option<Shape>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Extent {
        Distance { expr: String },
        ThroughAll,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    enum Shape {
        Round,
        Square(f64),
        Pair(u8, u8),
        Named { side: f64 },
    }

    fn parse(value: Value) -> Result<Params, String> {
        let Value::Object(map) = value else {
            panic!("params are objects")
        };
        from_params::<Params>(&map).map_err(|error| match error {
            FeatureError::InvalidParams { detail } => detail,
            other => panic!("expected InvalidParams, got {other:?}"),
        })
    }

    #[test]
    fn valid_params_deserialize() {
        let params = parse(json!({
            "body": "b-1",
            "size": [1, 2],
            "extent": {"kind": "distance", "expr": "5"},
            "shape": {"square": 2.5},
            "unknown": {"anything": [1, null]}
        }))
        .expect("the params are valid");

        assert_eq!(params.size, [1, 2]);
        assert_eq!(params.shape, Some(Shape::Square(2.5)));
    }

    #[test]
    fn missing_fields_name_only_the_field() {
        assert_eq!(
            parse(json!({"extent": {"kind": "distance"}, "body": "b-1"})),
            Err("the setting 'expr' is missing".to_string())
        );
        assert_eq!(
            parse(json!({})),
            Err("the setting 'body' is missing".to_string())
        );
    }

    #[test]
    fn unknown_tags_name_the_tag_field() {
        assert_eq!(
            parse(json!({"body": "b-1", "extent": {"kind": "sideways"}})),
            Err("'sideways' is not a valid choice for 'extent.kind'".to_string())
        );
    }

    #[test]
    fn unknown_unit_variants_name_their_field() {
        assert_eq!(
            parse(json!({"body": "b-1", "shape": "oval"})),
            Err("'oval' is not a valid choice for 'shape'".to_string())
        );
    }

    #[test]
    fn wrong_types_name_the_deepest_field() {
        assert_eq!(
            parse(json!({"body": "b-1", "size": [1, "two"]})),
            Err(
                r#"'size[1]' has the wrong form (invalid type: string "two", expected u32)"#
                    .to_string()
            )
        );
    }

    #[test]
    fn externally_tagged_variants_with_content() {
        let pair = parse(json!({"body": "b-1", "shape": {"pair": [1, 2]}})).expect("valid");
        let named = parse(json!({"body": "b-1", "shape": {"named": {"side": 3}}})).expect("valid");
        let round = parse(json!({"body": "b-1", "shape": "round"})).expect("valid");

        assert_eq!(pair.shape, Some(Shape::Pair(1, 2)));
        assert_eq!(named.shape, Some(Shape::Named { side: 3.0 }));
        assert_eq!(round.shape, Some(Shape::Round));
    }

    #[test]
    fn variant_content_errors_name_the_variant() {
        assert_eq!(
            parse(json!({"body": "b-1", "shape": {"named": {"side": "x"}}})),
            Err(
                r#"'shape.named.side' has the wrong form (invalid type: string "x", expected f64)"#
                    .to_string()
            )
        );
    }

    #[test]
    fn enum_values_of_the_wrong_shape_are_rejected() {
        let detail = parse(json!({"body": "b-1", "shape": 3})).expect_err("a number is no shape");

        assert!(
            detail.starts_with("'shape' has the wrong form (invalid type: integer `3`"),
            "got {detail}"
        );
    }

    #[test]
    fn too_many_list_items_are_rejected() {
        let detail = parse(json!({"body": "b-1", "shape": {"pair": [1, 2, 3]}}))
            .expect_err("three values are too many");

        assert!(
            detail.starts_with("'shape.pair' has the wrong form (invalid length 3"),
            "got {detail}"
        );
    }

    #[test]
    fn paths_follow_the_field_path_grammar() {
        let root = Path::Root;
        let points = Path::Key(&root, "points");
        let item = Path::Index(&points, 2);
        let field = Path::Key(&item, "x");

        assert_eq!(field.to_string(), "points[2].x");
        assert_eq!(root.to_string(), "");
    }
}
