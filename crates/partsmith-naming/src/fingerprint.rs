//! Geometric fingerprints and their canonical text form (naming.md sections 2.3 and 3.7).

use std::borrow::Cow;
use std::fmt::{self, Write as _};
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize, Serializer};

use crate::error::NamingError;
use crate::kind::{ElementKind, GeometryKind};

/// Shortest direction `Fingerprint::new` normalizes.
const MIN_DIRECTION_LENGTH: f64 = 1e-12;

/// Components at most this large never decide the sign of a canonical axis.
const AXIS_SIGN_TOLERANCE: f64 = 1e-9;

/// Largest difference from 1 that a parsed direction's length may have.
const UNIT_LENGTH_TOLERANCE: f64 = 1e-4;

/// Geometric fingerprint of an element. Values are stored already rounded to the text precision
/// (section 3.7), so `parse(display(fp)) == fp`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "String")]
pub struct Fingerprint {
    /// Surface, curve or point kind.
    pub kind: GeometryKind,
    /// Unit outward normal (planes) or canonical unit axis or direction (other kinds), if the kind has one.
    pub direction: Option<[f64; 3]>,
    /// Area centroid (faces), length centroid (edges) or position (vertices), in millimeters.
    pub centroid: Option<[f64; 3]>,
    /// Area in square millimeters (faces) or length in millimeters (edges); `None` for vertices.
    pub size: Option<f64>,
    /// Radius in millimeters for cylinders, spheres, tori (major radius) and circles.
    pub radius: Option<f64>,
    /// Adjacency counts as defined in section 3.7.2.
    pub adjacency: Option<[u32; 2]>,
}

/// A field of the text form, in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    /// `n`: normal, axis or direction.
    Direction,
    /// `c`: centroid or position.
    Centroid,
    /// `a`: area of a face.
    Area,
    /// `l`: length of an edge.
    Length,
    /// `r`: radius.
    Radius,
    /// `adj`: adjacency counts.
    Adjacency,
}

impl Field {
    /// Every field.
    const ALL: [Field; 6] = [
        Field::Direction,
        Field::Centroid,
        Field::Area,
        Field::Length,
        Field::Radius,
        Field::Adjacency,
    ];

    /// The field name in the text form.
    fn token(self) -> &'static str {
        match self {
            Field::Direction => "n",
            Field::Centroid => "c",
            Field::Area => "a",
            Field::Length => "l",
            Field::Radius => "r",
            Field::Adjacency => "adj",
        }
    }

    /// The field named `token`, if any.
    fn from_token(token: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|field| field.token() == token)
    }

    /// Position in the canonical order; area and length share one position.
    fn rank(self) -> u8 {
        match self {
            Field::Direction => 0,
            Field::Centroid => 1,
            Field::Area | Field::Length => 2,
            Field::Radius => 3,
            Field::Adjacency => 4,
        }
    }

    /// True when fingerprints of `kind` may carry this field.
    fn applies_to(self, kind: GeometryKind) -> bool {
        match self {
            Field::Direction => kind.has_direction(),
            Field::Centroid | Field::Adjacency => true,
            Field::Area => kind.element_kind() == ElementKind::Face,
            Field::Length => kind.element_kind() == ElementKind::Edge,
            Field::Radius => kind.has_radius(),
        }
    }

    /// The size field of `kind`: area for faces, length otherwise.
    fn size_of(kind: GeometryKind) -> Field {
        if kind.element_kind() == ElementKind::Face {
            Field::Area
        } else {
            Field::Length
        }
    }
}

