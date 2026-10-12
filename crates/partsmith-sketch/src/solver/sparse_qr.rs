//! A sparse Householder QR factorization that takes columns one at a time, without column pivoting.
//!
//! This is the rank-revealing factorization of spec sections 3.7.4 and 3.8. Each column is a sparse
//! vector. It is transformed by the reflectors of the columns accepted before it; what is left outside
//! their pivot rows is its component orthogonal to their span. A column is dependent when that part
//! is at most `tolerance` times its original norm, and then it gets no reflector. Otherwise a
//! Householder reflector maps that part onto one new pivot row (the lowest row index it touches,
//! which keeps fill local for document-ordered sketches). Only the reflectors whose rows overlap a
//! column are applied to it, in ascending order.
//!
//! With the accepted columns as `A`, the factorization is `A = Q [R; 0]` with `Q` the product of the
//! reflectors and `R` upper triangular in acceptance order. Reflectors and the columns of `R` are
//! stored back to back in flat arrays, and every buffer keeps its capacity across `clear`, so the
//! repeated factorizations of a solve allocate almost nothing.

/// What happened to an added column.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Added {
    /// The column is independent of the accepted ones and now has a reflector.
    Accepted,
    /// The column lies in the span of the accepted columns; `projection` holds its coordinates
    /// `Q^T column` on the pivot rows as (reflector, value), ascending.
    Dependent { projection: Vec<(usize, f64)> },
}

/// A dense scratch vector with the list of rows it touches.
#[derive(Clone, Debug, Default)]
struct Workspace {
    values: Vec<f64>,
    touched: Vec<bool>,
    support: Vec<usize>,
}

impl Workspace {
    /// Makes the workspace all zero with `dimension` rows.
    fn reset(&mut self, dimension: usize) {
        self.clear();
        self.values.resize(dimension, 0.0);
        self.touched.resize(dimension, false);
    }

    fn add(&mut self, row: usize, value: f64) {
        if !self.touched[row] {
            self.touched[row] = true;
            self.support.push(row);
        }
        self.values[row] += value;
    }

    /// Sets every touched row back to zero.
    fn clear(&mut self) {
        for &row in &self.support {
            self.values[row] = 0.0;
            self.touched[row] = false;
        }
        self.support.clear();
    }
}

/// The order in which `SparseQr::apply` applies reflectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    /// First to last: `Q^T x`.
    Ascending,
    /// Last to first: `Q x`.
    Descending,
}

/// The factorization of the columns added so far.
#[derive(Clone, Debug, Default)]
pub(crate) struct SparseQr {
    /// Pivot row of each reflector.
    pivots: Vec<usize>,
    /// `beta` of each reflector `I - beta v v^T`.
    betas: Vec<f64>,
    /// Diagonal entry of `R` in the column of each reflector.
    diagonals: Vec<f64>,
    /// Where each reflector's entries start in `vector_rows` and `vector_values`, plus the end of the last.
    vector_starts: Vec<usize>,
    /// Rows of the vectors `v`, ascending within each reflector.
    vector_rows: Vec<usize>,
    /// Entries of the vectors `v` on those rows.
    vector_values: Vec<f64>,
    /// Where each reflector's column of `R` starts in `above`, plus the end of the last.
    above_starts: Vec<usize>,
    /// Entries of `R` above the diagonal: (earlier reflector, value), ascending within each column.
    above: Vec<(usize, f64)>,
    /// For each row, the reflectors whose vectors contain it, ascending.
    touching: Vec<Vec<usize>>,
    /// For each row, the reflector it is the pivot of.
    pivot_of: Vec<Option<usize>>,
    workspace: Workspace,
    /// For each reflector, whether it waits in `queue`; all false between calls of `apply`.
    queued: Vec<bool>,
    /// Reflectors waiting to be applied, sorted so that the next one is last.
    queue: Vec<usize>,
}

impl SparseQr {
    /// An empty factorization of columns with `dimension` rows.
    pub(crate) fn new(dimension: usize) -> SparseQr {
        let mut factorization = SparseQr::default();
        factorization.clear(dimension);
        factorization
    }

