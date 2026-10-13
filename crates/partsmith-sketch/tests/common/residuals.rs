//! An independent re-implementation of the constraint rows of spec section 3.6.5, used to check
//! solver output (tests 189, 194 and 198).
//!
//! Warm-start choices (side, scale, tangent mode) are recomputed from the geometry being checked,
//! and touch points follow the structural rule of section 3.6.4.

use std::collections::BTreeMap;
use std::f64::consts::PI;

use partsmith_sketch::{
    Alignment, ConstraintKind, DimensionValues, DistanceTarget, Geometry, Id, ORIGIN, PointRef,
    PointRole, SketchConstraint, SketchEntity, SketchParams, SolveResult, X_AXIS, Y_AXIS,
};

use super::plane::{
    Vector, add, cross, dot, norm, safe_length, scale, signed_distance, subtract, unit,
};
use super::point_at_angle;

/// Geometry lookups on one list of entities.
struct Shapes<'a> {
    entities: &'a [SketchEntity],
}

impl<'a> Shapes<'a> {
    fn entity(&self, entity: &Id) -> &'a SketchEntity {
        self.entities
            .iter()
            .find(|candidate| &candidate.id == entity)
            .unwrap_or_else(|| panic!("entity {entity} is in the checked geometry"))
    }

    fn is_line(&self, entity: &Id) -> bool {
        let text = entity.as_str();
        text == X_AXIS
            || text == Y_AXIS
            || matches!(self.entity(entity).geometry, Geometry::Line { .. })
    }

    fn point(&self, point: &PointRef) -> Vector {
        if point.entity.as_str() == ORIGIN {
            return [0.0, 0.0];
        }
        let entity = self.entity(&point.entity);
        match (&entity.geometry, point.role) {
            (Geometry::Point { position }, PointRole::Point) => *position,
            (Geometry::Line { start, .. }, PointRole::Start) => *start,
            (Geometry::Line { end, .. }, PointRole::End) => *end,
            (Geometry::Circle { center, .. }, PointRole::Center)
            | (Geometry::Arc { center, .. }, PointRole::Center) => *center,
            (
                Geometry::Arc {
                    center,
                    radius,
                    start_angle,
                    ..
                },
                PointRole::Start,
            ) => point_at_angle(*center, *radius, *start_angle),
            (
                Geometry::Arc {
                    center,
                    radius,
                    end_angle,
                    ..
                },
                PointRole::End,
            ) => point_at_angle(*center, *radius, *end_angle),
            (geometry, role) => panic!("{point} does not resolve: {role:?} of {geometry:?}"),
        }
    }

    /// Start and end of a line, or the constants of an axis.
    fn line(&self, line: &Id) -> (Vector, Vector) {
        match line.as_str() {
            X_AXIS => ([0.0, 0.0], [1.0, 0.0]),
            Y_AXIS => ([0.0, 0.0], [0.0, 1.0]),
            _ => match &self.entity(line).geometry {
                Geometry::Line { start, end } => (*start, *end),
                other => panic!("{line} is a line, found {other:?}"),
            },
        }
    }

    /// Center and radius of a circle or arc.
    fn curve(&self, curve: &Id) -> (Vector, f64) {
        match &self.entity(curve).geometry {
            Geometry::Circle { center, radius } => (*center, *radius),
            Geometry::Arc {
                center,
                radius,
                start_angle,
                ..
            } => {
                let start = point_at_angle(*center, *radius, *start_angle);
                (*center, norm(subtract(start, *center)))
            }
            other => panic!("{curve} is a circle or arc, found {other:?}"),
        }
    }

    /// `size` of section 3.6.3, or `None` for an axis.
    fn size(&self, entity: &Id) -> Option<f64> {
        let text = entity.as_str();
        if text == X_AXIS || text == Y_AXIS {
            return None;
        }
        if self.is_line(entity) {
            let (a, b) = self.line(entity);
            Some(norm(subtract(b, a)))
        } else {
            Some(self.curve(entity).1)
        }
    }

    /// The angular scale `S` of section 3.6.3.
    fn angular_scale(&self, first: &Id, second: &Id) -> f64 {
        let product = match (self.size(first), self.size(second)) {
            (Some(first_size), Some(second_size)) => first_size * second_size,
            (Some(size), None) | (None, Some(size)) => size * size,
            (None, None) => 1.0,
        };
        product.sqrt().max(1.0)
    }
}

