//! The native constraint solver of ADR 0004 (spec sections 2.6 and 3.6 to 3.12).

mod diagnosis;
mod drag;
mod issue;
mod layout;
mod measure;
mod numeric;
mod rows;
mod sparse_qr;
mod substitution;
mod system;

use std::collections::BTreeMap;

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use drag::NativeDragSession;
pub use issue::SolveIssue;

use crate::SOLVER_TOLERANCE;
use crate::entity::SketchEntity;
use crate::error::SketchError;
use crate::params::{DimensionValues, SketchParams};
use crate::point_ref::PointRef;
use crate::validation::check_values;
use diagnosis::diagnose;
use measure::measure;
use numeric::{Outcome, Problem};
use sparse_qr::SparseQr;
use substitution::Substitution;
use system::{Component, System};

/// A sketch solver backend. `NativeSolver` is the only M1 implementation.
pub trait Solver {
    /// The interactive drag state this backend returns.
    type Drag: DragSession;

    /// Solves the sketch from its stored geometry and diagnoses it.
    fn solve(
        &self,
        sketch: &SketchParams,
        values: &DimensionValues,
    ) -> Result<SolveResult, SketchError>;

    /// Solves and diagnoses once, then prepares fast repeated solves that pull `target` toward a cursor.
    fn begin_drag(
        &self,
        sketch: &SketchParams,
        values: &DimensionValues,
        target: &DragTarget,
    ) -> Result<Self::Drag, SketchError>;
}

/// An interactive drag. Owned by the caller's session; holds no global state.
pub trait DragSession: Send {
    /// The result of the full solve performed when the drag began.
    fn initial(&self) -> &SolveResult;

    /// Solves one frame with the target weakly pinned to `cursor` (sketch coordinates).
    fn update(&mut self, cursor: [f64; 2]) -> Result<SolveResult, SketchError>;
}

/// The native Rust solver of ADR 0004. Stateless; `Send + Sync`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeSolver;

impl Solver for NativeSolver {
    type Drag = NativeDragSession;

    fn solve(
        &self,
        sketch: &SketchParams,
        values: &DimensionValues,
    ) -> Result<SolveResult, SketchError> {
        sketch.validate()?;
        check_values(sketch, values)?;
        Ok(solve_checked(sketch, values).result)
    }

    fn begin_drag(
        &self,
        sketch: &SketchParams,
        values: &DimensionValues,
        target: &DragTarget,
    ) -> Result<NativeDragSession, SketchError> {
        NativeDragSession::begin(sketch, values, target)
    }
}

/// What the user drags.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DragTarget {
    /// A point, moved to the cursor.
    Point {
        /// The dragged point.
        point: PointRef,
    },
    /// A whole entity grabbed at `grab` (sketch coordinates at drag start).
    Entity {
        /// The dragged entity.
        entity: Id,
        /// Where the pointer grabbed it.
        grab: [f64; 2],
    },
}

/// Outcome of a solve or a drag frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SolveResult {
    /// Constraint state of the whole sketch.
    pub status: SketchStatus,
    /// True when some component did not reach `SOLVER_TOLERANCE`; those components keep their input geometry.
    pub failed: bool,
    /// Solved entities: same order, ids and construction flags as the input.
    pub entities: Vec<SketchEntity>,
    /// Remaining degrees of freedom.
    pub dof: u32,
    /// Constraints that only repeat what earlier constraints determine, in document order.
    pub redundant: Vec<Id>,
    /// Constraints that take part in an unsatisfiable combination, in document order.
    pub conflicting: Vec<Id>,
    /// Constraint state of every entity, keyed by entity id.
    pub entity_states: BTreeMap<Id, EntityState>,
    /// Measured value of every dimensional constraint (driving and reference), mm or rad.
    pub measured: BTreeMap<Id, f64>,
}

