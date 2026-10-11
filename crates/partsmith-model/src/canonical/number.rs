//! Number normalization and the written number form (file-format.md section 3.3).

use std::fmt::Write;

use serde_json::{Number, Value};

/// 2^53: integral values below this magnitude are exact integers in a double.
const EXACT_INTEGER_LIMIT: f64 = 9_007_199_254_740_992.0;

/// Writes one number in canonical form (file-format.md section 3.3).
pub fn format_number(number: &Number) -> String {
    let mut output = String::new();
    write_number(&mut output, number);
    output
}

/// Appends one number in canonical form: integral values with magnitude below 2^53 (zero of
/// either sign included) as integers, every other value as the shortest text that parses back to it.
pub(super) fn write_number(output: &mut String, number: &Number) {
    let value = number_value(number);
    match exact_integer(value) {
        Some(integer) => write!(output, "{integer}").expect("writing to a String never fails"),
        None => output.push_str(ryu::Buffer::new().format_finite(value)),
    }
}

/// Characters in the canonical form of a number, counted without writing it.
pub(super) fn number_width(number: &Number) -> usize {
    let value = number_value(number);
    match exact_integer(value) {
        Some(integer) => {
            let digits = integer
                .unsigned_abs()
                .checked_ilog10()
                .map_or(1, |exponent| exponent as usize + 1);
            usize::from(integer < 0) + digits
        }
        None => ryu::Buffer::new().format_finite(value).len(),
    }
}

/// Rewrites every number in `value` to its normalized form (file-format.md section 3.3).
pub fn normalize_numbers(value: &mut Value) {
    match value {
        Value::Number(number) => *number = normalized(number),
        Value::Array(items) => items.iter_mut().for_each(normalize_numbers),
        Value::Object(members) => members.values_mut().for_each(normalize_numbers),
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

/// The normalized form of a number: an integer (`u64` when not negative, otherwise `i64`) when
/// its value is integral with magnitude below 2^53, where negative zero becomes 0; otherwise the
/// nearest `f64`.
pub(crate) fn normalized(number: &Number) -> Number {
    let value = number_value(number);
    match exact_integer(value) {
        Some(integer) => match u64::try_from(integer) {
            Ok(non_negative) => Number::from(non_negative),
            Err(_) => Number::from(integer),
        },
        None => Number::from_f64(value).expect("a number's value is finite"),
    }
}

/// The value as an integer when it is integral with magnitude below 2^53, so the conversion is exact.
fn exact_integer(value: f64) -> Option<i64> {
    // Negative zero converts to the integer 0.
    (value.fract() == 0.0 && value.abs() < EXACT_INTEGER_LIMIT).then_some(value as i64)
}

/// The number's value as a double: integers beyond 2^53 round to the nearest double.
fn number_value(number: &Number) -> f64 {
    number
        .as_f64()
        .expect("serde_json numbers without arbitrary precision are finite doubles or integers")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn float(value: f64) -> Number {
        Number::from_f64(value).expect("finite")
    }

    #[test]
    fn integral_floats_print_as_integers() {
        assert_eq!(format_number(&float(-123.0)), "-123");
        assert_eq!(
            format_number(&float(9_007_199_254_740_991.0)),
            "9007199254740991"
        );
    }

    #[test]
    fn small_and_large_magnitudes_switch_to_exponents() {
        assert_eq!(format_number(&float(0.000_012_5)), "0.0000125");
        assert_eq!(format_number(&float(1.25e-6)), "1.25e-6");
        assert_eq!(format_number(&float(1.5e16)), "1.5e16");
        assert_eq!(
            format_number(&float(123_456_789_012_345.6)),
            "123456789012345.6"
        );
    }

    #[test]
    fn negative_integers_beyond_two_to_the_53_are_floats() {
        assert_eq!(
            format_number(&Number::from(-9_007_199_254_740_993_i64)),
            "-9007199254740992.0"
        );
    }

    #[test]
    fn normalized_integers_are_unsigned_when_not_negative() {
        assert!(normalized(&float(3.0)).is_u64());
        assert!(normalized(&float(-0.0)).is_u64());
        assert!(normalized(&float(-3.0)).is_i64());
        assert!(normalized(&float(3.5)).is_f64());
        assert!(normalized(&Number::from(-4_i64)).is_i64());
    }

    #[test]
    fn integers_at_two_to_the_53_become_floats() {
        let number = normalized(&Number::from(9_007_199_254_740_992_u64));

        assert!(number.is_f64());
        assert_eq!(number.as_f64(), Some(9_007_199_254_740_992.0));
    }

    #[test]
    fn normalize_numbers_walks_objects() {
        let mut value = json!({"a": {"b": [1.0, "x", null, true]}});

        normalize_numbers(&mut value);

        assert!(value["a"]["b"][0].is_u64());
    }

    #[test]
    fn widths_match_the_written_form() {
        let numbers = [
            Number::from(0_u64),
            Number::from(9_u64),
            Number::from(10_u64),
            Number::from(-1_i64),
            Number::from(-100_i64),
            Number::from(u64::MAX),
            float(-0.0),
            float(0.5),
            float(-1e-7),
            float(1.7976931348623157e308),
            float(9_007_199_254_740_991.0),
        ];
        for number in numbers {
            assert_eq!(
                number_width(&number),
                format_number(&number).len(),
                "{number}"
            );
        }
    }

    #[test]
    fn exact_integer_rejects_fractions_and_huge_values() {
        assert_eq!(exact_integer(2.5), None);
        assert_eq!(exact_integer(1e300), None);
        assert_eq!(exact_integer(-7.0), Some(-7));
    }
}
