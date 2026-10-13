//! Validation of sketch params and dimension values (spec sections 3.4, 3.5 and 3.16).

use std::collections::{BTreeMap, BTreeSet};

use partsmith_model::Id;

use crate::constraint::{Alignment, ConstraintKind, DistanceTarget, SketchConstraint};
use crate::entity::EntityKind;
use crate::error::SketchError;
use crate::params::{DimensionValues, SketchParams};
use crate::plane::SketchPlane;
use crate::point_ref::{PointRef, PointRole};
use crate::reserved::{is_axis, is_origin, is_reserved};

/// Checks ids, the plane, geometry and references, returning the first problem in the order of section 3.4.
pub(crate) fn validate(sketch: &SketchParams) -> Result<(), SketchError> {
    check_ids(sketch)?;
    if let SketchPlane::Face { fallback, .. } = &sketch.plane {
        fallback.validate()?;
    }
    for entity in &sketch.entities {
        if !entity
            .geometry
            .numbers()
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(SketchError::NonFiniteGeometry {
                entity: entity.id.clone(),
            });
        }
        if let Some(radius) = entity.geometry.radius()
            && radius < 0.0
        {
            return Err(SketchError::NegativeRadius {
                entity: entity.id.clone(),
                kind: entity.kind(),
                radius,
            });
        }
    }
    let kinds: BTreeMap<&str, EntityKind> = sketch
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity.kind()))
        .collect();
    for constraint in &sketch.constraints {
        Checker {
            constraint,
            kinds: &kinds,
        }
        .check()?;
    }
    Ok(())
}