impl Fingerprint {
    /// Builds a fingerprint from computed geometry: drops fields the kind does not allow, normalizes and
    /// canonicalizes the direction, rounds every value, and leaves `adjacency` as `None`.
    pub fn new(
        kind: GeometryKind,
        direction: Option<[f64; 3]>,
        centroid: [f64; 3],
        size: Option<f64>,
        radius: Option<f64>,
    ) -> Result<Fingerprint, NamingError> {
        let given = Fingerprint {
            kind,
            direction: direction.filter(|_| kind.has_direction()),
            centroid: Some(centroid),
            size: size.filter(|_| kind.has_size()),
            radius: radius.filter(|_| kind.has_radius()),
            adjacency: None,
        };
        let invalid = |reason: String| NamingError::InvalidFingerprint {
            text: given.to_string(),
            reason,
        };
        if kind.requires_direction() && given.direction.is_none() {
            return Err(invalid(required(Field::Direction, kind)));
        }
        if kind.has_size() && given.size.is_none() {
            return Err(invalid(required(Field::size_of(kind), kind)));
        }
        let values = given
            .direction
            .into_iter()
            .flatten()
            .chain(centroid)
            .chain(given.size)
            .chain(given.radius);
        for value in values {
            if !value.is_finite() {
                return Err(invalid(not_a_number(&value.to_string())));
            }
        }
        if given.size.is_some_and(is_negative) || given.radius.is_some_and(is_negative) {
            return Err(invalid(NEGATIVE.to_owned()));
        }
        let direction = match given.direction {
            Some(direction) => {
                Some(unit_direction(direction, kind).ok_or_else(|| invalid(NOT_UNIT.to_owned()))?)
            }
            None => None,
        };
        Ok(Fingerprint {
            kind,
            direction,
            centroid: Some(centroid.map(round_to_text)),
            size: given.size.map(round_to_text),
            radius: given.radius.map(round_to_text),
            adjacency: None,
        })
    }

    /// The same fingerprint with adjacency counts set.
    pub fn with_adjacency(self, adjacency: [u32; 2]) -> Fingerprint {
        Fingerprint {
            adjacency: Some(adjacency),
            ..self
        }
    }

    /// Parses the text form (section 3.7.4).
    pub fn parse(text: &str) -> Result<Fingerprint, NamingError> {
        let invalid = |reason: String| NamingError::InvalidFingerprint {
            text: text.to_owned(),
            reason,
        };
        let mut tokens = split_ascii(text, b'|');
        let kind_token = tokens.next().unwrap_or_default();
        let kind = GeometryKind::from_token(kind_token)
            .ok_or_else(|| invalid(format!("unknown kind \"{kind_token}\"")))?;
        let mut fingerprint = Fingerprint {
            kind,
            direction: None,
            centroid: None,
            size: None,
            radius: None,
            adjacency: None,
        };
        let mut next_rank = 0;
        for token in tokens {
            let (name, value) = split_once_ascii(token, b'=').unwrap_or((token, ""));
            let field = Field::from_token(name)
                .ok_or_else(|| invalid(format!("unknown field \"{name}\"")))?;
            if field.rank() < next_rank {
                return Err(invalid(format!(
                    "field \"{name}\" is repeated or out of order"
                )));
            }
            next_rank = field.rank() + 1;
            if !field.applies_to(kind) {
                return Err(invalid(format!(
                    "field \"{name}\" does not apply to {kind}"
                )));
            }
            match field {
                Field::Direction => {
                    let direction = parse_reals::<3>(field, value).map_err(invalid)?;
                    if (length(direction) - 1.0).abs() > UNIT_LENGTH_TOLERANCE {
                        return Err(invalid(NOT_UNIT.to_owned()));
                    }
                    fingerprint.direction = Some(if kind.keeps_direction_sign() {
                        direction
                    } else {
                        canonical_axis(direction)
                    });
                }
                Field::Centroid => {
                    fingerprint.centroid = Some(parse_reals::<3>(field, value).map_err(invalid)?);
                }
                Field::Area | Field::Length | Field::Radius => {
                    let [magnitude] = parse_reals::<1>(field, value).map_err(invalid)?;
                    if is_negative(magnitude) {
                        return Err(invalid(NEGATIVE.to_owned()));
                    }
                    if field == Field::Radius {
                        fingerprint.radius = Some(magnitude);
                    } else {
                        fingerprint.size = Some(magnitude);
                    }
                }
                Field::Adjacency => {
                    fingerprint.adjacency = Some(parse_counts(field, value).map_err(invalid)?);
                }
            }
        }
        Ok(fingerprint)
    }
}

