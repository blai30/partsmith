//! The number format of the text form (naming.md section 3.7.3).

use std::fmt;

/// Decimal places of the text form.
const DECIMALS: usize = 6;

/// `10^DECIMALS`.
const DECIMAL_SCALE: u128 = 1_000_000;

/// Bits of the stored significand of an `f64`.
const SIGNIFICAND_BITS: u32 = 52;

/// Largest power of two `scaled_magnitude` multiplies by: the scaled significand is below 2^73, so
/// the product stays below 2^127.
const MAX_SCALED_EXPONENT: u32 = 54;

/// Formats a real value as the text form does (section 3.7.3): six decimals, trailing zeros and a
/// trailing dot removed, and `-0` written as `0`.
fn format_number(value: f64) -> String {
    let mut text = String::new();
    write_number(&mut text, value).expect("writing to a String does not fail");
    text
}

/// Writes `value` in the text number format. Equivalent to the literal procedure of
/// `format_number_by_formatting`, without its slow fixed-point formatting for every value whose
/// magnitude is below 2^64.
pub(super) fn write_number(output: &mut impl fmt::Write, value: f64) -> fmt::Result {
    let parts = scaled_magnitude(value).and_then(|scaled| {
        let whole = u64::try_from(scaled / DECIMAL_SCALE).ok()?;
        let fraction = u32::try_from(scaled % DECIMAL_SCALE).ok()?;
        Some((whole, fraction))
    });
    let Some((whole, fraction)) = parts else {
        return output.write_str(&format_number_by_formatting(value));
    };
    if value.is_sign_negative() && (whole, fraction) != (0, 0) {
        output.write_char('-')?;
    }
    write_integer(output, whole)?;
    if fraction != 0 {
        let mut digits = [b'0'; DECIMALS];
        let mut rest = fraction;
        for digit in digits.iter_mut().rev() {
            *digit = b'0' + (rest % 10) as u8;
            rest /= 10;
        }
        let kept = digits
            .iter()
            .rposition(|digit| *digit != b'0')
            .map_or(0, |last| last + 1);
        output.write_char('.')?;
        output.write_str(digits_text(&digits[..kept]))?;
    }
    Ok(())
}

/// Writes the decimal digits of `value`.
pub(super) fn write_integer(output: &mut impl fmt::Write, mut value: u64) -> fmt::Result {
    // u64::MAX has 20 digits.
    let mut digits = [b'0'; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    output.write_str(digits_text(&digits[start..]))
}

/// ASCII decimal digits as text.
fn digits_text(digits: &[u8]) -> &str {
    std::str::from_utf8(digits).expect("decimal digits are ASCII")
}

/// The literal number format of section 3.7.3: `format!("{:.6}")`, trailing zeros and a trailing
/// dot removed, `-0` replaced by `0`.
fn format_number_by_formatting(value: f64) -> String {
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        let kept = text.trim_end_matches('0').trim_end_matches('.').len();
        text.truncate(kept);
    }
    if text == "-0" {
        text = "0".to_owned();
    }
    text
}

/// `|value| * 10^6` rounded to the nearest integer, ties to even, computed exactly from the binary
/// value as fixed-point formatting rounds it; `None` for non-finite values and magnitudes of 2^107
/// or more.
fn scaled_magnitude(value: f64) -> Option<u128> {
    if !value.is_finite() {
        return None;
    }
    let bits = value.abs().to_bits();
    let biased_exponent = i32::try_from(bits >> SIGNIFICAND_BITS).ok()?;
    let stored = bits & ((1 << SIGNIFICAND_BITS) - 1);
    // |value| = significand * 2^exponent exactly; subnormals have no implicit leading bit.
    let (significand, exponent) = if biased_exponent == 0 {
        (stored, -1074)
    } else {
        (stored | (1 << SIGNIFICAND_BITS), biased_exponent - 1075)
    };
    let product = u128::from(significand) * DECIMAL_SCALE;
    let shift = exponent.unsigned_abs();
    if exponent >= 0 {
        return (shift <= MAX_SCALED_EXPONENT).then(|| product << shift);
    }
    if shift >= u128::BITS {
        return Some(0);
    }
    let quotient = product >> shift;
    let remainder = product - (quotient << shift);
    let half = 1_u128 << (shift - 1);
    let round_up = remainder > half || (remainder == half && quotient % 2 == 1);
    Some(quotient + u128::from(round_up))
}

