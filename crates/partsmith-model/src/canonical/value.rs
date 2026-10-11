//! Serializing any value to normalized JSON, rejecting NaN and infinity with their location.

use serde::Serialize;
use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use serde_json::Value;

use super::{CanonicalError, normalize_numbers};
use crate::path::JsonPath;

/// Serializes `value` to JSON, rejecting NaN and infinity, with numbers normalized.
///
/// `serde_json` writes non-finite numbers as `null`, so a first pass walks the value to find
/// them with their path; the second pass is `serde_json::to_value`.
pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, CanonicalError> {
    value
        .serialize(FiniteCheck {
            path: &JsonPath::ROOT,
        })
        .map_err(|problem| match problem {
            Problem::NonFinite { path } => CanonicalError::NonFiniteNumber { path },
            Problem::Custom { message } => CanonicalError::Serialize { message },
        })?;
    let mut json = serde_json::to_value(value).map_err(|error| CanonicalError::Serialize {
        message: error.to_string(),
    })?;
    normalize_numbers(&mut json);
    Ok(json)
}

/// What the finiteness check found.
#[derive(Debug, thiserror::Error)]
enum Problem {
    /// A non-finite number at `path`.
    #[error("{path}: the number is not finite")]
    NonFinite {
        /// Where the number is, or `value` for the root.
        path: String,
    },
    /// The value's `Serialize` implementation reported an error.
    #[error("{message}")]
    Custom {
        /// Its message.
        message: String,
    },
}

impl ser::Error for Problem {
    fn custom<T: std::fmt::Display>(message: T) -> Problem {
        Problem::Custom {
            message: message.to_string(),
        }
    }
}

/// A serializer that produces nothing and fails at the first non-finite number.
struct FiniteCheck<'a> {
    /// Where the value being serialized is.
    path: &'a JsonPath<'a>,
}

impl FiniteCheck<'_> {
    /// Accepts `value` when it is finite.
    fn number(&self, value: f64) -> Result<(), Problem> {
        if value.is_finite() {
            return Ok(());
        }
        let path = if self.path.is_root() {
            "value".to_string()
        } else {
            self.path.to_string()
        };
        Err(Problem::NonFinite { path })
    }
}

impl<'a> Serializer for FiniteCheck<'a> {
    type Ok = ();
    type Error = Problem;
    type SerializeSeq = Elements<'a>;
    type SerializeTuple = Elements<'a>;
    type SerializeTupleStruct = Elements<'a>;
    type SerializeTupleVariant = Elements<'a>;
    type SerializeMap = Members<'a>;
    type SerializeStruct = Fields<'a>;
    type SerializeStructVariant = Fields<'a>;

    fn serialize_bool(self, _value: bool) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_i8(self, _value: i8) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_i16(self, _value: i16) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_i32(self, _value: i32) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_i64(self, _value: i64) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_i128(self, _value: i128) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_u8(self, _value: u8) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_u16(self, _value: u16) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_u32(self, _value: u32) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_u64(self, _value: u64) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_u128(self, _value: u128) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_f32(self, value: f32) -> Result<(), Problem> {
        self.number(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Result<(), Problem> {
        self.number(value)
    }

    fn serialize_char(self, _value: char) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_str(self, _value: &str) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Problem> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<(), Problem> {
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Problem> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Problem> {
        // Externally tagged: `{"variant": value}`.
        let path = self.path.key(variant);
        value.serialize(FiniteCheck { path: &path })
    }

    fn serialize_seq(self, _length: Option<usize>) -> Result<Elements<'a>, Problem> {
        Ok(Elements::new(*self.path))
    }

    fn serialize_tuple(self, _length: usize) -> Result<Elements<'a>, Problem> {
        Ok(Elements::new(*self.path))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Elements<'a>, Problem> {
        Ok(Elements::new(*self.path))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _length: usize,
    ) -> Result<Elements<'a>, Problem> {
        Ok(Elements::new(self.path.key(variant)))
    }

