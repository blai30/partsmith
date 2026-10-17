//! The number form used in messages, details and region fingerprints (spec sections 3.13.6, 4 and 4.4).

use std::fmt;

/// A number written rounded to 6 decimal places with Rust's `Display` for `f64`, a rounded zero written `0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rounded(pub(crate) f64);

impl Rounded {
    /// The value rounded to 6 decimal places, with a rounded zero (including `-0`) as `0.0`.
    pub(crate) fn value(self) -> f64 {
        let rounded = (self.0 * 1e6).round() / 1e6;
        if rounded == 0.0 { 0.0 } else { rounded }
    }
}

impl fmt::Display for Rounded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.value())
    }
}

/// A sketch point written as `(u, v)` with both coordinates in the rounded form.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RoundedPoint(pub(crate) [f64; 2]);

impl fmt::Display for RoundedPoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "({}, {})",
            Rounded(self.0[0]),
            Rounded(self.0[1])
        )
    }
}

/// A sketch point written as `u,v` with both coordinates in the rounded form, as details write points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RoundedPair(pub(crate) [f64; 2]);

impl fmt::Display for RoundedPair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{},{}", Rounded(self.0[0]), Rounded(self.0[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integral_values_print_without_a_fraction() {
        assert_eq!(Rounded(-1.0).to_string(), "-1");
        assert_eq!(Rounded(4000.0).to_string(), "4000");
    }

    #[test]
    fn fractions_keep_their_shortest_form() {
        assert_eq!(Rounded(2.5).to_string(), "2.5");
    }

    #[test]
    fn values_round_to_six_decimals() {
        assert_eq!(Rounded(std::f64::consts::PI).to_string(), "3.141593");
        assert_eq!(Rounded(-2.0 / 3.0).to_string(), "-0.666667");
    }

    #[test]
    fn rounded_zero_prints_as_zero() {
        assert_eq!(Rounded(1e-9).to_string(), "0");
        assert_eq!(Rounded(-0.0).to_string(), "0");
        assert_eq!(Rounded(-1e-7).to_string(), "0");
    }

    #[test]
    fn rounded_zero_value_is_positive_zero() {
        assert_eq!(Rounded(-1e-9).value().to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn points_print_in_parentheses() {
        assert_eq!(RoundedPoint([1.0000004, -0.0]).to_string(), "(1, 0)");
        assert_eq!(RoundedPoint([80.0, 50.0]).to_string(), "(80, 50)");
    }

    #[test]
    fn pairs_print_without_parentheses_or_spaces() {
        assert_eq!(RoundedPair([0.0, 0.3]).to_string(), "0,0.3");
        assert_eq!(RoundedPair([-0.0, 1.0 / 3.0]).to_string(), "0,0.333333");
    }
}
