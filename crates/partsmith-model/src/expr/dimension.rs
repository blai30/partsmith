//! Physical dimensions, quantities and their display text (expressions.md sections 3.7 and 3.16).

use std::f64::consts::PI;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Units;

/// Radians in one degree.
const RADIANS_PER_DEGREE: f64 = PI / 180.0;

/// Display values below this magnitude are rounded to six decimal places.
const ROUNDING_LIMIT: f64 = 1e15;

/// Physical dimension as integer exponents of length and angle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Dimension {
    /// Exponent of length.
    pub length: i8,
    /// Exponent of angle.
    pub angle: i8,
}

impl Dimension {
    /// A plain number (dimensionless).
    pub const NONE: Dimension = Dimension {
        length: 0,
        angle: 0,
    };
    /// Length.
    pub const LENGTH: Dimension = Dimension {
        length: 1,
        angle: 0,
    };
    /// Area.
    pub const AREA: Dimension = Dimension {
        length: 2,
        angle: 0,
    };
    /// Volume.
    pub const VOLUME: Dimension = Dimension {
        length: 3,
        angle: 0,
    };
    /// Angle.
    pub const ANGLE: Dimension = Dimension {
        length: 0,
        angle: 1,
    };
    /// Largest absolute exponent an evaluation may produce.
    pub const MAX_EXPONENT: i8 = 16;

    /// True for `NONE`.
    pub fn is_plain(self) -> bool {
        self == Dimension::NONE
    }

    /// Words for messages, such as "a length" (expressions.md section 3.7).
    pub fn describe(self) -> String {
        match self {
            Dimension::NONE => "a plain number".to_string(),
            Dimension::LENGTH => "a length".to_string(),
            Dimension::AREA => "an area".to_string(),
            Dimension::VOLUME => "a volume".to_string(),
            Dimension::ANGLE => "an angle".to_string(),
            other => format!("a quantity in {}", other.label("mm", "rad")),
        }
    }

    /// Display unit label, such as "mm^2" (expressions.md section 3.16).
    pub fn unit_label(self, units: Units) -> String {
        self.label(units.symbol(), "deg")
    }

    /// Internal units in one display unit of this dimension (expressions.md section 3.7).
    pub fn display_factor(self, units: Units) -> f64 {
        units.millimeters().powi(i32::from(self.length))
            * RADIANS_PER_DEGREE.powi(i32::from(self.angle))
    }

    /// The dimension with these exponents, or `None` when one is beyond `MAX_EXPONENT`.
    pub(crate) fn from_exponents(length: i32, angle: i32) -> Option<Dimension> {
        let exponent = |value: i32| {
            i8::try_from(value)
                .ok()
                .filter(|exponent| exponent.abs() <= Dimension::MAX_EXPONENT)
        };
        Some(Dimension {
            length: exponent(length)?,
            angle: exponent(angle)?,
        })
    }

    /// The dimension of a product, or `None` on overflow.
    pub(crate) fn times(self, other: Dimension) -> Option<Dimension> {
        Dimension::from_exponents(
            i32::from(self.length) + i32::from(other.length),
            i32::from(self.angle) + i32::from(other.angle),
        )
    }

    /// The dimension of a quotient, or `None` on overflow.
    pub(crate) fn divided_by(self, other: Dimension) -> Option<Dimension> {
        Dimension::from_exponents(
            i32::from(self.length) - i32::from(other.length),
            i32::from(self.angle) - i32::from(other.angle),
        )
    }

    /// The dimension of a square root, or `None` when an exponent is odd.
    pub(crate) fn square_root(self) -> Option<Dimension> {
        let half = |exponent: i8| (exponent % 2 == 0).then_some(exponent / 2);
        Some(Dimension {
            length: half(self.length)?,
            angle: half(self.angle)?,
        })
    }

    /// The symbols with their exponents, each omitted when zero, `^n` only when n is not 1, joined by `*`.
    fn label(self, length_symbol: &str, angle_symbol: &str) -> String {
        [(length_symbol, self.length), (angle_symbol, self.angle)]
            .into_iter()
            .filter(|(_, exponent)| *exponent != 0)
            .map(|(symbol, exponent)| match exponent {
                1 => symbol.to_string(),
                _ => format!("{symbol}^{exponent}"),
            })
            .collect::<Vec<_>>()
            .join("*")
    }
}

