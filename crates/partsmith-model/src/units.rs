//! Document units, colors and body roles (model.md section 2.4).

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::expr::Unit;

/// Regular expression every serialized color matches.
const COLOR_PATTERN: &str = "^#[0-9A-Fa-f]{6}$";

/// Document units: the unit of bare numbers in length expressions and the display unit for lengths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    /// Millimeters, `"mm"`.
    #[default]
    Mm,
    /// Centimeters, `"cm"`.
    Cm,
    /// Meters, `"m"`.
    M,
    /// Inches, `"in"`.
    In,
}

impl Units {
    /// Every document unit, in declaration order.
    pub const ALL: [Units; 4] = [Units::Mm, Units::Cm, Units::M, Units::In];

    /// Millimeters in one of this unit: 1, 10, 1000 or 25.4.
    pub fn millimeters(self) -> f64 {
        self.unit().factor()
    }

    /// The symbol: "mm", "cm", "m" or "in".
    pub fn symbol(self) -> &'static str {
        self.unit().symbol()
    }

    /// The matching expression unit (expressions.md section 2).
    pub fn unit(self) -> Unit {
        match self {
            Units::Mm => Unit::Millimeter,
            Units::Cm => Unit::Centimeter,
            Units::M => Unit::Meter,
            Units::In => Unit::Inch,
        }
    }
}

impl FromStr for Units {
    type Err = UnitsError;

    fn from_str(text: &str) -> Result<Units, UnitsError> {
        Units::ALL
            .into_iter()
            .find(|units| units.symbol() == text)
            .ok_or_else(|| UnitsError::Unknown {
                text: text.to_string(),
            })
    }
}

impl fmt::Display for Units {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.symbol())
    }
}

impl<'de> Deserialize<'de> for Units {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Units, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Text that is not one of the four document units.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum UnitsError {
    /// The text is not `mm`, `cm`, `m` or `in`.
    #[error("'{text}' is not a document unit. Use mm, cm, m or in.")]
    Unknown {
        /// The rejected text.
        text: String,
    },
}

/// An sRGB color, serialized as "#rrggbb" with lowercase hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
}

impl Color {
    /// Creates a color from its channels.
    pub fn new(red: u8, green: u8, blue: u8) -> Color {
        Color { red, green, blue }
    }
}

impl FromStr for Color {
    type Err = ColorError;

    fn from_str(text: &str) -> Result<Color, ColorError> {
        let rgb = text
            .strip_prefix('#')
            .filter(|digits| digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .ok_or_else(|| ColorError::Invalid {
                text: text.to_string(),
            })?;
        let [_, red, green, blue] = rgb.to_be_bytes();
        Ok(Color::new(red, green, blue))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "#{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue
        )
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Color {
    fn schema_name() -> Cow<'static, str> {
        "Color".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Color").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": COLOR_PATTERN})
    }
}

/// Text that is not a `#rrggbb` color.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ColorError {
    /// The text is not `#` followed by six hex digits.
    #[error("'{text}' is not a color. Use # followed by six hex digits, such as #3b82f6.")]
    Invalid {
        /// The rejected text.
        text: String,
    },
}

/// What a body is for when it is printed.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BodyRole {
    /// A printed part (default, omitted when saved).
    #[default]
    Part,
    /// Negative volume: subtracted by the slicer.
    NegativeVolume,
    /// Region where the slicer must not place supports.
    SupportBlocker,
    /// Region where the slicer must place supports.
    SupportEnforcer,
    /// Region with its own slicer settings (the settings themselves come later).
    SettingsModifier,
}

impl BodyRole {
    /// True for `Part`; used to omit the default when saving.
    pub fn is_part(&self) -> bool {
        *self == BodyRole::Part
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_display_their_symbol() {
        let symbols: Vec<String> = Units::ALL.iter().map(ToString::to_string).collect();

        assert_eq!(symbols, ["mm", "cm", "m", "in"]);
    }

    #[test]
    fn units_parse_only_exact_symbols() {
        for units in Units::ALL {
            assert_eq!(units.symbol().parse::<Units>(), Ok(units));
        }
        assert!("Mm".parse::<Units>().is_err());
        assert!(" mm".parse::<Units>().is_err());
    }

    #[test]
    fn units_map_to_expression_units() {
        assert_eq!(Units::Cm.unit(), Unit::Centimeter);
        assert_eq!(Units::In.unit(), Unit::Inch);
    }

    #[test]
    fn invalid_units_report_the_units_message() {
        let error = serde_json::from_str::<Units>("\"yd\"").expect_err("yd is not a unit");

        assert!(
            error
                .to_string()
                .contains("'yd' is not a document unit. Use mm, cm, m or in."),
            "{error}"
        );
    }

    #[test]
    fn colors_display_lowercase_and_zero_padded() {
        assert_eq!(Color::new(0, 10, 255).to_string(), "#000aff");
    }

    #[test]
    fn colors_reject_non_ascii_text_of_the_right_length() {
        assert!("#ééé".parse::<Color>().is_err());
    }

    #[test]
    fn colors_reject_a_sign_inside_the_digits() {
        assert!("#+12345".parse::<Color>().is_err());
    }

    #[test]
    fn invalid_colors_report_the_color_message() {
        let error = serde_json::from_str::<Color>("\"red\"").expect_err("red is not a color");

        assert!(
            error.to_string().contains("'red' is not a color."),
            "{error}"
        );
    }

    #[test]
    fn color_schema_is_a_string_pattern() {
        let schema = schemars::schema_for!(Color);

        assert_eq!(
            schema.get("pattern"),
            Some(&serde_json::json!(COLOR_PATTERN))
        );
    }

    #[test]
    fn only_part_is_the_default_role() {
        assert!(BodyRole::Part.is_part());
        assert!(!BodyRole::SupportEnforcer.is_part());
    }

    #[test]
    fn roles_deserialize_from_snake_case() {
        assert_eq!(
            serde_json::from_str::<BodyRole>("\"negative_volume\"").ok(),
            Some(BodyRole::NegativeVolume)
        );
    }
}
