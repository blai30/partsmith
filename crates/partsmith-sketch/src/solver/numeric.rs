//! Dog-leg with a Levenberg-Marquardt fallback on the substituted problem of one component (spec section 3.7.4).
//!
//! The variable values `y` are scaled as `y = D z`; steps, trust radii and norms live in `z`.

use super::sparse_qr::{Added, SparseQr};
use super::substitution::Substitution;
use super::system::System;
use crate::vector::{dot, length};
use crate::{MAX_ITERATIONS, RANK_TOLERANCE, SOLVER_TOLERANCE};

/// Smallest initial trust radius.
const MIN_TRUST_RADIUS: f64 = 1e-6;

/// Relative step length below which an iteration makes no progress.
const MIN_RELATIVE_STEP: f64 = 1e-14;

/// Largest residual (mm) at which a converged point needs no refinement; far below `SOLVER_TOLERANCE`.
const REFINED_RESIDUAL: f64 = 1e-12;

/// Most Gauss-Newton steps that refine a converged point.
const MAX_REFINEMENT_STEPS: usize = 3;

/// The numerical problem of one component: its remaining rows over its variables.
pub(crate) struct Problem<'a> {
    system: &'a System,
    substitution: &'a Substitution,
    /// The diagonal of `D`: `1 / DRAG_WEIGHT` for dragged variables, else 1.
    scales: Vec<f64>,
    /// The full parameter vector; the component's parameters are rewritten on each evaluation.
    x: Vec<f64>,
    /// Scratch for one row's gradient.
    gradient: Vec<(usize, f64)>,
    /// The factorization of the Gauss-Newton steps, reused so that its plans and buffers are made once.
    factorization: &'a mut SparseQr,
}

/// Residuals and the Jacobian with respect to `z` at one point.
struct Evaluation {
    residuals: Vec<f64>,
    /// The sparse rows of the Jacobian back to back: (variable, partial derivative), ascending within a row.
    entries: Vec<(usize, f64)>,
    /// Where each row starts in `entries`, plus the end of the last.
    starts: Vec<usize>,
}

impl Evaluation {
    /// The sparse Jacobian rows in row order.
    fn rows(&self) -> impl Iterator<Item = &[(usize, f64)]> {
        self.starts
            .windows(2)
            .map(|bounds| &self.entries[bounds[0]..bounds[1]])
    }

    /// `J^T v` for a vector with one entry per row.
    fn transpose_times(&self, vector: &[f64], variable_count: usize) -> Vec<f64> {
        let mut product = vec![0.0; variable_count];
        for (row, &value) in self.rows().zip(vector) {
            for &(variable, partial) in row {
                product[variable] += partial * value;
            }
        }
        product
    }

    /// `J v` for a vector with one entry per variable.
    fn times(&self, vector: &[f64]) -> Vec<f64> {
        self.rows()
            .map(|row| {
                row.iter()
                    .map(|&(variable, partial)| partial * vector[variable])
                    .sum()
            })
            .collect()
    }

    /// The largest absolute residual, or 0 without rows.
    fn largest_residual(&self) -> f64 {
        self.residuals
            .iter()
            .fold(0.0, |largest, residual| largest.max(residual.abs()))
    }

    /// True when every row holds within `SOLVER_TOLERANCE`.
    fn converged(&self) -> bool {
        self.residuals
            .iter()
            .all(|residual| residual.abs() <= SOLVER_TOLERANCE)
    }
}

