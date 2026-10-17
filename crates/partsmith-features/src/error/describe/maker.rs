//! What descriptions read about the feature that made a face and about its sketch (features.md section 3.12).

use std::str::FromStr;

use partsmith_model::{Dimension, FeatureRecord, FieldUnit, Id, Units};
use partsmith_sketch::{Geometry, PointRef, SketchEntity, SketchParams};
use serde_json::Value;

use super::super::message::{Names, format_value};
use crate::builtin::extrude::Extrude;
use crate::builtin::hole::Hole;
use crate::builtin::revolve::Revolve;
use crate::builtin::{CUTTING_TYPES, PRIMITIVE_TYPES};
use crate::feature::Feature;

/// How a feature that is not in the document is named (section 3.12).
pub(super) const NOT_IN_DOCUMENT: &str = "a feature that is not in this document";

/// The feature that made a face, as the document knows it.
pub(super) struct Maker<'a> {
    /// Its label; None when it is not in the document.
    pub(super) label: Option<String>,
    /// Its record, when the names know it.
    pub(super) record: Option<&'a FeatureRecord>,
}

impl<'a> Maker<'a> {
    /// The feature `feature` (an id's text) in `names`.
    pub(super) fn of(names: &'a dyn Names, feature: &str) -> Maker<'a> {
        let id = Id::new(feature).ok();
        let label = id.as_ref().and_then(|id| names.feature_label(id));
        let record = id.as_ref().and_then(|id| names.feature_record(id));
        Maker { label, record }
    }

    /// True when the feature is in the document.
    pub(super) fn exists(&self) -> bool {
        self.label.is_some()
    }

    /// The label in single quotes, or `a feature that is not in this document`.
    pub(super) fn quoted(&self) -> String {
        self.label
            .as_ref()
            .map_or_else(|| NOT_IN_DOCUMENT.to_string(), |label| format!("'{label}'"))
    }

    /// The record's type, when known.
    pub(super) fn kind(&self) -> Option<&'a str> {
        self.record.map(|record| record.kind.as_str())
    }

    /// True for a type.
    pub(super) fn is(&self, kind: &str) -> bool {
        self.kind() == Some(kind)
    }

    /// True for a primitive.
    pub(super) fn is_primitive(&self) -> bool {
        self.kind()
            .is_some_and(|kind| PRIMITIVE_TYPES.contains(&kind))
    }

    /// True for an extrude or a revolve.
    pub(super) fn is_sweep(&self) -> bool {
        self.is(Extrude::TYPE) || self.is(Revolve::TYPE)
    }

    /// The param at `path`.
    pub(super) fn param(&self, path: &[&str]) -> Option<&'a Value> {
        let (first, rest) = path.split_first()?;
        let mut value = self.record?.params.get(*first)?;
        for key in rest {
            value = value.get(key)?;
        }
        Some(value)
    }

    /// True for an extrude, revolve or primitive with Operation Cut: its faces are the floor and walls of a cut.
    pub(super) fn is_cut(&self) -> bool {
        self.kind()
            .is_some_and(|kind| CUTTING_TYPES.contains(&kind))
            && self.param(&["op"]).and_then(Value::as_str) == Some("cut")
    }

    /// The sketch the feature reads: the profile's for an extrude or revolve, the point sketch for a hole.
    pub(super) fn sketch(&self) -> Option<Id> {
        let value = if self.is(Hole::TYPE) {
            self.param(&["sketch"])
        } else {
            self.param(&["profile", "sketch"])
        }?;
        Id::new(value.as_str()?).ok()
    }
}

/// A sketch entity's geometry: from the sketch's result in the run, else from its stored record. None when the
/// sketch does not have the entity.
pub(super) fn entity_geometry(names: &dyn Names, sketch: &Id, entity: &str) -> Option<Geometry> {
    match names.sketch_result(sketch) {
        Some(result) => result
            .solved
            .entities
            .iter()
            .find(|candidate| candidate.id.as_str() == entity)
            .map(|found| found.geometry.clone()),
        None => stored_entities(names, sketch)?
            .into_iter()
            .find(|candidate| candidate.id.as_str() == entity)
            .map(|found| found.geometry),
    }
}