/// Reason for a direction that cannot be or is not a unit vector.
const NOT_UNIT: &str = "the direction is not a unit vector";

/// Reason for a negative size or radius.
const NEGATIVE: &str = "a size or radius cannot be negative";

/// Reason for a value that is not a number.
fn not_a_number(value: &str) -> String {
    format!("\"{value}\" is not a number")
}

/// Reason for a field `Fingerprint::new` needs but was not given.
fn required(field: Field, kind: GeometryKind) -> String {
    format!("field \"{}\" is required for {kind}", field.token())
}

/// True for values below zero (negative zero is not negative).
fn is_negative(value: f64) -> bool {
    value < 0.0
}

/// Euclidean length of a vector.
fn length(vector: [f64; 3]) -> f64 {
    vector
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt()
}

/// The unit direction of `direction`, rounded to text precision and, for every kind but a plane,
/// canonicalized; `None` when `direction` is too short to normalize.
///
/// Rounding comes before canonicalization so the sign is decided by the components the text keeps,
/// which keeps `parse(display(fp)) == fp` even when a component smaller than the text precision
/// would otherwise decide the sign.
fn unit_direction(direction: [f64; 3], kind: GeometryKind) -> Option<[f64; 3]> {
    let largest = direction
        .iter()
        .fold(0.0_f64, |largest, component| largest.max(component.abs()));
    if largest == 0.0 {
        return None;
    }
    // Dividing by the largest component first keeps the squares of the length in range, so a
    // direction with huge components does not overflow to an infinite length.
    let scaled = direction.map(|component| component / largest);
    let scaled_length = length(scaled);
    if largest * scaled_length < MIN_DIRECTION_LENGTH {
        return None;
    }
    let unit = scaled.map(|component| round_to_text(component / scaled_length));
    Some(if kind.keeps_direction_sign() {
        unit
    } else {
        canonical_axis(unit)
    })
}

/// `axis`, negated if needed so its first component larger than 1e-9 in magnitude is positive.
fn canonical_axis(axis: [f64; 3]) -> [f64; 3] {
    let deciding = axis
        .into_iter()
        .find(|component| component.abs() > AXIS_SIGN_TOLERANCE);
    match deciding {
        // Adding zero turns the negative zeros the negation makes into positive zeros.
        Some(component) if component < 0.0 => axis.map(|component| -component + 0.0),
        _ => axis,
    }
}

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
pub(crate) fn format_number(value: f64) -> String {
    let mut text = String::new();
    write_number(&mut text, value).expect("writing to a String does not fail");
    text
}

