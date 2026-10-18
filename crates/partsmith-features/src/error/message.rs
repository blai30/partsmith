//! Message building blocks: current names, value formatting, nouns and reasons (features.md sections 2.8 and 3.12).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use partsmith_model::{
    Dimension, Document, EmptyContext, Expr, FeatureManifest, FeatureRecord, FieldUnit, Id,
    Quantity, Scope, Units,
};
use partsmith_naming::ElementKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::field::FieldRef;
use crate::context::expected_dimension;
use crate::output::{AbsentFace, FeatureOutput, OutputData, SketchResult};
use crate::registry::Registry;

/// The magnitude in display units from which a value is written in exponent form (section 3.12).
const EXPONENT_FORM_FROM: f64 = 1e15;

/// The most significant digits of a value in exponent form (section 3.12).
const EXPONENT_FORM_DIGITS: usize = 6;

/// The decimals a value is usually written with (expressions.md section 3.16).
const USUAL_DECIMALS: usize = 6;

/// The most significant digits of a value written apart from a strict bound (section 3.12).
const APART_DIGITS: i32 = 15;

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

/// Why a sketch on a face could not be placed on it (section 3.6, features/sketch.md section 3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackCause {
    /// The body is empty or the face reference is lost.
    Missing,
    /// Several faces fit the reference.
    Ambiguous,
    /// A resolved face is not flat.
    NotPlanar,
    /// The resolved faces do not lie in one plane. Phase 2's coplanar check reports it.
    NotCoplanar,
}

/// Where a sketch that could not be placed on its face was placed instead (features/sketch.md section 3.3).
#[derive(Clone, Debug, PartialEq)]
pub enum Placed {
    /// On the face as it was just before `by`, the earlier writer of the body that removed it, moved with the body
    /// since.
    BeforeRemoval {
        /// The writer that removed the face.
        by: Id,
    },
    /// At the position saved with the sketch (`fallback`).
    Saved,
}

/// Why a join, cut, intersect or other feature changed nothing (section 3.15).
#[derive(Clone, Debug, PartialEq)]
pub enum NoEffectHow {
    /// A join whose tool lies entirely inside the body.
    Inside,
    /// A cut or intersect whose tool starts `gap` mm beyond the body along the feature's direction.
    Away {
        /// How far beyond the body the tool starts, in millimeters.
        gap: f64,
    },
    /// A cut or intersect whose tool ends `gap` mm short of the body along the feature's direction.
    Short {
        /// How far short of the body the tool ends, in millimeters.
        gap: f64,
    },
    /// Any other cut or intersect that changes nothing, or one without a direction.
    Beside,
    /// An intersect whose shape covers the whole body.
    Covers,
    /// A fillet or chamfer whose every edge selected for `field`, two or more, was skipped.
    Skipped {
        /// The edge list.
        field: FieldRef,
    },
    /// A hole whose every point, two or more, misses the body.
    Missed,
    /// A pattern whose every count is 1.
    NoCopies {
        /// Its count fields.
        fields: Vec<FieldRef>,
    },
    /// A move body that moves by zero and turns by whole turns or not at all.
    NotMoved,
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
    /// The faces the feature's output in the current run records absent (section 2.7), for element descriptions
    /// (section 3.12); none outside a run.
    fn absent_faces(&self, feature: &Id) -> &[AbsentFace] {
        let _ = feature;
        &[]
    }

    /// The document's feature records in timeline order, for element descriptions that follow faces from body to
    /// body (section 3.12); none outside a document.
    fn records(&self) -> &[FeatureRecord] {
        &[]
    }

    /// True when the feature has an output in the current run, for element descriptions that follow faces from body
    /// to body (section 3.12); false outside a run.
    fn built(&self, feature: &Id) -> bool {
        let _ = feature;
        false
    }

    /// The value of a record's expression in internal units, evaluated as a field of `unit` with the document's
    /// parameters (dotted names do not resolve), for element descriptions (section 3.12); None when it does not
    /// evaluate, and outside a document.
    fn value(&self, expr: &Expr, unit: FieldUnit) -> Option<f64> {
        let _ = (expr, unit);
        None
    }