/// Checks the values of driving dimensions in document order (section 3.5).
pub(crate) fn check_values(
    sketch: &SketchParams,
    values: &DimensionValues,
) -> Result<(), SketchError> {
    for constraint in &sketch.constraints {
        if !constraint.is_driving_dimension() {
            continue;
        }
        let type_name = constraint.type_name();
        let id = || constraint.id.clone();
        let value = *values
            .get(&constraint.id)
            .ok_or_else(|| SketchError::MissingValue {
                constraint: id(),
                type_name,
            })?;
        if !value.is_finite() {
            return Err(SketchError::NonFiniteValue {
                constraint: id(),
                type_name,
            });
        }
        match &constraint.kind {
            ConstraintKind::Distance {
                target: DistanceTarget::PointLine { .. },
                ..
            } if value < 0.0 => {
                return Err(SketchError::ValueNegative {
                    constraint: id(),
                    value,
                });
            }
            ConstraintKind::Distance {
                target: DistanceTarget::Points { .. } | DistanceTarget::Line { .. },
                ..
            }
            | ConstraintKind::Radius { .. }
            | ConstraintKind::Diameter { .. }
                if value <= 0.0 =>
            {
                return Err(SketchError::ValueNotPositive {
                    constraint: id(),
                    type_name,
                    value,
                    points: matches!(
                        constraint.kind,
                        ConstraintKind::Distance {
                            target: DistanceTarget::Points { .. },
                            ..
                        }
                    ),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Walks entities then constraints: the first reserved id or repeated id fails.
fn check_ids(sketch: &SketchParams) -> Result<(), SketchError> {
    let ids = sketch
        .entities
        .iter()
        .map(|entity| &entity.id)
        .chain(sketch.constraints.iter().map(|constraint| &constraint.id));
    let mut seen = BTreeSet::new();
    for id in ids {
        if is_reserved(id.as_str()) {
            return Err(SketchError::ReservedId { id: id.clone() });
        }
        if !seen.insert(id) {
            return Err(SketchError::DuplicateId { id: id.clone() });
        }
    }
    Ok(())
}

/// The entity kinds a curve field accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Accepts {
    /// A line; `axis` tells whether `x_axis` and `y_axis` are accepted too.
    Line { axis: bool },
    /// A circle or arc.
    Circular,
    /// A line, circle or arc; `axis` tells whether the axes are accepted too.
    Any { axis: bool },
}

impl Accepts {
    /// The `expected` text of `WrongEntityKind`.
    fn expected(self) -> &'static str {
        match self {
            Accepts::Line { .. } => "a line",
            Accepts::Circular => "a circle or arc",
            Accepts::Any { .. } => "a line, circle or arc",
        }
    }

    fn accepts_axis(self) -> bool {
        match self {
            Accepts::Line { axis } | Accepts::Any { axis } => axis,
            Accepts::Circular => false,
        }
    }

    fn accepts(self, kind: EntityKind) -> bool {
        match self {
            Accepts::Line { .. } => kind == EntityKind::Line,
            Accepts::Circular => matches!(kind, EntityKind::Circle | EntityKind::Arc),
            Accepts::Any { .. } => kind != EntityKind::Point,
        }
    }
}

/// Whether a checked curve field holds a straight line (a line entity or an axis) or a circle or arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Straight,
    Circular,
}

/// Checks the references of one constraint (section 3.4, step 4).
struct Checker<'a> {
    constraint: &'a SketchConstraint,
    kinds: &'a BTreeMap<&'a str, EntityKind>,
}

impl Checker<'_> {
    fn check(&self) -> Result<(), SketchError> {
        match &self.constraint.kind {
            ConstraintKind::Coincident { points } => {
                self.points(points)?;
                self.distinct_entities(&points[0].entity, &points[1].entity)
            }
            ConstraintKind::PointOnObject { point, curve } => {
                self.point(point)?;
                self.curve(curve, Accepts::Any { axis: true })?;
                self.not_own(point, curve)
            }
            ConstraintKind::Horizontal(alignment)
            | ConstraintKind::Vertical(alignment)
            | ConstraintKind::HorizontalDistance {
                target: alignment, ..
            }
            | ConstraintKind::VerticalDistance {
                target: alignment, ..
            } => match alignment {
                Alignment::Line { line } => {
                    self.curve(line, Accepts::Line { axis: false }).map(drop)
                }
                Alignment::Points { points } => {
                    self.points(points)?;
                    self.distinct_points(points)
                }
            },
            ConstraintKind::Parallel { lines }
            | ConstraintKind::Perpendicular { lines }
            | ConstraintKind::Collinear { lines }
            | ConstraintKind::Angle { lines, .. } => {
                for line in lines {
                    self.curve(line, Accepts::Line { axis: true })?;
                }
                self.distinct_entities(&lines[0], &lines[1])
            }
            ConstraintKind::Tangent { curves } => {
                let second = match self.curve(&curves[0], Accepts::Any { axis: true })? {
                    Shape::Straight => Accepts::Circular,
                    Shape::Circular => Accepts::Any { axis: true },
                };
                self.curve(&curves[1], second)?;
                self.distinct_entities(&curves[0], &curves[1])
            }
            ConstraintKind::Concentric { curves } => {
                for curve in curves {
                    self.curve(curve, Accepts::Circular)?;
                }
                self.distinct_entities(&curves[0], &curves[1])
            }
            ConstraintKind::Equal { curves } => {
                let second = match self.curve(&curves[0], Accepts::Any { axis: false })? {
                    Shape::Straight => Accepts::Line { axis: false },
                    Shape::Circular => Accepts::Circular,
                };
                self.curve(&curves[1], second)?;
                self.distinct_entities(&curves[0], &curves[1])
            }
            ConstraintKind::Symmetric { points, line } => {
                self.points(points)?;
                self.curve(line, Accepts::Line { axis: true })?;
                self.distinct_points(points)?;
                self.not_own(&points[0], line)?;
                self.not_own(&points[1], line)
            }
            ConstraintKind::Midpoint { point, line } => {
                self.point(point)?;
                self.curve(line, Accepts::Line { axis: false })?;
                self.not_own(point, line)
            }
            ConstraintKind::Fix { target } => self.reference(target, true),
            ConstraintKind::Distance { target, .. } => match target {
                DistanceTarget::Points { points } => {
                    self.points(points)?;
                    self.distinct_points(points)
                }
                DistanceTarget::PointLine { point, line } => {
                    self.point(point)?;
                    self.curve(line, Accepts::Line { axis: true })?;
                    self.not_own(point, line)
                }
                DistanceTarget::Line { line } => {
                    self.curve(line, Accepts::Line { axis: false }).map(drop)
                }
            },
            ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => {
                self.curve(curve, Accepts::Circular).map(drop)
            }
        }
    }

    fn id(&self) -> Id {
        self.constraint.id.clone()
    }

    fn type_name(&self) -> &'static str {
        self.constraint.type_name()
    }

    fn misuse(&self, reference: String) -> SketchError {
        SketchError::ReservedMisuse {
            constraint: self.id(),
            type_name: self.type_name(),
            reference,
        }
    }

    /// The kind of a referenced entity, or `UnknownEntity`.
    fn kind(&self, entity: &Id) -> Result<EntityKind, SketchError> {
        self.kinds
            .get(entity.as_str())
            .copied()
            .ok_or_else(|| SketchError::UnknownEntity {
                constraint: self.id(),
                entity: entity.clone(),
            })
    }

    fn points(&self, points: &[PointRef; 2]) -> Result<(), SketchError> {
        self.point(&points[0])?;
        self.point(&points[1])
    }

    fn point(&self, point: &PointRef) -> Result<(), SketchError> {
        self.reference(point, false)
    }

    /// A point field, or the `fix` target when `whole_entity` (a bare id then names an entity of any kind).
    fn reference(&self, point: &PointRef, whole_entity: bool) -> Result<(), SketchError> {
        if is_origin(point) {
            return Ok(());
        }
        let name = point.entity.as_str();
        if is_reserved(name) {
            return Err(self.misuse(point.to_string()));
        }
        let kind = self.kind(&point.entity)?;
        let bare_entity = whole_entity && point.role == PointRole::Point;
        if bare_entity || kind.has_role(point.role) {
            Ok(())
        } else {
            Err(SketchError::InvalidPointRole {
                constraint: self.id(),
                point: point.clone(),
                kind,
            })
        }
    }

    /// A curve field: reserved names where accepted, then an existing entity of an accepted kind.
    fn curve(&self, curve: &Id, accepts: Accepts) -> Result<Shape, SketchError> {
        let name = curve.as_str();
        if is_axis(name) && accepts.accepts_axis() {
            return Ok(Shape::Straight);
        }
        if is_reserved(name) {
            return Err(self.misuse(name.to_owned()));
        }
        let kind = self.kind(curve)?;
        if !accepts.accepts(kind) {
            return Err(SketchError::WrongEntityKind {
                constraint: self.id(),
                type_name: self.type_name(),
                entity: curve.clone(),
                found: kind,
                expected: accepts.expected(),
            });
        }
        Ok(if kind == EntityKind::Line {
            Shape::Straight
        } else {
            Shape::Circular
        })
    }

    fn distinct_entities(&self, first: &Id, second: &Id) -> Result<(), SketchError> {
        if first == second {
            return Err(SketchError::SameEntity {
                constraint: self.id(),
                type_name: self.type_name(),
                entity: first.clone(),
            });
        }
        Ok(())
    }

    fn distinct_points(&self, points: &[PointRef; 2]) -> Result<(), SketchError> {
        if points[0] == points[1] {
            return Err(SketchError::SamePoint {
                constraint: self.id(),
                type_name: self.type_name(),
                point: points[0].clone(),
            });
        }
        Ok(())
    }

    fn not_own(&self, point: &PointRef, entity: &Id) -> Result<(), SketchError> {
        if &point.entity == entity {
            return Err(SketchError::PointOnOwnEntity {
                constraint: self.id(),
                type_name: self.type_name(),
                point: point.clone(),
                entity: entity.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::DimensionExpr;
    use crate::entity::Geometry;
    use crate::test_support::{self, entity, id, line, point};
    use partsmith_model::Expr;

    /// The line e-1 and the arc e-2 with these constraints.
    fn sketch(constraints: Vec<ConstraintKind>) -> SketchParams {
        test_support::sketch(
            vec![
                line("e-1", [0.0, 0.0], [1.0, 0.0]),
                entity(
                    "e-2",
                    Geometry::Arc {
                        center: [0.0, 0.0],
                        radius: 1.0,
                        start_angle: 0.0,
                        end_angle: 1.0,
                    },
                ),
            ],
            constraints,
        )
    }

    fn dimension() -> DimensionExpr {
        DimensionExpr {
            expr: Expr::new("1 mm"),
            driving: true,
        }
    }

    #[test]
    fn axis_with_a_role_is_a_misuse_in_a_point_field() {
        let params = sketch(vec![ConstraintKind::Coincident {
            points: [point("e-1.start"), point("x_axis.start")],
        }]);

        assert_eq!(
            params.validate(),
            Err(SketchError::ReservedMisuse {
                constraint: id("c-0"),
                type_name: "coincident",
                reference: "x_axis.start".to_owned(),
            })
        );
    }

    #[test]
    fn tangent_with_an_arc_first_accepts_an_axis_second() {
        let params = sketch(vec![ConstraintKind::Tangent {
            curves: [id("e-2"), id("x_axis")],
        }]);

        assert_eq!(params.validate(), Ok(()));
    }

    #[test]
    fn fix_accepts_a_bare_arc_id() {
        let params = sketch(vec![ConstraintKind::Fix {
            target: point("e-2"),
        }]);

        assert_eq!(params.validate(), Ok(()));
    }

    #[test]
    fn unknown_entity_is_reported_before_role_problems() {
        let params = sketch(vec![ConstraintKind::Coincident {
            points: [point("e-9.center"), point("e-1.center")],
        }]);

        assert_eq!(
            params.validate(),
            Err(SketchError::UnknownEntity {
                constraint: id("c-0"),
                entity: id("e-9"),
            })
        );
    }

    #[test]
    fn diameter_of_a_line_is_a_wrong_kind() {
        let params = sketch(vec![ConstraintKind::Diameter {
            curve: id("e-1"),
            dimension: dimension(),
        }]);

        assert_eq!(
            params.validate(),
            Err(SketchError::WrongEntityKind {
                constraint: id("c-0"),
                type_name: "diameter",
                entity: id("e-1"),
                found: EntityKind::Line,
                expected: "a circle or arc",
            })
        );
    }

    #[test]
    fn symmetric_reports_the_second_point_on_its_line() {
        let params = sketch(vec![ConstraintKind::Symmetric {
            points: [point("e-2.start"), point("e-1.end")],
            line: id("e-1"),
        }]);

        assert_eq!(
            params.validate(),
            Err(SketchError::PointOnOwnEntity {
                constraint: id("c-0"),
                type_name: "symmetric",
                point: point("e-1.end"),
                entity: id("e-1"),
            })
        );
    }

    #[test]
    fn values_of_reference_dimensions_are_not_checked() {
        let mut reference = dimension();
        reference.driving = false;
        let params = sketch(vec![ConstraintKind::Radius {
            curve: id("e-2"),
            dimension: reference,
        }]);

        assert_eq!(check_values(&params, &DimensionValues::new()), Ok(()));
    }

    #[test]
    fn angles_accept_any_finite_value() {
        let params = sketch(vec![ConstraintKind::Angle {
            lines: [id("e-1"), id("x_axis")],
            dimension: dimension(),
        }]);
        let values = DimensionValues::from([(id("c-0"), -7.0)]);

        assert_eq!(check_values(&params, &values), Ok(()));
    }
}