/// Point identification through `coincident` constraints (section 3.6.4).
struct Identification {
    parent: BTreeMap<PointRef, PointRef>,
}

impl Identification {
    fn new(sketch: &SketchParams) -> Identification {
        let mut identification = Identification {
            parent: BTreeMap::new(),
        };
        for constraint in &sketch.constraints {
            if let ConstraintKind::Coincident { points } = &constraint.kind {
                identification.union(&points[0], &points[1]);
            }
        }
        identification
    }

    fn root(&self, point: &PointRef) -> PointRef {
        let mut current = point.clone();
        while let Some(parent) = self.parent.get(&current) {
            current = parent.clone();
        }
        current
    }

    fn union(&mut self, first: &PointRef, second: &PointRef) {
        let first_root = self.root(first);
        let second_root = self.root(second);
        if first_root != second_root {
            self.parent.insert(first_root, second_root);
        }
    }

    fn identified(&self, first: &PointRef, second: &PointRef) -> bool {
        self.root(first) == self.root(second)
    }
}

/// The touch point of `tangent [first, second]` (section 3.6.4), from the sketch's constraints only.
fn touch_point(
    sketch: &SketchParams,
    identification: &Identification,
    first: &Id,
    second: &Id,
) -> Option<PointRef> {
    let has_ends = |curve: &Id| {
        sketch.entities.iter().any(|entity| {
            &entity.id == curve
                && matches!(
                    entity.geometry,
                    Geometry::Line { .. } | Geometry::Arc { .. }
                )
        })
    };
    let on_curve = |point: &PointRef, curve: &Id| {
        let on_end = has_ends(curve)
            && [PointRole::Start, PointRole::End].into_iter().any(|role| {
                identification.identified(
                    point,
                    &PointRef {
                        entity: curve.clone(),
                        role,
                    },
                )
            });
        on_end
            || sketch
                .constraints
                .iter()
                .any(|constraint| match &constraint.kind {
                    ConstraintKind::PointOnObject {
                        point: other,
                        curve: other_curve,
                    } => other_curve == curve && identification.identified(point, other),
                    _ => false,
                })
    };

    let mut candidates = Vec::new();
    for curve in [first, second] {
        if has_ends(curve) {
            for role in [PointRole::Start, PointRole::End] {
                candidates.push(PointRef {
                    entity: curve.clone(),
                    role,
                });
            }
        }
    }
    for constraint in &sketch.constraints {
        if let ConstraintKind::PointOnObject { point, curve } = &constraint.kind
            && (curve == first || curve == second)
        {
            candidates.push(point.clone());
        }
    }
    candidates
        .into_iter()
        .find(|candidate| on_curve(candidate, first) && on_curve(candidate, second))
}

