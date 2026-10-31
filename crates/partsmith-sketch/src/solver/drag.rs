//! Drag solving with a weakly pinned target (spec section 3.9).

use super::layout::Layout;
use super::sparse_qr::SparseQr;
use super::system::System;
use super::{
    DragSession, DragTarget, Outcome, SolveResult, measure, solve_checked, solve_component,
};
use crate::DRAG_WEIGHT;
use crate::entity::{EntityKind, SketchEntity};
use crate::error::SketchError;
use crate::params::{DimensionValues, SketchParams};
use crate::point_ref::PointRole;
use crate::reserved::is_reserved;
use crate::validation::check_values;
use crate::vector::{norm, subtract};

/// Drag state of `NativeSolver`: the prepared system and the previous frame's geometry.
pub struct NativeDragSession {
    sketch: SketchParams,
    /// Rows with warm-start choices and fix values taken from the initial result; its warm start is
    /// the initial result's parameter vector.
    system: System,
    /// Index of the component that contains the dragged entity.
    component: usize,
    target: Dragged,
    initial: SolveResult,
    /// The geometry of the last frame that converged (the initial result before the first frame).
    previous: Vec<SketchEntity>,
    /// True when a component other than the dragged one failed in the initial solve.
    other_failed: bool,
    /// The factorization of the dragged component, whose plans every frame reuses.
    factorization: SparseQr,
}

/// The parameters a drag moves, by target kind; points are given by the parameter of their `x`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Dragged {
    /// A point, moved to the cursor.
    Point(usize),
    /// A whole line with these end points, translated with the cursor from `grab`.
    Line { ends: [usize; 2], grab: [f64; 2] },
    /// A circle whose radius follows the cursor.
    Circle { center: usize, radius: usize },
    /// An arc with these end points whose radius follows the cursor and whose angles stay.
    Arc { center: usize, ends: [usize; 2] },
}

impl NativeDragSession {
    /// Validates the sketch, values and target, solves once and prepares the dragged component.
    pub(super) fn begin(
        sketch: &SketchParams,
        values: &DimensionValues,
        target: &DragTarget,
    ) -> Result<NativeDragSession, SketchError> {
        sketch.validate()?;
        check_values(sketch, values)?;
        let (entity, dragged) = resolve_target(&Layout::new(&sketch.entities), target)?;
        let solved = solve_checked(sketch, values);
        let initial = solved.result;
        let system = System::new(sketch, values, &initial.entities);
        let component = system
            .components
            .iter()
            .position(|component| component.entities.contains(&entity))
            .expect("every entity belongs to one component");
        let other_failed = solved
            .failed_components
            .iter()
            .enumerate()
            .any(|(index, failed)| *failed && index != component);
        Ok(NativeDragSession {
            sketch: sketch.clone(),
            system,
            component,
            target: dragged,
            previous: initial.entities.clone(),
            initial,
            other_failed,
            factorization: SparseQr::default(),
        })
    }

    /// The dragged parameters and their targets for a cursor, on the warm start `warm`.
    fn targets(&self, cursor: [f64; 2], warm: &[f64]) -> Vec<(usize, f64)> {
        let point = |index: usize| [warm[index], warm[index + 1]];
        match self.target {
            Dragged::Point(index) => vec![(index, cursor[0]), (index + 1, cursor[1])],
            Dragged::Line { ends, grab } => {
                // The line moves with the cursor from where it was when the drag began.
                let begin = &self.system.warm_start;
                let offset = subtract(cursor, grab);
                ends.into_iter()
                    .flat_map(|end| {
                        [
                            (end, begin[end] + offset[0]),
                            (end + 1, begin[end + 1] + offset[1]),
                        ]
                    })
                    .collect()
            }
            Dragged::Circle { center, radius } => {
                vec![(radius, norm(subtract(cursor, point(center))))]
            }
            Dragged::Arc { center, ends } => {
                let middle = point(center);
                let radius = norm(subtract(point(ends[0]), middle));
                if radius <= 0.0 {
                    return Vec::new();
                }
                let factor = norm(subtract(cursor, middle)) / radius;
                ends.into_iter()
                    .flat_map(|end| {
                        let offset = subtract(point(end), middle);
                        [
                            (end, middle[0] + offset[0] * factor),
                            (end + 1, middle[1] + offset[1] * factor),
                        ]
                    })
                    .collect()
            }
        }
    }
}

impl DragSession for NativeDragSession {
    fn initial(&self) -> &SolveResult {
        &self.initial
    }

