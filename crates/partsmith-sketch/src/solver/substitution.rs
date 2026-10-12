//! Substitution of trivial equalities (spec section 3.7.3).
//!
//! The parameters of a component are grouped into classes that must be equal. A class is frozen when
//! a `fix` or a coincidence with the origin pins it. Free classes that some remaining row reads become
//! the variables of the numerical problem.

use super::rows::Equality;
use super::system::{Component, System};
use crate::SOLVER_TOLERANCE;

/// The classes of one component and the variables of its numerical problem.
#[derive(Clone, Debug)]
pub(crate) struct Substitution {
    /// Global parameter indices of the component, ascending.
    parameters: Vec<usize>,
    /// Union-find parent of each local parameter; a root is the lowest local index of its class.
    parents: Vec<usize>,
    /// The frozen value of each class root, if frozen.
    frozen: Vec<Option<f64>>,
    /// The value of each class root.
    values: Vec<f64>,
    /// Rows of the numerical problem (global row indices, ascending): every row that is not identically satisfied.
    pub(crate) rows: Vec<usize>,
    /// Class root of each variable, ascending.
    variables: Vec<usize>,
    /// Variable index of each class root.
    variable_of_root: Vec<Option<usize>>,
}

impl Substitution {
    /// Groups the parameters of `component` with the warm start `x` (section 3.7.3).
    pub(crate) fn new(system: &System, component: &Component, x: &[f64]) -> Substitution {
        let count = component.parameters.len();
        let mut substitution = Substitution {
            parameters: component.parameters.clone(),
            parents: (0..count).collect(),
            frozen: vec![None; count],
            values: Vec::new(),
            rows: Vec::new(),
            variables: Vec::new(),
            variable_of_root: vec![None; count],
        };
        for &constraint in &component.constraints {
            for row in &system.rows[system.constraint_rows[constraint].clone()] {
                match row.equality {
                    Some(Equality::Pair(first, second)) => substitution.merge(first, second),
                    Some(Equality::Zero(parameter)) => substitution.freeze_at_zero(parameter),
                    None => {}
                }
            }
            for &(parameter, value) in &system.freezes[constraint] {
                let root = substitution.root_of(parameter);
                substitution.frozen[root].get_or_insert(value);
            }
        }
        substitution.values = substitution.class_values(x);
        substitution.rows = component
            .rows
            .iter()
            .copied()
            .filter(|&row| match system.rows[row].equality {
                Some(Equality::Pair(first, second)) => {
                    substitution.root_of(first) != substitution.root_of(second)
                }
                _ => true,
            })
            .collect();
        let mut used = vec![false; count];
        for &row in &substitution.rows {
            for parameter in system.rows[row].parameters() {
                used[substitution.root_of(parameter)] = true;
            }
        }
        for root in 0..count {
            if substitution.parents[root] == root && used[root] && substitution.frozen[root].is_none() {
                substitution.variable_of_root[root] = Some(substitution.variables.len());
                substitution.variables.push(root);
            }
        }
        substitution
    }

    fn local(&self, parameter: usize) -> usize {
        self.parameters
            .binary_search(&parameter)
            .expect("rows of a component only read parameters of its entities")
    }

    fn root(&self, mut local: usize) -> usize {
        while self.parents[local] != local {
            local = self.parents[local];
        }
        local
    }

    fn root_of(&self, parameter: usize) -> usize {
        self.root(self.local(parameter))
    }

    /// `x_first = x_second`: merges the two classes unless both are frozen at values more than the tolerance apart.
    fn merge(&mut self, first: usize, second: usize) {
        let (first, second) = (self.root_of(first), self.root_of(second));
        if first == second {
            return;
        }
        let (low, high) = (first.min(second), first.max(second));
        let frozen = match (self.frozen[low], self.frozen[high]) {
            (Some(low_value), Some(high_value)) => {
                if (low_value - high_value).abs() > SOLVER_TOLERANCE {
                    return;
                }
                Some(low_value)
            }
            (low_value, high_value) => low_value.or(high_value),
        };
        self.parents[high] = low;
        self.frozen[low] = frozen;
    }

    /// `x = 0` against a coordinate of `origin`: freezes the class at exactly 0 unless it is frozen elsewhere.
    fn freeze_at_zero(&mut self, parameter: usize) {
        let root = self.root_of(parameter);
        match self.frozen[root] {
            Some(value) if value.abs() > SOLVER_TOLERANCE => {}
            _ => self.frozen[root] = Some(0.0),
        }
    }

    /// The value of every class root: its frozen value, the common value of its members, or their mean.
    fn class_values(&self, x: &[f64]) -> Vec<f64> {
        let count = self.parameters.len();
        let mut members: Vec<Vec<f64>> = vec![Vec::new(); count];
        for local in 0..count {
            members[self.root(local)].push(x[self.parameters[local]]);
        }
        members
            .into_iter()
            .enumerate()
            .map(|(root, values)| match self.frozen[root] {
                Some(value) => value,
                // Only roots have members; the entries of other parameters are never read.
                None if values.is_empty() => 0.0,
                None => {
                    let first = values[0];
                    if values.iter().all(|value| value.to_bits() == first.to_bits()) {
                        first
                    } else {
                        values.iter().sum::<f64>() / values.len() as f64
                    }
                }
            })
            .collect()
    }

    /// Number of variables.
    pub(crate) fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// The variable a parameter belongs to, or `None` for a frozen or unused class.
    pub(crate) fn variable(&self, parameter: usize) -> Option<usize> {
        self.variable_of_root[self.root_of(parameter)]
    }