/// The rows of one constraint on the checked geometry (empty for reference dimensions).
fn rows(
    sketch: &SketchParams,
    identification: &Identification,
    constraint: &SketchConstraint,
    shapes: &Shapes<'_>,
    fixed: &Shapes<'_>,
    values: &DimensionValues,
) -> Vec<f64> {
    let value = || {
        *values
            .get(&constraint.id)
            .unwrap_or_else(|| panic!("driving dimension {} has a value", constraint.id))
    };
    match &constraint.kind {
        ConstraintKind::Coincident { points } => {
            let difference = subtract(shapes.point(&points[0]), shapes.point(&points[1]));
            vec![difference[0], difference[1]]
        }
        ConstraintKind::PointOnObject { point, curve } => {
            let p = shapes.point(point);
            if shapes.is_line(curve) {
                let (a, b) = shapes.line(curve);
                vec![signed_distance(p, a, b)]
            } else {
                let (center, radius) = shapes.curve(curve);
                vec![norm(subtract(p, center)) - radius]
            }
        }
        ConstraintKind::Horizontal(alignment) => {
            let (p, q) = alignment_points(shapes, alignment);
            vec![q[1] - p[1]]
        }
        ConstraintKind::Vertical(alignment) => {
            let (p, q) = alignment_points(shapes, alignment);
            vec![q[0] - p[0]]
        }
        ConstraintKind::Parallel { lines } => {
            let (first, second) = line_directions(shapes, lines);
            vec![shapes.angular_scale(&lines[0], &lines[1]) * cross(first, second)]
        }
        ConstraintKind::Perpendicular { lines } => {
            let (first, second) = line_directions(shapes, lines);
            vec![shapes.angular_scale(&lines[0], &lines[1]) * dot(first, second)]
        }
        ConstraintKind::Tangent { curves } => {
            vec![tangent_row(sketch, identification, shapes, curves)]
        }
        ConstraintKind::Collinear { lines } => {
            let (a1, b1) = shapes.line(&lines[0]);
            let (a2, b2) = shapes.line(&lines[1]);
            vec![signed_distance(a2, a1, b1), signed_distance(b2, a1, b1)]
        }
        ConstraintKind::Concentric { curves } => {
            let difference = subtract(shapes.curve(&curves[0]).0, shapes.curve(&curves[1]).0);
            vec![difference[0], difference[1]]
        }
        ConstraintKind::Equal { curves } => {
            if shapes.is_line(&curves[0]) {
                let (a1, b1) = shapes.line(&curves[0]);
                let (a2, b2) = shapes.line(&curves[1]);
                vec![norm(subtract(b1, a1)) - norm(subtract(b2, a2))]
            } else {
                vec![shapes.curve(&curves[0]).1 - shapes.curve(&curves[1]).1]
            }
        }
        ConstraintKind::Symmetric { points, line } => {
            let p = shapes.point(&points[0]);
            let q = shapes.point(&points[1]);
            let (a, b) = shapes.line(line);
            let direction = subtract(b, a);
            vec![
                signed_distance(scale(add(p, q), 0.5), a, b),
                dot(direction, subtract(q, p)) / safe_length(direction),
            ]
        }
        ConstraintKind::Midpoint { point, line } => {
            let p = shapes.point(point);
            let (a, b) = shapes.line(line);
            vec![p[0] - (a[0] + b[0]) / 2.0, p[1] - (a[1] + b[1]) / 2.0]
        }
        ConstraintKind::Fix { target } => fix_rows(shapes, fixed, target),
        ConstraintKind::Distance { target, dimension } => {
            if !dimension.driving {
                return Vec::new();
            }
            let measured = match target {
                DistanceTarget::Points { points } => {
                    norm(subtract(shapes.point(&points[1]), shapes.point(&points[0])))
                }
                DistanceTarget::Line { line } => {
                    let (a, b) = shapes.line(line);
                    norm(subtract(b, a))
                }
                DistanceTarget::PointLine { point, line } => {
                    let (a, b) = shapes.line(line);
                    // The side choice recomputed from this geometry makes the row |sd| - v.
                    signed_distance(shapes.point(point), a, b).abs()
                }
            };
            vec![measured - value()]
        }
        ConstraintKind::HorizontalDistance { target, dimension } => {
            if !dimension.driving {
                return Vec::new();
            }
            let (p, q) = alignment_points(shapes, target);
            vec![(q[0] - p[0]) - value()]
        }
        ConstraintKind::VerticalDistance { target, dimension } => {
            if !dimension.driving {
                return Vec::new();
            }
            let (p, q) = alignment_points(shapes, target);
            vec![(q[1] - p[1]) - value()]
        }
        ConstraintKind::Radius { curve, dimension } => {
            if !dimension.driving {
                return Vec::new();
            }
            vec![shapes.curve(curve).1 - value()]
        }
        ConstraintKind::Diameter { curve, dimension } => {
            if !dimension.driving {
                return Vec::new();
            }
            vec![shapes.curve(curve).1 - value() / 2.0]
        }
        ConstraintKind::Angle { lines, dimension } => {
            if !dimension.driving {
                return Vec::new();
            }
            let (first, second) = line_directions(shapes, lines);
            let target = value();
            let scale_factor = shapes.angular_scale(&lines[0], &lines[1]);
            vec![
                scale_factor
                    * (cross(first, second) * target.cos() - dot(first, second) * target.sin()),
            ]
        }
    }
}