impl SolveResult {
    /// User-facing issues in display order: conflicts, then redundancies, then non-convergence.
    pub fn issues(&self, sketch: &SketchParams) -> Vec<SolveIssue> {
        let mut issues = Vec::new();
        if !self.conflicting.is_empty() {
            issues.push(SolveIssue::Conflicting {
                constraints: self.conflicting.clone(),
            });
        }
        for id in &self.redundant {
            if let Some(constraint) = sketch.constraint(id) {
                issues.push(SolveIssue::Redundant {
                    constraint: id.clone(),
                    type_name: constraint.type_name(),
                    dimension: constraint.is_driving_dimension(),
                });
            }
        }
        if self.failed && self.conflicting.is_empty() {
            issues.push(SolveIssue::NotConverged);
        }
        issues
    }
}

/// Constraint state of a sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SketchStatus {
    /// Some degrees of freedom remain and no constraint is redundant or conflicting.
    UnderConstrained,
    /// No degree of freedom remains and no constraint is redundant or conflicting.
    FullyConstrained,
    /// Some constraint repeats what others determine; none conflicts.
    Redundant,
    /// Some constraints cannot all hold.
    Conflicting,
}

/// Constraint state of one entity, used for coloring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityState {
    /// Some of its parameters can still move.
    UnderConstrained,
    /// All of its parameters are determined.
    FullyConstrained,
    /// A conflicting constraint references it.
    Conflicting,
}

/// A solve result together with the components that failed.
struct Solved {
    result: SolveResult,
    /// For each component of the system, whether it failed.
    failed_components: Vec<bool>,
}

/// Solves a validated sketch whose values were checked (sections 3.7 and 3.8).
fn solve_checked(sketch: &SketchParams, values: &DimensionValues) -> Solved {
    let system = System::new(sketch, values, &sketch.entities);
    let warm = &system.warm_start;
    let mut solved = warm.clone();
    let mut diagnosis_point = warm.clone();
    let mut failed_components = Vec::with_capacity(system.components.len());
    let mut factorization = SparseQr::default();
    for component in &system.components {
        let untouched = component
            .rows
            .iter()
            .all(|&row| system.rows[row].value(warm).abs() <= SOLVER_TOLERANCE);
        let failed = if untouched {
            false
        } else {
            match solve_component(&system, component, warm, &[], &mut factorization) {
                Outcome::Converged(x) => {
                    copy_parameters(component, &x, &mut solved);
                    copy_parameters(component, &x, &mut diagnosis_point);
                    false
                }
                // A failed component is diagnosed where it was given, never at the LM end point,
                // whose nearly dependent rows would make the diagnosis depend on the iterations.
                Outcome::Failed(_) => true,
            }
        };
        failed_components.push(failed);
    }
    let entities = system.layout.entities(&sketch.entities, warm, &solved);
    let diagnosis = diagnose(sketch, &system, &diagnosis_point);
    let measured = measure(sketch, &system.layout, &entities);
    let failed = failed_components.contains(&true);
    Solved {
        result: SolveResult {
            status: status(&diagnosis.redundant, &diagnosis.conflicting, diagnosis.dof),
            failed,
            entities,
            dof: diagnosis.dof,
            redundant: diagnosis.redundant,
            conflicting: diagnosis.conflicting,
            entity_states: diagnosis.entity_states,
            measured,
        },
        failed_components,
    }
}