    /// The manifest of the feature's type when the feature exists and its type is known, for the fixes that read a
    /// field's hint (section 4.1, `invalid_params`); none outside a document, which the provided method returns.
    fn manifest(&self, feature: &Id) -> Option<&FeatureManifest> {
        let _ = feature;
        None
    }
}

/// `Names` for a document, using the registry for default feature labels and type labels.
#[derive(Clone, Copy, Debug)]
pub struct DocumentNames<'a> {
    /// The document.
    pub document: &'a Document,
    /// The registry.
    pub registry: &'a Registry,
    /// Outputs of the current run by feature id, for sketch results and absent faces; empty outside a run.
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

    fn absent_faces(&self, feature: &Id) -> &[AbsentFace] {
        self.outputs
            .get(feature)
            .map_or(&[], |output| output.absent.as_slice())
    }

    fn records(&self) -> &[FeatureRecord] {
        &self.document.features
    }

    fn built(&self, feature: &Id) -> bool {
        self.outputs.contains_key(feature)
    }

    fn value(&self, expr: &Expr, unit: FieldUnit) -> Option<f64> {
        let parameters = self.document.evaluate_parameters(&EmptyContext);
        let scope = Scope {
            units: self.document.units,
            parameters: &parameters,
            context: &EmptyContext,
        };
        expr.evaluate(&scope, Some(expected_dimension(unit)))
            .ok()
            .map(|quantity| quantity.value)
    }

    fn manifest(&self, feature: &Id) -> Option<&FeatureManifest> {
        let record = self.document.feature(feature)?;
        self.registry
            .get(&record.kind)
            .map(|registered| registered.manifest())
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
/// plain numbers and counts render the same way without a unit, so a whole count is an integer. A
/// finite value whose magnitude in display units is at least 1e15 is written in exponent form with
/// at most 6 significant digits, `1e200 mm`, never as hundreds of digits; an infinite one renders
/// as `Quantity::display` writes it.
pub fn format_value(kind: FieldUnit, value: f64, units: Units) -> String {
    let number = display_number(kind, value, units);
    if number.is_finite() && number.abs() >= EXPONENT_FORM_FROM {
        return with_unit_label(kind, exponent_form(number), units);
    }
    let quantity = match kind {
        FieldUnit::Length => Quantity::length(value),
        FieldUnit::Angle => Quantity::angle(value),
        FieldUnit::Scalar | FieldUnit::Count => Quantity::plain(value),
    };
    quantity.display(units)
}

/// `value` as `format_value` writes it, with more decimals when it would read like `bound`: the fewest that tell
/// it apart, up to 15 significant digits (section 3.12, "Values next to a strict bound"), so a message never says
/// that a value is out of range while showing the bound itself.
pub(crate) fn format_value_apart(kind: FieldUnit, value: f64, bound: f64, units: Units) -> String {
    let usual = format_value(kind, value, units);
    if usual != format_value(kind, bound, units) {
        return usual;
    }
    let number = display_number(kind, value, units);
    let limit = display_number(kind, bound, units);
    if number == 0.0 || !number.is_finite() {
        return usual;
    }
    let magnitude = number.abs().log10().floor();
    // The decimals that keep the value within the significant digits; `magnitude` is a small whole number.
    let most = usize::try_from(APART_DIGITS - 1 - magnitude as i32).unwrap_or(0);
    (USUAL_DECIMALS + 1..=most)
        .map(|decimals| (fixed(number, decimals), fixed(limit, decimals)))
        .find(|(value, limit)| value != limit)
        .map_or(usual, |(value, _)| with_unit_label(kind, value, units))
}

/// The value in display units: millimeters, radians and plain numbers divided by the display factor.
fn display_number(kind: FieldUnit, value: f64, units: Units) -> f64 {
    value / dimension_of(kind).display_factor(units)
}

/// The dimension values of a field unit have.
fn dimension_of(kind: FieldUnit) -> Dimension {
    match kind {
        FieldUnit::Length => Dimension::LENGTH,
        FieldUnit::Angle => Dimension::ANGLE,
        FieldUnit::Scalar | FieldUnit::Count => Dimension::NONE,
    }
}

/// `number`, already in display units, followed by the unit of `kind` as `Quantity::display` writes it.
fn with_unit_label(kind: FieldUnit, number: String, units: Units) -> String {
    match kind {
        FieldUnit::Length | FieldUnit::Angle => {
            format!("{number} {}", dimension_of(kind).unit_label(units))
        }
        FieldUnit::Scalar | FieldUnit::Count => number,
    }
}

/// `number` in exponent form with at most 6 significant digits and no trailing zeros: `1e200`, `-1.5e20`.
fn exponent_form(number: f64) -> String {
    let text = format!(
        "{number:.precision$e}",
        precision = EXPONENT_FORM_DIGITS - 1
    );
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("scientific notation always has an exponent");
    format!("{}e{exponent}", without_trailing_zeros(mantissa))
}

/// `number` with `decimals` decimals, trailing zeros removed, and negative zero written as 0.
fn fixed(number: f64, decimals: usize) -> String {
    let text = format!("{number:.decimals$}");
    let trimmed = without_trailing_zeros(&text);
    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// A decimal text without the zeros that end its decimals, nor a decimal point left last: `2.50` is `2.5` and `3.0`
/// is `3`, while a whole number keeps its zeros.
fn without_trailing_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

/// The text of a computed length limit (section 3.12): the value in display units rounded down to
/// two significant digits, then formatted, so it never suggests more than what worked.
pub(crate) fn suggested_limit(millimeters: f64, units: Units) -> String {
    rounded_length(millimeters, units, Rounding::Down)
}

/// A length in display units rounded up to two significant digits, then formatted: the radius that a shell's
/// fix of a fillet's rounds asks for, which is never below the value it was computed from (section 4.3).
pub(crate) fn rounded_up_length(millimeters: f64, units: Units) -> String {
    rounded_length(millimeters, units, Rounding::Up)
}

/// A length in display units rounded to two significant digits the way `rounding` says, then formatted.
fn rounded_length(millimeters: f64, units: Units, rounding: Rounding) -> String {
    let factor = Dimension::LENGTH.display_factor(units);
    let rounded = round_to_two_digits(millimeters / factor, rounding);
    format_value(FieldUnit::Length, rounded * factor, units)
}

/// Which way a value is rounded to two significant digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rounding {
    /// Toward zero.
    Down,
    /// Away from zero.
    Up,
}

/// `value` rounded to two significant digits, toward zero or away from it. Zero, negative and
/// non-finite values are returned unchanged.
///
/// The digits are cut from the shortest decimal text that reads back as `value`, so a value
/// already at two digits keeps them (2.3, although 2.3 / 0.1 is 22.999999999999996 in binary),
/// and the result never passes `value` the wrong way: the cut text is that decimal shortened or
/// raised in its last kept digit, and reading decimals rounds monotonically.
fn round_to_two_digits(value: f64, rounding: Rounding) -> f64 {
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
    let digits: Vec<u32> = mantissa
        .chars()
        .filter_map(|character| character.to_digit(10))
        .collect();
    let kept = digits.len().min(2);
    let mut leading = digits[..kept]
        .iter()
        .fold(0_u32, |number, digit| number * 10 + digit);
    let dropped = digits[kept..].iter().any(|digit| *digit != 0);
    if rounding == Rounding::Up && dropped {
        leading += 1;
    }
    let shift = i32::try_from(kept).expect("at most two digits") - 1;
    format!("{leading}e{}", exponent - shift)
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
            assert!(
                round_to_two_digits(value, Rounding::Down) <= value,
                "value {value}"
            );
            assert!(
                round_to_two_digits(value, Rounding::Up) >= value,
                "value {value}"
            );
        }
    }

    #[test]
    fn lengths_round_up_to_two_significant_digits() {
        assert_eq!(rounded_up_length(3.15, Units::Mm), "3.2 mm");
        assert_eq!(rounded_up_length(3.1, Units::Mm), "3.1 mm");
        assert_eq!(rounded_up_length(99.5, Units::Mm), "100 mm");
        assert_eq!(rounded_up_length(25.4 * 0.123, Units::In), "0.13 in");
    }

    #[test]
    fn huge_values_are_written_in_exponent_form() {
        assert_eq!(
            format_value(FieldUnit::Length, 1e200, Units::Mm),
            "1e200 mm"
        );
        assert_eq!(format_value(FieldUnit::Count, 1e20, Units::Mm), "1e20");
        assert_eq!(
            format_value(FieldUnit::Length, -1.5e20, Units::Mm),
            "-1.5e20 mm"
        );
    }

    #[test]
    fn infinite_values_render_as_quantities_do() {
        assert_eq!(
            format_value(FieldUnit::Length, f64::INFINITY, Units::Mm),
            Quantity::length(f64::INFINITY).display(Units::Mm)
        );
        assert_eq!(
            format_value(FieldUnit::Count, f64::NEG_INFINITY, Units::Mm),
            Quantity::plain(f64::NEG_INFINITY).display(Units::Mm)
        );
    }

    #[test]
    fn a_value_reading_like_its_bound_gets_the_digits_that_tell_it_apart() {
        assert_eq!(
            format_value_apart(FieldUnit::Length, 1e-7, 0.0, Units::Mm),
            "0.0000001 mm"
        );
        assert_eq!(
            format_value_apart(FieldUnit::Length, 6.0000001, 6.0, Units::Mm),
            "6.0000001 mm"
        );
        assert_eq!(
            format_value_apart(FieldUnit::Length, 0.0, 0.0, Units::Mm),
            "0 mm"
        );
        assert_eq!(
            format_value_apart(FieldUnit::Length, 4.0, 5.0, Units::Mm),
            "4 mm"
        );
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
    fn document_names_give_the_records_in_order_and_values_with_the_parameters() {
        let mut document = Document::new(Units::Mm);
        document.parameters.push(partsmith_model::Parameter {
            id: Id::new("p-1").expect("a valid id"),
            name: "quarter".to_string(),
            expr: Expr::new("90 deg"),
            comment: None,
        });
        for (id, offset) in [("f-dp2", "5"), ("f-dp1", "quarter / 1 deg")] {
            document.features.push(FeatureRecord {
                id: Id::new(id).expect("a valid id"),
                kind: "datum_plane".to_string(),
                name: None,
                suppressed: false,
                params: serde_json::Map::from_iter([(
                    "offset".to_string(),
                    serde_json::json!(offset),
                )]),
            });
        }
        let registry = Registry::new();
        let outputs = BTreeMap::new();
        let names = DocumentNames {
            document: &document,
            registry: &registry,
            outputs: &outputs,
        };

        let order: Vec<&str> = names
            .records()
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(order, ["f-dp2", "f-dp1"]);
        assert_eq!(
            names.value(&Expr::new("quarter * 2"), FieldUnit::Angle),
            Some(PI)
        );
        assert_eq!(
            names.value(&Expr::new("quarter / 1 deg"), FieldUnit::Length),
            Some(90.0)
        );
        assert_eq!(names.value(&Expr::new("unknown"), FieldUnit::Angle), None);
        assert_eq!(names.value(&Expr::new("5 mm"), FieldUnit::Angle), None);
    }

    #[test]
    fn document_names_tell_the_features_with_an_output_in_the_run() {
        let document = Document::new(Units::Mm);
        let registry = Registry::new();
        let built = Id::new("f-built").expect("a valid id");
        let failed = Id::new("f-failed").expect("a valid id");
        let output = FeatureOutput::new(OutputData::Bodies(Vec::new()), Vec::new());
        let outputs = BTreeMap::from([(built.clone(), Arc::new(output))]);
        let names = DocumentNames {
            document: &document,
            registry: &registry,
            outputs: &outputs,
        };

        assert!(names.built(&built));
        assert!(!names.built(&failed));
    }

    #[test]
    fn names_outside_a_document_know_no_records_no_outputs_and_no_values() {
        let feature = Id::new("f-1").expect("a valid id");

        assert!(IdNames.records().is_empty());
        assert!(!IdNames.built(&feature));
        assert_eq!(IdNames.value(&Expr::new("90 deg"), FieldUnit::Angle), None);
    }

    #[test]
    fn plurals_of_element_kinds() {
        assert_eq!(plural(ElementKind::Vertex), "vertices");
        assert_eq!(plural(ElementKind::Face), "faces");
    }
}
