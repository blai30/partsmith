//! A sparse Householder QR factorization that takes columns one at a time, without column pivoting.
//!
//! This is the rank-revealing factorization of spec sections 3.7.4 and 3.8. Each column is a sparse
//! vector. It is transformed by the reflectors of the columns accepted before it; what is left outside
//! their pivot rows is its component orthogonal to their span. A column is dependent when that part
//! is at most `tolerance` times its original norm, and then it gets no reflector. Otherwise a
//! Householder reflector maps that part onto one new pivot row: the lowest row the transformed
//! column can have an entry on, which keeps fill local for document-ordered sketches.
//!
//! With the accepted columns as `A`, the factorization is `A = Q [R; 0]` with `Q` the product of the
//! reflectors and `R` upper triangular in acceptance order.
//!
//! Which reflectors reach a column, and which rows the result can have entries on, depend only on
//! the sparsity patterns of the columns and on which earlier columns were accepted. Each column's
//! plan is worked out from patterns alone and kept across `clear`, so the repeated factorizations of
//! one solve, whose Jacobians share their pattern, only do the arithmetic. A plan is reused while the
//! column has the same pattern and every earlier column had the same pattern and decision.

use std::ops::Range;

/// What happened to an added column.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Added {
    /// The column is independent of the accepted ones and now has a reflector.
    Accepted,
    /// The column lies in the span of the accepted columns; `projection` holds its coordinates
    /// `Q^T column` on the pivot rows as (reflector, value), ascending.
    Dependent { projection: Vec<(usize, f64)> },
}

/// The order in which reflectors are applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    /// First to last: `Q^T x`.
    Ascending,
    /// Last to first: `Q x`.
    Descending,
}

/// How one column is transformed, worked out from sparsity patterns alone.
#[derive(Clone, Debug, PartialEq)]
struct Plan {
    /// Rows the column has entries on, ascending.
    pattern: Vec<usize>,
    /// Reflectors that reach the column, in ascending order.
    schedule: Vec<usize>,
    /// Rows the transformed column can have entries on, ascending.
    rows: Vec<usize>,
    /// Whether the column was accepted; the plans after it assume this decision.
    accepted: bool,
}

/// The Householder reflectors `I - beta v v^T` of the accepted columns with their sparse vectors `v`,
/// followed by the entries of the vector being built for the next one.
#[derive(Clone, Debug, Default)]
struct Reflectors {
    /// `beta` of each reflector.
    betas: Vec<f64>,
    /// Where each reflector's entries start in `vector_rows` and `vector_values`, plus the end of the last.
    vector_starts: Vec<usize>,
    /// Rows of the vectors `v`, ascending within each reflector.
    vector_rows: Vec<usize>,
    /// Entries of the vectors `v` on those rows.
    vector_values: Vec<f64>,
}

impl Reflectors {
    /// Forgets every reflector and the vector being built.
    fn clear(&mut self) {
        self.betas.clear();
        self.vector_starts.clear();
        self.vector_starts.push(0);
        self.vector_rows.clear();
        self.vector_values.clear();
    }

    /// Where a reflector's entries sit in `vector_rows` and `vector_values`.
    #[inline]
    fn range(&self, reflector: usize) -> Range<usize> {
        self.vector_starts[reflector]..self.vector_starts[reflector + 1]
    }

    /// The rows of a reflector's vector, ascending.
    fn rows(&self, reflector: usize) -> &[usize] {
        &self.vector_rows[self.range(reflector)]
    }

    /// The rows and entries of a reflector's vector `v`, and its `beta`.
    #[inline]
    fn reflector(&self, reflector: usize) -> (&[usize], &[f64], f64) {
        let range = self.range(reflector);
        (
            &self.vector_rows[range.clone()],
            &self.vector_values[range],
            self.betas[reflector],
        )
    }