    fn serialize_map(self, _length: Option<usize>) -> Result<Members<'a>, Problem> {
        Ok(Members {
            path: *self.path,
            key: String::new(),
        })
    }

    fn serialize_struct(self, _name: &'static str, _length: usize) -> Result<Fields<'a>, Problem> {
        Ok(Fields { path: *self.path })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _length: usize,
    ) -> Result<Fields<'a>, Problem> {
        Ok(Fields {
            path: self.path.key(variant),
        })
    }
}

/// Elements of a sequence, tuple or tuple variant.
struct Elements<'a> {
    /// Where the container is.
    path: JsonPath<'a>,
    /// Position of the next element.
    next: usize,
}

impl<'a> Elements<'a> {
    /// The elements of the container at `path`.
    fn new(path: JsonPath<'a>) -> Elements<'a> {
        Elements { path, next: 0 }
    }

    /// Checks the next element.
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Problem> {
        let path = self.path.index(self.next);
        self.next += 1;
        value.serialize(FiniteCheck { path: &path })
    }
}

impl SerializeSeq for Elements<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Problem> {
        self.element(value)
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

impl SerializeTuple for Elements<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Problem> {
        self.element(value)
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

impl SerializeTupleStruct for Elements<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Problem> {
        self.element(value)
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

impl SerializeTupleVariant for Elements<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Problem> {
        self.element(value)
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

/// Members of a map.
struct Members<'a> {
    /// Where the map is.
    path: JsonPath<'a>,
    /// Text of the key whose value comes next.
    key: String,
}

impl SerializeMap for Members<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Problem> {
        self.key = match serde_json::to_value(key) {
            Ok(Value::String(text)) => text,
            Ok(other) => other.to_string(),
            Err(error) => return Err(ser::Error::custom(error)),
        };
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Problem> {
        let path = self.path.key(&self.key);
        value.serialize(FiniteCheck { path: &path })
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

/// Fields of a struct or struct variant.
struct Fields<'a> {
    /// Where the struct is.
    path: JsonPath<'a>,
}

impl Fields<'_> {
    /// Checks one field.
    fn field<T: Serialize + ?Sized>(&self, key: &'static str, value: &T) -> Result<(), Problem> {
        let path = self.path.key(key);
        value.serialize(FiniteCheck { path: &path })
    }
}

impl SerializeStruct for Fields<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Problem> {
        self.field(key, value)
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

impl SerializeStructVariant for Fields<'_> {
    type Ok = ();
    type Error = Problem;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Problem> {
        self.field(key, value)
    }

    fn end(self) -> Result<(), Problem> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    #[derive(Serialize)]
    enum Shape {
        Point(f64, f64),
        Circle { radius: f64 },
        Size(f64),
    }

    #[test]
    fn non_finite_map_values_are_located_by_key() {
        let map = BTreeMap::from([("depth", f64::NEG_INFINITY)]);

        assert_eq!(
            to_value(&map),
            Err(CanonicalError::NonFiniteNumber {
                path: "depth".to_string()
            })
        );
    }

    #[test]
    fn variants_add_their_name_to_the_path() {
        assert_eq!(
            to_value(&vec![Shape::Point(0.0, f64::NAN)]),
            Err(CanonicalError::NonFiniteNumber {
                path: "[0].Point[1]".to_string()
            })
        );
        assert_eq!(
            to_value(&Shape::Circle {
                radius: f64::INFINITY
            }),
            Err(CanonicalError::NonFiniteNumber {
                path: "Circle.radius".to_string()
            })
        );
        assert_eq!(
            to_value(&Shape::Size(f64::NAN)),
            Err(CanonicalError::NonFiniteNumber {
                path: "Size".to_string()
            })
        );
    }

    #[test]
    fn optional_numbers_are_checked() {
        assert_eq!(
            to_value(&Some(f32::NAN)),
            Err(CanonicalError::NonFiniteNumber {
                path: "value".to_string()
            })
        );
    }

    #[test]
    fn finite_values_serialize_normalized() {
        assert_eq!(
            to_value(&Shape::Point(1.0, 2.5)),
            Ok(json!({"Point": [1, 2.5]}))
        );
    }

    #[test]
    fn keys_that_json_cannot_hold_are_serialize_errors() {
        let map = BTreeMap::from([(vec![1], 1)]);

        assert!(matches!(
            to_value(&map),
            Err(CanonicalError::Serialize { .. })
        ));
    }
}