fn alignment_points(shapes: &Shapes<'_>, alignment: &Alignment) -> (Vector, Vector) {
    match alignment {
        Alignment::Line { line } => shapes.line(line),
        Alignment::Points { points } => (shapes.point(&points[0]), shapes.point(&points[1])),
    }
}

fn line_directions(shapes: &Shapes<'_>, lines: &[Id; 2]) -> (Vector, Vector) {
    let (a1, b1) = shapes.line(&lines[0]);
    let (a2, b2) = shapes.line(&lines[1]);
    (unit(subtract(b1, a1)), unit(subtract(b2, a2)))
}

fn tangent_row(
    sketch: &SketchParams,
    identification: &Identification,
    shapes: &Shapes<'_>,
    curves: &[Id; 2],
) -> f64 {
    let touch = touch_point(sketch, identification, &curves[0], &curves[1]);
    let first_is_line = shapes.is_line(&curves[0]);
    let second_is_line = shapes.is_line(&curves[1]);
    let scale_factor = shapes.angular_scale(&curves[0], &curves[1]);
    if first_is_line || second_is_line {
        let (line, curve) = if first_is_line {
            (&curves[0], &curves[1])
        } else {
            (&curves[1], &curves[0])
        };
        let (a, b) = shapes.line(line);
        let (center, radius) = shapes.curve(curve);
        match touch {
            Some(point) => {
                let t = shapes.point(&point);
                scale_factor * dot(unit(subtract(b, a)), unit(subtract(t, center)))
            }
            None => signed_distance(center, a, b).abs() - radius,
        }
    } else {
        let (first_center, first_radius) = shapes.curve(&curves[0]);
        let (second_center, second_radius) = shapes.curve(&curves[1]);
        match touch {
            Some(point) => {
                let t = shapes.point(&point);
                scale_factor
                    * cross(
                        unit(subtract(t, first_center)),
                        unit(subtract(t, second_center)),
                    )
            }
            None => {
                let distance = norm(subtract(second_center, first_center));
                let external = distance - first_radius - second_radius;
                let internal = distance - (first_radius - second_radius).abs();
                if external.abs() <= internal.abs() {
                    external
                } else {
                    internal
                }
            }
        }
    }
}

