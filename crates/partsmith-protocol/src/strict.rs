//! Strict deserialization of field-less variants in request enums.
//!
//! Serde ignores unknown fields of unit variants in internally tagged enums, even with
//! `deny_unknown_fields`. Request enums mark their unit variants with
//! `#[serde(deserialize_with = "no_fields")]` so they reject unknown fields like every other
//! request value (protocol.md section 2.3).

use std::fmt;

use serde::Deserializer;
use serde::de::{Error, MapAccess, Visitor};

/// Reads the fields of a variant that has none besides its tag, and rejects any field.
pub(crate) fn no_fields<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    deserializer.deserialize_map(NoFields)
}

/// Accepts only a map without entries.
struct NoFields;

impl<'de> Visitor<'de> for NoFields {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("no fields besides the tag")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        match map.next_key::<String>()? {
            Some(name) => Err(A::Error::unknown_field(&name, &[])),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// A request-style enum with one field-less variant.
    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum Shape {
        #[serde(deserialize_with = "no_fields")]
        Empty,
        Square {
            side: f64,
        },
    }

    #[test]
    fn field_less_variant_without_fields_deserializes() {
        let shape: Shape = serde_json::from_str(r#"{"kind":"empty"}"#).expect("deserializes");

        assert_eq!(shape, Shape::Empty);
    }

    #[test]
    fn field_less_variant_with_a_field_is_rejected() {
        let error = serde_json::from_str::<Shape>(r#"{"kind":"empty","side":1}"#)
            .expect_err("an unknown field is rejected");

        assert!(
            error.to_string().contains("unknown field `side`"),
            "{error}"
        );
    }

    #[test]
    fn other_variants_are_unaffected() {
        let shape: Shape =
            serde_json::from_str(r#"{"kind":"square","side":2.0}"#).expect("deserializes");

        assert_eq!(shape, Shape::Square { side: 2.0 });
    }

    #[test]
    fn field_less_variant_accepts_an_empty_map() {
        let value = serde_json::json!({});

        assert!(no_fields(&value).is_ok());
    }
}