impl<'a> Problem<'a> {
    /// The problem of a substituted component; `x` holds the warm start and `scales` one entry per variable.
    pub(crate) fn new(
        system: &'a System,
        substitution: &'a Substitution,
        scales: Vec<f64>,
        x: Vec<f64>,
        factorization: &'a mut SparseQr,
    ) -> Problem<'a> {
        Problem {
            system,
            substitution,
            scales,
            x,
            gradient: Vec::new(),
            factorization,
        }
    }

    /// The full parameter vector for these variable values.
    pub(crate) fn parameters(&mut self, variables: &[f64]) -> &[f64] {
        self.substitution.write(variables, &mut self.x);
        &self.x
    }

    fn evaluate(&mut self, variables: &[f64]) -> Evaluation {
        self.substitution.write(variables, &mut self.x);
        let count = self.substitution.rows.len();
        let mut residuals = Vec::with_capacity(count);
        let mut entries = Vec::new();
        let mut starts = Vec::with_capacity(count + 1);
        starts.push(0);
        for &row in &self.substitution.rows {
            self.gradient.clear();
            residuals.push(self.system.rows[row].value_and_gradient(&self.x, &mut self.gradient));
            let start = entries.len();
            for &(parameter, partial) in &self.gradient {
                if let Some(variable) = self.substitution.variable(parameter) {
                    entries.push((variable, partial * self.scales[variable]));
                }
            }
            entries[start..].sort_by_key(|&(variable, _)| variable);
            // Partials of one variable (from several parameters of its class) are summed in order.
            let mut end = start;
            for index in start..entries.len() {
                let (variable, partial) = entries[index];
                if end > start && entries[end - 1].0 == variable {
                    entries[end - 1].1 += partial;
                } else {
                    entries[end] = (variable, partial);
                    end += 1;
                }
            }
            entries.truncate(end);
            starts.push(end);
        }
        Evaluation {
            residuals,
            entries,
            starts,
        }
    }

    /// The minimum-norm Gauss-Newton step at `evaluation` on the rows that pass the sequential rank test.
    fn gauss_newton_step(&mut self, evaluation: &Evaluation) -> Vec<f64> {
        self.factorization.clear(self.scales.len());
        let mut accepted_residuals = Vec::new();
        for (row, &residual) in evaluation.rows().zip(&evaluation.residuals) {
            if self.factorization.add(row, RANK_TOLERANCE) == Added::Accepted {
                accepted_residuals.push(-residual);
            }
        }
        self.factorization
            .minimum_norm_solution(&accepted_residuals)
    }

    /// `y + D h` for the variable values `y`.
    fn advance(&self, variables: &[f64], step: &[f64]) -> Vec<f64> {
        variables
            .iter()
            .zip(step)
            .zip(&self.scales)
            .map(|((value, change), scale)| value + scale * change)
            .collect()
    }

    /// `norm(z)` for `z = y / D` with the variable values `y`.
    fn scaled_norm(&self, variables: &[f64]) -> f64 {
        variables
            .iter()
            .zip(&self.scales)
            .map(|(value, scale)| (value / scale).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    fn too_small(&self, step: &[f64], variables: &[f64]) -> bool {
        length(step) <= MIN_RELATIVE_STEP * (self.scaled_norm(variables) + MIN_RELATIVE_STEP)
    }
}

/// How the numerical solve of a component ended.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Outcome {
    /// Every row holds within `SOLVER_TOLERANCE` at these variable values.
    Converged(Vec<f64>),
    /// Neither method converged; these are the variable values where Levenberg-Marquardt stopped.
    Failed(Vec<f64>),
}

/// Runs dog-leg from `start`, then Levenberg-Marquardt from `start` again when dog-leg does not converge.
/// A converged point is refined before it is returned.
pub(crate) fn solve(problem: &mut Problem<'_>, start: &[f64]) -> Outcome {
    let (solution, evaluation) = match dog_leg(problem, start) {
        Some(converged) => converged,
        None => match levenberg_marquardt(problem, start) {
            Ok(converged) => converged,
            Err(end) => return Outcome::Failed(end),
        },
    };
    Outcome::Converged(refine(problem, solution, evaluation))
}

/// `F(z) - F(z + h)` computed from the two residual vectors without cancellation of the squares.
fn actual_reduction(before: &[f64], after: &[f64]) -> f64 {
    0.5 * before
        .iter()
        .zip(after)
        .map(|(old, new)| (old - new) * (old + new))
        .sum::<f64>()
}

/// The gain ratio, or negative infinity when the model predicts no decrease or the new point is not finite.
fn gain_ratio(actual: f64, predicted: f64) -> f64 {
    if predicted > 0.0 && actual.is_finite() {
        actual / predicted
    } else {
        f64::NEG_INFINITY
    }
}

