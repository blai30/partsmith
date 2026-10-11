//! Units an expression can name (expressions.md section 3.6).

use std::f64::consts::PI;

use super::Dimension;

/// Units an expression can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    /// `um`
    Micrometer,
    /// `mm`
    Millimeter,
    /// `cm`
    Centimeter,
    /// `m`
    Meter,
    /// `in`
    Inch,
    /// `ft`
    Foot,
    /// `deg` or the degree sign
    Degree,
    /// `rad`
    Radian,
}

impl Unit {
    /// Every unit, in declaration order.
    pub const ALL: [Unit; 8] = [
        Unit::Micrometer,
        Unit::Millimeter,
        Unit::Centimeter,
        Unit::Meter,
        Unit::Inch,
        Unit::Foot,
        Unit::Degree,
        Unit::Radian,
    ];

    /// The unit's name: "um", "mm", "cm", "m", "in", "ft", "deg" or "rad".
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Micrometer => "um",
            Unit::Millimeter => "mm",
            Unit::Centimeter => "cm",
            Unit::Meter => "m",
            Unit::Inch => "in",
            Unit::Foot => "ft",
            Unit::Degree => "deg",
            Unit::Radian => "rad",
        }
    }

    /// The unit for a name; also maps "°" (U+00B0) to `Degree`.
    pub fn from_symbol(symbol: &str) -> Option<Unit> {
        if symbol == "°" {
            return Some(Unit::Degree);
        }
        Unit::ALL.into_iter().find(|unit| unit.symbol() == symbol)
    }

    /// `Dimension::LENGTH` or `Dimension::ANGLE`.
    pub fn dimension(self) -> Dimension {
        match self {
            Unit::Degree | Unit::Radian => Dimension::ANGLE,
            Unit::Micrometer
            | Unit::Millimeter
            | Unit::Centimeter
            | Unit::Meter
            | Unit::Inch
            | Unit::Foot => Dimension::LENGTH,
        }
    }

    /// Internal units (millimeters or radians) in one of this unit.
    pub fn factor(self) -> f64 {
        match self {
            Unit::Micrometer => 0.001,
            Unit::Millimeter => 1.0,
            Unit::Centimeter => 10.0,
            Unit::Meter => 1000.0,
            Unit::Inch => 25.4,
            Unit::Foot => 304.8,
            Unit::Degree => PI / 180.0,
            Unit::Radian => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_round_trip() {
        for unit in Unit::ALL {
            assert_eq!(Unit::from_symbol(unit.symbol()), Some(unit));
        }
    }

    #[test]
    fn unknown_symbols_are_none() {
        assert_eq!(Unit::from_symbol("inch"), None);
        assert_eq!(Unit::from_symbol(""), None);
    }

    #[test]
    fn angles_and_lengths() {
        assert_eq!(Unit::Radian.dimension(), Dimension::ANGLE);
        assert_eq!(Unit::Foot.dimension(), Dimension::LENGTH);
    }

    #[test]
    fn degrees_convert_to_radians() {
        assert_eq!(Unit::Degree.factor() * 180.0, PI);
    }
}
