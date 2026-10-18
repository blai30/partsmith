//! `MessageContext`: the words for entities, points and constraints, and numbers in document units (spec section 4).

use std::collections::BTreeMap;

use partsmith_model::{Expr, Id, Quantity, Units};

use super::{joined, type_words};
use crate::constraint::{DimensionExpr, SketchConstraint};
use crate::entity::{EntityKind, Geometry, SketchEntity, point_at_angle};
use crate::number::RoundedPoint;
use crate::params::{DimensionValues, SketchParams, ValueKind};
use crate::point_ref::{PointRef, PointRole};
use crate::reserved;

/// What a message needs to name entities and constraints the way the user sees them (section 4).
#[derive(Clone, Copy, Debug)]
pub struct MessageContext<'a> {
    /// The sketch's label, for example "Sketch 1" (features.md section 3.12).
    pub label: &'a str,
    /// The sketch as stored.
    pub sketch: &'a SketchParams,
    /// The solved entities (`SolveResult::entities`) when a solve ran; entities are described from them when they
    /// have the entity, otherwise from `sketch`.
    pub solved: Option<&'a [SketchEntity]>,
    /// Driving dimension values in internal units, as passed to `solve`; empty when they are not known.
    pub values: &'a DimensionValues,
    /// Measured dimension values (`SolveResult::measured`) when a solve ran; titles of reference dimensions use them.
    pub measured: Option<&'a BTreeMap<Id, f64>>,
    /// Document units: coordinates and lengths are written in them.
    pub units: Units,
}

/// Whether a dimension's title names its entities, which it does only when another title in the same message
/// would read the same (section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DimensionEntities {
    /// `Distance 40 mm`.
    Omitted,
    /// `Distance 40 mm on the line from (0, 0) to (40, 0)`.
    Named,
}