/// Powell's dog-leg method: the converged point with its evaluation, or `None` when it stops without converging.
///
/// A step at most `MIN_RELATIVE_STEP` long ends the iteration, but only after it was tried, so a last
/// tiny step that completes convergence is kept.
fn dog_leg(problem: &mut Problem<'_>, start: &[f64]) -> Option<(Vec<f64>, Evaluation)> {
    let variable_count = start.len();
    let mut variables = start.to_vec();
    let mut current = problem.evaluate(&variables);
    if current.converged() {
        return Some((variables, current));
    }
    let mut gauss_newton = problem.gauss_newton_step(&current);
    let mut radius = length(&gauss_newton).max(MIN_TRUST_RADIUS);
    for _ in 0..MAX_ITERATIONS {
        let gradient = current.transpose_times(&current.residuals, variable_count);
        let gradient_image = current.times(&gradient);
        let curvature = dot(&gradient_image, &gradient_image);
        if curvature == 0.0 {
            return None;
        }
        let gradient_norm = length(&gradient);
        let alpha = gradient_norm * gradient_norm / curvature;
        let steepest: Vec<f64> = gradient.iter().map(|value| -alpha * value).collect();
        let step = if length(&gauss_newton) <= radius {
            gauss_newton.clone()
        } else if alpha * gradient_norm >= radius {
            gradient
                .iter()
                .map(|value| -radius / gradient_norm * value)
                .collect()
        } else {
            blend(&steepest, &gauss_newton, radius)
        };
        let stalled = problem.too_small(&step, &variables);
        let step_norm = length(&step);
        let candidate = problem.advance(&variables, &step);
        let trial = problem.evaluate(&candidate);
        let step_image = current.times(&step);
        let predicted = -dot(&step, &gradient) - 0.5 * dot(&step_image, &step_image);
        let rho = gain_ratio(
            actual_reduction(&current.residuals, &trial.residuals),
            predicted,
        );
        if rho > 0.75 {
            radius = radius.max(3.0 * step_norm);
        }
        if rho < 0.25 {
            radius /= 2.0;
        }
        if rho > 0.0 {
            variables = candidate;
            current = trial;
            if current.converged() {
                return Some((variables, current));
            }
            gauss_newton = problem.gauss_newton_step(&current);
        }
        if stalled {
            return None;
        }
    }
    None
}

/// The point on the segment from `steepest` to `gauss_newton` at distance `radius` from the origin.
fn blend(steepest: &[f64], gauss_newton: &[f64], radius: f64) -> Vec<f64> {
    let difference: Vec<f64> = gauss_newton
        .iter()
        .zip(steepest)
        .map(|(target, start)| target - start)
        .collect();
    let along = dot(steepest, &difference);
    let length_squared = dot(&difference, &difference);
    let room = radius * radius - dot(steepest, steepest);
    let root = (along * along + length_squared * room).sqrt();
    let beta = if along <= 0.0 {
        (root - along) / length_squared
    } else {
        room / (along + root)
    };
    steepest
        .iter()
        .zip(&difference)
        .map(|(start, change)| start + beta * change)
        .collect()
}