/// `value` rounded to the text precision: the value its text form parses to.
pub(super) fn round_to_text(value: f64) -> f64 {
    format_number(value)
        .parse()
        .expect("the text form of a finite number is a decimal number")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_number_examples() {
        assert_eq!(format_number(40.0), "40");
        assert_eq!(format_number(314.1592653589793), "314.159265");
        assert_eq!(format_number(std::f64::consts::FRAC_1_SQRT_2), "0.707107");
        assert_eq!(format_number(-1e-7), "0");
        assert_eq!(format_number(0.0078125), "0.007812");
        assert_eq!(format_number(1234567.8912345), "1234567.891235");
    }

    #[test]
    fn format_number_of_tiny_and_huge_values() {
        assert_eq!(format_number(f64::MIN_POSITIVE), "0");
        assert_eq!(format_number(5e-324), "0");
        assert_eq!(format_number(-4e-7), "0");
        assert_eq!(format_number(1e20), "100000000000000000000");
        assert_eq!(format_number(1e300), format_number_by_formatting(1e300));
    }

    #[test]
    fn format_number_of_non_finite_values() {
        assert_eq!(format_number(f64::NAN), "NaN");
        assert_eq!(format_number(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn scaled_magnitude_rounds_ties_to_even() {
        assert_eq!(scaled_magnitude(0.0078125), Some(7812));
        assert_eq!(scaled_magnitude(0.0234375), Some(23438));
        assert_eq!(scaled_magnitude(-2.5), Some(2_500_000));
    }

    #[test]
    fn scaled_magnitude_gives_up_on_huge_values() {
        assert_eq!(scaled_magnitude(2.0_f64.powi(107)), None);
        assert!(scaled_magnitude(2.0_f64.powi(106)).is_some());
        assert_eq!(scaled_magnitude(f64::INFINITY), None);
    }

    proptest::proptest! {
        #[test]
        fn format_number_matches_fixed_point_formatting(value in proptest::num::f64::NORMAL | proptest::num::f64::SUBNORMAL | proptest::num::f64::ZERO) {
            proptest::prop_assert_eq!(format_number(value), format_number_by_formatting(value));
        }

        #[test]
        fn format_number_matches_on_binary_ties(numerator in -10_000_000_i64..10_000_000, power in 7_i32..30) {
            let value = numerator as f64 / 2.0_f64.powi(power);
            proptest::prop_assert_eq!(format_number(value), format_number_by_formatting(value));
        }

        #[test]
        fn format_number_matches_on_part_sized_values(value in -1e9..1e9_f64) {
            proptest::prop_assert_eq!(format_number(value), format_number_by_formatting(value));
        }
    }

    #[test]
    fn write_integer_writes_extremes() {
        let mut text = String::new();
        write_integer(&mut text, 0).expect("writing to a String does not fail");
        text.push(' ');
        write_integer(&mut text, u64::MAX).expect("writing to a String does not fail");

        assert_eq!(text, "0 18446744073709551615");
    }

    #[test]
    fn format_number_writes_negative_zero_as_zero() {
        assert_eq!(format_number(-0.0), "0");
    }

    #[test]
    fn format_number_keeps_negative_values() {
        assert_eq!(format_number(-2.5), "-2.5");
    }

    #[test]
    fn round_to_text_matches_parsed_text() {
        assert_eq!(round_to_text(0.0078125), 0.007812);
        assert_eq!(round_to_text(-1e-7), 0.0);
        assert!(round_to_text(-1e-7).is_sign_positive());
    }
}
