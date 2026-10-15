//! Message building blocks: current names, value formatting, nouns and reasons (features.md sections 2.8 and 3.12).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use partsmith_model::{Dimension, Document, FeatureRecord, FieldUnit, Id, Quantity, Units};
use partsmith_naming::ElementKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::output::{FeatureOutput, OutputData, SketchResult};
use crate::registry::Registry;

/// A limit in a range error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bound {
    /// The value must be greater than this.
    GreaterThan(f64),
    /// The value must be this or more.
    AtLeast(f64),
    /// The value must be less than this.
    LessThan(f64),
    /// The value must be this or less.
    AtMost(f64),
}

impl Bound {
    /// The limit value in internal units.
    pub fn value(self) -> f64 {
        match self {
            Bound::GreaterThan(value)
            | Bound::AtLeast(value)
            | Bound::LessThan(value)
            | Bound::AtMost(value) => value,
        }
    }
}

/// The kind of thing a selection or lookup expected, for messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Noun {
    /// A body.
    Body,
    /// A sketch.
    Sketch,
    /// A datum plane.
    DatumPlane,
    /// A sketch region.
    Region,
    /// A sketch point.
    Point,
    /// A sketch line.
    Line,
    /// A face.
    Face,
    /// An edge.
    Edge,
}

impl fmt::Display for Noun {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Noun::Body => "body",
            Noun::Sketch => "sketch",
            Noun::DatumPlane => "datum plane",
            Noun::Region => "region",
            Noun::Point => "point",
            Noun::Line => "line",
            Noun::Face => "face",
            Noun::Edge => "edge",
        })
    }
}

/// Why a feature ignored part of its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IgnoredReason {
    /// A selected edge whose faces meet without a corner.
    SmoothEdge,
    /// A tip angle of a hole that goes through all.
    TipAngleThroughAll,
}

/// Why a sketch on a face used its fallback placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    /// The face can no longer be found, or its body is empty.
    Missing,
    /// The face reference matches several faces.
    Ambiguous,
    /// The face is no longer flat.
    NotPlanar,
}

/// Looks up what messages need about the document: names, types, records and sketch results (section 3.12).
pub trait Names {
    /// The body's name, if the body exists.
    fn body_name(&self, body: &Id) -> Option<String>;
    /// The feature's label (section 3.12), if the feature exists.
    fn feature_label(&self, feature: &Id) -> Option<String>;
    /// The manifest label of the feature's type, for example "Fillet" (the type name for an unknown type), if the
    /// feature exists. `{this}` uses it (section 3.12).
    fn type_label(&self, feature: &Id) -> Option<String>;
    /// The feature's record, for element descriptions and sketch messages (section 3.12).
    fn feature_record(&self, feature: &Id) -> Option<&FeatureRecord>;
    /// The sketch's result in the current run, when it built.
    fn sketch_result(&self, sketch: &Id) -> Option<&SketchResult>;
}

/// `Names` for a document, using the registry for default feature labels and type labels.
#[derive(Clone, Copy, Debug)]
pub struct DocumentNames<'a> {
    /// The document.
    pub document: &'a Document,
    /// The registry.
    pub registry: &'a Registry,
    /// Outputs of the current run by feature id, for sketch results; empty outside a run.
    pub outputs: &'a BTreeMap<Id, Arc<FeatureOutput>>,
}

impl Names for DocumentNames<'_> {
    fn body_name(&self, body: &Id) -> Option<String> {
        self.document.body(body).map(|found| found.name.clone())
    }

    fn feature_label(&self, feature: &Id) -> Option<String> {
        self.registry.label(self.document, feature)
    }

    fn type_label(&self, feature: &Id) -> Option<String> {
        let record = self.document.feature(feature)?;
        Some(self.registry.get(&record.kind).map_or_else(
            || record.kind.clone(),
            |registered| registered.manifest().label.clone(),
        ))
    }

    fn feature_record(&self, feature: &Id) -> Option<&FeatureRecord> {
        self.document.feature(feature)
    }

    fn sketch_result(&self, sketch: &Id) -> Option<&SketchResult> {
        match &self.outputs.get(sketch)?.data {
            OutputData::Sketch(result) => Some(result),
            _ => None,
        }
    }
}

/// `Names` that names every body and feature by its id and knows no types, records or sketch results. `Display`
/// uses it (section 2.8).
pub(crate) struct IdNames;

impl Names for IdNames {
    fn body_name(&self, body: &Id) -> Option<String> {
        Some(body.to_string())
    }

    fn feature_label(&self, feature: &Id) -> Option<String> {
        Some(feature.to_string())
    }

    fn type_label(&self, _feature: &Id) -> Option<String> {
        None
    }

    fn feature_record(&self, _feature: &Id) -> Option<&FeatureRecord> {
        None
    }

    fn sketch_result(&self, _sketch: &Id) -> Option<&SketchResult> {
        None
    }
}