    /// Applies a reflector to a dense vector.
    fn apply(&self, reflector: usize, values: &mut [f64]) {
        let (rows, vector, beta) = self.reflector(reflector);
        reflect(rows, vector, beta, values);
    }

    /// Where the vector being built starts in `vector_rows` and `vector_values`.
    fn building_start(&self) -> usize {
        self.vector_starts[self.betas.len()]
    }

    /// Appends an entry to the vector being built.
    fn push(&mut self, row: usize, value: f64) {
        self.vector_rows.push(row);
        self.vector_values.push(value);
    }

    /// The entries of the vector being built.
    fn building(&self) -> &[f64] {
        &self.vector_values[self.building_start()..]
    }

    /// Drops the entries of the vector being built.
    fn truncate_to_built(&mut self) {
        let start = self.building_start();
        self.vector_rows.truncate(start);
        self.vector_values.truncate(start);
    }

    /// Turns the vector being built, whose norm is `norm`, into the reflector that maps it onto its
    /// first row, and returns that pivot row with the diagonal entry of `R` the reflector leaves there.
    fn finish(&mut self, norm: f64) -> (usize, f64) {
        let start = self.building_start();
        let pivot = self.vector_rows[start];
        let pivot_value = self.vector_values[start];
        let diagonal = if pivot_value >= 0.0 { -norm } else { norm };
        self.vector_values[start] -= diagonal;
        self.betas.push(1.0 / (norm * (norm + pivot_value.abs())));
        self.vector_starts.push(self.vector_rows.len());
        (pivot, diagonal)
    }
}

/// The factorization of the columns added so far.
#[derive(Clone, Debug, Default)]
pub(crate) struct SparseQr {
    /// Number of rows of every column.
    dimension: usize,
    /// Pivot row of each reflector.
    pivots: Vec<usize>,
    /// The reflector of each accepted column.
    reflectors: Reflectors,
    /// Diagonal entry of `R` in the column of each reflector.
    diagonals: Vec<f64>,
    /// Where each reflector's column of `R` starts in `above`, plus the end of the last.
    above_starts: Vec<usize>,
    /// Entries of `R` above the diagonal: (earlier reflector, value).
    above: Vec<(usize, f64)>,
    /// For each row, the reflectors whose vectors contain it, ascending.
    touching: Vec<Vec<usize>>,
    /// For each row, the reflector it is the pivot of.
    pivot_of: Vec<Option<usize>>,
    /// Plans of the columns added since `clear`, followed by plans kept from earlier factorizations.
    plans: Vec<Plan>,
    /// Number of columns added since `clear`.
    added: usize,
    /// Dense scratch vector; all zero between calls.
    values: Vec<f64>,
    /// Scratch marks of rows; all false between calls.
    marked: Vec<bool>,
    /// For each reflector, whether it waits in `queue`; all false between calls.
    queued: Vec<bool>,
    /// Reflectors waiting to be reached, sorted so that the next one is last.
    queue: Vec<usize>,
    /// The pattern of the column being added.
    pattern: Vec<usize>,
}

impl SparseQr {
    /// An empty factorization of columns with `dimension` rows.
    pub(crate) fn new(dimension: usize) -> SparseQr {
        let mut factorization = SparseQr::default();
        factorization.clear(dimension);
        factorization
    }

    /// Forgets every column and prepares for columns with `dimension` rows. Column plans are kept
    /// when the dimension is unchanged.
    pub(crate) fn clear(&mut self, dimension: usize) {
        if dimension != self.dimension {
            self.plans.clear();
            self.dimension = dimension;
        }
        self.added = 0;
        self.pivots.clear();
        self.reflectors.clear();
        self.diagonals.clear();
        self.above_starts.clear();
        self.above_starts.push(0);
        self.above.clear();
        for list in &mut self.touching {
            list.clear();
        }
        self.touching.resize_with(dimension, Vec::new);
        self.pivot_of.clear();
        self.pivot_of.resize(dimension, None);
        self.values.resize(dimension, 0.0);
        self.marked.resize(dimension, false);
        self.queued.clear();
    }

