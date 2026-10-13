//! Measured values of dimensional constraints (spec section 3.10).

use std::collections::BTreeMap;
use std::f64::consts::PI;

use partsmith_model::Id;

use super::layout::Layout;
use crate::constraint::{Alignment, ConstraintKind, DistanceTarget};
use crate::entity::{Geometry, SketchEntity};
use crate::params::SketchParams;
use crate::point_ref::PointRef;
use crate::reserved::{axis_line, is_origin};
use crate::vector::{cross, dot, norm, signed_distance, subtract};

/// The value of every dimensional constraint, driving or reference, measured on `entities`, which
/// are in the order of `layout`.
pub(crate) fn measure(
    sketch: &SketchParams,
    layout: &Layout,
    entities: &[SketchEntity],
) -> BTreeMap<Id, f64> {
    let shapes = Shapes { layout, entities };
    sketch
        .constraints
        .iter()
        .filter_map(|constraint| {
            let value = match &constraint.kind {
                ConstraintKind::Distance { target, .. } => match target {
                    DistanceTarget::Points { points } => {
                        norm(subtract(shapes.point(&points[1]), shapes.point(&points[0])))
                    }
                    DistanceTarget::Line { line } => {
                        let (start, end) = shapes.line(line);
                        norm(subtract(end, start))
                    }
                    DistanceTarget::PointLine { point, line } => {
                        let (start, end) = shapes.line(line);
                        signed_distance(shapes.point(point), start, end).abs()
                    }
                },
                ConstraintKind::HorizontalDistance { target, .. } => {
                    let (first, second) = shapes.alignment(target);
                    second[0] - first[0]
                }
                ConstraintKind::VerticalDistance { target, .. } => {
                    let (first, second) = shapes.alignment(target);
                    second[1] - first[1]
                }
                ConstraintKind::Radius { curve, .. } => shapes.radius(curve),
                ConstraintKind::Diameter { curve, .. } => 2.0 * shapes.radius(curve),
                ConstraintKind::Angle { lines, .. } => {
                    let (first_start, first_end) = shapes.line(&lines[0]);
                    let (second_start, second_end) = shapes.line(&lines[1]);
                    let first = subtract(first_end, first_start);
                    let second = subtract(second_end, second_start);
                    let angle = cross(first, second)
                        .atan2(dot(first, second))
                        .rem_euclid(PI);
                    // Rounding can land exactly on pi, which is the same direction as 0.
                    if angle >= PI { 0.0 } else { angle }
                }
                _ => return None,
            };
            Some((constraint.id.clone(), value))
        })
        .collect()
}

/// Geometry lookups on one list of entities in layout order.
struct Shapes<'a> {
    layout: &'a Layout,
    entities: &'a [SketchEntity],
}

impl Shapes<'_> {
    fn entity(&self, id: &Id) -> &SketchEntity {
        let index = self
            .layout
            .entity_index(id)
            .expect("validation guarantees that constraints name existing entities");
        &self.entities[index]
    }

    fn point(&self, point: &PointRef) -> [f64; 2] {
        if is_origin(point) {
            return [0.0, 0.0];
        }
        self.entity(&point.entity)
            .point(point.role)
            .expect("validation guarantees that point roles exist on their entities")
    }

    fn line(&self, line: &Id) -> ([f64; 2], [f64; 2]) {
        if let Some(axis) = axis_line(line.as_str()) {
            return axis;
        }
        match self.entity(line).geometry {
            Geometry::Line { start, end } => (start, end),
            _ => unreachable!("validation guarantees that line fields name lines"),
        }
    }

    fn alignment(&self, alignment: &Alignment) -> ([f64; 2], [f64; 2]) {
        match alignment {
            Alignment::Line { line } => self.line(line),
            Alignment::Points { points } => (self.point(&points[0]), self.point(&points[1])),
        }
    }

    fn radius(&self, curve: &Id) -> f64 {
        self.entity(curve)
            .geometry
            .radius()
            .expect("validation guarantees that radius fields name circles or arcs")
    }
}
