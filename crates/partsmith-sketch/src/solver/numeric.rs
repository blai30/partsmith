//! The trust region with the exact step and a Levenberg-Marquardt fallback on the substituted
//! problem of one component (spec section 3.7.4).
//!
//! The variable values `y` are scaled as `y = D z`; steps, trust radii and norms live in `z`.

mod cholesky;

use super::sparse_qr::{Added, SparseQr};
use super::substitution::Substitution;
use super::system::System;
use crate::vector::{dot, length};
use crate::{MAX_ITERATIONS, RANK_TOLERANCE, SOLVER_TOLERANCE};
use cholesky::Cholesky;

/// Relative norm below which a Jacobian row counts as dependent for the Gauss-Newton step of the
/// first trust-region pass. It is larger than the diagnosis's `RANK_TOLERANCE`. A row that only
/// repeats others near a solution is left out instead of driving the step through a part of rounding
/// size (section 3.7.4).
const STEP_RANK_TOLERANCE: f64 = 1e-6;

/// Smallest first trust radius.
const MIN_TRUST_RADIUS: f64 = 1e-6;

/// The first damping of the trust region and of Levenberg-Marquardt, relative to the largest
/// squared column norm of `J`.
const FIRST_DAMPING: f64 = 1e-3;

/// The smallest damping of the exact step, relative to the largest squared column norm of `J`. It
/// keeps the damping far above the squared singular value of a direction that only a row within the
/// step tolerance of the span sets (ADR 0058).
const DAMPING_FLOOR: f64 = 1e-10;

/// How far, relative to the trust radius, the length of the exact step may be from the radius.
const RADIUS_BAND: f64 = 0.1;

/// Most dampings Moré's iteration tries for one exact step.
const MAX_DAMPING_ITERATIONS: usize = 10;

/// Relative step length below which an iteration makes no progress.
const MIN_RELATIVE_STEP: f64 = 1e-14;

/// Largest residual (mm) at which a converged point needs no refinement; far below `SOLVER_TOLERANCE`.
const REFINED_RESIDUAL: f64 = 1e-12;

/// Most Gauss-Newton steps that refine a converged point.
const MAX_REFINEMENT_STEPS: usize = 3;

/// The factorizations of a component's steps. They are kept across the solves of a component and
/// the frames of a drag. Their plans and patterns are worked out once.
#[derive(Debug, Default)]
pub(crate) struct Factorizations {
    /// The rank-revealing QR of the Gauss-Newton steps.
    gauss_newton: SparseQr,
    /// The Cholesky factorization of the damped steps.
    damped: Cholesky,
}