/// The position of a sketch point: from the sketch's result in the run, else from its stored record.
pub(super) fn sketch_point(names: &dyn Names, sketch: &Id, point: &str) -> Option<[f64; 2]> {
    let point = PointRef::from_str(point).ok()?;
    match names.sketch_result(sketch) {
        Some(result) => result.point(&point),
        None => stored_params(names, sketch)?.point(&point),
    }
}

/// The stored entities of a sketch record.
fn stored_entities(names: &dyn Names, sketch: &Id) -> Option<Vec<SketchEntity>> {
    Some(stored_params(names, sketch)?.entities)
}

/// The stored params of a sketch record.
pub(crate) fn stored_params(names: &dyn Names, sketch: &Id) -> Option<SketchParams> {
    let record = names.feature_record(sketch)?;
    serde_json::from_value(Value::Object(record.params.clone())).ok()
}

/// A sketch entity in words with its coordinates (section 3.12): `the line from (80, 40) to (70, 50)`, `the arc
/// from (u1, v1) to (u2, v2)`, `the circle at (u, v)`, `the point at (u, v)`.
pub(super) fn entity_words(geometry: &Geometry, units: Units) -> String {
    match *geometry {
        Geometry::Line { start, end } => format!(
            "the line from {} to {}",
            sketch_coordinates(start, units),
            sketch_coordinates(end, units)
        ),
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let at = |angle: f64| {
                [
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                ]
            };
            format!(
                "the arc from {} to {}",
                sketch_coordinates(at(start_angle), units),
                sketch_coordinates(at(end_angle), units)
            )
        }
        Geometry::Circle { center, .. } => {
            format!("the circle at {}", sketch_coordinates(center, units))
        }
        Geometry::Point { position } => {
            format!("the point at {}", sketch_coordinates(position, units))
        }
    }
}

/// True for a circle or an arc.
pub(super) fn is_round(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::Circle { .. } | Geometry::Arc { .. })
}

/// Sketch coordinates in document units without the unit, as section 3.12 writes them: `(80, 40)`.
pub fn sketch_coordinates(point: [f64; 2], units: Units) -> String {
    format!(
        "({}, {})",
        coordinate(point[0], units),
        coordinate(point[1], units)
    )
}

/// World coordinates in document units without the unit: `(40, 0, 30)`.
pub(super) fn world_coordinates(point: [f64; 3], units: Units) -> String {
    format!(
        "({}, {}, {})",
        coordinate(point[0], units),
        coordinate(point[1], units),
        coordinate(point[2], units)
    )
}

/// One length in document units, written as `format_value` writes it, without the unit.
fn coordinate(millimeters: f64, units: Units) -> String {
    format_value(
        FieldUnit::Scalar,
        millimeters / Dimension::LENGTH.display_factor(units),
        units,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_are_written_in_document_units_without_the_unit() {
        assert_eq!(sketch_coordinates([80.0, -0.0], Units::Mm), "(80, 0)");
        assert_eq!(
            world_coordinates([25.4, 0.0, 12.7], Units::In),
            "(1, 0, 0.5)"
        );
    }

    #[test]
    fn entities_read_as_their_kind_and_points() {
        let line = Geometry::Line {
            start: [80.0, 40.0],
            end: [70.0, 50.0],
        };
        let circle = Geometry::Circle {
            center: [40.0, 25.0],
            radius: 10.0,
        };

        assert_eq!(
            entity_words(&line, Units::Mm),
            "the line from (80, 40) to (70, 50)"
        );
        assert_eq!(entity_words(&circle, Units::Mm), "the circle at (40, 25)");
        assert!(is_round(&circle));
        assert!(!is_round(&line));
    }
}