/// A value in internal units (millimeters and radians) with its dimension.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Quantity {
    /// The value in internal units.
    pub value: f64,
    /// Its dimension.
    pub dimension: Dimension,
}

impl Quantity {
    /// A quantity with this value and dimension.
    pub fn new(value: f64, dimension: Dimension) -> Quantity {
        Quantity { value, dimension }
    }

    /// A plain number.
    pub fn plain(value: f64) -> Quantity {
        Quantity::new(value, Dimension::NONE)
    }

    /// A length in millimeters.
    pub fn length(millimeters: f64) -> Quantity {
        Quantity::new(millimeters, Dimension::LENGTH)
    }

    /// An angle in radians.
    pub fn angle(radians: f64) -> Quantity {
        Quantity::new(radians, Dimension::ANGLE)
    }

    /// Display text in document units, which parses back to the same quantity (expressions.md section 3.16).
    pub fn display(&self, units: Units) -> String {
        let mut number = self.value / self.dimension.display_factor(units);
        if number.abs() < ROUNDING_LIMIT {
            number = (number * 1e6).round() / 1e6;
        }
        if number == 0.0 {
            // Negative zero displays as 0.
            number = 0.0;
        }
        let label = self.dimension.unit_label(units);
        match self.dimension {
            Dimension::NONE => format!("{number}"),
            Dimension::LENGTH | Dimension::ANGLE => format!("{number} {label}"),
            _ => format!("{number} * {label}"),
        }
    }

    /// This plain quantity read in display units of `dimension` (promotion, expressions.md section 3.7).
    pub(crate) fn promoted(self, dimension: Dimension, units: Units) -> Quantity {
        Quantity::new(self.value * dimension.display_factor(units), dimension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PER_ANGLE: Dimension = Dimension {
        length: 0,
        angle: -2,
    };

    #[test]
    fn only_none_is_plain() {
        assert!(Dimension::NONE.is_plain());
        assert!(!Dimension::ANGLE.is_plain());
    }

    #[test]
    fn other_dimensions_are_described_in_internal_units() {
        assert_eq!(PER_ANGLE.describe(), "a quantity in rad^-2");
        assert_eq!(
            Dimension {
                length: 4,
                angle: 0
            }
            .describe(),
            "a quantity in mm^4"
        );
    }

    #[test]
    fn angle_labels_use_degrees() {
        assert_eq!(Dimension::ANGLE.unit_label(Units::In), "deg");
        assert_eq!(PER_ANGLE.unit_label(Units::Mm), "deg^-2");
    }

    #[test]
    fn display_factor_of_none_is_one() {
        assert_eq!(Dimension::NONE.display_factor(Units::In), 1.0);
    }

    #[test]
    fn display_factor_of_volume_in_centimeters() {
        assert_eq!(Dimension::VOLUME.display_factor(Units::Cm), 1000.0);
    }

    #[test]
    fn products_add_exponents() {
        assert_eq!(
            Dimension::LENGTH.times(Dimension::AREA),
            Some(Dimension::VOLUME)
        );
        assert_eq!(
            Dimension::AREA.divided_by(Dimension::LENGTH),
            Some(Dimension::LENGTH)
        );
    }

    #[test]
    fn exponents_beyond_sixteen_overflow() {
        let largest = Dimension::from_exponents(16, -16).expect("16 is allowed");

        assert_eq!(largest.times(Dimension::LENGTH), None);
        assert_eq!(largest.divided_by(Dimension::ANGLE), None);
        assert_eq!(Dimension::from_exponents(1000, 0), None);
    }

    #[test]
    fn square_roots_halve_even_exponents() {
        assert_eq!(Dimension::AREA.square_root(), Some(Dimension::LENGTH));
        assert_eq!(PER_ANGLE.square_root(), Some(Dimension { length: 0, angle: -1 }));
        assert_eq!(Dimension::VOLUME.square_root(), None);
    }

    #[test]
    fn negative_values_display_with_a_sign() {
        assert_eq!(Quantity::length(-12.5).display(Units::Mm), "-12.5 mm");
    }

    #[test]
    fn negative_zero_displays_as_zero() {
        assert_eq!(Quantity::length(-0.0).display(Units::Cm), "0 cm");
    }

    #[test]
    fn plain_numbers_promote_in_display_units() {
        assert_eq!(
            Quantity::plain(2.0).promoted(Dimension::LENGTH, Units::Cm),
            Quantity::length(20.0)
        );
    }
}
