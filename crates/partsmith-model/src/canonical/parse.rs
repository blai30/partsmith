//! Strict JSON parsing for `load` (file-format.md section 3.7, step 2).

use std::cell::RefCell;
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::normalized;
use crate::LoadError;

/// Strict JSON parse used by `load`: rejects duplicate keys and deep nesting, normalizes numbers.
///
/// The text must be RFC 8259 JSON with arrays and objects nested at most 127 levels deep (the
/// JSON parser's recursion limit) and numbers within the double range. A key repeated in one
/// object is `DuplicateKey`; anything else wrong is `Syntax`.
pub fn parse(text: &str) -> Result<Value, LoadError> {
    let duplicate_key = RefCell::new(None);
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let result = StrictValue {
        duplicate_key: &duplicate_key,
    }
    .deserialize(&mut deserializer)
    .and_then(|value| deserializer.end().map(|()| value));
    result.map_err(|error| match duplicate_key.take() {
        Some(key) => LoadError::DuplicateKey {
            key,
            line: error.line(),
            column: error.column(),
        },
        None => syntax_error(&error),
    })
}

/// `Syntax` with the parser's description, without the position suffix its message carries.
fn syntax_error(error: &serde_json::Error) -> LoadError {
    let (line, column) = (error.line(), error.column());
    let text = error.to_string();
    let suffix = format!(" at line {line} column {column}");
    let message = text.strip_suffix(&suffix).unwrap_or(&text).to_string();
    LoadError::Syntax {
        line,
        column,
        message,
    }
}

/// Deserializes any JSON value, normalizing numbers and stopping at the first repeated key,
/// which it records in `duplicate_key`.
#[derive(Clone, Copy)]
struct StrictValue<'a> {
    /// The repeated key, once one is found.
    duplicate_key: &'a RefCell<Option<String>>,
}

impl<'de> DeserializeSeed<'de> for StrictValue<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictValue<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(normalized(&Number::from(value))))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(normalized(&Number::from(value))))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        let number = Number::from_f64(value)
            .ok_or_else(|| E::custom("a number is outside the range of a double"))?;
        Ok(Value::Number(normalized(&number)))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = sequence.next_element_seed(self)? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut members = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if members.contains_key(&key) {
                let message = format!("duplicate key {key}");
                *self.duplicate_key.borrow_mut() = Some(key);
                return Err(de::Error::custom(message));
            }
            let value = map.next_value_seed(self)?;
            members.insert(key, value);
        }
        Ok(Value::Object(members))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn duplicate_keys_report_the_line_of_the_repeat() {
        let result = parse("{\n  \"a\": {\"b\": 1,\n  \"b\": 2}\n}");

        assert_eq!(
            result,
            Err(LoadError::DuplicateKey {
                key: "b".to_string(),
                line: 3,
                column: 5
            })
        );
    }

    #[test]
    fn the_same_key_in_different_objects_is_fine() {
        assert_eq!(
            parse(r#"[{"a": 1}, {"a": 2}]"#),
            Ok(json!([{"a": 1}, {"a": 2}]))
        );
    }

    #[test]
    fn syntax_errors_carry_the_parser_description() {
        assert_eq!(
            parse("[1,]"),
            Err(LoadError::Syntax {
                line: 1,
                column: 4,
                message: "trailing comma".to_string()
            })
        );
    }

    #[test]
    fn trailing_text_is_a_syntax_error() {
        assert!(matches!(parse("1 2"), Err(LoadError::Syntax { .. })));
    }

    #[test]
    fn escapes_are_decoded() {
        assert_eq!(parse(r#""é\/😀""#), Ok(json!("é/😀")));
    }

    #[test]
    fn lone_surrogates_are_syntax_errors() {
        assert!(matches!(
            parse(r#""\ud800""#),
            Err(LoadError::Syntax { .. })
        ));
    }

    #[test]
    fn nesting_stops_at_127_levels() {
        let nested = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));

        assert!(parse(&nested(127)).is_ok());
        assert!(matches!(parse(&nested(128)), Err(LoadError::Syntax { .. })));
    }

    #[test]
    fn negative_zero_and_large_integers_are_normalized() {
        let value = parse("[-0, -0.0, 18446744073709551615]").expect("valid JSON");

        assert!(value[0].is_u64() && value[1].is_u64());
        assert!(value[2].is_f64());
    }
}