/// Levenberg-Marquardt: the converged point with its evaluation, or `Err` with the point where it
/// stopped without converging.
///
/// Like dog-leg, it tries a step at most `MIN_RELATIVE_STEP` long before it stops.
fn levenberg_marquardt(
    problem: &mut Problem<'_>,
    start: &[f64],
) -> Result<(Vec<f64>, Evaluation), Vec<f64>> {
    let variable_count = start.len();
    let mut variables = start.to_vec();
    let mut current = problem.evaluate(&variables);
    if current.converged() {
        return Ok((variables, current));
    }
    let mut diagonal = vec![0.0; variable_count];
    for row in current.rows() {
        for &(variable, partial) in row {
            diagonal[variable] += partial * partial;
        }
    }
    let mut damping = 1e-3 * diagonal.iter().copied().fold(0.0, f64::max);
    if damping == 0.0 {
        return Err(variables);
    }
    let mut growth = 2.0;
    for _ in 0..MAX_ITERATIONS {
        let gradient = current.transpose_times(&current.residuals, variable_count);
        let step = damped_step(&current, damping, variable_count);
        let stalled = problem.too_small(&step, &variables);
        let candidate = problem.advance(&variables, &step);
        let trial = problem.evaluate(&candidate);
        let predicted = 0.5
            * step
                .iter()
                .zip(&gradient)
                .map(|(change, slope)| change * (damping * change - slope))
                .sum::<f64>();
        let rho = gain_ratio(
            actual_reduction(&current.residuals, &trial.residuals),
            predicted,
        );
        if rho > 0.0 {
            variables = candidate;
            current = trial;
            if current.converged() {
                return Ok((variables, current));
            }
            damping *= (1.0 - (2.0 * rho - 1.0).powi(3)).max(1.0 / 3.0);
            growth = 2.0;
        } else {
            damping *= growth;
            growth *= 2.0;
        }
        if stalled {
            return Err(variables);
        }
    }
    Err(variables)
}

/// Takes minimum-norm Gauss-Newton steps from a converged point while they lower its largest residual,
/// until that is at most `REFINED_RESIDUAL`.
///
/// Converged rows hold within `SOLVER_TOLERANCE` only. Returning arcs moves their end points by up to
/// half the arc rule's residual (section 3.7.6), so a solution that barely converged could come back
/// with a row slightly above the tolerance, and solving the output again would move it. Refined, the
/// conversion moves points far less than the tolerance, so the output solves back to itself bitwise.
/// Every accepted step lowers the largest residual, so the point stays converged.
fn refine(problem: &mut Problem<'_>, mut variables: Vec<f64>, mut current: Evaluation) -> Vec<f64> {
    for _ in 0..MAX_REFINEMENT_STEPS {
        let largest = current.largest_residual();
        if largest <= REFINED_RESIDUAL {
            break;
        }
        let step = problem.gauss_newton_step(&current);
        let candidate = problem.advance(&variables, &step);
        let trial = problem.evaluate(&candidate);
        let finite = trial.residuals.iter().all(|residual| residual.is_finite());
        if !finite || trial.largest_residual() >= largest {
            break;
        }
        variables = candidate;
        current = trial;
    }
    variables
}

/// The solution of `(J^T J + mu I) h = -J^T r`, as the least-squares solution of `[J; sqrt(mu) I] h = [-r; 0]`.
fn damped_step(evaluation: &Evaluation, damping: f64, variable_count: usize) -> Vec<f64> {
    let rows = evaluation.residuals.len();
    let mut columns: Vec<Vec<(usize, f64)>> = vec![Vec::new(); variable_count];
    for (row, partials) in evaluation.rows().enumerate() {
        for &(variable, partial) in partials {
            columns[variable].push((row, partial));
        }
    }
    let mut factorization = SparseQr::new(rows + variable_count);
    let diagonal = damping.sqrt();
    for (variable, column) in columns.iter_mut().enumerate() {
        column.push((rows + variable, diagonal));
        factorization.add(column, 0.0);
    }
    let mut right_side: Vec<f64> = evaluation.residuals.iter().map(|value| -value).collect();
    right_side.resize(rows + variable_count, 0.0);
    factorization.least_squares(&right_side)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_lands_on_the_trust_radius() {
        let step = blend(&[1.0, 0.0], &[3.0, 4.0], 2.0);

        assert!((length(&step) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn actual_reduction_is_half_the_drop_in_squares() {
        assert_eq!(actual_reduction(&[3.0, 4.0], &[0.0, 0.0]), 12.5);
    }

    #[test]
    fn gain_ratio_rejects_non_finite_points() {
        assert_eq!(gain_ratio(f64::NAN, 1.0), f64::NEG_INFINITY);
        assert_eq!(gain_ratio(1.0, 0.0), f64::NEG_INFINITY);
        assert_eq!(gain_ratio(1.0, 2.0), 0.5);
    }
}
