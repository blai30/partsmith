//! Message building blocks: current names, value formatting, nouns, reasons and repair data (features.md sections 2.8 and 3.12).

use std::fmt;

use partsmith_model::{Dimension, Document, FieldUnit, Id, Quantity, Units};
use partsmith_naming::{ElementKind, ElementName, Reference};
use partsmith_sketch::RegionRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// Data the repair panel needs to show a lost reference and re-pick it.
#[derive(Clone, Debug, PartialEq)]
pub enum Repair {
    /// A face, edge or vertex reference. `pointer` is the JSON Pointer of the reference in the record.
    Element {
        /// JSON Pointer of the reference in the feature record, for example "/edges/1".
        pointer: String,
        /// The body the reference is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// True when the reference matches several elements; false when it matches none.
        ambiguous: bool,
        /// The elements an ambiguous reference matches, best first.
        candidates: Vec<ElementName>,
    },
    /// A profile region. `pointer` is the JSON Pointer of the region ref in the record.
    Region {
        /// JSON Pointer of the region ref in the feature record, for example "/profile/regions/0".
        pointer: String,
        /// The sketch the region belongs to.
        sketch: Id,
        /// The stored region ref.
        region: RegionRef,
        /// True when the region ref matches several regions; false when it matches none.
        ambiguous: bool,
    },
}

/// Looks up current display names for ids in messages.
pub trait Names {
    /// The body's name, if the body exists.
    fn body_name(&self, body: &Id) -> Option<String>;
    /// The feature's label (section 3.12), if the feature exists.
    fn feature_label(&self, feature: &Id) -> Option<String>;
}

/// `Names` for a document, using the registry for default feature labels.
#[derive(Clone, Copy, Debug)]
pub struct DocumentNames<'a> {
    /// The document.
    pub document: &'a Document,
    /// The registry.
    pub registry: &'a Registry,
}

impl Names for DocumentNames<'_> {
    fn body_name(&self, body: &Id) -> Option<String> {
        self.document.body(body).map(|found| found.name.clone())
    }

    fn feature_label(&self, feature: &Id) -> Option<String> {
        self.registry.label(self.document, feature)
    }
}

/// `Names` that knows no names, so every id renders as itself. Used by `Display`.
pub(crate) struct IdNames;

impl Names for IdNames {
    fn body_name(&self, _body: &Id) -> Option<String> {
        None
    }

    fn feature_label(&self, _feature: &Id) -> Option<String> {
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

/// Relative slack that keeps a value already at two significant digits, such as 2.3, from
/// dropping a digit through binary rounding (2.3 / 0.1 is 22.999999999999996).
const DIGIT_SLACK: f64 = 1e-9;

/// `value` rounded toward zero to two significant digits. Zero, negative and non-finite values
/// are returned unchanged.
fn round_down_to_two_digits(value: f64) -> f64 {
    if !(value.is_finite() && value > 0.0) {
        return value;
    }
    let step = 10f64.powf(value.log10().floor() - 1.0);
    ((value / step) * (1.0 + DIGIT_SLACK)).floor() * step
}

/// Renders the parts of messages with current names and document units.
pub(crate) struct Renderer<'a> {
    /// Current names.
    pub(crate) names: &'a dyn Names,
    /// Document units.
    pub(crate) units: Units,
}

impl Renderer<'_> {
    /// The body's current name, or its id.
    pub(crate) fn body(&self, body: &Id) -> String {
        self.names
            .body_name(body)
            .unwrap_or_else(|| body.to_string())
    }

    /// The feature's current label, or its id.
    pub(crate) fn feature(&self, feature: &Id) -> String {
        self.names
            .feature_label(feature)
            .unwrap_or_else(|| feature.to_string())
    }

    /// A value in display units.
    pub(crate) fn value(&self, kind: FieldUnit, value: f64) -> String {
        format_value(kind, value, self.units)
    }

    /// A length in display units.
    pub(crate) fn length(&self, millimeters: f64) -> String {
        self.value(FieldUnit::Length, millimeters)
    }

    /// The text of a computed length limit.
    pub(crate) fn limit(&self, millimeters: f64) -> String {
        suggested_limit(millimeters, self.units)
    }
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