/// Substitutes and solves one component from the warm start `warm` (sections 3.7.3 to 3.7.5).
///
/// `drag_targets` lists dragged parameters with their targets (section 3.9); it is empty outside a drag.
/// `factorization` is reused between calls so that repeated solves of one component plan their
/// factorizations once. The outcome holds the full parameter vector at the solution, or where
/// Levenberg-Marquardt stopped.
fn solve_component(
    system: &System,
    component: &Component,
    warm: &[f64],
    drag_targets: &[(usize, f64)],
    factorization: &mut SparseQr,
) -> Outcome {
    let mut substitution = Substitution::new(system, component, warm);
    let mut scales = vec![1.0; substitution.variable_count()];
    for variable in substitution.drag(drag_targets) {
        scales[variable] = 1.0 / crate::DRAG_WEIGHT;
    }
    let start = substitution.variable_values();
    let mut problem = Problem::new(system, &substitution, scales, warm.to_vec(), factorization);
    match numeric::solve(&mut problem, &start) {
        Outcome::Converged(variables) => {
            Outcome::Converged(problem.parameters(&variables).to_vec())
        }
        Outcome::Failed(variables) => Outcome::Failed(problem.parameters(&variables).to_vec()),
    }
}

/// Copies the component's parameters from `source` into `target`.
fn copy_parameters(component: &Component, source: &[f64], target: &mut [f64]) {
    for &parameter in &component.parameters {
        target[parameter] = source[parameter];
    }
}

