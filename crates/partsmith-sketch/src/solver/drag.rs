//! Drag solving with a weakly pinned target (spec section 3.9).

use super::layout::Layout;
use super::system::System;
use super::{
    DragSession, DragTarget, Outcome, SolveResult, measure, solve_checked, solve_component,
};
use crate::entity::{Geometry, SketchEntity};
use crate::error::SketchError;
use crate::params::{DimensionValues, SketchParams};
use crate::reserved::is_reserved;
use crate::validation::check_values;
use crate::vector::{norm, subtract};

/// Drag state of `NativeSolver`: the prepared system and the previous frame's geometry.
pub struct NativeDragSession {
    sketch: SketchParams,
    /// Rows with warm-start choices and fix values taken from the initial result.
    system: System,
    /// Index of the component that contains the dragged entity.
    component: usize,
    target: Dragged,
    initial: SolveResult,
    /// Parameters of the initial result, for translating a dragged line.
    begin: Vec<f64>,
    /// The geometry of the last frame that converged (the initial result before the first frame).
    previous: Vec<SketchEntity>,
    /// True when a component other than the dragged one failed in the initial solve.
    other_failed: bool,
}

/// The parameters a drag moves, by target kind.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Dragged {
    /// A point whose `x` is this parameter, moved to the cursor.
    Point(usize),
    /// A whole line whose parameters start here, translated with the cursor from `grab`.
    Line { start: usize, grab: [f64; 2] },
    /// A circle whose parameters start here; its radius follows the cursor.
    Circle(usize),
    /// An arc whose parameters start here; its radius follows the cursor and its angles stay.
    Arc(usize),
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
        let (entity, dragged) = resolve_target(sketch, &Layout::new(&sketch.entities), target)?;
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
        let begin = system.layout.vector(&initial.entities);
        Ok(NativeDragSession {
            sketch: sketch.clone(),
            system,
            component,
            target: dragged,
            previous: initial.entities.clone(),
            initial,
            begin,
            other_failed,
        })
    }

    /// The dragged parameters and their targets for a cursor, on the warm start `warm`.
    fn targets(&self, cursor: [f64; 2], warm: &[f64]) -> Vec<(usize, f64)> {
        let point = |index: usize| [warm[index], warm[index + 1]];
        match self.target {
            Dragged::Point(index) => vec![(index, cursor[0]), (index + 1, cursor[1])],
            Dragged::Line { start, grab } => {
                let offset = subtract(cursor, grab);
                (0..4)
                    .map(|i| (start + i, self.begin[start + i] + offset[i % 2]))
                    .collect()
            }
            Dragged::Circle(center) => {
                vec![(center + 2, norm(subtract(cursor, point(center))))]
            }
            Dragged::Arc(center) => {
                let middle = point(center);
                let radius = norm(subtract(point(center + 2), middle));
                if radius <= 0.0 {
                    return Vec::new();
                }
                let factor = norm(subtract(cursor, middle)) / radius;
                [center + 2, center + 4]
                    .into_iter()
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
        let (solved, failed) = match solve_component(&self.system, component, &warm, &targets) {
            Outcome::Converged(x) => (x, false),
            Outcome::Failed(_) => (warm.clone(), true),
        };
        let entities = self.system.layout.entities(&self.previous, &warm, &solved);
        let measured = measure(&self.sketch, &entities);
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
fn resolve_target(
    sketch: &SketchParams,
    layout: &Layout,
    target: &DragTarget,
) -> Result<(usize, Dragged), SketchError> {
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
    let geometry = &sketch.entities[index].geometry;
    let start = layout.parameters(index).start;
    let dragged = match target {
        DragTarget::Point { point } => Dragged::Point(
            layout
                .point_parameter(index, geometry, point.role)
                .ok_or_else(unknown)?,
        ),
        DragTarget::Entity { grab, .. } => {
            if !grab.iter().all(|value| value.is_finite()) {
                return Err(unknown());
            }
            match geometry {
                Geometry::Point { .. } => Dragged::Point(start),
                Geometry::Line { .. } => Dragged::Line { start, grab: *grab },
                Geometry::Circle { .. } => Dragged::Circle(start),
                Geometry::Arc { .. } => Dragged::Arc(start),
            }
        }
    };
    Ok((index, dragged))
}