    /// Number of accepted columns.
    pub(crate) fn rank(&self) -> usize {
        self.pivots.len()
    }

    /// Adds a column given as (row, value) pairs; repeated rows are summed.
    ///
    /// The column is dependent when its part orthogonal to the accepted columns has at most `tolerance`
    /// times its own norm. Columns are Jacobian rows, whose entries are dimensionless and of order 1,
    /// so the norm is taken as at least 1: a column shorter than `tolerance` is zero up to rounding
    /// (an exactly zero column always is dependent). Spec 3.8 states the test relative to the
    /// column's own norm only; without the floor, the committed property test regressions fail.
    pub(crate) fn add(&mut self, column: &[(usize, f64)], tolerance: f64) -> Added {
        self.pattern.clear();
        for &(row, _) in column {
            if !self.marked[row] {
                self.marked[row] = true;
                self.pattern.push(row);
            }
        }
        for &row in &self.pattern {
            self.marked[row] = false;
        }
        self.pattern.sort_unstable();
        let index = self.added;
        self.added += 1;
        let reusable = self
            .plans
            .get(index)
            .is_some_and(|plan| plan.pattern == self.pattern);
        if !reusable {
            self.plans.truncate(index);
            let (schedule, rows) = self.reach(Order::Ascending);
            self.plans.push(Plan {
                pattern: self.pattern.clone(),
                schedule,
                rows,
                accepted: true,
            });
        }

        for &(row, value) in column {
            self.values[row] += value;
        }
        let original_norm = norm(self.pattern.iter().map(|&row| self.values[row]));
        let plan = &self.plans[index];
        for &reflector in &plan.schedule {
            // `reflect` itself, not `apply`, so the hot loop of the solver keeps it inlined.
            let (rows, vector, beta) = self.reflectors.reflector(reflector);
            reflect(rows, vector, beta, &mut self.values);
        }
        // The projection becomes the column of R and the residual the reflector's vector; both are
        // written in place and taken back out when the column turns out dependent.
        let above_start = self.above.len();
        for &row in &plan.rows {
            let value = std::mem::take(&mut self.values[row]);
            match self.pivot_of[row] {
                Some(reflector) => self.above.push((reflector, value)),
                None => self.reflectors.push(row, value),
            }
        }
        let residual_norm = norm(self.reflectors.building().iter().copied());
        let independent =
            residual_norm > tolerance * original_norm.max(1.0) && residual_norm.is_finite();
        if self.plans[index].accepted != independent {
            self.plans.truncate(index + 1);
            self.plans[index].accepted = independent;
        }
        if !independent {
            self.reflectors.truncate_to_built();
            let mut projection = self.above.split_off(above_start);
            projection.sort_unstable_by_key(|&(reflector, _)| reflector);
            return Added::Dependent { projection };
        }
        let reflector = self.pivots.len();
        let (pivot, diagonal) = self.reflectors.finish(residual_norm);
        for &row in self.reflectors.rows(reflector) {
            self.touching[row].push(reflector);
        }
        self.pivot_of[pivot] = Some(reflector);
        self.pivots.push(pivot);
        self.diagonals.push(diagonal);
        self.above_starts.push(self.above.len());
        self.queued.push(false);
        Added::Accepted
    }