/// Writes `value` in the text number format. Equivalent to the literal procedure of
/// `format_number_by_formatting`, without its slow fixed-point formatting for every value whose
/// magnitude is below 2^64.
fn write_number(output: &mut impl fmt::Write, value: f64) -> fmt::Result {
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
fn write_integer(output: &mut impl fmt::Write, mut value: u64) -> fmt::Result {
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
fn round_to_text(value: f64) -> f64 {
    format_number(value)
        .parse()
        .expect("the text form of a finite number is a decimal number")
}

/// Parses `COUNT` comma-separated reals of the form `["-"] 1*DIGIT ["." 1*DIGIT]`.
fn parse_reals<const COUNT: usize>(field: Field, value: &str) -> Result<[f64; COUNT], String> {
    let mut reals = [0.0; COUNT];
    let mut components = split_ascii(value, b',');
    for real in &mut reals {
        let component = components
            .next()
            .ok_or_else(|| values_needed(field, COUNT))?;
        *real = parse_real(component)?;
    }
    if components.next().is_some() {
        return Err(values_needed(field, COUNT));
    }
    Ok(reals)
}

/// Parses `F,X` adjacency counts, each `1*DIGIT` fitting in `u32`.
fn parse_counts(field: Field, value: &str) -> Result<[u32; 2], String> {
    let (faces, edges) = split_once_ascii(value, b',').ok_or_else(|| values_needed(field, 2))?;
    let parse_count = |count: &str| {
        let digits_only = !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit());
        digits_only
            .then(|| count.parse::<u32>().ok())
            .flatten()
            .ok_or_else(|| not_a_number(count))
    };
    if edges.bytes().any(|byte| byte == b',') {
        return Err(values_needed(field, 2));
    }
    Ok([parse_count(faces)?, parse_count(edges)?])
}

/// `text` split at the first `separator`, an ASCII character. A byte scan is cheaper than a `char`
/// pattern on the short pieces of a fingerprint.
fn split_once_ascii(text: &str, separator: u8) -> Option<(&str, &str)> {
    let position = text.bytes().position(|byte| byte == separator)?;
    // An ASCII separator lies between character boundaries.
    Some((text.get(..position)?, text.get(position + 1..)?))
}

/// `text` split at every `separator`, an ASCII character, like `str::split`.
fn split_ascii(text: &str, separator: u8) -> impl Iterator<Item = &str> {
    let mut rest = Some(text);
    std::iter::from_fn(move || {
        let current = rest?;
        match split_once_ascii(current, separator) {
            Some((piece, remainder)) => {
                rest = Some(remainder);
                Some(piece)
            }
            None => {
                rest = None;
                Some(current)
            }
        }
    })
}

/// Reason for a field with the wrong number of values.
fn values_needed(field: Field, count: usize) -> String {
    format!("field \"{}\" needs {count} values", field.token())
}

/// Parses one real of the form `["-"] 1*DIGIT ["." 1*DIGIT]`.
fn parse_real(text: &str) -> Result<f64, String> {
    let is_digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let well_formed = match split_once_ascii(unsigned, b'.') {
        Some((whole, fraction)) => is_digits(whole) && is_digits(fraction),
        None => is_digits(unsigned),
    };
    if !well_formed {
        return Err(not_a_number(text));
    }
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| not_a_number(text))
}

/// Writes `|<field>=<x>,<y>,...` with each value in the text number format.
fn write_reals(formatter: &mut fmt::Formatter<'_>, field: Field, values: &[f64]) -> fmt::Result {
    formatter.write_char('|')?;
    formatter.write_str(field.token())?;
    formatter.write_char('=')?;
    for (position, value) in values.iter().enumerate() {
        if position > 0 {
            formatter.write_str(",")?;
        }
        write_number(formatter, *value)?;
    }
    Ok(())
}

impl fmt::Display for Fingerprint {
    /// Writes the canonical text form (section 3.7.3).
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.kind.token())?;
        if let Some(direction) = self.direction {
            write_reals(formatter, Field::Direction, &direction)?;
        }
        if let Some(centroid) = self.centroid {
            write_reals(formatter, Field::Centroid, &centroid)?;
        }
        if let Some(size) = self.size {
            write_reals(formatter, Field::size_of(self.kind), &[size])?;
        }
        if let Some(radius) = self.radius {
            write_reals(formatter, Field::Radius, &[radius])?;
        }
        if let Some([faces, edges]) = self.adjacency {
            formatter.write_str("|adj=")?;
            write_integer(formatter, u64::from(faces))?;
            formatter.write_char(',')?;
            write_integer(formatter, u64::from(edges))?;
        }
        Ok(())
    }
}

impl FromStr for Fingerprint {
    type Err = NamingError;

    fn from_str(text: &str) -> Result<Fingerprint, NamingError> {
        Fingerprint::parse(text)
    }
}

