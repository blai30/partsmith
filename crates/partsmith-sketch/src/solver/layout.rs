//! The flat parameter vector of spec section 3.6.1 and the conversion back to entities (section 3.7.6).

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::ops::Range;

use partsmith_model::Id;

use crate::entity::{Geometry, SketchEntity};
use crate::point_ref::PointRole;

/// Where each entity's parameters sit in the flat vector.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    /// First parameter of each entity, in entity order.
    offsets: Vec<usize>,
    /// Parameter count of each entity.
    counts: Vec<usize>,
    /// Entity index by id.
    indices: BTreeMap<Id, usize>,
}

impl Layout {
    /// The layout of these entities in document order.
    pub(crate) fn new(entities: &[SketchEntity]) -> Layout {
        let mut offsets = Vec::with_capacity(entities.len());
        let mut counts = Vec::with_capacity(entities.len());
        let mut next = 0;
        for entity in entities {
            let count = parameter_count(&entity.geometry);
            offsets.push(next);
            counts.push(count);
            next += count;
        }
        let indices = entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (entity.id.clone(), index))
            .collect();
        Layout {
            offsets,
            counts,
            indices,
        }
    }

    /// Total parameter count.
    pub(crate) fn len(&self) -> usize {
        self.offsets
            .last()
            .zip(self.counts.last())
            .map_or(0, |(offset, count)| offset + count)
    }

    /// The index of the entity with this id.
    pub(crate) fn entity_index(&self, id: &Id) -> Option<usize> {
        self.indices.get(id).copied()
    }

    /// The parameters of one entity.
    pub(crate) fn parameters(&self, entity: usize) -> Range<usize> {
        self.offsets[entity]..self.offsets[entity] + self.counts[entity]
    }

    /// The first parameter of a point role of an entity (its `x`; `y` follows), or `None` when the role does not apply.
    pub(crate) fn point_parameter(
        &self,
        entity: usize,
        geometry: &Geometry,
        role: PointRole,
    ) -> Option<usize> {
        let offset = self.offsets[entity];
        let relative = match (geometry, role) {
            (Geometry::Point { .. }, PointRole::Point)
            | (Geometry::Line { .. }, PointRole::Start)
            | (Geometry::Circle { .. } | Geometry::Arc { .. }, PointRole::Center) => 0,
            (Geometry::Line { .. }, PointRole::End) | (Geometry::Arc { .. }, PointRole::Start) => 2,
            (Geometry::Arc { .. }, PointRole::End) => 4,
            _ => return None,
        };
        Some(offset + relative)
    }

    /// The flat parameter vector of these entities; arc start and end points come from the stored angles.
    pub(crate) fn vector(&self, entities: &[SketchEntity]) -> Vec<f64> {
        let mut values = Vec::with_capacity(self.len());
        for entity in entities {
            match &entity.geometry {
                Geometry::Point { position } => values.extend(position),
                Geometry::Line { start, end } => {
                    values.extend(start);
                    values.extend(end);
                }
                Geometry::Circle { center, radius } => {
                    values.extend(center);
                    values.push(*radius);
                }
                Geometry::Arc { center, .. } => {
                    let start = entity
                        .point(PointRole::Start)
                        .expect("an arc has a start point");
                    let end = entity
                        .point(PointRole::End)
                        .expect("an arc has an end point");
                    values.extend(center);
                    values.extend(start);
                    values.extend(end);
                }
            }
        }
        values
    }

    /// The entities for parameter vector `solved`: unchanged entities (bitwise equal to `before`) are kept exactly as given.
    pub(crate) fn entities(
        &self,
        given: &[SketchEntity],
        before: &[f64],
        solved: &[f64],
    ) -> Vec<SketchEntity> {
        given
            .iter()
            .enumerate()
            .map(|(index, entity)| {
                let range = self.parameters(index);
                let unchanged = before[range.clone()]
                    .iter()
                    .zip(&solved[range.clone()])
                    .all(|(old, new)| old.to_bits() == new.to_bits());
                if unchanged {
                    entity.clone()
                } else {
                    SketchEntity {
                        id: entity.id.clone(),
                        geometry: geometry_from(&entity.geometry, &solved[range]),
                        construction: entity.construction,
                    }
                }
            })
            .collect()
    }
}

/// Number of solver parameters of a geometry: point 2, line 4, circle 3, arc 6.
fn parameter_count(geometry: &Geometry) -> usize {
    match geometry {
        Geometry::Point { .. } => 2,
        Geometry::Line { .. } => 4,
        Geometry::Circle { .. } => 3,
        Geometry::Arc { .. } => 6,
    }
}