impl MessageContext<'_> {
    /// The words for an entity or a reserved reference (section 4), or `None` when the sketch has no such entity.
    pub fn entity(&self, id: &Id) -> Option<String> {
        if let Some(words) = reserved::words(id.as_str()) {
            return Some(words.to_owned());
        }
        self.described_entity(id)
            .map(|entity| self.describe(entity))
    }

    /// The title of a constraint (section 4), or `None` when the sketch has no constraint with this id.
    pub fn title(&self, constraint: &Id) -> Option<String> {
        self.sketch
            .constraint(constraint)
            .map(|found| self.title_of(found, DimensionEntities::Omitted, None))
    }

    /// The words for an entity, or for one this context does not have: its kind when it is known ("a circle of
    /// this sketch"), otherwise "an entity of this sketch".
    pub(crate) fn entity_or_kind(&self, id: &Id, kind: Option<EntityKind>) -> String {
        self.entity(id).unwrap_or_else(|| match kind {
            Some(kind) => of_this_sketch(kind),
            None => "an entity of this sketch".to_owned(),
        })
    }

    /// `{point}` of section 4: the entity for a bare id, `the start of {entity}`, `the end of {entity}` or
    /// `the center of {entity}` for a role.
    pub(crate) fn point_words(&self, point: &PointRef) -> String {
        let entity = self.entity_or_kind(&point.entity, None);
        match point.role {
            PointRole::Point => entity,
            PointRole::Start => format!("the start of {entity}"),
            PointRole::End => format!("the end of {entity}"),
            PointRole::Center => format!("the center of {entity}"),
        }
    }

    /// The entity a message describes: the solved one when `solved` has it, otherwise the stored one.
    pub(crate) fn described_entity(&self, id: &Id) -> Option<&SketchEntity> {
        self.solved
            .and_then(|solved| solved.iter().find(|entity| &entity.id == id))
            .or_else(|| self.sketch.entity(id))
    }

    /// A sketch point in document units without a unit, written `(u, v)` (section 4, "Numbers").
    pub(crate) fn position(&self, point: [f64; 2]) -> String {
        let millimeters = self.units.millimeters();
        RoundedPoint(point.map(|coordinate| coordinate / millimeters)).to_string()
    }

    /// A length in document units with its unit, as `Quantity::display` writes it: `5 mm`, `0.19685 in`.
    pub(crate) fn length(&self, millimeters: f64) -> String {
        Quantity::length(millimeters).display(self.units)
    }

    /// The title of a constraint, or for one the sketch does not have, `{Type}` of `type_name` when it is known,
    /// otherwise "a constraint".
    pub(crate) fn title_or(&self, constraint: &Id, type_name: Option<&str>) -> String {
        self.title(constraint)
            .unwrap_or_else(|| type_name.map_or_else(|| "a constraint".to_owned(), type_words))
    }

    /// The title of a constraint without `left_out` among its entities, for the reference `ReservedMisuse` names
    /// (section 4.1); `{Type}` of `type_name` for a constraint the sketch does not have.
    pub(crate) fn title_leaving_out(
        &self,
        constraint: &Id,
        type_name: &str,
        left_out: &str,
    ) -> String {
        match self.sketch.constraint(constraint) {
            Some(found) => self.title_of(found, DimensionEntities::Omitted, Some(left_out)),
            None => type_words(type_name),
        }
    }

    /// The titles of the constraints one message names, in the given order, which hosts also label that message's
    /// fixes with. Each dimension title among titles that would read the same names its entities (section 4).
    pub fn titles(&self, constraints: &[&Id]) -> Vec<String> {
        let plain: Vec<String> = constraints
            .iter()
            .map(|constraint| self.title_or(constraint, None))
            .collect();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for title in &plain {
            *counts.entry(title.as_str()).or_default() += 1;
        }
        constraints
            .iter()
            .zip(&plain)
            .map(
                |(constraint, title)| match self.sketch.constraint(constraint) {
                    Some(found) if counts[title.as_str()] > 1 && found.dimension().is_some() => {
                        self.title_of(found, DimensionEntities::Named, None)
                    }
                    _ => title.clone(),
                },
            )
            .collect()
    }

    /// The words for a stored or solved entity (section 4, `{entity}`).
    fn describe(&self, entity: &SketchEntity) -> String {
        if !entity.geometry.is_finite() {
            return of_this_sketch(entity.kind());
        }
        match &entity.geometry {
            Geometry::Point { position } => format!("the point at {}", self.position(*position)),
            Geometry::Line { start, end } => format!(
                "the line from {} to {}",
                self.position(*start),
                self.position(*end)
            ),
            Geometry::Circle { center, radius } => format!(
                "the circle at {} with radius {}",
                self.position(*center),
                self.length(*radius)
            ),
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => format!(
                "the arc from {} to {} around {}",
                self.position(point_at_angle(*center, *radius, *start_angle)),
                self.position(point_at_angle(*center, *radius, *end_angle)),
                self.position(*center)
            ),
        }
    }

    /// `{title}` of section 4, leaving out the reference `left_out` when one is given.
    fn title_of(
        &self,
        constraint: &SketchConstraint,
        dimension_entities: DimensionEntities,
        left_out: Option<&str>,
    ) -> String {
        let mut title = type_words(constraint.type_name());
        match constraint.dimension() {
            Some(dimension) => {
                let value = self.dimension_value(constraint, dimension);
                if !value.is_empty() {
                    title.push(' ');
                    title.push_str(&value);
                }
                if dimension_entities == DimensionEntities::Named {
                    title.push_str(&self.entity_clause(constraint, left_out));
                }
            }
            None => title.push_str(&self.entity_clause(constraint, left_out)),
        }
        title
    }

    /// The value part of a dimension's title: its value with its unit (the driving value from `values`, or the
    /// measured value of a reference dimension), followed by the expression in parentheses when it names a
    /// parameter or a printer value; the trimmed expression when no finite value is known.
    fn dimension_value(&self, constraint: &SketchConstraint, dimension: &DimensionExpr) -> String {
        let expression = dimension.expr.as_str().trim();
        let known = if dimension.driving {
            self.values.get(&constraint.id)
        } else {
            self.measured
                .and_then(|measured| measured.get(&constraint.id))
        };
        let Some(value) = known.copied().filter(|value| value.is_finite()) else {
            return expression.to_owned();
        };
        let quantity = match constraint.value_kind() {
            Some(ValueKind::Angle) => Quantity::angle(value),
            _ => Quantity::length(value),
        };
        let shown = quantity.display(self.units);
        if names_a_value(&dimension.expr) {
            format!("{shown} ({expression})")
        } else {
            shown
        }
    }

    /// ` on {entity}` or ` between {entities}` for the entities the constraint's fields reference, in field order
    /// without repeats and without `left_out`; empty when the sketch has none of them.
    fn entity_clause(&self, constraint: &SketchConstraint, left_out: Option<&str>) -> String {
        let mut entities: Vec<&Id> = Vec::new();
        for entity in constraint.kind.references() {
            if Some(entity.as_str()) != left_out && !entities.contains(&entity) {
                entities.push(entity);
            }
        }
        let words: Vec<String> = entities
            .into_iter()
            .filter_map(|entity| self.entity(entity))
            .collect();
        match words.as_slice() {
            [] => String::new(),
            [only] => format!(" on {only}"),
            _ => format!(" between {}", joined(&words)),
        }
    }
}

/// The words for an entity that cannot be described by its geometry: `a line of this sketch`.
fn of_this_sketch(kind: EntityKind) -> String {
    format!("{} of this sketch", kind.with_article())
}

/// True when the expression names a parameter or a printer value, so a title shows it next to the value.
fn names_a_value(expr: &Expr) -> bool {
    expr.references()
        .is_ok_and(|references| !references.parameters.is_empty() || !references.values.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_that_name_parameters_or_printer_values_are_shown() {
        assert!(names_a_value(&Expr::new("width")));
        assert!(names_a_value(&Expr::new("width / 2 + 1 mm")));
        assert!(names_a_value(&Expr::new("printer.fit.slip")));
    }

    #[test]
    fn plain_and_unreadable_expressions_are_not_shown() {
        assert!(!names_a_value(&Expr::new("10 mm")));
        assert!(!names_a_value(&Expr::new("90 deg")));
        assert!(!names_a_value(&Expr::new("(")));
    }

    #[test]
    fn entities_of_unknown_geometry_are_named_by_kind() {
        assert_eq!(of_this_sketch(EntityKind::Arc), "an arc of this sketch");
        assert_eq!(of_this_sketch(EntityKind::Point), "a point of this sketch");
    }
}