fn fix_rows(shapes: &Shapes<'_>, fixed: &Shapes<'_>, target: &PointRef) -> Vec<f64> {
    if target.entity.as_str() == ORIGIN {
        return vec![0.0, 0.0];
    }
    let entity = shapes.entity(&target.entity);
    let whole =
        target.role == PointRole::Point && !matches!(entity.geometry, Geometry::Point { .. });
    if !whole {
        let difference = subtract(shapes.point(target), fixed.point(target));
        return vec![difference[0], difference[1]];
    }
    let fixed_entity = fixed.entity(&target.entity);
    match (&entity.geometry, &fixed_entity.geometry) {
        (
            Geometry::Line { start, end },
            Geometry::Line {
                start: fixed_start,
                end: fixed_end,
            },
        ) => vec![
            start[0] - fixed_start[0],
            start[1] - fixed_start[1],
            end[0] - fixed_end[0],
            end[1] - fixed_end[1],
        ],
        (
            Geometry::Circle { center, radius },
            Geometry::Circle {
                center: fixed_center,
                radius: fixed_radius,
            },
        ) => vec![
            center[0] - fixed_center[0],
            center[1] - fixed_center[1],
            radius - fixed_radius,
        ],
        (Geometry::Arc { .. }, Geometry::Arc { .. }) => {
            let start_ref = PointRef {
                entity: target.entity.clone(),
                role: PointRole::Start,
            };
            let end_ref = PointRef {
                entity: target.entity.clone(),
                role: PointRole::End,
            };
            let center = shapes.curve(&target.entity).0;
            let fixed_center = fixed.curve(&target.entity).0;
            let start = shapes.point(&start_ref);
            let fixed_start = fixed.point(&start_ref);
            let end = shapes.point(&end_ref);
            let fixed_end = fixed.point(&end_ref);
            vec![
                center[0] - fixed_center[0],
                center[1] - fixed_center[1],
                start[0] - fixed_start[0],
                start[1] - fixed_start[1],
                cross(
                    unit(subtract(fixed_end, fixed_center)),
                    subtract(end, center),
                ),
            ]
        }
        (geometry, fixed_geometry) => {
            panic!("fixed entity changed kind: {fixed_geometry:?} became {geometry:?}")
        }
    }
}

/// Every row of `sketch`'s constraints, evaluated on `entities`, whose absolute value exceeds `tolerance`.
///
/// `fix_reference` holds the warm-start geometry that `fix` constraints freeze.
pub fn violations(
    sketch: &SketchParams,
    fix_reference: &[SketchEntity],
    entities: &[SketchEntity],
    values: &DimensionValues,
    tolerance: f64,
) -> Vec<String> {
    let shapes = Shapes { entities };
    let fixed = Shapes {
        entities: fix_reference,
    };
    let identification = Identification::new(sketch);
    let mut found = Vec::new();
    for constraint in &sketch.constraints {
        for (row, residual) in rows(sketch, &identification, constraint, &shapes, &fixed, values)
            .into_iter()
            .enumerate()
        {
            if residual.is_nan() || residual.abs() > tolerance {
                found.push(format!(
                    "{} ({}) row {row}: residual {residual:e}",
                    constraint.id,
                    constraint.type_name()
                ));
            }
        }
    }
    found
}

/// Asserts every constraint row holds within `tolerance` on `entities`.
#[track_caller]
pub fn assert_constraints_hold(
    sketch: &SketchParams,
    fix_reference: &[SketchEntity],
    entities: &[SketchEntity],
    values: &DimensionValues,
    tolerance: f64,
) {
    let found = violations(sketch, fix_reference, entities, values, tolerance);
    assert!(
        found.is_empty(),
        "constraints do not hold within {tolerance}:\n{}",
        found.join("\n")
    );
}

/// Asserts that every driving dimension's measured value equals its value within `tolerance` (angles modulo pi).
#[track_caller]
pub fn assert_measured_match_values(
    sketch: &SketchParams,
    result: &SolveResult,
    values: &DimensionValues,
    tolerance: f64,
) {
    for constraint in &sketch.constraints {
        let Some(dimension) = constraint.dimension() else {
            continue;
        };
        if !dimension.driving {
            continue;
        }
        let value = values[&constraint.id];
        let measured = *result
            .measured
            .get(&constraint.id)
            .unwrap_or_else(|| panic!("{} has a measured value", constraint.id));
        let difference = if matches!(constraint.kind, ConstraintKind::Angle { .. }) {
            let wrapped = (measured - value).rem_euclid(PI);
            wrapped.min(PI - wrapped)
        } else {
            (measured - value).abs()
        };
        assert!(
            difference <= tolerance,
            "{} ({}): measured {measured} but the value is {value}",
            constraint.id,
            constraint.type_name()
        );
    }
}