    fn update(&mut self, cursor: [f64; 2]) -> Result<SolveResult, SketchError> {
        if !cursor.iter().all(|value| value.is_finite()) {
            return Err(SketchError::NonFiniteCursor);
        }
        let warm = self.system.layout.vector(&self.previous);
        let targets = self.targets(cursor, &warm);
        let component = &self.system.components[self.component];
        let mut solve = |drag_weight| {
            solve_component(
                &self.system,
                component,
                &warm,
                &targets,
                drag_weight,
                &mut self.factorization,
            )
        };
        // A frame that fails with the drag weight is solved again without it (section 3.9, step 5).
        let outcome = match solve(DRAG_WEIGHT) {
            Outcome::Failed => solve(1.0),
            converged => converged,
        };
        let (solved, failed) = match outcome {
            Outcome::Converged(x) => (x, false),
            Outcome::Failed => (warm.clone(), true),
        };
        let entities = self.system.layout.entities(&self.previous, &warm, &solved);
        let measured = measure(&self.sketch, &self.system.layout, &entities);
        if !failed {
            self.previous = entities.clone();
        }
        Ok(SolveResult {
            status: self.initial.status,
            failed: failed || self.other_failed,
            entities,
            dof: self.initial.dof,
            redundant: self.initial.redundant.clone(),
            conflicting: self.initial.conflicting.clone(),
            entity_states: self.initial.entity_states.clone(),
            measured,
        })
    }
}

/// The dragged entity's index and parameters, or `UnknownDragTarget` (section 3.9).
fn resolve_target(layout: &Layout, target: &DragTarget) -> Result<(usize, Dragged), SketchError> {
    let (name, text) = match target {
        DragTarget::Point { point } => (&point.entity, point.to_string()),
        DragTarget::Entity { entity, .. } => (entity, entity.to_string()),
    };
    let unknown = || SketchError::UnknownDragTarget {
        target: text.clone(),
    };
    if is_reserved(name.as_str()) {
        return Err(unknown());
    }
    let index = layout.entity_index(name).ok_or_else(unknown)?;
    let point = |role| {
        layout
            .point_parameter(index, role)
            .expect("every entity has the points of its kind")
    };
    let dragged = match target {
        DragTarget::Point { point } => Dragged::Point(
            layout
                .point_parameter(index, point.role)
                .ok_or_else(unknown)?,
        ),
        DragTarget::Entity { grab, .. } => {
            if !grab.iter().all(|value| value.is_finite()) {
                return Err(unknown());
            }
            let ends = || [point(PointRole::Start), point(PointRole::End)];
            match layout.kind(index) {
                EntityKind::Point => Dragged::Point(point(PointRole::Point)),
                EntityKind::Line => Dragged::Line {
                    ends: ends(),
                    grab: *grab,
                },
                EntityKind::Circle => Dragged::Circle {
                    center: point(PointRole::Center),
                    radius: layout
                        .radius_parameter(index)
                        .expect("a circle has a radius parameter"),
                },
                EntityKind::Arc => Dragged::Arc {
                    center: point(PointRole::Center),
                    ends: ends(),
                },
            }
        }
    };
    Ok((index, dragged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::ConstraintKind;
    use crate::entity::Geometry;
    use crate::test_support::{dimension, entity, id, point, sketch};

    #[test]
    fn the_weighted_solve_alone_fails_past_the_reach_of_an_arc() {
        // Test 257's frame: the end of an arc with a fixed start and radius 5 dragged to 10.0678
        // from that start, past the 10 it can reach. Section 3.9's step 5 makes this frame converge.
        let params = sketch(
            vec![entity(
                "e-1",
                Geometry::Arc {
                    center: [0.0, 0.0],
                    radius: 5.0,
                    start_angle: 0.0,
                    end_angle: 3.1,
                },
            )],
            vec![
                ConstraintKind::Fix {
                    target: point("e-1.start"),
                },
                ConstraintKind::Radius {
                    curve: id("e-1"),
                    dimension: dimension(5.0),
                },
            ],
        );
        let values: DimensionValues = [(id("c-1"), 5.0)].into_iter().collect();
        let mut session = NativeDragSession::begin(
            &params,
            &values,
            &DragTarget::Point {
                point: point("e-1.end"),
            },
        )
        .expect("the drag begins");
        let end = [5.0 * 3.1_f64.cos(), 5.0 * 3.1_f64.sin()];
        let warm = session.system.layout.vector(&session.previous);
        let targets = session.targets([end[0] - 0.07, end[1]], &warm);
        let component = &session.system.components[session.component];

        let outcome = solve_component(
            &session.system,
            component,
            &warm,
            &targets,
            DRAG_WEIGHT,
            &mut session.factorization,
        );

        assert_eq!(outcome, Outcome::Failed);
    }
}
