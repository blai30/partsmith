//! Parsing the text form (naming.md section 3.7.4).

use super::{Field, Fingerprint, FingerprintDefect, canonical_axis, is_negative, length};
use crate::error::NamingError;
use crate::kind::GeometryKind;

/// Largest difference from 1 that a parsed direction's length may have.
const UNIT_LENGTH_TOLERANCE: f64 = 1e-4;

impl Fingerprint {
    /// Parses the text form (section 3.7.4).
    pub fn parse(text: &str) -> Result<Fingerprint, NamingError> {
        let invalid = |defect: FingerprintDefect| NamingError::InvalidFingerprint {
            text: text.to_owned(),
            reason: defect.to_string(),
        };
        let mut tokens = split_ascii(text, b'|');
        let kind_token = tokens.next().unwrap_or_default();
        let kind = GeometryKind::from_token(kind_token)
            .ok_or_else(|| invalid(FingerprintDefect::UnknownKind(kind_token.to_owned())))?;
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
                .ok_or_else(|| invalid(FingerprintDefect::UnknownField(name.to_owned())))?;
            if field.rank() < next_rank {
                return Err(invalid(FingerprintDefect::OutOfOrder(field)));
            }
            next_rank = field.rank() + 1;
            if !field.applies_to(kind) {
                return Err(invalid(FingerprintDefect::DoesNotApply(field, kind)));
            }
            match field {
                Field::Direction => {
                    let direction = parse_reals::<3>(field, value).map_err(invalid)?;
                    if (length(direction) - 1.0).abs() > UNIT_LENGTH_TOLERANCE {
                        return Err(invalid(FingerprintDefect::NotUnit));
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
                        return Err(invalid(FingerprintDefect::Negative));
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

/// Parses `COUNT` comma-separated reals of the form `["-"] 1*DIGIT ["." 1*DIGIT]`.
fn parse_reals<const COUNT: usize>(
    field: Field,
    value: &str,
) -> Result<[f64; COUNT], FingerprintDefect> {
    let mut reals = [0.0; COUNT];
    let mut components = split_ascii(value, b',');
    for real in &mut reals {
        let Some(component) = components.next() else {
            return Err(FingerprintDefect::ValueCount(field, COUNT));
        };
        *real = parse_real(component)
            .ok_or_else(|| FingerprintDefect::NotANumber(component.to_owned()))?;
    }
    if components.next().is_some() {
        return Err(FingerprintDefect::ValueCount(field, COUNT));
    }
    Ok(reals)
}

/// Parses `F,X` adjacency counts, each `1*DIGIT` fitting in `u32`.
fn parse_counts(field: Field, value: &str) -> Result<[u32; 2], FingerprintDefect> {
    let Some((faces, edges)) = split_once_ascii(value, b',') else {
        return Err(FingerprintDefect::ValueCount(field, 2));
    };
    let parse_count = |count: &str| {
        let digits_only = !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit());
        digits_only
            .then(|| count.parse::<u32>().ok())
            .flatten()
            .ok_or_else(|| FingerprintDefect::NotANumber(count.to_owned()))
    };
    if edges.bytes().any(|byte| byte == b',') {
        return Err(FingerprintDefect::ValueCount(field, 2));
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

/// Parses one real of the form `["-"] 1*DIGIT ["." 1*DIGIT]`; `None` when `text` is not of that
/// form or its value is not finite.
fn parse_real(text: &str) -> Option<f64> {
    let is_digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let well_formed = match split_once_ascii(unsigned, b'.') {
        Some((whole, fraction)) => is_digits(whole) && is_digits(fraction),
        None => is_digits(unsigned),
    };
    if !well_formed {
        return None;
    }
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::tests::{parsed, reason};

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
        let negative = "a size or radius cannot be negative";

        assert_eq!(reason(Fingerprint::parse("cylinder|r=-1")), negative);
        assert_eq!(reason(Fingerprint::parse("plane|a=-0.5")), negative);
    }

    #[test]
    fn parse_reports_non_unit_direction() {
        let not_unit = "the direction is not a unit vector";

        assert_eq!(reason(Fingerprint::parse("plane|n=0,0,2")), not_unit);
        assert_eq!(reason(Fingerprint::parse("line|n=0,0,0")), not_unit);
    }

    #[test]
    fn parse_accepts_direction_within_tolerance() {
        assert!(Fingerprint::parse("line|n=0,0,1.00005").is_ok());
    }

    #[test]
    fn parse_keeps_plane_normal_sign() {
        assert_eq!(parsed("plane|n=0,-1,0").direction, Some([0.0, -1.0, 0.0]));
    }
}