/// The sketch status of section 3.8.
fn status(redundant: &[Id], conflicting: &[Id], dof: u32) -> SketchStatus {
    if !conflicting.is_empty() {
        SketchStatus::Conflicting
    } else if !redundant.is_empty() {
        SketchStatus::Redundant
    } else if dof == 0 {
        SketchStatus::FullyConstrained
    } else {
        SketchStatus::UnderConstrained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Geometry;
    use crate::test_support::id;

    #[test]
    fn conflicts_win_over_redundancy() {
        assert_eq!(
            status(&[id("c-1")], &[id("c-2")], 0),
            SketchStatus::Conflicting
        );
        assert_eq!(status(&[id("c-1")], &[], 3), SketchStatus::Redundant);
    }

    #[test]
    fn status_follows_the_degrees_of_freedom() {
        assert_eq!(status(&[], &[], 0), SketchStatus::FullyConstrained);
        assert_eq!(status(&[], &[], 2), SketchStatus::UnderConstrained);
    }

    #[test]
    fn an_arc_that_barely_converges_is_returned_so_that_it_solves_back_to_itself() {
        // A generated case whose dog-leg stops with residuals just under the tolerance; without the
        // refinement, the arc conversion pushed a row over it and a second solve moved the geometry.
        let sketch: SketchParams = serde_json::from_str(
            r#"{
              "plane": {"kind": "origin", "plane": "XY"},
              "entities": [
                {"id": "e-1", "type": "arc", "center": [-29.211621051278534, 71.52444974678612], "radius": 2.186651343819874, "start_angle": 0.08710887295788977, "end_angle": 4.018194978566759},
                {"id": "e-2", "type": "point", "position": [-28.14216649949164, 73.41106114750663]}
              ],
              "constraints": [
                {"id": "c-1", "type": "point_on_object", "point": "e-2", "curve": "e-1"},
                {"id": "c-2", "type": "horizontal_distance", "points": ["e-1.start", "e-2"], "expr": "-1.0830079422851284 mm"},
                {"id": "c-3", "type": "diameter", "curve": "e-1", "expr": "4.356816697371727 mm"},
                {"id": "c-4", "type": "horizontal_distance", "points": ["e-2", "e-1.end"], "expr": "-2.480733460235122 mm"}
              ]
            }"#,
        )
        .expect("valid sketch");
        let values: DimensionValues = [
            ("c-2", -1.0830079422851284),
            ("c-3", 4.356816697371727),
            ("c-4", -2.480733460235122),
        ]
        .into_iter()
        .map(|(constraint, value)| (id(constraint), value))
        .collect();
        let first = NativeSolver
            .solve(&sketch, &values)
            .expect("the sketch is valid");
        let mut solved = sketch.clone();
        solved.entities = first.entities.clone();

        let second = NativeSolver
            .solve(&solved, &values)
            .expect("the sketch is valid");

        assert!(!first.failed);
        assert_eq!(second.entities, first.entities);
    }

    /// NC08's triangle with Distance 30 mm and 40 mm on two sides and `third` mm on the last.
    fn triangle_with_third_side(third: f64) -> (SketchParams, DimensionValues) {
        let sketch: SketchParams = serde_json::from_str(&format!(
            r#"{{
              "plane": {{"kind": "origin", "plane": "XY"}},
              "entities": [
                {{"id": "e-1", "type": "line", "start": [0, 0], "end": [30, 0]}},
                {{"id": "e-2", "type": "line", "start": [30, 0], "end": [30, 40]}},
                {{"id": "e-3", "type": "line", "start": [30, 40], "end": [0, 0]}}
              ],
              "constraints": [
                {{"id": "c-1", "type": "distance", "line": "e-1", "expr": "30 mm"}},
                {{"id": "c-2", "type": "distance", "line": "e-2", "expr": "40 mm"}},
                {{"id": "c-3", "type": "distance", "line": "e-3", "expr": "{third} mm"}},
                {{"id": "c-4", "type": "coincident", "points": ["e-1.end", "e-2.start"]}},
                {{"id": "c-5", "type": "coincident", "points": ["e-2.end", "e-3.start"]}},
                {{"id": "c-6", "type": "coincident", "points": ["e-3.end", "e-1.start"]}}
              ]
            }}"#
        ))
        .expect("valid sketch");
        let values: DimensionValues = [("c-1", 30.0), ("c-2", 40.0), ("c-3", third)]
            .into_iter()
            .map(|(constraint, value)| (id(constraint), value))
            .collect();
        (sketch, values)
    }

    #[test]
    fn a_failed_component_is_diagnosed_where_it_was_given_whatever_side_no_triangle_can_have() {
        // Section 3.8: the iterations end near a degenerate shape whose rows are nearly dependent,
        // so a diagnosis there called some of these sides conflicting and others not.
        for third in [1.0, 5.0, 71.0, 75.0, 80.0, 90.0, 150.0, 500.0, 1000.0] {
            let (sketch, values) = triangle_with_third_side(third);

            let result = NativeSolver
                .solve(&sketch, &values)
                .expect("the sketch is valid");

            assert!(result.failed, "side {third}");
            assert!(
                result.conflicting.is_empty(),
                "side {third}: {:?}",
                result.conflicting
            );
            assert!(
                result.redundant.is_empty(),
                "side {third}: {:?}",
                result.redundant
            );
            assert_eq!(result.dof, 3, "side {third}");
            assert_eq!(
                result.status,
                SketchStatus::UnderConstrained,
                "side {third}"
            );
        }
    }

    #[test]
    fn solved_arc_angles_stay_below_a_full_turn() {
        // The start angle is a hair below 0; adding 2*pi to it rounds to exactly 2*pi.
        let sketch: SketchParams = serde_json::from_str(
            r#"{
              "plane": {"kind": "origin", "plane": "XY"},
              "entities": [
                {"id": "e-1", "type": "arc", "center": [0, 0], "radius": 5, "start_angle": -1e-17, "end_angle": 2}
              ],
              "constraints": [
                {"id": "c-1", "type": "fix", "target": "e-1.center"},
                {"id": "c-2", "type": "radius", "curve": "e-1", "expr": "6 mm"}
              ]
            }"#,
        )
        .expect("valid sketch");
        let values: DimensionValues = [(id("c-2"), 6.0)].into_iter().collect();

        let result = NativeSolver
            .solve(&sketch, &values)
            .expect("the sketch is valid");

        let Geometry::Arc {
            radius,
            start_angle,
            end_angle,
            ..
        } = result.entities[0].geometry
        else {
            panic!("an arc stays an arc, found {:?}", result.entities[0]);
        };
        assert!(!result.failed);
        assert!((radius - 6.0).abs() <= 1e-9);
        assert_eq!(start_angle.to_bits(), 0.0_f64.to_bits());
        assert!((0.0..std::f64::consts::TAU).contains(&end_angle));
    }
}