/// The numerical problem of one component: its remaining rows over its variables.
pub(crate) struct Problem<'a> {
    system: &'a System,
    substitution: &'a Substitution,
    /// The diagonal of `D`: the inverse drag weight for dragged variables (section 3.9), else 1.
    scales: Vec<f64>,
    /// The full parameter vector; the component's parameters are rewritten on each evaluation.
    x: Vec<f64>,
    /// Scratch for one row's gradient.
    gradient: Vec<(usize, f64)>,
    /// The factorizations of the steps, reused so that their plans and buffers are made once.
    factorizations: &'a mut Factorizations,
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

    /// `max(diag(J^T J))`, the largest squared column norm of `J`, or 0 without variables.
    fn largest_squared_column_norm(&self, variable_count: usize) -> f64 {
        let mut diagonal = vec![0.0; variable_count];
        for row in self.rows() {
            for &(variable, partial) in row {
                diagonal[variable] += partial * partial;
            }
        }
        diagonal.into_iter().fold(0.0, f64::max)
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

/// What the trust region uses at its current point besides the residuals and the Jacobian.
struct Linearization {
    /// `g = J^T r`.
    gradient: Vec<f64>,
    /// The minimum-norm Gauss-Newton step on the rows that pass the rank test.
    gauss_newton: Vec<f64>,
    /// `d = max(diag(J^T J))`, the scale of the first damping and of the damping floor.
    largest_squared_column_norm: f64,
}

/// The damped step `h(mu) = -(J^T J + mu I)^-1 g` at one point, with what Moré's iteration uses of it.
struct DampedStep {
    /// `h(mu)`.
    step: Vec<f64>,
    /// `norm(h(mu))`.
    length: f64,
    /// `q2(mu) = h(mu)^T (J^T J + mu I)^-1 h(mu)`. The length of the step falls by
    /// `shrinkage / length` per unit of damping.
    shrinkage: f64,
}

impl DampedStep {
    /// Whether Moré's iteration takes this step, its `iteration`th, for `radius`, with `damping` and
    /// the lower bound `lower` (the stop rules of section 3.7.4, step 2.2).
    fn ends_iteration(&self, radius: f64, damping: f64, lower: f64, iteration: usize) -> bool {
        let excess = self.length - radius;
        excess.abs() <= RADIUS_BAND * radius
            || (excess < 0.0 && damping <= lower)
            || self.shrinkage <= 0.0
            || !all_finite(&self.step)
            || iteration == MAX_DAMPING_ITERATIONS
    }
}

impl<'a> Problem<'a> {
    /// The problem of a substituted component; `x` holds the warm start and `scales` one entry per variable.
    pub(crate) fn new(
        system: &'a System,
        substitution: &'a Substitution,
        scales: Vec<f64>,
        x: Vec<f64>,
        factorizations: &'a mut Factorizations,
    ) -> Problem<'a> {
        Problem {
            system,
            substitution,
            scales,
            x,
            gradient: Vec::new(),
            factorizations,
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

    /// The minimum-norm Gauss-Newton step at `evaluation` on the rows that pass the sequential rank
    /// test with `tolerance`.
    fn gauss_newton_step(&mut self, evaluation: &Evaluation, tolerance: f64) -> Vec<f64> {
        let factorization = &mut self.factorizations.gauss_newton;
        factorization.clear(self.scales.len());
        let mut accepted_residuals = Vec::new();
        for (row, &residual) in evaluation.rows().zip(&evaluation.residuals) {
            if factorization.add(row, tolerance) == Added::Accepted {
                accepted_residuals.push(-residual);
            }
        }
        factorization.minimum_norm_solution(&accepted_residuals)
    }

    /// The trust region's view of `evaluation`'s point, with the Gauss-Newton step ranked at
    /// `tolerance`. It prepares the damped steps at that point.
    fn linearize(&mut self, evaluation: &Evaluation, tolerance: f64) -> Linearization {
        let variable_count = self.scales.len();
        self.factorizations
            .damped
            .prepare(&evaluation.starts, &evaluation.entries);
        Linearization {
            gradient: evaluation.transpose_times(&evaluation.residuals, variable_count),
            gauss_newton: self.gauss_newton_step(evaluation, tolerance),
            largest_squared_column_norm: evaluation.largest_squared_column_norm(variable_count),
        }
    }

    /// The damped step for `damping` at the point of the last [`Problem::linearize`], whose
    /// residuals and Jacobian `evaluation` holds: `h = -J^T y` with `(J J^T + mu I) y = r`, and
    /// `q2 = dot(J^T y, J^T v)` with `(J J^T + mu I) v = y` (section 3.7.4).
    fn damped_step(&mut self, evaluation: &Evaluation, damping: f64) -> DampedStep {
        let variable_count = self.scales.len();
        let cholesky = &mut self.factorizations.damped;
        cholesky.factor(damping);
        let row_solution = cholesky.solve(&evaluation.residuals);
        let image = evaluation.transpose_times(&row_solution, variable_count);
        let second_image =
            evaluation.transpose_times(&cholesky.solve(&row_solution), variable_count);
        let step: Vec<f64> = image.iter().map(|value| -value).collect();
        DampedStep {
            length: length(&step),
            shrinkage: dot(&image, &second_image),
            step,
        }
    }

    /// The minimizer of the Gauss-Newton model within `radius` at `evaluation`'s point, by Moré's
    /// iteration after MINPACK's `lmpar` from the damping `hint`, with the stop rules and the
    /// damping floor of section 3.7.4. Returns the step and its damping, 0 for the Gauss-Newton step.
    fn exact_step(
        &mut self,
        evaluation: &Evaluation,
        point: &Linearization,
        radius: f64,
        hint: f64,
    ) -> (Vec<f64>, f64) {
        let gauss_newton_length = length(&point.gauss_newton);
        if gauss_newton_length <= (1.0 + RADIUS_BAND) * radius {
            return (point.gauss_newton.clone(), 0.0);
        }
        let gradient_length = length(&point.gradient);
        let upper = gradient_length / radius;
        let mut lower = (DAMPING_FLOOR * point.largest_squared_column_norm).min(upper);
        let start = if hint > 0.0 {
            hint
        } else {
            gradient_length / gauss_newton_length
        };
        // Section 3.7.4 also lowers the upper bound on a short step. Only this first clamp reads it:
        // Newton's steps are bounded below alone, as in `lmpar`.
        let mut damping = start.max(lower).min(upper);
        let mut iteration = 1;
        loop {
            let damped = self.damped_step(evaluation, damping);
            if damped.ends_iteration(radius, damping, lower, iteration) {
                return (damped.step, damping);
            }
            let excess = damped.length - radius;
            if excess > 0.0 {
                lower = lower.max(damping);
            }
            // Newton's step on `1 / norm(h(mu)) - 1 / radius`.
            damping =
                lower.max(damping + excess / radius * damped.length.powi(2) / damped.shrinkage);
            iteration += 1;
        }
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
    /// Neither method converged.
    Failed,
}

/// Runs the trust region from `start` with the step tolerance and, when it does not converge, with
/// `RANK_TOLERANCE`, then Levenberg-Marquardt, each from `start` (section 3.7.4). A converged point
/// is refined with the tolerance of its pass, `RANK_TOLERANCE` after Levenberg-Marquardt, before it
/// is returned.
pub(crate) fn solve(problem: &mut Problem<'_>, start: &[f64]) -> Outcome {
    for tolerance in [STEP_RANK_TOLERANCE, RANK_TOLERANCE] {
        if let Some((solution, evaluation)) = trust_region(problem, start, tolerance) {
            return Outcome::Converged(refine(problem, solution, evaluation, tolerance));
        }
    }
    match levenberg_marquardt(problem, start) {
        Some((solution, evaluation)) => {
            Outcome::Converged(refine(problem, solution, evaluation, RANK_TOLERANCE))
        }
        None => Outcome::Failed,
    }
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

/// One pass of the trust region with the exact step, with Gauss-Newton steps ranked at `tolerance`:
/// the converged point with its evaluation, or `None` when it stops without converging.
///
/// The first radius is the length of the damped step at Levenberg-Marquardt's first damping, and
/// the radius then follows the steps by MINPACK's rule. A step at most `MIN_RELATIVE_STEP` long ends
/// the iteration, but only after it was tried, so a last tiny step that completes convergence is kept.
fn trust_region(
    problem: &mut Problem<'_>,
    start: &[f64],
    tolerance: f64,
) -> Option<(Vec<f64>, Evaluation)> {
    let mut variables = start.to_vec();
    let mut current = problem.evaluate(&variables);
    if current.converged() {
        return Some((variables, current));
    }
    let mut point = problem.linearize(&current, tolerance);
    let first_damping = FIRST_DAMPING * point.largest_squared_column_norm;
    if first_damping == 0.0 {
        return None;
    }
    let first = problem.damped_step(&current, first_damping);
    let mut radius = first.length.max(MIN_TRUST_RADIUS);
    let mut hint = first_damping;
    // The first exact step for a radius of exactly that length is the damped step itself.
    let mut first_step = (radius == first.length
        && length(&point.gauss_newton) > (1.0 + RADIUS_BAND) * radius)
        .then_some(first.step);
    for _ in 0..MAX_ITERATIONS {
        let gradient_image = current.times(&point.gradient);
        if dot(&gradient_image, &gradient_image) == 0.0 {
            return None;
        }
        let (step, damping) = match first_step.take() {
            Some(step) => (step, first_damping),
            None => problem.exact_step(&current, &point, radius, hint),
        };
        let stalled = problem.too_small(&step, &variables);
        let candidate = problem.advance(&variables, &step);
        let trial = problem.evaluate(&candidate);
        let step_image = current.times(&step);
        let predicted = -dot(&step, &point.gradient) - 0.5 * dot(&step_image, &step_image);
        let rho = gain_ratio(
            actual_reduction(&current.residuals, &trial.residuals),
            predicted,
        );
        (radius, hint) = next_radius(radius, counted_length(&step, radius), damping, rho);
        if rho > 0.0 {
            variables = candidate;
            current = trial;
            if current.converged() {
                return Some((variables, current));
            }
            point = problem.linearize(&current, tolerance);
        }
        if stalled {
            return None;
        }
    }
    None
}

/// The length the radius update reads for `step`: its norm, or `radius` when it is not finite
/// (section 3.7.4, step 4).
fn counted_length(step: &[f64], radius: f64) -> f64 {
    if all_finite(step) {
        length(step)
    } else {
        radius
    }
}

/// Whether every value is finite.
fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// The trust radius and the damping Moré's iteration starts from after a step of `length` with
/// `damping` and the gain ratio `rho` (MINPACK's rule with its reduction factor fixed at 0.5,
/// section 3.7.4).
fn next_radius(radius: f64, length: f64, damping: f64, rho: f64) -> (f64, f64) {
    if rho < 0.25 {
        (0.5 * radius.min(10.0 * length), 2.0 * damping)
    } else if damping == 0.0 || rho >= 0.75 {
        (2.0 * length, damping / 2.0)
    } else {
        (radius, damping)
    }
}

/// Levenberg-Marquardt: the converged point with its evaluation, or `None` when it stops without
/// converging.
///
/// Like the trust region, it tries a step at most `MIN_RELATIVE_STEP` long before it stops.
fn levenberg_marquardt(problem: &mut Problem<'_>, start: &[f64]) -> Option<(Vec<f64>, Evaluation)> {
    let variable_count = start.len();
    let mut variables = start.to_vec();
    let mut current = problem.evaluate(&variables);
    if current.converged() {
        return Some((variables, current));
    }
    let mut damping = FIRST_DAMPING * current.largest_squared_column_norm(variable_count);
    if damping == 0.0 {
        return None;
    }
    let mut growth = 2.0;
    for _ in 0..MAX_ITERATIONS {
        let gradient = current.transpose_times(&current.residuals, variable_count);
        let step = levenberg_marquardt_step(&current, damping, variable_count);
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
                return Some((variables, current));
            }
            damping *= (1.0 - (2.0 * rho - 1.0).powi(3)).max(1.0 / 3.0);
            growth = 2.0;
        } else {
            damping *= growth;
            growth *= 2.0;
        }
        if stalled {
            return None;
        }
    }
    None
}

/// Takes minimum-norm Gauss-Newton steps, ranked at `tolerance`, from a converged point while they
/// lower its largest residual, until that is at most `REFINED_RESIDUAL`.
///
/// Converged rows hold within `SOLVER_TOLERANCE` only. Returning arcs moves their end points by up to
/// half the arc rule's residual (section 3.7.6), so a solution that barely converged could come back
/// with a row slightly above the tolerance, and solving the output again would move it. Refined, the
/// conversion moves points far less than the tolerance, so the output solves back to itself bitwise.
/// Every accepted step lowers the largest residual, so the point stays converged.
fn refine(
    problem: &mut Problem<'_>,
    mut variables: Vec<f64>,
    mut current: Evaluation,
    tolerance: f64,
) -> Vec<f64> {
    for _ in 0..MAX_REFINEMENT_STEPS {
        let largest = current.largest_residual();
        if largest <= REFINED_RESIDUAL {
            break;
        }
        let step = problem.gauss_newton_step(&current, tolerance);
        let candidate = problem.advance(&variables, &step);
        let trial = problem.evaluate(&candidate);
        if !all_finite(&trial.residuals) || trial.largest_residual() >= largest {
            break;
        }
        variables = candidate;
        current = trial;
    }
    variables
}

/// Levenberg-Marquardt's step, the solution of `(J^T J + mu I) h = -J^T r`, as the least-squares
/// solution of `[J; sqrt(mu) I] h = [-r; 0]`.
fn levenberg_marquardt_step(
    evaluation: &Evaluation,
    damping: f64,
    variable_count: usize,
) -> Vec<f64> {
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
    use crate::constraint::ConstraintKind;
    use crate::params::{DimensionValues, SketchParams};
    use crate::solver::system::Component;
    use crate::test_support::{
        dimension_values, horizontal_distance_beside_a_distance, point, point_entity,
        points_distance, repeated_dimensions_of_seed_37618, repeated_dimensions_of_seed_637669,
        sketch, triangle_next_to_a_fold,
    };

    /// Runs `check` on the numerical problem of `component`, without drag scaling, with its
    /// substitution and the warm start's variable values.
    fn with_component_problem<T>(
        system: &System,
        component: &Component,
        check: impl FnOnce(&mut Problem<'_>, &Substitution, &[f64]) -> T,
    ) -> T {
        let substitution = Substitution::new(system, component, &system.warm_start);
        let start = substitution.variable_values();
        let mut factorizations = Factorizations::default();
        let mut problem = Problem::new(
            system,
            &substitution,
            vec![1.0; substitution.variable_count()],
            system.warm_start.clone(),
            &mut factorizations,
        );
        check(&mut problem, &substitution, &start)
    }

    /// Runs `check` on the numerical problem of the first component of `sketch`, as
    /// [`with_component_problem`] does.
    fn with_problem<T>(
        sketch: &SketchParams,
        values: &DimensionValues,
        check: impl FnOnce(&mut Problem<'_>, &Substitution, &[f64]) -> T,
    ) -> T {
        let system = System::new(sketch, values, &sketch.entities);
        with_component_problem(&system, &system.components[0], check)
    }

    /// Whether the first trust-region pass, with the step tolerance, converges on each component of
    /// `sketch` that is not untouched (section 3.7.2), in component order.
    fn first_pass_converges(sketch: &SketchParams, values: &DimensionValues) -> Vec<bool> {
        let system = System::new(sketch, values, &sketch.entities);
        system
            .components
            .iter()
            .filter(|component| {
                !component.rows.iter().all(|&row| {
                    system.rows[row].value(&system.warm_start).abs() <= SOLVER_TOLERANCE
                })
            })
            .map(|component| {
                with_component_problem(&system, component, |problem, _, start| {
                    trust_region(problem, start, STEP_RANK_TOLERANCE).is_some()
                })
            })
            .collect()
    }

    /// The first Gauss-Newton step of two points 10.5 apart in u and `10.5 * slope` in v, with a
    /// horizontal distance and a distance both set to 10: the distance row lies `slope` outside the
    /// span of the horizontal distance row. Returns the step and the variables of e-1's and e-2's
    /// coordinates (u, v, u, v).
    fn first_step(slope: f64) -> (Vec<f64>, [usize; 4]) {
        let (sketch, values) =
            horizontal_distance_beside_a_distance([10.5, 10.5 * slope], 10.0, 10.0);
        with_problem(&sketch, &values, |problem, substitution, start| {
            let variables = [0, 1, 2, 3].map(|parameter| {
                substitution
                    .variable(parameter)
                    .expect("every coordinate is a variable")
            });
            let evaluation = problem.evaluate(start);
            (
                problem.gauss_newton_step(&evaluation, STEP_RANK_TOLERANCE),
                variables,
            )
        })
    }

    #[test]
    fn a_row_within_the_step_tolerance_of_the_span_is_left_out_of_the_step() {
        // 1e-7 outside the span: above RANK_TOLERANCE (1e-8), within the step tolerance (1e-6).
        let (step, [first_u, first_v, second_u, second_v]) = first_step(1e-7);

        assert_eq!(step[first_v], 0.0, "e-1 does not move in v");
        assert_eq!(step[second_v], 0.0, "e-2 does not move in v");
        assert!(
            (step[second_u] - step[first_u] + 0.5).abs() <= 1e-12,
            "the step makes up the horizontal distance: {step:?}"
        );
    }

    #[test]
    fn a_row_farther_than_the_step_tolerance_from_the_span_is_taken_in() {
        let (step, [_, first_v, _, second_v]) = first_step(1e-5);

        assert!(
            step[first_v] != 0.0 && step[second_v] != 0.0,
            "the distance row moves the points in v: {step:?}"
        );
    }

    #[test]
    fn the_first_pass_stops_short_next_to_a_fold() {
        // Test 258's solve: next to the fold the step tolerance leaves out a row whose residual
        // steps through the other rows cannot make up.
        let (sketch, values) = triangle_next_to_a_fold();

        let outcome = with_problem(&sketch, &values, |problem, _, start| {
            trust_region(problem, start, STEP_RANK_TOLERANCE)
        });

        assert!(
            outcome.is_none(),
            "the trust region converged with the step tolerance"
        );
    }

    #[test]
    fn levenberg_marquardt_stops_short_next_to_a_fold() {
        // With the first pass and this, test 258 would fail without the second pass.
        let (sketch, values) = triangle_next_to_a_fold();

        let outcome = with_problem(&sketch, &values, |problem, _, start| {
            levenberg_marquardt(problem, start)
        });

        assert!(outcome.is_none(), "Levenberg-Marquardt converged");
    }

    #[test]
    fn the_second_pass_converges_next_to_a_fold() {
        let (sketch, values) = triangle_next_to_a_fold();

        let outcome = with_problem(&sketch, &values, |problem, _, start| {
            trust_region(problem, start, RANK_TOLERANCE)
        });

        assert!(
            outcome.is_some(),
            "the trust region did not converge with RANK_TOLERANCE"
        );
    }

    #[test]
    fn the_first_pass_converges_on_the_repeated_dimensions_of_seed_37618() {
        // Test 262: the damped step minimizes the model over every row, the repeated ones included.
        let (sketch, values) = repeated_dimensions_of_seed_37618();

        assert_eq!(first_pass_converges(&sketch, &values), [true]);
    }

    #[test]
    fn the_first_pass_converges_on_the_repeated_dimensions_of_seed_637669() {
        // Test 262: the line with its repeated length and the circle with its radius.
        let (sketch, values) = repeated_dimensions_of_seed_637669();

        assert_eq!(first_pass_converges(&sketch, &values), [true, true]);
    }

    #[test]
    fn the_trust_region_returns_a_start_that_has_converged() {
        // Step 1 of each pass: every row already holds.
        let (sketch, values) = horizontal_distance_beside_a_distance([10.0, 0.0], 10.0, 10.0);

        let (outcome, start) = with_problem(&sketch, &values, |problem, _, start| {
            (
                trust_region(problem, start, STEP_RANK_TOLERANCE),
                start.to_vec(),
            )
        });

        let (variables, _) = outcome.expect("the start has converged");
        assert_eq!(variables, start);
    }

    #[test]
    fn the_trust_region_stops_when_no_variable_reaches_a_failing_row() {
        // Step 1 of each pass: mu0 is 0 when every row that fails is constant. Two fixed points are
        // 10 apart, with a distance of 5.
        let params = sketch(
            vec![
                point_entity("e-1", [0.0, 0.0]),
                point_entity("e-2", [10.0, 0.0]),
            ],
            vec![
                ConstraintKind::Fix {
                    target: point("e-1"),
                },
                ConstraintKind::Fix {
                    target: point("e-2"),
                },
                points_distance("e-1", "e-2", 5.0),
            ],
        );
        let values = dimension_values(&[("c-2", 5.0)]);

        let outcome = with_problem(&params, &values, |problem, substitution, start| {
            assert_eq!(substitution.variable_count(), 0, "every point is fixed");
            trust_region(problem, start, STEP_RANK_TOLERANCE)
        });

        assert!(outcome.is_none(), "the trust region converged");
    }

    /// Runs `check` on the problem of `horizontal_distance_beside_a_distance`, with its evaluation
    /// at the warm start and the trust region's view of it with the step tolerance.
    fn with_linearized<T>(
        position: [f64; 2],
        horizontal: f64,
        distance: f64,
        check: impl FnOnce(&mut Problem<'_>, &Evaluation, &Linearization) -> T,
    ) -> T {
        let (sketch, values) =
            horizontal_distance_beside_a_distance(position, horizontal, distance);
        with_problem(&sketch, &values, |problem, _, start| {
            let evaluation = problem.evaluate(start);
            let point = problem.linearize(&evaluation, STEP_RANK_TOLERANCE);
            check(problem, &evaluation, &point)
        })
    }

    #[test]
    fn the_damped_step_solves_the_damped_normal_equations() {
        // (J^T J + mu I) h = -g, with the two rows 16 degrees apart.
        with_linearized([10.5, 3.0], 10.0, 10.0, |problem, evaluation, point| {
            let damping = 0.25;

            let damped = problem.damped_step(evaluation, damping);

            let normal =
                evaluation.transpose_times(&evaluation.times(&damped.step), damped.step.len());
            for ((product, step), slope) in normal.iter().zip(&damped.step).zip(&point.gradient) {
                let left = product + damping * step + slope;
                assert!(left.abs() <= 1e-12, "{left} left over in {:?}", damped.step);
            }
            assert_eq!(damped.length, length(&damped.step));
        });
    }

    #[test]
    fn the_shrinkage_gives_the_rate_at_which_the_damped_step_shortens() {
        // d norm(h) / d mu = -q2 / norm(h).
        with_linearized([10.5, 3.0], 10.0, 10.0, |problem, evaluation, _| {
            let damping = 0.25;

            let damped = problem.damped_step(evaluation, damping);
            let nudged = problem.damped_step(evaluation, damping * (1.0 + 1e-6));

            let rate = (nudged.length - damped.length) / (damping * 1e-6);
            let expected = -damped.shrinkage / damped.length;
            assert!(
                (rate - expected).abs() <= 1e-4 * expected.abs(),
                "the length falls by {rate}, not {expected}"
            );
        });
    }

    #[test]
    fn the_exact_step_is_the_gauss_newton_step_within_the_band() {
        with_linearized([10.5, 3.0], 10.0, 10.0, |problem, evaluation, point| {
            let radius = length(&point.gauss_newton) / 1.05;

            let (step, damping) = problem.exact_step(evaluation, point, radius, 1.0);

            assert_eq!(damping, 0.0);
            assert_eq!(step, point.gauss_newton);
        });
    }

    #[test]
    fn the_exact_step_fills_the_radius_within_the_band() {
        with_linearized([10.5, 3.0], 10.0, 10.0, |problem, evaluation, point| {
            let radius = length(&point.gauss_newton) / 4.0;

            let (step, damping) = problem.exact_step(evaluation, point, radius, 0.0);

            assert!(damping > 0.0, "the step is not damped");
            assert!(
                (length(&step) - radius).abs() <= RADIUS_BAND * radius,
                "a step {} long for the radius {radius}",
                length(&step)
            );
        });
    }

    #[test]
    fn the_damping_floor_keeps_a_step_shorter_than_the_radius() {
        // The distance row lies 2e-6 outside the span of the horizontal distance row, above the
        // step tolerance, and asks for 1e-6 more. The Gauss-Newton step moves each point about
        // 0.25 in v to give it. Damped at the floor, the step stays far inside the radius.
        let slope: f64 = 2e-6;
        let distance = 10.0 * (1.0 + slope * slope).sqrt() - 1e-6;
        with_linearized(
            [10.0, 10.0 * slope],
            10.0,
            distance,
            |problem, evaluation, point| {
                let radius = 0.1;
                assert!(
                    length(&point.gauss_newton) > (1.0 + RADIUS_BAND) * radius,
                    "the Gauss-Newton step is {} long",
                    length(&point.gauss_newton)
                );

                let (step, damping) = problem.exact_step(evaluation, point, radius, 0.0);

                assert_eq!(damping, DAMPING_FLOOR * point.largest_squared_column_norm);
                assert!(
                    length(&step) < (1.0 - RADIUS_BAND) * radius,
                    "a step {} long",
                    length(&step)
                );
            },
        );
    }

    #[test]
    fn the_radius_halves_after_a_poor_step_and_the_damping_doubles() {
        assert_eq!(next_radius(4.0, 1.0, 0.5, 0.1), (2.0, 1.0));
        assert_eq!(next_radius(4.0, 0.1, 0.5, f64::NEG_INFINITY), (0.5, 1.0));
    }

    #[test]
    fn the_radius_doubles_the_step_after_a_good_or_gauss_newton_step() {
        assert_eq!(next_radius(4.0, 1.5, 0.5, 0.8), (3.0, 0.25));
        assert_eq!(next_radius(4.0, 1.5, 0.0, 0.5), (3.0, 0.0));
    }

    #[test]
    fn the_radius_and_the_damping_stay_after_a_fair_damped_step() {
        assert_eq!(next_radius(4.0, 1.5, 0.5, 0.5), (4.0, 0.5));
    }

    /// A damped step `length` long along the first variable, with `shrinkage` as its `q2`.
    fn damped(length: f64, shrinkage: f64) -> DampedStep {
        DampedStep {
            step: vec![length, 0.0],
            length,
            shrinkage,
        }
    }

    #[test]
    fn a_damped_step_within_the_band_ends_the_iteration() {
        assert!(damped(1.05, 1.0).ends_iteration(1.0, 1.0, 0.5, 1));
        assert!(damped(0.95, 1.0).ends_iteration(1.0, 1.0, 0.5, 1));
    }

    #[test]
    fn a_damped_step_outside_the_band_goes_on() {
        assert!(!damped(2.0, 1.0).ends_iteration(1.0, 1.0, 0.5, 1));
        assert!(!damped(0.5, 1.0).ends_iteration(1.0, 1.0, 0.5, 1));
    }

    #[test]
    fn a_short_step_at_the_lower_bound_ends_the_iteration() {
        assert!(damped(0.5, 1.0).ends_iteration(1.0, 0.5, 0.5, 1));
    }

    #[test]
    fn a_long_step_at_the_lower_bound_goes_on() {
        assert!(!damped(2.0, 1.0).ends_iteration(1.0, 0.5, 0.5, 1));
    }

    #[test]
    fn a_step_whose_q2_is_not_positive_ends_the_iteration() {
        assert!(damped(2.0, 0.0).ends_iteration(1.0, 1.0, 0.5, 1));
        assert!(damped(2.0, -1e-30).ends_iteration(1.0, 1.0, 0.5, 1));
    }

    #[test]
    fn a_step_that_is_not_finite_ends_the_iteration() {
        // Its length and q2 are not numbers either, so no other rule holds.
        let step = DampedStep {
            step: vec![f64::NAN, 0.0],
            length: f64::NAN,
            shrinkage: f64::NAN,
        };

        assert!(step.ends_iteration(1.0, 1.0, 0.5, 1));
    }

    #[test]
    fn the_tenth_damping_ends_the_iteration() {
        let step = damped(2.0, 1.0);

        assert!(!step.ends_iteration(1.0, 1.0, 0.5, 9));
        assert!(step.ends_iteration(1.0, 1.0, 0.5, 10));
    }

    #[test]
    fn a_finite_step_counts_its_length_for_the_radius() {
        assert_eq!(counted_length(&[3.0, 4.0], 1.0), 5.0);
    }

    #[test]
    fn a_step_that_is_not_finite_counts_as_long_as_the_radius() {
        assert_eq!(counted_length(&[f64::NAN, 4.0], 1.0), 1.0);
        assert_eq!(counted_length(&[f64::INFINITY, 4.0], 1.0), 1.0);
    }

    #[test]
    fn the_largest_squared_column_norm_sums_each_column_over_the_rows() {
        let evaluation = Evaluation {
            residuals: vec![0.0, 0.0],
            entries: vec![(0, 1.0), (1, 2.0), (0, 3.0)],
            starts: vec![0, 2, 3],
        };

        assert_eq!(evaluation.largest_squared_column_norm(2), 10.0);
    }

    #[test]
    fn the_largest_squared_column_norm_is_0_without_variables() {
        // A constant row, whose parameters are all frozen.
        let evaluation = Evaluation {
            residuals: vec![1.0],
            entries: Vec::new(),
            starts: vec![0, 0],
        };

        assert_eq!(evaluation.largest_squared_column_norm(0), 0.0);
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