    /// The current values of the variables.
    pub(crate) fn variable_values(&self) -> Vec<f64> {
        self.variables.iter().map(|&root| self.values[root]).collect()
    }

    /// Starts each free class that contains a dragged parameter at that parameter's target (section 3.9):
    /// the target of its lowest-index dragged parameter when it contains several. Frozen classes keep
    /// their value. Returns the variables of the dragged classes.
    pub(crate) fn drag(&mut self, targets: &[(usize, f64)]) -> Vec<usize> {
        let mut targets = targets.to_vec();
        targets.sort_by_key(|&(parameter, _)| parameter);
        let mut started = vec![false; self.parameters.len()];
        let mut variables = Vec::new();
        for (parameter, target) in targets {
            let root = self.root_of(parameter);
            if self.frozen[root].is_some() || started[root] {
                continue;
            }
            started[root] = true;
            self.values[root] = target;
            variables.extend(self.variable_of_root[root]);
        }
        variables
    }

    /// Writes every class value into the component's parameters of `x`, with `variables` as the variable values.
    pub(crate) fn write(&self, variables: &[f64], x: &mut [f64]) {
        for (local, &parameter) in self.parameters.iter().enumerate() {
            let root = self.root(local);
            x[parameter] = match self.variable_of_root[root] {
                Some(variable) => variables[variable],
                None => self.values[root],
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::{ConstraintKind, SketchConstraint};
    use crate::entity::{Geometry, SketchEntity};
    use crate::params::{DimensionValues, SketchParams};
    use crate::plane::{OriginPlane, SketchPlane};
    use partsmith_model::Id;

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn line(name: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
        SketchEntity {
            id: id(name),
            geometry: Geometry::Line { start, end },
            construction: false,
        }
    }

    fn substitution(entities: Vec<SketchEntity>, kinds: Vec<ConstraintKind>) -> (Substitution, Vec<f64>) {
        let sketch = SketchParams {
            plane: SketchPlane::Origin {
                plane: OriginPlane::Xy,
            },
            entities,
            constraints: kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| SketchConstraint {
                    id: id(&format!("c-{index}")),
                    kind,
                })
                .collect(),
        };
        let system = System::new(&sketch, &DimensionValues::new(), &sketch.entities);
        let x = system.layout.vector(&sketch.entities);
        (Substitution::new(&system, &system.components[0], &x), x)
    }

    fn point(text: &str) -> crate::PointRef {
        text.parse().expect("valid point reference")
    }

    #[test]
    fn coincident_points_share_their_mean() {
        let (substitution, x) = substitution(
            vec![
                line("e-1", [0.0, 0.0], [10.0, 0.0]),
                line("e-2", [10.0, 2.0], [20.0, 2.0]),
            ],
            vec![ConstraintKind::Coincident {
                points: [point("e-1.end"), point("e-2.start")],
            }],
        );
        let mut solved = x.clone();

        substitution.write(&substitution.variable_values(), &mut solved);

        assert_eq!(&solved[2..6], &[10.0, 1.0, 10.0, 1.0]);
        assert!(substitution.rows.is_empty(), "both coincident rows are skipped");
        assert_eq!(substitution.variable_count(), 0);
    }

    #[test]
    fn fix_freezes_the_merged_class_at_the_fixed_value() {
        let (substitution, x) = substitution(
            vec![line("e-1", [0.0, 0.0], [10.0, 1.0])],
            vec![
                ConstraintKind::Fix {
                    target: point("e-1.start"),
                },
                ConstraintKind::Horizontal(crate::Alignment::Line { line: id("e-1") }),
            ],
        );
        let mut solved = x.clone();

        substitution.write(&substitution.variable_values(), &mut solved);

        assert_eq!(solved, [0.0, 0.0, 10.0, 0.0]);
    }

    #[test]
    fn frozen_classes_with_different_values_stay_apart() {
        let (substitution, _) = substitution(
            vec![line("e-1", [0.0, 0.0], [10.0, 1.0])],
            vec![
                ConstraintKind::Fix {
                    target: point("e-1"),
                },
                ConstraintKind::Horizontal(crate::Alignment::Line { line: id("e-1") }),
            ],
        );

        assert_eq!(substitution.rows.len(), 5, "the four fix rows and the horizontal row remain");
        assert_eq!(substitution.variable_count(), 0);
    }

    #[test]
    fn dragged_classes_start_at_the_lowest_parameter_target() {
        let (mut substitution, x) = substitution(
            vec![line("e-1", [0.0, 0.0], [10.0, 1.0])],
            vec![ConstraintKind::Horizontal(crate::Alignment::Line { line: id("e-1") })],
        );
        let mut solved = x.clone();

        let dragged = substitution.drag(&[(3, 7.0), (1, 4.0), (0, 3.0)]);
        substitution.write(&substitution.variable_values(), &mut solved);

        assert_eq!(solved, [3.0, 4.0, 10.0, 4.0]);
        assert!(dragged.is_empty(), "no remaining row reads these classes");
    }

    #[test]
    fn coincidence_with_the_origin_freezes_at_zero() {
        let (substitution, x) = substitution(
            vec![line("e-1", [3.0, 4.0], [10.0, 1.0])],
            vec![ConstraintKind::Coincident {
                points: [point("e-1.start"), point("origin")],
            }],
        );
        let mut solved = x.clone();

        substitution.write(&substitution.variable_values(), &mut solved);

        assert_eq!(solved, [0.0, 0.0, 10.0, 1.0]);
    }
}