    /// Forgets every column and prepares for columns with `dimension` rows, keeping allocated capacity.
    pub(crate) fn clear(&mut self, dimension: usize) {
        self.pivots.clear();
        self.betas.clear();
        self.diagonals.clear();
        self.vector_starts.clear();
        self.vector_starts.push(0);
        self.vector_rows.clear();
        self.vector_values.clear();
        self.above_starts.clear();
        self.above_starts.push(0);
        self.above.clear();
        for list in &mut self.touching {
            list.clear();
        }
        self.touching.resize_with(dimension, Vec::new);
        self.pivot_of.clear();
        self.pivot_of.resize(dimension, None);
        self.workspace.reset(dimension);
        self.queued.clear();
        self.queue.clear();
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
    /// (an exactly zero column always is dependent).
    pub(crate) fn add(&mut self, column: &[(usize, f64)], tolerance: f64) -> Added {
        for &(row, value) in column {
            self.workspace.add(row, value);
        }
        let original_norm = norm(
            self.workspace
                .support
                .iter()
                .map(|&row| self.workspace.values[row]),
        );
        self.apply(Order::Ascending);
        self.workspace.support.sort_unstable();
        // The projection becomes the column of R and the residual the reflector's vector; both are
        // written in place and taken back out when the column turns out dependent.
        let above_start = self.above.len();
        let vector_start = self.vector_rows.len();
        for &row in &self.workspace.support {
            let value = self.workspace.values[row];
            match self.pivot_of[row] {
                Some(reflector) => self.above.push((reflector, value)),
                None if value != 0.0 => {
                    self.vector_rows.push(row);
                    self.vector_values.push(value);
                }
                None => {}
            }
        }
        self.workspace.clear();
        self.above[above_start..].sort_unstable_by_key(|&(reflector, _)| reflector);
        let residual_norm = norm(self.vector_values[vector_start..].iter().copied());
        let independent =
            residual_norm > tolerance * original_norm.max(1.0) && residual_norm.is_finite();
        if !independent {
            self.vector_rows.truncate(vector_start);
            self.vector_values.truncate(vector_start);
            return Added::Dependent {
                projection: self.above.split_off(above_start),
            };
        }
        let index = self.pivots.len();
        let pivot = self.vector_rows[vector_start];
        let pivot_value = self.vector_values[vector_start];
        let diagonal = if pivot_value >= 0.0 {
            -residual_norm
        } else {
            residual_norm
        };
        self.vector_values[vector_start] -= diagonal;
        for &row in &self.vector_rows[vector_start..] {
            self.touching[row].push(index);
        }
        self.pivot_of[pivot] = Some(index);
        self.pivots.push(pivot);
        self.betas
            .push(1.0 / (residual_norm * (residual_norm + pivot_value.abs())));
        self.diagonals.push(diagonal);
        self.vector_starts.push(self.vector_rows.len());
        self.above_starts.push(self.above.len());
        self.queued.push(false);
        Added::Accepted
    }

    /// The vector `v` of a reflector as its rows and entries.
    fn vector(&self, reflector: usize) -> (&[usize], &[f64]) {
        let range = self.vector_starts[reflector]..self.vector_starts[reflector + 1];
        (&self.vector_rows[range.clone()], &self.vector_values[range])
    }

    /// The entries of `R` above the diagonal in a reflector's column.
    fn above(&self, reflector: usize) -> &[(usize, f64)] {
        &self.above[self.above_starts[reflector]..self.above_starts[reflector + 1]]
    }

    /// Applies to the workspace, in ascending order (`Q^T x`) or descending order (`Q x`), every reflector
    /// whose rows overlap it. A reflector that fills in new rows queues the reflectors after it in that
    /// order which touch them; each reflector is queued at most once.
    fn apply(&mut self, order: Order) {
        for &row in &self.workspace.support {
            for &reflector in &self.touching[row] {
                enqueue(&mut self.queue, &mut self.queued, reflector, order);
            }
        }
        while let Some(reflector) = self.queue.pop() {
            self.queued[reflector] = false;
            let touched_before = self.workspace.support.len();
            let range = self.vector_starts[reflector]..self.vector_starts[reflector + 1];
            reflect(
                &self.vector_rows[range.clone()],
                &self.vector_values[range],
                self.betas[reflector],
                &mut self.workspace,
            );
            for &row in &self.workspace.support[touched_before..] {
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
    }

    /// The minimum-norm `h` with `A^T h = right_side`, where `right_side` has one entry per accepted column.
    pub(crate) fn minimum_norm_solution(&self, right_side: &[f64]) -> Vec<f64> {
        let mut solution = vec![0.0; self.touching.len()];
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
            self.reflect_dense(reflector, &mut solution);
        }
        solution
    }

    /// The `h` minimizing `norm(A h - right_side)`, one entry per accepted column; `right_side` is dense.
    pub(crate) fn least_squares(&self, right_side: &[f64]) -> Vec<f64> {
        let mut transformed = right_side.to_vec();
        for reflector in 0..self.rank() {
            self.reflect_dense(reflector, &mut transformed);
        }
        let projection: Vec<f64> = self.pivots.iter().map(|&pivot| transformed[pivot]).collect();
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
        let mut squared = vec![0.0; self.touching.len()];
        for row in 0..self.touching.len() {
            if self.pivot_of[row].is_some() {
                continue;
            }
            // Q e_row is a unit vector orthogonal to every accepted column; together they span the complement.
            self.workspace.add(row, 1.0);
            self.apply(Order::Descending);
            for &other in &self.workspace.support {
                squared[other] += self.workspace.values[other] * self.workspace.values[other];
            }
            self.workspace.clear();
        }
        squared.into_iter().map(f64::sqrt).collect()
    }

    /// Applies one reflector to a dense vector.
    fn reflect_dense(&self, reflector: usize, values: &mut [f64]) {
        let (rows, vector) = self.vector(reflector);
        let product: f64 = rows
            .iter()
            .zip(vector)
            .map(|(&row, &value)| value * values[row])
            .sum();
        if product == 0.0 {
            return;
        }
        let factor = self.betas[reflector] * product;
        for (&row, &value) in rows.iter().zip(vector) {
            values[row] -= factor * value;
        }
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

/// Applies the reflector `I - beta v v^T` to the workspace; rows it touches for the first time are
/// appended to the workspace's support.
fn reflect(rows: &[usize], vector: &[f64], beta: f64, workspace: &mut Workspace) {
    let product: f64 = rows
        .iter()
        .zip(vector)
        .map(|(&row, &value)| value * workspace.values[row])
        .sum();
    if product == 0.0 {
        return;
    }
    let factor = beta * product;
    for (&row, &value) in rows.iter().zip(vector) {
        workspace.add(row, -factor * value);
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
            let value: f64 = row.iter().zip(&solution).map(|(a, b)| a * b).sum();
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
