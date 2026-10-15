//! The mirrored drawing of a sketch across its x axis, with the constraints, point references and region
//! references rewritten to hold on it (spec section 3.17).

use partsmith_model::Id;

use crate::constraint::{Alignment, ConstraintKind, DistanceTarget, SketchConstraint};
use crate::entity::{Geometry, SketchEntity};
use crate::params::SketchParams;
use crate::point_ref::{PointRef, PointRole};
use crate::profile::{KeyEdge, RegionFingerprint, RegionKey, RegionLoop, RegionRef};

impl SketchParams {
    /// The same sketch mirrored across its x axis, every constraint rewritten to hold on the mirrored geometry
    /// (section 3.17). Ids, order, expressions, flags and the plane are unchanged.
    pub fn mirrored(&self) -> SketchParams {
        SketchParams {
            plane: self.plane.clone(),
            entities: self.entities.iter().map(mirrored_entity).collect(),
            constraints: self
                .constraints
                .iter()
                .map(|constraint| SketchConstraint {
                    id: constraint.id.clone(),
                    kind: self.mirrored_constraint(&constraint.kind),
                })
                .collect(),
        }
    }

    /// What a point reference of this sketch names in `mirrored()`: the start and end of an arc swap, every other
    /// reference is unchanged (section 3.17).
    pub fn mirrored_point(&self, point: &PointRef) -> PointRef {
        let role = match point.role {
            PointRole::Start if self.is_arc(&point.entity) => PointRole::End,
            PointRole::End if self.is_arc(&point.entity) => PointRole::Start,
            role => role,
        };
        PointRef {
            entity: point.entity.clone(),
            role,
        }
    }

    /// A region reference of this sketch as it reads on `mirrored()` (section 3.17).
    pub fn mirrored_region(&self, region: &RegionRef) -> RegionRef {
        // Mirroring reverses every loop; tracing it again in its own sense reverses each line entry back, while
        // an arc or circle turns with its loop. An entity the sketch does not have keeps its flag.
        let flipped = |entry: &KeyEdge| KeyEdge {
            entity: entry.entity.clone(),
            reversed: entry.reversed != self.is_line(&entry.entity),
        };
        let key = RegionKey::new(
            region.key.outer().iter().map(flipped).collect(),
            region
                .key
                .holes()
                .iter()
                .map(|hole| hole.iter().map(flipped).collect())
                .collect(),
        );
        let outline = region.outline.as_ref().map(|outline| {
            let entries = outline.entries().iter().rev().map(flipped).collect();
            RegionLoop::new(entries).expect("a mirrored loop has as many entries as its loop")
        });
        RegionRef {
            key,
            fp: RegionFingerprint {
                area: region.fp.area,
                centroid: mirrored_coordinates(region.fp.centroid),
            },
            outline,
        }
    }

    /// A constraint of this sketch rewritten to hold on `mirrored()`.
    fn mirrored_constraint(&self, kind: &ConstraintKind) -> ConstraintKind {
        let point = |point: &PointRef| self.mirrored_point(point);
        let pair = |[first, second]: &[PointRef; 2]| [point(first), point(second)];
        match kind {
            ConstraintKind::Coincident { points } => ConstraintKind::Coincident {
                points: pair(points),
            },
            ConstraintKind::PointOnObject {
                point: target,
                curve,
            } => ConstraintKind::PointOnObject {
                point: point(target),
                curve: curve.clone(),
            },
            ConstraintKind::Horizontal(alignment) => {
                ConstraintKind::Horizontal(self.mirrored_alignment(alignment))
            }
            ConstraintKind::Vertical(alignment) => {
                ConstraintKind::Vertical(self.mirrored_alignment(alignment))
            }
            ConstraintKind::Symmetric { points, line } => ConstraintKind::Symmetric {
                points: pair(points),
                line: line.clone(),
            },
            ConstraintKind::Midpoint {
                point: target,
                line,
            } => ConstraintKind::Midpoint {
                point: point(target),
                line: line.clone(),
            },
            ConstraintKind::Fix { target } => ConstraintKind::Fix {
                target: point(target),
            },
            ConstraintKind::Distance { target, dimension } => ConstraintKind::Distance {
                target: match target {
                    DistanceTarget::Points { points } => DistanceTarget::Points {
                        points: pair(points),
                    },
                    DistanceTarget::PointLine {
                        point: target,
                        line,
                    } => DistanceTarget::PointLine {
                        point: point(target),
                        line: line.clone(),
                    },
                    DistanceTarget::Line { line } => DistanceTarget::Line { line: line.clone() },
                },
                dimension: dimension.clone(),
            },
            ConstraintKind::HorizontalDistance { target, dimension } => {
                ConstraintKind::HorizontalDistance {
                    target: self.mirrored_alignment(target),
                    dimension: dimension.clone(),
                }
            }
            // Mirroring negates every v, so the points swap to measure the same value again.
            ConstraintKind::VerticalDistance { target, dimension } => {
                let points = match target {
                    Alignment::Points {
                        points: [first, second],
                    } => [point(second), point(first)],
                    Alignment::Line { line } => [
                        PointRef {
                            entity: line.clone(),
                            role: PointRole::End,
                        },
                        PointRef {
                            entity: line.clone(),
                            role: PointRole::Start,
                        },
                    ],
                };
                ConstraintKind::VerticalDistance {
                    target: Alignment::Points { points },
                    dimension: dimension.clone(),
                }
            }
            // Mirroring reverses the sense of turning, so the lines swap to measure the same angle again.
            ConstraintKind::Angle {
                lines: [first, second],
                dimension,
            } => ConstraintKind::Angle {
                lines: [second.clone(), first.clone()],
                dimension: dimension.clone(),
            },
            ConstraintKind::Parallel { .. }
            | ConstraintKind::Perpendicular { .. }
            | ConstraintKind::Tangent { .. }
            | ConstraintKind::Collinear { .. }
            | ConstraintKind::Concentric { .. }
            | ConstraintKind::Equal { .. }
            | ConstraintKind::Radius { .. }
            | ConstraintKind::Diameter { .. } => kind.clone(),
        }
    }