impl TryFrom<String> for Fingerprint {
    type Error = NamingError;

    fn try_from(text: String) -> Result<Fingerprint, NamingError> {
        Fingerprint::parse(&text)
    }
}

impl From<Fingerprint> for String {
    fn from(fingerprint: Fingerprint) -> String {
        fingerprint.to_string()
    }
}

impl Serialize for Fingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl JsonSchema for Fingerprint {
    fn schema_name() -> Cow<'static, str> {
        "Fingerprint".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Fingerprint").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        let kinds: Vec<&str> = GeometryKind::ALL.iter().map(|kind| kind.token()).collect();
        let pattern = format!("^({})(\\|[a-z]+=[-0-9.,]+)*$", kinds.join("|"));
        json_schema!({"type": "string", "pattern": pattern})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    fn reason(result: Result<Fingerprint, NamingError>) -> String {
        match result {
            Err(NamingError::InvalidFingerprint { reason, .. }) => reason,
            other => panic!("expected InvalidFingerprint, got {other:?}"),
        }
    }

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
    fn split_ascii_matches_str_split() {
        for text in ["", "a", "a|b", "|a||b|", "plane|n=0,0,1"] {
            let pieces: Vec<&str> = split_ascii(text, b'|').collect();

            assert_eq!(pieces, text.split('|').collect::<Vec<&str>>(), "{text:?}");
        }
    }