/// Formats a value in display units for messages and suggestions (section 3.12).
///
/// Lengths and angles render as `Quantity::display` does (6 decimals, trimmed, with the unit);
/// plain numbers and counts render the same way without a unit, so a whole count is an integer.
pub fn format_value(kind: FieldUnit, value: f64, units: Units) -> String {
    let quantity = match kind {
        FieldUnit::Length => Quantity::length(value),
        FieldUnit::Angle => Quantity::angle(value),
        FieldUnit::Scalar | FieldUnit::Count => Quantity::plain(value),
    };
    quantity.display(units)
}

/// The text of a computed length limit (section 3.12): the value in display units rounded down to
/// two significant digits, then formatted, so it never suggests more than what worked.
pub(crate) fn suggested_limit(millimeters: f64, units: Units) -> String {
    let factor = Dimension::LENGTH.display_factor(units);
    let truncated = round_down_to_two_digits(millimeters / factor);
    format_value(FieldUnit::Length, truncated * factor, units)
}

/// `value` rounded toward zero to two significant digits. Zero, negative and non-finite values
/// are returned unchanged.
///
/// The digits are cut from the shortest decimal text that reads back as `value`, so a value
/// already at two digits keeps them (2.3, although 2.3 / 0.1 is 22.999999999999996 in binary),
/// and the result never exceeds `value`: the cut text is at most that decimal, and reading
/// decimals rounds monotonically.
fn round_down_to_two_digits(value: f64) -> f64 {
    if !(value.is_finite() && value > 0.0) {
        return value;
    }
    // `{:e}` writes the shortest round-trip mantissa and its exponent, for example `2.396e0`.
    let text = format!("{value:e}");
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("scientific notation always has an exponent");
    let exponent: i32 = exponent
        .parse()
        .expect("the exponent of scientific notation is an integer");
    let digits: String = mantissa
        .chars()
        .filter(char::is_ascii_digit)
        .take(2)
        .collect();
    let shift = i32::try_from(digits.len()).expect("at most two digits") - 1;
    format!("{digits}e{}", exponent - shift)
        .parse()
        .expect("digits followed by an exponent are a number")
}

/// The plural of an element kind: faces, edges, vertices.
pub(crate) fn plural(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::Face => "faces",
        ElementKind::Edge => "edges",
        ElementKind::Vertex => "vertices",
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn nouns_render_lowercase_words() {
        assert_eq!(Noun::DatumPlane.to_string(), "datum plane");
        assert_eq!(Noun::Edge.to_string(), "edge");
    }

    #[test]
    fn bound_value_is_the_limit() {
        assert_eq!(Bound::LessThan(5.0).value(), 5.0);
        assert_eq!(Bound::AtLeast(-1.0).value(), -1.0);
    }

    #[test]
    fn reasons_serialize_snake_case() {
        assert_eq!(
            serde_json::to_value(FallbackReason::NotPlanar).expect("serializes"),
            "not_planar"
        );
        assert_eq!(
            serde_json::to_value(IgnoredReason::TipAngleThroughAll).expect("serializes"),
            "tip_angle_through_all"
        );
    }

    #[test]
    fn angles_render_in_degrees_in_any_document() {
        assert_eq!(format_value(FieldUnit::Angle, PI, Units::In), "180 deg");
    }

    #[test]
    fn counts_render_as_integers() {
        assert_eq!(
            format_value(FieldUnit::Count, 4294967295.0, Units::Mm),
            "4294967295"
        );
    }

    #[test]
    fn limits_round_down_to_two_significant_digits() {
        assert_eq!(suggested_limit(2.396, Units::Mm), "2.3 mm");
        assert_eq!(suggested_limit(2.396, Units::In), "0.094 in");
        assert_eq!(suggested_limit(0.0789, Units::Mm), "0.078 mm");
        assert_eq!(suggested_limit(123.0, Units::Mm), "120 mm");
    }

    #[test]
    fn limits_already_at_two_digits_keep_them() {
        assert_eq!(suggested_limit(2.3, Units::Mm), "2.3 mm");
        assert_eq!(suggested_limit(0.1, Units::Mm), "0.1 mm");
        assert_eq!(suggested_limit(25.4, Units::In), "1 in");
    }

    #[test]
    fn limits_just_under_two_digits_round_down() {
        assert_eq!(suggested_limit(2.39999999995, Units::Mm), "2.3 mm");
        assert_eq!(suggested_limit(0.99999999999, Units::Mm), "0.99 mm");
        assert_eq!(suggested_limit(19.9999999999, Units::Mm), "19 mm");
    }

    #[test]
    fn limits_never_exceed_the_value() {
        for value in [
            2.39999999995,
            0.30000000000000004,
            1.0,
            7.77,
            99.99999999999,
            1e-7,
        ] {
            assert!(round_down_to_two_digits(value) <= value, "value {value}");
        }
    }

    #[test]
    fn limits_in_centimeters_and_meters() {
        assert_eq!(suggested_limit(2.396, Units::Cm), "0.23 cm");
        assert_eq!(suggested_limit(1999.0, Units::M), "1.9 m");
    }

    #[test]
    fn zero_limit_renders_zero() {
        assert_eq!(suggested_limit(0.0, Units::Mm), "0 mm");
    }

    #[test]
    fn plurals_of_element_kinds() {
        assert_eq!(plural(ElementKind::Vertex), "vertices");
        assert_eq!(plural(ElementKind::Face), "faces");
    }
}