    /// An alignment with `mirrored_point` applied to its points.
    fn mirrored_alignment(&self, alignment: &Alignment) -> Alignment {
        match alignment {
            Alignment::Line { line } => Alignment::Line { line: line.clone() },
            Alignment::Points {
                points: [first, second],
            } => Alignment::Points {
                points: [self.mirrored_point(first), self.mirrored_point(second)],
            },
        }
    }

    /// True when this sketch has a line with this id.
    fn is_line(&self, entity: &Id) -> bool {
        self.entity(entity)
            .is_some_and(|found| matches!(found.geometry, Geometry::Line { .. }))
    }

    /// True when this sketch has an arc with this id.
    fn is_arc(&self, entity: &Id) -> bool {
        self.entity(entity)
            .is_some_and(|found| matches!(found.geometry, Geometry::Arc { .. }))
    }
}

/// An entity mirrored across the x axis; an arc runs counterclockwise again, from the mirror of its old end to
/// the mirror of its old start.
fn mirrored_entity(entity: &SketchEntity) -> SketchEntity {
    let geometry = match entity.geometry {
        Geometry::Point { position } => Geometry::Point {
            position: mirrored_coordinates(position),
        },
        Geometry::Line { start, end } => Geometry::Line {
            start: mirrored_coordinates(start),
            end: mirrored_coordinates(end),
        },
        Geometry::Circle { center, radius } => Geometry::Circle {
            center: mirrored_coordinates(center),
            radius,
        },
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Geometry::Arc {
            center: mirrored_coordinates(center),
            radius,
            start_angle: negated(end_angle),
            end_angle: negated(start_angle),
        },
    };
    SketchEntity {
        id: entity.id.clone(),
        geometry,
        construction: entity.construction,
    }
}

/// `(u, -v)`.
fn mirrored_coordinates([u, v]: [f64; 2]) -> [f64; 2] {
    [u, negated(v)]
}

/// `-value`, exact, except that zero stays positive so a mirrored document never writes `-0.0`.
fn negated(value: f64) -> f64 {
    0.0 - value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{entity, line, point, sketch};

    #[test]
    fn zero_coordinates_stay_positive() {
        let mirrored = mirrored_entity(&line("e-1", [0.0, 0.0], [5.0, -0.0]));

        let Geometry::Line { start, end } = mirrored.geometry else {
            panic!("a line stays a line")
        };
        assert_eq!(start[1].to_bits(), 0.0_f64.to_bits());
        assert_eq!(end[1].to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn an_arc_starting_at_zero_ends_at_zero() {
        let arc = entity(
            "e-1",
            Geometry::Arc {
                center: [1.0, 2.0],
                radius: 3.0,
                start_angle: 0.0,
                end_angle: 1.0,
            },
        );

        let mirrored = mirrored_entity(&arc);

        assert_eq!(
            mirrored.geometry,
            Geometry::Arc {
                center: [1.0, -2.0],
                radius: 3.0,
                start_angle: -1.0,
                end_angle: 0.0,
            }
        );
    }

    #[test]
    fn a_line_keeps_its_ends() {
        let params = sketch(vec![line("e-1", [0.0, 0.0], [1.0, 0.0])], Vec::new());

        assert_eq!(
            params.mirrored_point(&point("e-1.start")),
            point("e-1.start")
        );
    }
}