    #[test]
    fn split_once_ascii_splits_at_first_separator() {
        assert_eq!(split_once_ascii("a=b=c", b'='), Some(("a", "b=c")));
        assert_eq!(split_once_ascii("abc", b'='), None);
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

    #[test]
    fn canonical_axis_flips_first_significant_negative_component() {
        assert_eq!(canonical_axis([0.0, -1.0, 0.5]), [0.0, 1.0, -0.5]);
        assert_eq!(canonical_axis([1e-10, -1.0, 0.0]), [-1e-10, 1.0, 0.0]);
    }

    #[test]
    fn canonical_axis_makes_positive_zeros() {
        let axis = canonical_axis([0.0, 0.0, -1.0]);

        assert!(axis[0].is_sign_positive() && axis[1].is_sign_positive());
    }

    #[test]
    fn unit_direction_decides_sign_on_rounded_components() {
        let direction = unit_direction([-1e-7, 1.0, 0.0], GeometryKind::Line);

        assert_eq!(direction, Some([0.0, 1.0, 0.0]));
    }

    #[test]
    fn unit_direction_rejects_tiny_vectors() {
        assert_eq!(unit_direction([1e-13, 0.0, 0.0], GeometryKind::Line), None);
        assert_eq!(unit_direction([5e-324, 0.0, 0.0], GeometryKind::Line), None);
    }

    /// A component of a diagonal unit vector at text precision (0.707107).
    fn diagonal() -> f64 {
        round_to_text(std::f64::consts::FRAC_1_SQRT_2)
    }

    #[test]
    fn unit_direction_keeps_vectors_just_long_enough() {
        let direction = unit_direction([9e-13, 9e-13, 0.0], GeometryKind::Line);

        assert_eq!(direction, Some([diagonal(), diagonal(), 0.0]));
    }

    #[test]
    fn unit_direction_of_huge_vectors_is_unit() {
        assert_eq!(
            unit_direction([1e200, 0.0, 0.0], GeometryKind::Line),
            Some([1.0, 0.0, 0.0])
        );
        assert_eq!(
            unit_direction([f64::MAX, -f64::MAX, 0.0], GeometryKind::Plane),
            Some([diagonal(), -diagonal(), 0.0])
        );
    }

    #[test]
    fn new_with_huge_direction_round_trips() {
        let built = Fingerprint::new(
            GeometryKind::Line,
            Some([1e200, 0.0, 0.0]),
            [0.0; 3],
            Some(1.0),
            None,
        )
        .expect("a huge direction is still a direction");
        let text = built.to_string();

        assert_eq!(text, "line|n=1,0,0|c=0,0,0|l=1");
        assert_eq!(Fingerprint::parse(&text), Ok(built));
    }

    #[test]
    fn new_drops_radius_of_plane() {
        let built = Fingerprint::new(
            GeometryKind::Plane,
            Some([0.0, 0.0, 1.0]),
            [0.0, 0.0, 0.0],
            Some(1.0),
            Some(3.0),
        )
        .expect("valid values");

        assert_eq!(built.radius, None);
    }

    #[test]
    fn new_requires_size_for_curves() {
        let result = Fingerprint::new(
            GeometryKind::Circle,
            Some([0.0, 0.0, 1.0]),
            [0.0, 0.0, 0.0],
            None,
            Some(1.0),
        );

        assert_eq!(reason(result), "field \"l\" is required for circle");
    }

    #[test]
    fn new_requires_direction_for_lines() {
        let result = Fingerprint::new(GeometryKind::Line, None, [0.0; 3], Some(1.0), None);

        assert_eq!(reason(result), "field \"n\" is required for line");
    }

    #[test]
    fn new_rejects_infinite_radius() {
        let result = Fingerprint::new(
            GeometryKind::Sphere,
            None,
            [0.0; 3],
            Some(1.0),
            Some(f64::INFINITY),
        );

        assert_eq!(reason(result), "\"inf\" is not a number");
    }

    #[test]
    fn new_rejects_negative_radius() {
        let result = Fingerprint::new(GeometryKind::Sphere, None, [0.0; 3], Some(1.0), Some(-1.0));

        assert_eq!(reason(result), NEGATIVE);
    }

    #[test]
    fn new_error_text_shows_given_values() {
        let result = Fingerprint::new(
            GeometryKind::Plane,
            Some([0.0, 0.0, 0.0]),
            [1.0, 2.0, 3.0],
            Some(4.0),
            None,
        );

        assert_eq!(
            result,
            Err(NamingError::InvalidFingerprint {
                text: "plane|n=0,0,0|c=1,2,3|a=4".to_owned(),
                reason: NOT_UNIT.to_owned()
            })
        );
    }

    #[test]
    fn new_keeps_vertex_without_size() {
        let built = Fingerprint::new(GeometryKind::Point, None, [1.0, 2.0, 3.0], None, None)
            .expect("a point needs no size");

        assert_eq!(built.to_string(), "point|c=1,2,3");
    }

    #[test]
    fn with_adjacency_sets_counts_only() {
        let built = parsed("point|c=1,2,3").with_adjacency([3, 3]);

        assert_eq!(built.to_string(), "point|c=1,2,3|adj=3,3");
    }

    #[test]
    fn parse_kind_only() {
        assert_eq!(parsed("torus").to_string(), "torus");
    }

    #[test]
    fn parse_full_circle() {
        let circle = parsed("circle|n=0,0,1|c=0,0,10|l=31.415927|r=5|adj=2,1");

        assert_eq!(circle.size, Some(31.415927));
        assert_eq!(circle.radius, Some(5.0));
        assert_eq!(circle.adjacency, Some([2, 1]));
    }

    #[test]
    fn parse_reports_unknown_kind() {
        assert_eq!(
            reason(Fingerprint::parse("plain|c=0,0,0")),
            "unknown kind \"plain\""
        );
    }

    #[test]
    fn parse_reports_unknown_field() {
        assert_eq!(
            reason(Fingerprint::parse("plane|x=1")),
            "unknown field \"x\""
        );
    }

    #[test]
    fn parse_reports_field_order() {
        assert_eq!(
            reason(Fingerprint::parse("plane|c=0,0,0|n=0,0,1")),
            "field \"n\" is repeated or out of order"
        );
    }

    #[test]
    fn parse_rejects_area_after_length_slot() {
        assert_eq!(
            reason(Fingerprint::parse("line|l=1|l=2")),
            "field \"l\" is repeated or out of order"
        );
    }

    #[test]
    fn parse_reports_field_that_does_not_apply() {
        assert_eq!(
            reason(Fingerprint::parse("point|n=0,0,1")),
            "field \"n\" does not apply to point"
        );
    }

    #[test]
    fn parse_reports_malformed_numbers() {
        assert_eq!(
            reason(Fingerprint::parse("plane|c=1e3,0,0")),
            "\"1e3\" is not a number"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|c=+1,0,0")),
            "\"+1\" is not a number"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|c=1.,0,0")),
            "\"1.\" is not a number"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|c=.5,0,0")),
            "\".5\" is not a number"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|a=")),
            "\"\" is not a number"
        );
    }

    #[test]
    fn parse_rejects_huge_number_that_overflows() {
        let text = format!("plane|a={}", "9".repeat(400));

        assert!(reason(Fingerprint::parse(&text)).ends_with("is not a number"));
    }

    #[test]
    fn parse_reports_wrong_value_counts() {
        assert_eq!(
            reason(Fingerprint::parse("plane|c=0,0")),
            "field \"c\" needs 3 values"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|c=0,0,0,0")),
            "field \"c\" needs 3 values"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|adj=4")),
            "field \"adj\" needs 2 values"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|adj=4,4,4")),
            "field \"adj\" needs 2 values"
        );
    }

    #[test]
    fn parse_rejects_signed_or_oversized_counts() {
        assert_eq!(
            reason(Fingerprint::parse("plane|adj=+4,4")),
            "\"+4\" is not a number"
        );
        assert_eq!(
            reason(Fingerprint::parse("plane|adj=4294967296,4")),
            "\"4294967296\" is not a number"
        );
    }

    #[test]
    fn parse_reports_negative_size() {
        assert_eq!(reason(Fingerprint::parse("cylinder|r=-1")), NEGATIVE);
        assert_eq!(reason(Fingerprint::parse("plane|a=-0.5")), NEGATIVE);
    }

    #[test]
    fn parse_reports_non_unit_direction() {
        assert_eq!(reason(Fingerprint::parse("plane|n=0,0,2")), NOT_UNIT);
        assert_eq!(reason(Fingerprint::parse("line|n=0,0,0")), NOT_UNIT);
    }

    #[test]
    fn parse_accepts_direction_within_tolerance() {
        assert!(Fingerprint::parse("line|n=0,0,1.00005").is_ok());
    }

    #[test]
    fn parse_keeps_plane_normal_sign() {
        assert_eq!(parsed("plane|n=0,-1,0").direction, Some([0.0, -1.0, 0.0]));
    }

    #[test]
    fn display_uses_length_for_curves_and_area_for_surfaces() {
        assert_eq!(parsed("line|l=2").to_string(), "line|l=2");
        assert_eq!(parsed("cone|a=2").to_string(), "cone|a=2");
    }

    #[test]
    fn serializes_as_text_form() {
        let json = serde_json::to_string(&parsed("point|c=1,2,3|adj=3,3"))
            .expect("a fingerprint serializes");

        assert_eq!(json, "\"point|c=1,2,3|adj=3,3\"");
    }

    #[test]
    fn deserializing_invalid_text_fails() {
        assert!(serde_json::from_str::<Fingerprint>("\"plane|n=0,0,2\"").is_err());
    }

    #[test]
    fn json_schema_pattern_lists_every_kind() {
        let schema = schemars::schema_for!(Fingerprint);

        assert_eq!(
            schema.as_value()["pattern"],
            "^(plane|cylinder|cone|sphere|torus|revolution|extrusion|bspline_surface|bezier_surface|offset_surface|other_surface|line|circle|ellipse|hyperbola|parabola|bspline_curve|bezier_curve|offset_curve|other_curve|point)(\\|[a-z]+=[-0-9.,]+)*$"
        );
    }
}
