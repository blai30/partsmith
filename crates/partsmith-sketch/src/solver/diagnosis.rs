//! Degrees of freedom, redundant and conflicting constraints, and entity states (spec section 3.8).

use std::collections::{BTreeMap, BTreeSet};

use partsmith_model::Id;

use super::EntityState;
use super::rows::RowOwner;
use super::sparse_qr::{Added, SparseQr};
use super::system::System;
use crate::params::SketchParams;
use crate::{CONFLICT_TOLERANCE, RANK_TOLERANCE};

/// Largest distance from a parameter's unit vector to the span of the accepted rows for it to count as determined.
const DETERMINED_TOLERANCE: f64 = 1e-6;

/// Relative size below which a coefficient does not involve its row in a conflict.
const INVOLVED_TOLERANCE: f64 = 1e-8;

/// The outcome of a diagnosis.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Diagnosis {
    pub(crate) dof: u32,
    pub(crate) redundant: Vec<Id>,
    pub(crate) conflicting: Vec<Id>,
    pub(crate) entity_states: BTreeMap<Id, EntityState>,
}

/// Diagnoses the full, unsubstituted system at the diagnosis point `x`.
pub(crate) fn diagnose(sketch: &SketchParams, system: &System, x: &[f64]) -> Diagnosis {
    let parameter_count = system.layout.len();
    let mut factorization = SparseQr::new(parameter_count);
    let mut accepted_residuals = Vec::new();
    let mut redundant = vec![false; sketch.constraints.len()];
    let mut conflicting = vec![false; sketch.constraints.len()];
    let mut gradient = Vec::new();
    for row in &system.rows {
        gradient.clear();
        let residual = row.value_and_gradient(x, &mut gradient);
        match factorization.add(&gradient, RANK_TOLERANCE) {
            Added::Accepted => accepted_residuals.push((residual, row.owner)),
            Added::Dependent { projection } => {
                let coefficients = factorization.coefficients(&projection);
                let explained: f64 = coefficients
                    .iter()
                    .zip(&accepted_residuals)
                    .map(|(coefficient, (accepted, _))| coefficient * accepted)
                    .sum();
                if (residual - explained).abs() > CONFLICT_TOLERANCE {
                    mark(row.owner, &mut conflicting);
                    let largest = coefficients.iter().fold(0.0_f64, |largest, coefficient| {
                        largest.max(coefficient.abs())
                    });
                    for (coefficient, (_, owner)) in coefficients.iter().zip(&accepted_residuals) {
                        if coefficient.abs() > INVOLVED_TOLERANCE * largest {
                            mark(*owner, &mut conflicting);
                        }
                    }
                } else {
                    mark(row.owner, &mut redundant);
                }
            }
        }
    }
    let rank = factorization.rank();
    let distances = factorization.complement_distances();
    let ids = |selected: &dyn Fn(usize) -> bool| -> Vec<Id> {
        sketch
            .constraints
            .iter()
            .enumerate()
            .filter(|&(index, _)| selected(index))
            .map(|(_, constraint)| constraint.id.clone())
            .collect()
    };
    let conflicting_ids = ids(&|index| conflicting[index]);
    // A constraint that is conflicting is not also listed as redundant.
    let redundant_ids = ids(&|index| redundant[index] && !conflicting[index]);
    let conflicting_entities: BTreeSet<&Id> = sketch
        .constraints
        .iter()
        .enumerate()
        .filter(|&(index, _)| conflicting[index])
        .flat_map(|(_, constraint)| constraint.referenced_entities())
        .collect();
    let entity_states = sketch
        .entities
        .iter()
        .enumerate()
        .map(|(index, entity)| {
            let state = if conflicting_entities.contains(&entity.id) {
                EntityState::Conflicting
            } else if system
                .layout
                .parameters(index)
                .all(|parameter| distances[parameter] <= DETERMINED_TOLERANCE)
            {
                EntityState::FullyConstrained
            } else {
                EntityState::UnderConstrained
            };
            (entity.id.clone(), state)
        })
        .collect();
    Diagnosis {
        dof: u32::try_from(parameter_count - rank)
            .expect("a sketch has fewer than 2^32 solver parameters"),
        redundant: redundant_ids,
        conflicting: conflicting_ids,
        entity_states,
    }
}

/// Flags the constraint that owns a row; arc rules are never reported.
fn mark(owner: RowOwner, flags: &mut [bool]) {
    if let RowOwner::Constraint(constraint) = owner {
        flags[constraint] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::horizontal_distance_beside_a_distance;

    #[test]
    fn a_row_within_the_step_tolerance_of_the_span_still_counts_as_independent() {
        // The distance row lies 1e-7 outside the span of the horizontal distance row: within the
        // Gauss-Newton step's tolerance (1e-6), above RANK_TOLERANCE, which the diagnosis keeps.
        let position = [10.0, 1e-6];
        let (sketch, values) =
            horizontal_distance_beside_a_distance(position, 10.0, position[0].hypot(position[1]));
        let system = System::new(&sketch, &values, &sketch.entities);

        let diagnosis = diagnose(&sketch, &system, &system.warm_start);

        assert_eq!(diagnosis.dof, 2);
        assert!(diagnosis.redundant.is_empty(), "{:?}", diagnosis.redundant);
        assert!(
            diagnosis.conflicting.is_empty(),
            "{:?}",
            diagnosis.conflicting
        );
    }
}