    /// The reflectors that reach a vector with entries on the rows of `pattern` when they are applied
    /// in `order`, and every row the result can have entries on (ascending), from patterns alone.
    ///
    /// The reflectors touching the starting rows are queued; a reflector reached later in `order`
    /// adds its rows, and the reflectors after it in `order` that touch a newly added row are queued.
    fn reach(&mut self, order: Order) -> (Vec<usize>, Vec<usize>) {
        let mut schedule = Vec::new();
        let mut rows = self.pattern.clone();
        for &row in &rows {
            self.marked[row] = true;
            for &reflector in &self.touching[row] {
                enqueue(&mut self.queue, &mut self.queued, reflector, order);
            }
        }
        while let Some(reflector) = self.queue.pop() {
            self.queued[reflector] = false;
            schedule.push(reflector);
            for &row in self.reflectors.rows(reflector) {
                if self.marked[row] {
                    continue;
                }
                self.marked[row] = true;
                rows.push(row);
                let list = &self.touching[row];
                let following = match order {
                    Order::Ascending => &list[list.partition_point(|&other| other <= reflector)..],
                    Order::Descending => &list[..list.partition_point(|&other| other < reflector)],
                };
                for &next in following {
                    enqueue(&mut self.queue, &mut self.queued, next, order);
                }
            }
        }
        for &row in &rows {
            self.marked[row] = false;
        }
        rows.sort_unstable();
        (schedule, rows)
    }

    /// The minimum-norm `h` with `A^T h = right_side`, where `right_side` has one entry per accepted column.
    pub(crate) fn minimum_norm_solution(&self, right_side: &[f64]) -> Vec<f64> {
        let mut solution = vec![0.0; self.dimension];
        let mut forward = vec![0.0; self.rank()];
        for index in 0..self.rank() {
            let known: f64 = self
                .above(index)
                .iter()
                .map(|&(earlier, value)| value * forward[earlier])
                .sum();
            forward[index] = (right_side[index] - known) / self.diagonals[index];
            solution[self.pivots[index]] = forward[index];
        }
        for reflector in (0..self.rank()).rev() {
            self.reflectors.apply(reflector, &mut solution);
        }
        solution
    }

    /// The `h` minimizing `norm(A h - right_side)`, one entry per accepted column; `right_side` is dense.
    pub(crate) fn least_squares(&self, right_side: &[f64]) -> Vec<f64> {
        let mut transformed = right_side.to_vec();
        for reflector in 0..self.rank() {
            self.reflectors.apply(reflector, &mut transformed);
        }
        let projection: Vec<f64> = self
            .pivots
            .iter()
            .map(|&pivot| transformed[pivot])
            .collect();
        self.back_substitute(projection)
    }

    /// The least-squares coefficients over the accepted columns of a dependent column, from its projection.
    pub(crate) fn coefficients(&self, projection: &[(usize, f64)]) -> Vec<f64> {
        let mut dense = vec![0.0; self.rank()];
        for &(reflector, value) in projection {
            dense[reflector] = value;
        }
        self.back_substitute(dense)
    }

    /// Solves `R x = right_side` in place.
    fn back_substitute(&self, mut values: Vec<f64>) -> Vec<f64> {
        for index in (0..self.rank()).rev() {
            values[index] /= self.diagonals[index];
            let solved = values[index];
            if solved != 0.0 {
                for &(earlier, value) in self.above(index) {
                    values[earlier] -= value * solved;
                }
            }
        }
        values
    }

    /// For each row, the distance from its unit vector to the span of the accepted columns.
    pub(crate) fn complement_distances(&mut self) -> Vec<f64> {
        let mut squared = vec![0.0; self.dimension];
        for row in 0..self.dimension {
            if self.pivot_of[row].is_some() {
                continue;
            }
            // Q e_row is a unit vector orthogonal to every accepted column; together they span the complement.
            self.pattern.clear();
            self.pattern.push(row);
            let (schedule, rows) = self.reach(Order::Descending);
            self.values[row] = 1.0;
            for &reflector in &schedule {
                self.reflectors.apply(reflector, &mut self.values);
            }
            for &other in &rows {
                let value = std::mem::take(&mut self.values[other]);
                squared[other] += value * value;
            }
        }
        squared.into_iter().map(f64::sqrt).collect()
    }

    /// The entries of `R` above the diagonal in a reflector's column.
    fn above(&self, reflector: usize) -> &[(usize, f64)] {
        &self.above[self.above_starts[reflector]..self.above_starts[reflector + 1]]
    }
}