/// Geometry of the same kind as `kind` from its solved parameters (section 3.7.6).
fn geometry_from(kind: &Geometry, values: &[f64]) -> Geometry {
    match kind {
        Geometry::Point { .. } => Geometry::Point {
            position: [values[0], values[1]],
        },
        Geometry::Line { .. } => Geometry::Line {
            start: [values[0], values[1]],
            end: [values[2], values[3]],
        },
        Geometry::Circle { .. } => Geometry::Circle {
            center: [values[0], values[1]],
            radius: values[2].abs(),
        },
        Geometry::Arc { .. } => {
            let center = [values[0], values[1]];
            let start = [values[2] - center[0], values[3] - center[1]];
            let end = [values[4] - center[0], values[5] - center[1]];
            Geometry::Arc {
                center,
                radius: (start[0].hypot(start[1]) + end[0].hypot(end[1])) / 2.0,
                start_angle: stored_angle(start),
                end_angle: stored_angle(end),
            }
        }
    }
}

/// The direction of `offset` as an angle in [0, 2*pi): negative `atan2` values get 2*pi added and -0 becomes 0.
///
/// A negative angle closer to 0 than half an ulp of 2*pi rounds to exactly 2*pi when it is added; that
/// direction is +u, so it becomes 0 and the result stays below 2*pi.
fn stored_angle(offset: [f64; 2]) -> f64 {
    let angle = offset[1].atan2(offset[0]);
    if angle < 0.0 {
        let turned = angle + TAU;
        if turned < TAU { turned } else { 0.0 }
    } else if angle == 0.0 {
        0.0
    } else {
        angle
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn entity(id: &str, geometry: Geometry) -> SketchEntity {
        SketchEntity {
            id: Id::new(id).expect("valid id"),
            geometry,
            construction: false,
        }
    }

    #[test]
    fn offsets_follow_parameter_counts() {
        let layout = Layout::new(&[
            entity(
                "e-1",
                Geometry::Point {
                    position: [0.0, 0.0],
                },
            ),
            entity(
                "e-2",
                Geometry::Arc {
                    center: [0.0, 0.0],
                    radius: 1.0,
                    start_angle: 0.0,
                    end_angle: 1.0,
                },
            ),
            entity(
                "e-3",
                Geometry::Circle {
                    center: [0.0, 0.0],
                    radius: 1.0,
                },
            ),
        ]);

        assert_eq!(layout.parameters(1), 2..8);
        assert_eq!(layout.parameters(2), 8..11);
        assert_eq!(layout.len(), 11);
    }

    #[test]
    fn negative_angles_move_into_one_turn() {
        assert!((stored_angle([0.0, -1.0]) - 3.0 * FRAC_PI_2).abs() < 1e-15);
        assert_eq!(stored_angle([-1.0, 0.0]), PI);
    }

    #[test]
    fn negative_zero_angle_becomes_zero() {
        assert_eq!(stored_angle([1.0, -0.0]).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn tiny_negative_angle_becomes_zero_instead_of_a_full_turn() {
        // atan2 gives -1e-17, and -1e-17 + 2*pi rounds to exactly 2*pi.
        assert_eq!(stored_angle([6.0, -6e-17]).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn solved_arc_angles_stay_below_a_full_turn() {
        let geometry = geometry_from(
            &Geometry::Arc {
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: 1.0,
            },
            &[0.0, 0.0, 6.0, -6e-17, 0.0, 6.0],
        );

        let Geometry::Arc { start_angle, .. } = geometry else {
            panic!("an arc stays an arc, found {geometry:?}");
        };
        assert_eq!(start_angle.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn circle_radius_is_made_positive() {
        let geometry = geometry_from(
            &Geometry::Circle {
                center: [0.0, 0.0],
                radius: 1.0,
            },
            &[1.0, 2.0, -3.0],
        );

        assert_eq!(
            geometry,
            Geometry::Circle {
                center: [1.0, 2.0],
                radius: 3.0
            }
        );
    }

    #[test]
    fn unchanged_entities_are_returned_as_given() {
        let given = vec![entity(
            "e-1",
            Geometry::Arc {
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 7.0,
                end_angle: 8.0,
            },
        )];
        let layout = Layout::new(&given);
        let values = layout.vector(&given);

        assert_eq!(layout.entities(&given, &values, &values), given);
    }
}