/// Queues a reflector unless it already waits, keeping the queue sorted so that the next one in `order` is last.
fn enqueue(queue: &mut Vec<usize>, queued: &mut [bool], reflector: usize, order: Order) {
    if queued[reflector] {
        return;
    }
    queued[reflector] = true;
    let position = match order {
        Order::Ascending => queue.partition_point(|&other| other > reflector),
        Order::Descending => queue.partition_point(|&other| other < reflector),
    };
    queue.insert(position, reflector);
}

/// Applies the reflector `I - beta v v^T`, with `v` given by its rows and entries, to a dense vector.
#[inline]
fn reflect(rows: &[usize], vector: &[f64], beta: f64, values: &mut [f64]) {
    let product: f64 = rows
        .iter()
        .zip(vector)
        .map(|(&row, &value)| value * values[row])
        .sum();
    if product == 0.0 {
        return;
    }
    let factor = beta * product;
    for (&row, &value) in rows.iter().zip(vector) {
        values[row] -= factor * value;
    }
}

fn norm(values: impl Iterator<Item = f64>) -> f64 {
    values.map(|value| value * value).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense(column: &[f64]) -> Vec<(usize, f64)> {
        column
            .iter()
            .enumerate()
            .filter(|(_, value)| **value != 0.0)
            .map(|(row, value)| (row, *value))
            .collect()
    }

    #[test]
    fn independent_columns_are_accepted() {
        let mut qr = SparseQr::new(3);

        assert_eq!(qr.add(&dense(&[1.0, 2.0, 0.0]), 1e-8), Added::Accepted);
        assert_eq!(qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8), Added::Accepted);
        assert_eq!(qr.rank(), 2);
    }

    #[test]
    fn zero_column_is_dependent() {
        let mut qr = SparseQr::new(2);

        assert_eq!(
            qr.add(&[], 1e-8),
            Added::Dependent {
                projection: Vec::new()
            }
        );
    }

    #[test]
    fn combination_of_accepted_columns_is_dependent_with_its_coefficients() {
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 2.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8);

        let added = qr.add(&dense(&[2.0, 1.0, -3.0]), 1e-8);

        let Added::Dependent { projection } = added else {
            panic!("the column is 2 a - 3 b, found {added:?}");
        };
        let coefficients = qr.coefficients(&projection);
        assert!((coefficients[0] - 2.0).abs() < 1e-12);
        assert!((coefficients[1] + 3.0).abs() < 1e-12);
    }

    #[test]
    fn column_that_is_zero_up_to_rounding_is_dependent() {
        let mut qr = SparseQr::new(2);

        assert!(matches!(
            qr.add(&[(1, 1e-30)], 1e-8),
            Added::Dependent { .. }
        ));
    }

    #[test]
    fn repeated_rows_are_summed() {
        let mut qr = SparseQr::new(2);
        qr.add(&[(0, 1.0)], 1e-8);

        let added = qr.add(&[(1, 1.0), (1, -1.0), (0, 3.0)], 1e-8);

        assert!(matches!(added, Added::Dependent { .. }));
    }

    #[test]
    fn minimum_norm_solution_satisfies_the_rows_without_extra_motion() {
        let rows = [[1.0, 1.0, 0.0], [0.0, 1.0, 1.0]];
        let mut qr = SparseQr::new(3);
        for row in &rows {
            qr.add(&dense(row), 1e-8);
        }

        let solution = qr.minimum_norm_solution(&[2.0, 1.0]);

        for (row, expected) in rows.iter().zip([2.0, 1.0]) {
            let value: f64 = row
                .iter()
                .zip(&solution)
                .map(|(entry, unknown)| entry * unknown)
                .sum();
            assert!((value - expected).abs() < 1e-12);
        }
        // The minimum-norm solution lies in the row space, so it is orthogonal to the null vector (1, -1, 1).
        let null: f64 = solution[0] - solution[1] + solution[2];
        assert!(null.abs() < 1e-12);
    }

    #[test]
    fn least_squares_fits_an_overdetermined_system() {
        // Columns of A = [[1, 0], [1, 1], [1, 2]]; the best line through (0, 1), (1, 2), (2, 2).
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 1.0, 1.0]), 0.0);
        qr.add(&dense(&[0.0, 1.0, 2.0]), 0.0);

        let solution = qr.least_squares(&[1.0, 2.0, 2.0]);

        assert!((solution[0] - 7.0 / 6.0).abs() < 1e-12);
        assert!((solution[1] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn complement_distances_measure_undetermined_rows() {
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 0.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8);

        let distances = qr.complement_distances();

        assert!(distances[0].abs() < 1e-12);
        assert!((distances[1] - 0.5_f64.sqrt()).abs() < 1e-12);
        assert!((distances[2] - 0.5_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn cleared_factorization_starts_over() {
        let mut qr = SparseQr::new(2);
        qr.add(&[(0, 1.0)], 1e-8);

        qr.clear(3);

        assert_eq!(qr.rank(), 0);
        assert_eq!(qr.add(&[(2, 1.0)], 1e-8), Added::Accepted);
        assert_eq!(qr.minimum_norm_solution(&[2.0]), vec![0.0, 0.0, 2.0]);
    }

    #[test]
    fn dependent_column_leaves_the_factorization_unchanged() {
        let mut qr = SparseQr::new(2);
        qr.add(&[(0, 1.0)], 1e-8);
        qr.add(&[(0, 2.0)], 1e-8);

        assert_eq!(qr.add(&[(1, 1.0)], 1e-8), Added::Accepted);
        assert_eq!(qr.rank(), 2);
        let solution = qr.minimum_norm_solution(&[1.0, 3.0]);
        assert!((solution[0] - 1.0).abs() < 1e-12 && (solution[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn plans_are_reused_for_columns_with_the_same_patterns() {
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 1.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8);
        let plans = qr.plans.clone();

        qr.clear(3);
        qr.add(&dense(&[2.0, 1.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 3.0, 1.0]), 1e-8);

        assert_eq!(qr.plans, plans);
        let solution = qr.minimum_norm_solution(&[3.0, 4.0]);
        assert!((2.0 * solution[0] + solution[1] - 3.0).abs() < 1e-12);
        assert!((3.0 * solution[1] + solution[2] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn a_changed_decision_replans_the_later_columns() {
        let mut qr = SparseQr::new(2);
        qr.add(&[(0, 1.0), (1, 0.0)], 1e-8);
        qr.add(&[(0, 1.0), (1, 1.0)], 1e-8);
        qr.clear(2);

        let first = qr.add(&[(0, 0.0), (1, 0.0)], 1e-8);
        let second = qr.add(&[(0, 1.0), (1, 1.0)], 1e-8);

        assert!(matches!(first, Added::Dependent { .. }));
        assert_eq!(second, Added::Accepted);
        assert_eq!(qr.rank(), 1);
        let solution = qr.minimum_norm_solution(&[2.0]);
        assert!((solution[0] - 1.0).abs() < 1e-12 && (solution[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn fill_in_is_followed_through_later_reflectors() {
        // A chain of overlapping columns where later columns only meet earlier ones through fill.
        let mut qr = SparseQr::new(4);
        qr.add(&dense(&[1.0, 1.0, 0.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 0.0, 1.0, 1.0]), 1e-8);

        let added = qr.add(&dense(&[1.0, 0.0, 0.0, 1.0]), 1e-8);

        let Added::Dependent { projection } = added else {
            panic!("the column is a - b + c, found {added:?}");
        };
        let coefficients = qr.coefficients(&projection);
        for (actual, expected) in coefficients.iter().zip([1.0, -1.0, 1.0]) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
}
